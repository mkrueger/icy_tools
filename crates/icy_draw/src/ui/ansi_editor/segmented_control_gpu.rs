//! GPU-accelerated Segmented Control with Shader Background
//!
//! Uses a WGSL shader for the background (shadows, glow, selection highlights)
//! and a Canvas overlay for text rendering with the BitFont.

use super::glyph_renderer::{FLAG_BG_ONLY, FLAG_DRAW_BG, GlyphInstance, GlyphUniforms, QuadVertex, build_glyph_atlas_rgba, cp437_index, font_key};
use super::segmented_layout::{
    BORDER_WIDTH, CORNER_RADIUS, MAX_SEGMENTS, NO_HOVER, PREVIEW_GLYPH_HEIGHT, SEGMENT_HEIGHT, SEGMENT_PADDING_H, SHADOW_PADDING, SegmentContentType,
};
use iced::wgpu::util::DeviceExt;
use iced::{
    Color, Element, Length, Point, Rectangle, Size, Theme,
    mouse::{self, Cursor},
    widget::{
        self,
        canvas::{self, Cache, Frame, Geometry},
        shader,
    },
};
use icy_engine::{BitFont, Palette};
use icy_engine_gui::theme::main_area_background;
use std::num::NonZeroU64;
use std::sync::Once;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

// ═══════════════════════════════════════════════════════════════════════════
// One-pass GPU SegmentedControl (background + glyphs in one pipeline)
// ═══════════════════════════════════════════════════════════════════════════

/// Additional flag for `GlyphInstance::flags` used by the one-pass shader.
/// When set, the instance is treated as the segmented control background quad.
const FLAG_SEGMENTED_BG: u32 = 16;

/// Combined uniforms for the one-pass segmented control shader.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default)]
struct SegmentedOnePassUniforms {
    // Glyph uniforms
    clip_size: [f32; 2],
    atlas_size: [f32; 2],
    glyph_size: [f32; 2],
    _pad0: [f32; 2],

    // Segmented control
    num_segments: u32,
    selected_mask: u32,
    hovered_segment: u32,
    _pad1: u32,

    corner_radius: f32,
    // WGSL `vec3<f32>` in uniforms has 16-byte alignment/size.
    // The WGSL layout inserts 12 bytes of padding after `corner_radius` to align the next field.
    _pad_corner: [f32; 3],
    // Represents WGSL `_pad2: vec3<f32>` (takes 16 bytes in uniform layout).
    _pad2: [f32; 4],

    // Background clear color (theme)
    bg_color: [f32; 4],

    // Segment widths (up to 8)
    segment_widths: [[f32; 4]; 2],
}

unsafe impl bytemuck::Pod for SegmentedOnePassUniforms {}
unsafe impl bytemuck::Zeroable for SegmentedOnePassUniforms {}

#[derive(Clone)]
struct SegmentedOnePassProgram<T: Clone> {
    segment_widths: Vec<f32>,
    selected_mask: u32,
    hovered_index: Arc<AtomicU32>,
    bg_color: Color,
    segments: Vec<Segment<T>>,
    font: Option<BitFont>,
    char_colors: Option<CharColors>,
    // For CharClicked in single-select: the current selected index
    selected_index: usize,
    multi_select: bool,
}

impl<T: Clone> SegmentedOnePassProgram<T> {
    fn build_instances(&self, scale: f32, bounds: &Rectangle, glyph_w: f32, glyph_h: f32) -> Vec<GlyphInstance> {
        let size_w = (bounds.width * scale).round().max(1.0);
        let size_h = (bounds.height * scale).round().max(1.0);

        // Background instance clears the whole widget rect.
        let mut instances: Vec<GlyphInstance> = Vec::with_capacity(256);
        instances.push(GlyphInstance {
            pos: [0.0, 0.0],
            size: [size_w, size_h],
            fg: [0.0, 0.0, 0.0, 0.0],
            bg: [0.0, 0.0, 0.0, 0.0],
            glyph: 0,
            flags: FLAG_SEGMENTED_BG,
            _pad: [0, 0],
        });

        let content_x = SHADOW_PADDING + BORDER_WIDTH;
        let content_y = SHADOW_PADDING + BORDER_WIDTH;

        // Physical content box
        let control_h_px = ((bounds.height - SHADOW_PADDING * 2.0 - BORDER_WIDTH * 2.0) * scale).round().max(1.0);
        let content_y_px = (content_y * scale).floor();
        let content_h_px = control_h_px;

        let default_text = Color::from_rgb(0.85, 0.85, 0.85);
        let selected_text = Color::WHITE;
        let shadow = Color::from_rgba(0.0, 0.0, 0.0, 0.5);

        // Text glyph size in physical pixels (scaled + snapped)
        let text_glyph_w_px = (glyph_w * scale).round().max(1.0);
        let text_glyph_h_px = (glyph_h * scale).round().max(1.0);
        let text_y_px = (content_y_px + ((content_h_px - text_glyph_h_px) / 2.0).floor()).floor();

        // Preview magnification for Char segments (integer in physical pixels)
        let target_preview_h_px = (PREVIEW_GLYPH_HEIGHT * scale).round().max(1.0);
        let preview_magnify_px = (target_preview_h_px / glyph_h.max(1.0)).floor().max(1.0);
        let preview_glyph_w_px = (glyph_w.max(1.0) * preview_magnify_px).round().max(1.0);
        let preview_glyph_h_px = (glyph_h.max(1.0) * preview_magnify_px).round().max(1.0);
        let preview_y_px = (content_y_px + ((content_h_px - preview_glyph_h_px) / 2.0).floor()).floor();

        // Segment positioning
        let mut seg_start_x = content_x;
        for (idx, seg) in self.segments.iter().enumerate() {
            let seg_w = self.segment_widths.get(idx).copied().unwrap_or(0.0);
            if seg_w <= 0.0 {
                seg_start_x += seg_w;
                continue;
            }

            let seg_start_x_px = (seg_start_x * scale).floor();
            let seg_w_px = (seg_w * scale).floor().max(1.0);
            let is_selected = (self.selected_mask & (1u32 << idx)) != 0;

            match &seg.content {
                SegmentContent::Text(text) => {
                    let fg = if is_selected { selected_text } else { default_text };
                    let chars: Vec<char> = text.chars().collect();
                    let text_w_px = (chars.len() as f32 * text_glyph_w_px).round();
                    let base_x_px = (seg_start_x_px + ((seg_w_px - text_w_px) / 2.0).floor()).floor();

                    for (ci, ch) in chars.iter().copied().enumerate() {
                        let glyph = cp437_index(ch) & 0xFF;
                        let gx = (base_x_px + (ci as f32 * text_glyph_w_px)).floor();

                        // shadow
                        instances.push(GlyphInstance {
                            pos: [(gx + 1.0).floor(), (text_y_px + 1.0).floor()],
                            size: [text_glyph_w_px, text_glyph_h_px],
                            fg: [shadow.r, shadow.g, shadow.b, shadow.a],
                            bg: [0.0, 0.0, 0.0, 0.0],
                            glyph,
                            flags: 0,
                            _pad: [0, 0],
                        });

                        // main
                        instances.push(GlyphInstance {
                            pos: [gx, text_y_px],
                            size: [text_glyph_w_px, text_glyph_h_px],
                            fg: [fg.r, fg.g, fg.b, 1.0],
                            bg: [0.0, 0.0, 0.0, 0.0],
                            glyph,
                            flags: 0,
                            _pad: [0, 0],
                        });
                    }
                }
                SegmentContent::Char(ch) => {
                    let glyph = cp437_index(*ch) & 0xFF;
                    let gx = (seg_start_x_px + ((seg_w_px - preview_glyph_w_px) / 2.0).floor()).floor();

                    if let Some(ref colors) = self.char_colors {
                        if idx == self.selected_index {
                            // caret bg + fg
                            instances.push(GlyphInstance {
                                pos: [gx, preview_y_px],
                                size: [preview_glyph_w_px, preview_glyph_h_px],
                                fg: [colors.fg.r, colors.fg.g, colors.fg.b, 1.0],
                                bg: [colors.bg.r, colors.bg.g, colors.bg.b, 1.0],
                                glyph,
                                flags: FLAG_DRAW_BG,
                                _pad: [0, 0],
                            });
                        } else {
                            let fg = if is_selected { selected_text } else { default_text };
                            instances.push(GlyphInstance {
                                pos: [(gx + 1.0).floor(), (preview_y_px + 1.0).floor()],
                                size: [preview_glyph_w_px, preview_glyph_h_px],
                                fg: [shadow.r, shadow.g, shadow.b, shadow.a],
                                bg: [0.0, 0.0, 0.0, 0.0],
                                glyph,
                                flags: 0,
                                _pad: [0, 0],
                            });
                            instances.push(GlyphInstance {
                                pos: [gx, preview_y_px],
                                size: [preview_glyph_w_px, preview_glyph_h_px],
                                fg: [fg.r, fg.g, fg.b, 1.0],
                                bg: [0.0, 0.0, 0.0, 0.0],
                                glyph,
                                flags: 0,
                                _pad: [0, 0],
                            });
                        }
                    } else {
                        let fg = if is_selected { selected_text } else { default_text };
                        instances.push(GlyphInstance {
                            pos: [(gx + 1.0).floor(), (preview_y_px + 1.0).floor()],
                            size: [preview_glyph_w_px, preview_glyph_h_px],
                            fg: [shadow.r, shadow.g, shadow.b, shadow.a],
                            bg: [0.0, 0.0, 0.0, 0.0],
                            glyph,
                            flags: 0,
                            _pad: [0, 0],
                        });
                        instances.push(GlyphInstance {
                            pos: [gx, preview_y_px],
                            size: [preview_glyph_w_px, preview_glyph_h_px],
                            fg: [fg.r, fg.g, fg.b, 1.0],
                            bg: [0.0, 0.0, 0.0, 0.0],
                            glyph,
                            flags: 0,
                            _pad: [0, 0],
                        });
                    }
                }
            }

            seg_start_x += seg_w;
        }

        instances
    }
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> shader::Program<SegmentedControlMessage<T>> for SegmentedOnePassProgram<T> {
    type State = ();
    type Primitive = SegmentedOnePassPrimitive<T>;

    fn draw(&self, _state: &Self::State, cursor: Cursor, bounds: Rectangle) -> Self::Primitive {
        // We only use cursor for interaction; rendering uses stored hovered_index.
        let _ = (cursor, bounds);

        SegmentedOnePassPrimitive {
            segment_widths: self.segment_widths.clone(),
            selected_mask: self.selected_mask,
            hovered_index: self.hovered_index.clone(),
            bg_color: self.bg_color,
            segments: self.segments.clone(),
            font: self.font.clone(),
            char_colors: self.char_colors.clone(),
            selected_index: self.selected_index,
            multi_select: self.multi_select,
            viewport_x: Arc::new(AtomicU32::new(0)),
            viewport_y: Arc::new(AtomicU32::new(0)),
            viewport_w: Arc::new(AtomicU32::new(0)),
            viewport_h: Arc::new(AtomicU32::new(0)),
            uniform_offset_bytes: Arc::new(AtomicU32::new(0)),
            instance_offset_bytes: Arc::new(AtomicU32::new(0)),
            instance_count: Arc::new(AtomicU32::new(0)),
        }
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: Cursor,
    ) -> Option<iced::widget::Action<SegmentedControlMessage<T>>> {
        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let new_hover = cursor.position_in(bounds).and_then(|pos| segment_at_x(pos.x, &self.segment_widths));
                let new_raw = new_hover.map(|i| i as u32).unwrap_or(NO_HOVER);
                let old_raw = self.hovered_index.swap(new_raw, Ordering::Relaxed);
                if old_raw != new_raw {
                    return Some(iced::widget::Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::CursorLeft) => {
                let old_raw = self.hovered_index.swap(NO_HOVER, Ordering::Relaxed);
                if old_raw != NO_HOVER {
                    return Some(iced::widget::Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(pos) = cursor.position_in(bounds) else {
                    return None;
                };
                let Some(idx) = segment_at_x(pos.x, &self.segment_widths) else {
                    return None;
                };

                let Some(seg) = self.segments.get(idx) else {
                    return None;
                };
                let value = seg.value.clone();
                if self.multi_select {
                    Some(iced::widget::Action::publish(SegmentedControlMessage::Toggled(value)))
                } else {
                    let is_char = matches!(seg.content, SegmentContent::Char(_));
                    let is_already_selected = idx == self.selected_index;
                    if is_char && is_already_selected {
                        Some(iced::widget::Action::publish(SegmentedControlMessage::CharClicked(value)))
                    } else {
                        Some(iced::widget::Action::publish(SegmentedControlMessage::Selected(value)))
                    }
                }
            }
            _ => None,
        }
    }

    fn mouse_interaction(&self, _state: &Self::State, bounds: Rectangle, cursor: Cursor) -> mouse::Interaction {
        if cursor.position_in(bounds).is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

#[derive(Clone, Debug)]
struct SegmentedOnePassPrimitive<T: Clone> {
    segment_widths: Vec<f32>,
    selected_mask: u32,
    hovered_index: Arc<AtomicU32>,
    bg_color: Color,
    segments: Vec<Segment<T>>,
    font: Option<BitFont>,
    char_colors: Option<CharColors>,
    selected_index: usize,
    multi_select: bool,
    viewport_x: Arc<AtomicU32>,
    viewport_y: Arc<AtomicU32>,
    viewport_w: Arc<AtomicU32>,
    viewport_h: Arc<AtomicU32>,
    uniform_offset_bytes: Arc<AtomicU32>,
    instance_offset_bytes: Arc<AtomicU32>,
    instance_count: Arc<AtomicU32>,
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> shader::Primitive for SegmentedOnePassPrimitive<T> {
    type Pipeline = SegmentedOnePassRenderer;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        bounds: &Rectangle,
        viewport: &iced::advanced::graphics::Viewport,
    ) {
        let scale = viewport.scale_factor();
        // Convert logical widget bounds to physical pixels.
        let origin_x = (bounds.x * scale).round().max(0.0);
        let origin_y = (bounds.y * scale).round().max(0.0);
        let size_w = (bounds.width * scale).round().max(1.0);
        let size_h = (bounds.height * scale).round().max(1.0);

        self.viewport_x.store(origin_x as u32, Ordering::Relaxed);
        self.viewport_y.store(origin_y as u32, Ordering::Relaxed);
        self.viewport_w.store(size_w as u32, Ordering::Relaxed);
        self.viewport_h.store(size_h as u32, Ordering::Relaxed);

        let hovered_raw = self.hovered_index.load(Ordering::Relaxed);

        let (glyph_w, glyph_h) = if let Some(font) = &self.font {
            let key = font_key(font);
            if pipeline.atlas_key != Some(key) {
                let (aw, ah, rgba) = build_glyph_atlas_rgba(font);
                pipeline.update_atlas(device, queue, key, aw, ah, &rgba);
            }
            let s = font.size();
            (s.width.max(1) as f32, s.height.max(1) as f32)
        } else {
            self.instance_count.store(0, Ordering::Relaxed);
            return;
        };

        // Pack segment widths in physical pixels
        let mut seg_widths = [[0.0f32; 4]; 2];
        for (i, w) in self.segment_widths.iter().enumerate().take(MAX_SEGMENTS) {
            seg_widths[i / 4][i % 4] = w * scale;
        }

        let uniforms = SegmentedOnePassUniforms {
            clip_size: [size_w, size_h],
            atlas_size: [pipeline.atlas_w as f32, pipeline.atlas_h as f32],
            glyph_size: [glyph_w, glyph_h],
            _pad0: [0.0, 0.0],
            num_segments: self.segment_widths.len().min(MAX_SEGMENTS) as u32,
            selected_mask: self.selected_mask,
            hovered_segment: hovered_raw,
            _pad1: 0,
            corner_radius: CORNER_RADIUS * scale,
            _pad_corner: [0.0, 0.0, 0.0],
            _pad2: [0.0, 0.0, 0.0, 0.0],
            bg_color: [self.bg_color.r, self.bg_color.g, self.bg_color.b, self.bg_color.a],
            segment_widths: seg_widths,
        };

        let uniform_slot = pipeline.next_uniform.fetch_add(1, Ordering::Relaxed) % pipeline.uniform_capacity;
        let uniform_offset = (uniform_slot as u64) * pipeline.uniform_stride;
        self.uniform_offset_bytes.store(uniform_offset as u32, Ordering::Relaxed);
        queue.write_buffer(&pipeline.uniform_buffer, uniform_offset, bytemuck::bytes_of(&uniforms));

        // Build instances
        let program = SegmentedOnePassProgram {
            segment_widths: self.segment_widths.clone(),
            selected_mask: self.selected_mask,
            hovered_index: self.hovered_index.clone(),
            bg_color: self.bg_color,
            segments: self.segments.clone(),
            font: self.font.clone(),
            char_colors: self.char_colors.clone(),
            selected_index: self.selected_index,
            multi_select: self.multi_select,
        };
        let instances = program.build_instances(scale, bounds, glyph_w, glyph_h);

        let slot = pipeline.next_instance_slot.fetch_add(1, Ordering::Relaxed) % pipeline.instance_slots;
        let instance_offset = (slot as u64) * pipeline.instance_slot_stride;
        self.instance_offset_bytes.store(instance_offset as u32, Ordering::Relaxed);

        let max_instances = pipeline.instance_capacity_per_primitive as usize;
        let count = instances.len().min(max_instances);
        self.instance_count.store(count as u32, Ordering::Relaxed);
        if count == 0 {
            return;
        }
        let bytes = bytemuck::cast_slice(&instances[..count]);
        queue.write_buffer(&pipeline.instance_buffer, instance_offset, bytes);
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        let instance_count = self.instance_count.load(Ordering::Relaxed);
        if instance_count == 0 {
            return;
        }

        let mut pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
            label: Some("Segmented OnePass Render Pass"),
            color_attachments: &[Some(iced::wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: iced::wgpu::Operations {
                    load: iced::wgpu::LoadOp::Load,
                    store: iced::wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if clip_bounds.width > 0 && clip_bounds.height > 0 {
            pass.set_scissor_rect(clip_bounds.x, clip_bounds.y, clip_bounds.width, clip_bounds.height);

            let vx = self.viewport_x.load(Ordering::Relaxed) as f32;
            let vy = self.viewport_y.load(Ordering::Relaxed) as f32;
            let vw = self.viewport_w.load(Ordering::Relaxed).max(1) as f32;
            let vh = self.viewport_h.load(Ordering::Relaxed).max(1) as f32;
            pass.set_viewport(vx, vy, vw, vh, 0.0, 1.0);

            pass.set_pipeline(&pipeline.pipeline);
            let uniform_offset = self.uniform_offset_bytes.load(Ordering::Relaxed);
            pass.set_bind_group(0, &pipeline.bind_group, &[uniform_offset]);
            pass.set_vertex_buffer(0, pipeline.quad_vertex_buffer.slice(..));
            let instance_offset = self.instance_offset_bytes.load(Ordering::Relaxed) as u64;
            let instance_bytes = (instance_count as u64) * pipeline.instance_stride;
            pass.set_vertex_buffer(1, pipeline.instance_buffer.slice(instance_offset..(instance_offset + instance_bytes)));
            pass.draw(0..6, 0..instance_count);
        }
    }
}

pub struct SegmentedOnePassRenderer {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group: iced::wgpu::BindGroup,
    uniform_buffer: iced::wgpu::Buffer,
    uniform_stride: u64,
    uniform_capacity: u32,
    next_uniform: AtomicU32,
    quad_vertex_buffer: iced::wgpu::Buffer,
    instance_buffer: iced::wgpu::Buffer,
    instance_stride: u64,
    instance_capacity_per_primitive: u32,
    instance_slots: u32,
    instance_slot_stride: u64,
    next_instance_slot: AtomicU32,

    atlas_texture: iced::wgpu::Texture,
    atlas_view: iced::wgpu::TextureView,
    atlas_sampler: iced::wgpu::Sampler,
    atlas_key: Option<u64>,
    atlas_w: u32,
    atlas_h: u32,
}

impl SegmentedOnePassRenderer {
    fn update_atlas(&mut self, device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, key: u64, w: u32, h: u32, rgba: &[u8]) {
        if self.atlas_w != w || self.atlas_h != h {
            self.atlas_texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                label: Some("Segmented OnePass Glyph Atlas"),
                size: iced::wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: iced::wgpu::TextureDimension::D2,
                format: iced::wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: iced::wgpu::TextureUsages::TEXTURE_BINDING | iced::wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.atlas_view = self.atlas_texture.create_view(&iced::wgpu::TextureViewDescriptor::default());
            self.atlas_w = w;
            self.atlas_h = h;

            let bind_group_layout = self.pipeline.get_bind_group_layout(0);
            self.bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                label: Some("Segmented OnePass Bind Group"),
                layout: &bind_group_layout,
                entries: &[
                    iced::wgpu::BindGroupEntry {
                        binding: 0,
                        resource: iced::wgpu::BindingResource::Buffer(iced::wgpu::BufferBinding {
                            buffer: &self.uniform_buffer,
                            offset: 0,
                            size: NonZeroU64::new(std::mem::size_of::<SegmentedOnePassUniforms>() as u64),
                        }),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 1,
                        resource: iced::wgpu::BindingResource::TextureView(&self.atlas_view),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 2,
                        resource: iced::wgpu::BindingResource::Sampler(&self.atlas_sampler),
                    },
                ],
            });
        }

        queue.write_texture(
            iced::wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: iced::wgpu::Origin3d::ZERO,
                aspect: iced::wgpu::TextureAspect::All,
            },
            rgba,
            iced::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            iced::wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        self.atlas_key = Some(key);
    }
}

impl shader::Pipeline for SegmentedOnePassRenderer {
    fn new(device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("Segmented OnePass Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("segmented_control_onepass_shader.wgsl").into()),
        });

        let uniform_size = std::mem::size_of::<SegmentedOnePassUniforms>() as u64;
        let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        let uniform_stride = align_up(uniform_size, alignment);
        let uniform_capacity: u32 = 1024;
        let uniform_buffer_size = uniform_stride * (uniform_capacity as u64);

        let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("Segmented OnePass Uniforms (Dynamic)"),
            size: uniform_buffer_size,
            usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let atlas_texture = device.create_texture(&iced::wgpu::TextureDescriptor {
            label: Some("Segmented OnePass Atlas (init)"),
            size: iced::wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: iced::wgpu::TextureDimension::D2,
            format: iced::wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: iced::wgpu::TextureUsages::TEXTURE_BINDING | iced::wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            iced::wgpu::TexelCopyTextureInfo {
                texture: &atlas_texture,
                mip_level: 0,
                origin: iced::wgpu::Origin3d::ZERO,
                aspect: iced::wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 0],
            iced::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            iced::wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let atlas_view = atlas_texture.create_view(&iced::wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&iced::wgpu::SamplerDescriptor {
            label: Some("Segmented OnePass Atlas Sampler"),
            mag_filter: iced::wgpu::FilterMode::Nearest,
            min_filter: iced::wgpu::FilterMode::Nearest,
            mipmap_filter: iced::wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let quad: [QuadVertex; 6] = [
            QuadVertex {
                unit_pos: [0.0, 0.0],
                unit_uv: [0.0, 0.0],
            },
            QuadVertex {
                unit_pos: [1.0, 0.0],
                unit_uv: [1.0, 0.0],
            },
            QuadVertex {
                unit_pos: [0.0, 1.0],
                unit_uv: [0.0, 1.0],
            },
            QuadVertex {
                unit_pos: [0.0, 1.0],
                unit_uv: [0.0, 1.0],
            },
            QuadVertex {
                unit_pos: [1.0, 0.0],
                unit_uv: [1.0, 0.0],
            },
            QuadVertex {
                unit_pos: [1.0, 1.0],
                unit_uv: [1.0, 1.0],
            },
        ];
        let quad_vertex_buffer = device.create_buffer_init(&iced::wgpu::util::BufferInitDescriptor {
            label: Some("Segmented OnePass Quad"),
            contents: bytemuck::cast_slice(&quad),
            usage: iced::wgpu::BufferUsages::VERTEX,
        });

        let instance_stride = std::mem::size_of::<GlyphInstance>() as u64;
        let instance_capacity_per_primitive: u32 = 512;
        let instance_slots: u32 = 1024;
        let instance_slot_stride = instance_stride * (instance_capacity_per_primitive as u64);
        let instance_buffer_size = instance_slot_stride * (instance_slots as u64);
        let instance_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("Segmented OnePass Instances (Ring)"),
            size: instance_buffer_size,
            usage: iced::wgpu::BufferUsages::VERTEX | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("Segmented OnePass Bind Group Layout"),
            entries: &[
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: iced::wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: iced::wgpu::BindingType::Buffer {
                        ty: iced::wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(uniform_size),
                    },
                    count: None,
                },
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: iced::wgpu::ShaderStages::FRAGMENT,
                    ty: iced::wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: iced::wgpu::TextureViewDimension::D2,
                        sample_type: iced::wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: iced::wgpu::ShaderStages::FRAGMENT,
                    ty: iced::wgpu::BindingType::Sampler(iced::wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
            label: Some("Segmented OnePass Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                iced::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: iced::wgpu::BindingResource::Buffer(iced::wgpu::BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: NonZeroU64::new(uniform_size),
                    }),
                },
                iced::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: iced::wgpu::BindingResource::TextureView(&atlas_view),
                },
                iced::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: iced::wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&iced::wgpu::PipelineLayoutDescriptor {
            label: Some("Segmented OnePass Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_buffers = [
            iced::wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<QuadVertex>() as u64,
                step_mode: iced::wgpu::VertexStepMode::Vertex,
                attributes: &iced::wgpu::vertex_attr_array![
                    0 => Float32x2,
                    1 => Float32x2
                ],
            },
            iced::wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<GlyphInstance>() as u64,
                step_mode: iced::wgpu::VertexStepMode::Instance,
                attributes: &iced::wgpu::vertex_attr_array![
                    2 => Float32x2,
                    3 => Float32x2,
                    4 => Float32x4,
                    5 => Float32x4,
                    6 => Uint32,
                    7 => Uint32
                ],
            },
        ];

        let pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some("Segmented OnePass Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: iced::wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(iced::wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(iced::wgpu::ColorTargetState {
                    format,
                    blend: Some(iced::wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: iced::wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: iced::wgpu::PrimitiveState {
                topology: iced::wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: iced::wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group,
            uniform_buffer,
            uniform_stride,
            uniform_capacity,
            next_uniform: AtomicU32::new(0),
            quad_vertex_buffer,
            instance_buffer,
            instance_stride,
            instance_capacity_per_primitive,
            instance_slots,
            instance_slot_stride,
            next_instance_slot: AtomicU32::new(0),
            atlas_texture,
            atlas_view,
            atlas_sampler,
            atlas_key: None,
            atlas_w: 1,
            atlas_h: 1,
        }
    }
}

#[cfg(debug_assertions)]
static SEGMENTED_CONTROL_DEBUG_PRINT_ONCE: Once = Once::new();

/// Segment content type
#[derive(Clone, Debug)]
pub enum SegmentContent {
    /// Text label
    Text(String),
    /// Single character (rendered larger, can have popup)
    Char(char),
}

impl SegmentContent {
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }

    #[allow(dead_code)]
    pub fn char(c: char) -> Self {
        Self::Char(c)
    }

    /// Convert to layout content type for width calculation.
    fn to_layout_type(&self) -> SegmentContentType {
        match self {
            SegmentContent::Text(s) => SegmentContentType::Text(s.chars().count()),
            SegmentContent::Char(_) => SegmentContentType::Char,
        }
    }
}

/// Segment definition
#[derive(Clone, Debug)]
pub struct Segment<T: Clone> {
    pub content: SegmentContent,
    pub value: T,
}

impl<T: Clone> Segment<T> {
    pub fn new(content: SegmentContent, value: T) -> Self {
        Self { content, value }
    }

    pub fn text(label: impl Into<String>, value: T) -> Self {
        Self::new(SegmentContent::text(label), value)
    }

    pub fn char(c: char, value: T) -> Self {
        Self::new(SegmentContent::Char(c), value)
    }
}

/// Messages from the segmented control
#[derive(Clone, Debug)]
pub enum SegmentedControlMessage<T> {
    /// Selected a segment (single-select mode, replaces current selection)
    Selected(T),
    /// Toggled a segment (multi-select mode, toggles on/off)
    Toggled(T),
    /// Clicked on a char segment (for popup)
    CharClicked(T),
}

// ═══════════════════════════════════════════════════════════════════════════
// Hit Testing Helper
// ═══════════════════════════════════════════════════════════════════════════

/// Find which segment contains the given x coordinate.
/// Uses the same logic as `SegmentedLayout::hit_test`.
fn segment_at_x(x: f32, segment_widths: &[f32]) -> Option<usize> {
    let content_x = SHADOW_PADDING + BORDER_WIDTH;
    let local_x = x - content_x;
    if local_x < 0.0 {
        return None;
    }

    let mut seg_x = 0.0;
    for (idx, &width) in segment_widths.iter().enumerate() {
        if local_x >= seg_x && local_x < seg_x + width {
            return Some(idx);
        }
        seg_x += width;
    }

    None
}

// ═══════════════════════════════════════════════════════════════════════════
// GPU Shader Background
// ═══════════════════════════════════════════════════════════════════════════

/// Uniform data for the segmented control shader
#[repr(C, align(16))]
#[derive(Clone, Copy, Default, Debug)]
struct SegmentedControlUniforms {
    /// Widget top-left (in screen coordinates)
    widget_origin: [f32; 2],
    /// Widget dimensions
    widget_size: [f32; 2],
    /// Number of segments
    num_segments: u32,
    /// Selected segment bitmask (bit 0 = segment 0, bit 1 = segment 1, etc.)
    selected_mask: u32,
    /// Hovered segment index (0xFFFFFFFF = none)
    hovered_segment: u32,
    /// Padding / flags (unused)
    _flags: u32,
    /// Corner radius for pill shape
    corner_radius: f32,
    /// Time for animations
    time: f32,
    /// Padding
    _padding: [f32; 2],
    /// Background color from theme
    bg_color: [f32; 4],
    /// Segment widths (up to 8 segments, packed as 2 x vec4)
    segment_widths: [[f32; 4]; 2],
}

/// The shader program for the segmented control background
#[derive(Clone)]
pub struct SegmentedControlProgram {
    pub segment_widths: Vec<f32>,
    pub selected_mask: u32,
    pub hovered_index: Arc<AtomicU32>,
    pub bg_color: Color,
}

impl SegmentedControlProgram {
    /// Get segment index at cursor position
    #[allow(dead_code)]
    pub fn segment_at(&self, cursor_pos: Point, bounds: Rectangle) -> Option<usize> {
        if cursor_pos.y < 0.0 || cursor_pos.y >= bounds.height {
            return None;
        }

        let mut x = BORDER_WIDTH;
        for (idx, &width) in self.segment_widths.iter().enumerate() {
            if cursor_pos.x >= x && cursor_pos.x < x + width {
                return Some(idx);
            }
            x += width;
        }
        None
    }
}

impl shader::Program<SegmentedControlMessage<usize>> for SegmentedControlProgram {
    type State = ();
    type Primitive = SegmentedControlPrimitive;

    fn draw(&self, _state: &Self::State, _cursor: Cursor, _bounds: Rectangle) -> Self::Primitive {
        let hovered_raw = self.hovered_index.load(Ordering::Relaxed);
        let hovered_index = if hovered_raw == NO_HOVER { None } else { Some(hovered_raw as usize) };

        SegmentedControlPrimitive {
            segment_widths: self.segment_widths.clone(),
            selected_mask: self.selected_mask,
            hovered_index,
            bg_color: self.bg_color,
            uniform_offset_bytes: Arc::new(AtomicU32::new(0)),
        }
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: Cursor,
    ) -> Option<iced::widget::Action<SegmentedControlMessage<usize>>> {
        let _ = (state, event, bounds, cursor);
        None
    }

    fn mouse_interaction(&self, _state: &Self::State, _bounds: Rectangle, _cursor: Cursor) -> mouse::Interaction {
        // Die Interaktion wird vom Canvas-Overlay (oben) gehandhabt.
        mouse::Interaction::default()
    }
}

/// Primitive for rendering
#[derive(Clone, Debug)]
pub struct SegmentedControlPrimitive {
    pub segment_widths: Vec<f32>,
    pub selected_mask: u32,
    pub hovered_index: Option<usize>,
    pub bg_color: Color,
    /// Dynamic uniform offset (bytes) into the shared uniform buffer.
    /// Needed because iced may batch `prepare` for multiple primitives before rendering.
    uniform_offset_bytes: Arc<AtomicU32>,
}

impl shader::Primitive for SegmentedControlPrimitive {
    type Pipeline = SegmentedControlRenderer;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        _device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        bounds: &Rectangle,
        viewport: &iced::advanced::graphics::Viewport,
    ) {
        let scale = viewport.scale_factor();
        // `@builtin(position)` and scissor/viewport rects are in physical pixels.
        // Convert logical bounds to physical and round to match integer scissor rects.
        let origin_x = (bounds.x * scale).round();
        let origin_y = (bounds.y * scale).round();
        let size_w = (bounds.width * scale).round().max(1.0);
        let size_h = (bounds.height * scale).round().max(1.0);

        #[cfg(debug_assertions)]
        {
            SEGMENTED_CONTROL_DEBUG_PRINT_ONCE.call_once(|| {
                eprintln!(
                    "[segmented_control] origin=({:.1},{:.1}) size=({:.1},{:.1}) num_segments={} selected_mask={:08b} hovered={:?} widths={:?}",
                    origin_x,
                    origin_y,
                    size_w,
                    size_h,
                    self.segment_widths.len(),
                    self.selected_mask,
                    self.hovered_index,
                    self.segment_widths
                );
            });
        }

        // Pack segment widths into 2 x vec4
        let mut segment_widths = [[0.0f32; 4]; 2];
        for (i, &width) in self.segment_widths.iter().take(MAX_SEGMENTS).enumerate() {
            segment_widths[i / 4][i % 4] = width * scale;
        }

        let uniforms = SegmentedControlUniforms {
            widget_origin: [origin_x, origin_y],
            widget_size: [size_w, size_h],
            num_segments: self.segment_widths.len() as u32,
            selected_mask: self.selected_mask,
            hovered_segment: self.hovered_index.map(|i| i as u32).unwrap_or(0xFFFFFFFF),
            _flags: 0,
            corner_radius: CORNER_RADIUS * scale,
            time: 0.0,
            _padding: [0.0, 0.0],
            bg_color: [self.bg_color.r, self.bg_color.g, self.bg_color.b, self.bg_color.a],
            segment_widths,
        };

        // Allocate a unique uniform slot for this primitive.
        let slot = pipeline.next_uniform.fetch_add(1, Ordering::Relaxed) % pipeline.uniform_capacity;
        let offset = (slot as u64) * pipeline.uniform_stride;
        self.uniform_offset_bytes.store(offset as u32, Ordering::Relaxed);

        queue.write_buffer(&pipeline.uniform_buffer, offset, bytemuck::bytes_of(&uniforms));
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        let mut pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
            label: Some("Segmented Control Render Pass"),
            color_attachments: &[Some(iced::wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: iced::wgpu::Operations {
                    load: iced::wgpu::LoadOp::Load,
                    store: iced::wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if clip_bounds.width > 0 && clip_bounds.height > 0 {
            pass.set_scissor_rect(clip_bounds.x, clip_bounds.y, clip_bounds.width, clip_bounds.height);
            pass.set_viewport(
                clip_bounds.x as f32,
                clip_bounds.y as f32,
                clip_bounds.width as f32,
                clip_bounds.height as f32,
                0.0,
                1.0,
            );
            pass.set_pipeline(&pipeline.pipeline);
            let offset = self.uniform_offset_bytes.load(Ordering::Relaxed);
            pass.set_bind_group(0, &pipeline.bind_group, &[offset]);
            pass.draw(0..6, 0..1);
        }
    }
}

// Implement bytemuck traits for uniform buffer
unsafe impl bytemuck::Pod for SegmentedControlUniforms {}
unsafe impl bytemuck::Zeroable for SegmentedControlUniforms {}

/// GPU renderer for the segmented control background
pub struct SegmentedControlRenderer {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group: iced::wgpu::BindGroup,
    uniform_buffer: iced::wgpu::Buffer,
    uniform_stride: u64,
    uniform_capacity: u32,
    next_uniform: AtomicU32,
}

fn align_up(value: u64, alignment: u64) -> u64 {
    if alignment == 0 {
        return value;
    }
    ((value + alignment - 1) / alignment) * alignment
}

// ═══════════════════════════════════════════════════════════════════════════
// GPU Char Overlay (for Char segments)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone)]
struct SegmentedCharGlyphProgram {
    char_segments: Vec<(usize, char)>,
    segment_widths: Vec<f32>,
    selected_index: usize,
    font: Option<BitFont>,
    char_colors: Option<CharColors>,
}

impl<T: Clone> shader::Program<SegmentedControlMessage<T>> for SegmentedCharGlyphProgram {
    type State = ();
    type Primitive = SegmentedCharGlyphPrimitive;

    fn draw(&self, _state: &Self::State, _cursor: Cursor, _bounds: Rectangle) -> Self::Primitive {
        SegmentedCharGlyphPrimitive {
            char_segments: self.char_segments.clone(),
            segment_widths: self.segment_widths.clone(),
            selected_index: self.selected_index,
            font: self.font.clone(),
            char_colors: self.char_colors.clone(),
            uniform_offset_bytes: Arc::new(AtomicU32::new(0)),
            instance_offset_bytes: Arc::new(AtomicU32::new(0)),
            instance_count: Arc::new(AtomicU32::new(0)),
        }
    }

    fn update(
        &self,
        _state: &mut Self::State,
        _event: &iced::Event,
        _bounds: Rectangle,
        _cursor: Cursor,
    ) -> Option<iced::widget::Action<SegmentedControlMessage<T>>> {
        None
    }

    fn mouse_interaction(&self, _state: &Self::State, _bounds: Rectangle, _cursor: Cursor) -> mouse::Interaction {
        // Interaction is handled by the Canvas overlay.
        mouse::Interaction::default()
    }
}

#[derive(Clone, Debug)]
struct SegmentedCharGlyphPrimitive {
    char_segments: Vec<(usize, char)>,
    segment_widths: Vec<f32>,
    selected_index: usize,
    font: Option<BitFont>,
    char_colors: Option<CharColors>,
    /// Dynamic uniform offset (bytes) into the shared uniform buffer.
    uniform_offset_bytes: Arc<AtomicU32>,
    /// Byte offset into the shared instance ring buffer.
    instance_offset_bytes: Arc<AtomicU32>,
    /// Number of instances to draw for this primitive.
    instance_count: Arc<AtomicU32>,
}

impl shader::Primitive for SegmentedCharGlyphPrimitive {
    type Pipeline = SegmentedCharGlyphRenderer;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        bounds: &Rectangle,
        viewport: &iced::advanced::graphics::Viewport,
    ) {
        let scale = viewport.scale_factor();

        let size_w = (bounds.width * scale).round().max(1.0);
        let size_h = (bounds.height * scale).round().max(1.0);

        #[cfg(debug_assertions)]
        {
            static CHAR_GLYPH_DEBUG_ONCE: Once = Once::new();
            CHAR_GLYPH_DEBUG_ONCE.call_once(|| {
                eprintln!(
                    "[segmented_char_glyph] font={} char_segments={:?} char_colors={}",
                    self.font.is_some(),
                    self.char_segments,
                    self.char_colors.is_some()
                );
            });
        }

        // Upload/update atlas if necessary.
        let (glyph_w, glyph_h) = if let Some(font) = &self.font {
            let key = font_key(font);
            if pipeline.atlas_key != Some(key) {
                let (aw, ah, rgba) = build_glyph_atlas_rgba(font);
                pipeline.update_atlas(device, queue, key, aw, ah, &rgba);
            }
            let s = font.size();
            (s.width.max(1) as f32, s.height.max(1) as f32)
        } else {
            // No font: nothing to render.
            #[cfg(debug_assertions)]
            eprintln!("[segmented_char_glyph] No font, skipping render");
            self.instance_count.store(0, Ordering::Relaxed);
            return;
        };

        // Update uniforms
        let uniforms = GlyphUniforms {
            clip_size: [size_w, size_h],
            atlas_size: [pipeline.atlas_w as f32, pipeline.atlas_h as f32],
            glyph_size: [glyph_w, glyph_h],
            _pad: [0.0, 0.0],
        };
        let uniform_slot = pipeline.next_uniform.fetch_add(1, Ordering::Relaxed) % pipeline.uniform_capacity;
        let uniform_offset = (uniform_slot as u64) * pipeline.uniform_stride;
        self.uniform_offset_bytes.store(uniform_offset as u32, Ordering::Relaxed);
        queue.write_buffer(&pipeline.uniform_buffer, uniform_offset, bytemuck::bytes_of(&uniforms));

        // Build instances in clip-local coordinates; snap to physical pixels.
        let font_h = glyph_h.max(1.0);
        let font_w = glyph_w.max(1.0);

        // HiDPI-safe integer magnification: choose an integer scale in *physical pixels*.
        // Otherwise, e.g. 2x logical magnify at 1.25 scale becomes 2.5x physical and produces artifacts.
        let target_preview_h_px = (PREVIEW_GLYPH_HEIGHT * scale).round().max(1.0);
        let preview_magnify_px = (target_preview_h_px / font_h).floor().max(1.0);
        let preview_glyph_w_px = (font_w * preview_magnify_px).round().max(1.0);
        let preview_glyph_h_px = (font_h * preview_magnify_px).round().max(1.0);

        let control_h_px = ((bounds.height - SHADOW_PADDING * 2.0 - BORDER_WIDTH * 2.0) * scale).round().max(1.0);
        let base_y_px = ((SHADOW_PADDING + BORDER_WIDTH) * scale).floor() + ((control_h_px - preview_glyph_h_px) / 2.0).floor();

        let content_x = SHADOW_PADDING + BORDER_WIDTH;

        let default_text = Color::from_rgb(0.85, 0.85, 0.85);
        let selected_text = Color::WHITE;
        let shadow = Color::from_rgba(0.0, 0.0, 0.0, 0.5);

        let mut instances: Vec<GlyphInstance> = Vec::with_capacity(self.char_segments.len() * 3);

        for (seg_idx, ch) in self.char_segments.iter().copied() {
            // segment start
            let mut seg_start_x = content_x;
            for w in self.segment_widths.iter().take(seg_idx) {
                seg_start_x += *w;
            }
            let seg_w = self.segment_widths.get(seg_idx).copied().unwrap_or(0.0);
            if seg_w <= 0.0 {
                continue;
            }

            // Convert layout to physical pixels and center the glyph quad in pixels.
            let seg_start_x_px = (seg_start_x * scale).floor();
            let seg_w_px = (seg_w * scale).floor().max(1.0);

            let px = (seg_start_x_px + ((seg_w_px - preview_glyph_w_px) / 2.0).floor()).floor();
            let py = base_y_px.floor();
            let pw = preview_glyph_w_px;
            let ph = preview_glyph_h_px;

            let glyph = cp437_index(ch) & 0xFF;

            if let Some(ref colors) = self.char_colors {
                #[cfg(debug_assertions)]
                {
                    static COLOR_DEBUG_ONCE: Once = Once::new();
                    COLOR_DEBUG_ONCE.call_once(|| {
                        eprintln!(
                            "[segmented_char_glyph] glyph={} fg=({:.2},{:.2},{:.2}) bg=({:.2},{:.2},{:.2}) pos=({:.1},{:.1}) size=({:.1},{:.1})",
                            glyph, colors.fg.r, colors.fg.g, colors.fg.b, colors.bg.r, colors.bg.g, colors.bg.b, px, py, pw, ph
                        );
                    });
                }
                // Background fill + glyph (caret colors)
                instances.push(GlyphInstance {
                    pos: [px, py],
                    size: [pw, ph],
                    fg: [0.0, 0.0, 0.0, 0.0],
                    bg: [colors.bg.r, colors.bg.g, colors.bg.b, 1.0],
                    glyph: 0,
                    flags: FLAG_BG_ONLY,
                    _pad: [0, 0],
                });

                instances.push(GlyphInstance {
                    pos: [px, py],
                    size: [pw, ph],
                    fg: [colors.fg.r, colors.fg.g, colors.fg.b, 1.0],
                    bg: [0.0, 0.0, 0.0, 0.0],
                    glyph,
                    flags: 0,
                    _pad: [0, 0],
                });
            } else {
                let fg = if seg_idx == self.selected_index { selected_text } else { default_text };

                // Shadow
                instances.push(GlyphInstance {
                    pos: [(px + 1.0).floor(), (py + 1.0).floor()],
                    size: [pw, ph],
                    fg: [shadow.r, shadow.g, shadow.b, shadow.a],
                    bg: [0.0, 0.0, 0.0, 0.0],
                    glyph,
                    flags: 0,
                    _pad: [0, 0],
                });

                // Foreground
                instances.push(GlyphInstance {
                    pos: [px, py],
                    size: [pw, ph],
                    fg: [fg.r, fg.g, fg.b, 1.0],
                    bg: [0.0, 0.0, 0.0, 0.0],
                    glyph,
                    flags: 0,
                    _pad: [0, 0],
                });
            }
        }

        // Upload instances into a fixed-size ring slot to avoid last-write-wins.
        let slot = pipeline.next_instance_slot.fetch_add(1, Ordering::Relaxed) % pipeline.instance_slots;
        let instance_offset = (slot as u64) * pipeline.instance_slot_stride;
        self.instance_offset_bytes.store(instance_offset as u32, Ordering::Relaxed);

        let max_instances = pipeline.instance_capacity_per_primitive as usize;
        let count = instances.len().min(max_instances);
        self.instance_count.store(count as u32, Ordering::Relaxed);
        if count == 0 {
            return;
        }
        let bytes = bytemuck::cast_slice(&instances[..count]);
        queue.write_buffer(&pipeline.instance_buffer, instance_offset, bytes);
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        let instance_count = self.instance_count.load(Ordering::Relaxed);
        if instance_count == 0 {
            return;
        }

        let mut pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
            label: Some("Segmented Char Glyph Render Pass"),
            color_attachments: &[Some(iced::wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: iced::wgpu::Operations {
                    load: iced::wgpu::LoadOp::Load,
                    store: iced::wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if clip_bounds.width > 0 && clip_bounds.height > 0 {
            pass.set_scissor_rect(clip_bounds.x, clip_bounds.y, clip_bounds.width, clip_bounds.height);
            pass.set_viewport(
                clip_bounds.x as f32,
                clip_bounds.y as f32,
                clip_bounds.width as f32,
                clip_bounds.height as f32,
                0.0,
                1.0,
            );

            pass.set_pipeline(&pipeline.pipeline);
            let uniform_offset = self.uniform_offset_bytes.load(Ordering::Relaxed);
            pass.set_bind_group(0, &pipeline.bind_group, &[uniform_offset]);
            pass.set_vertex_buffer(0, pipeline.quad_vertex_buffer.slice(..));
            let instance_offset = self.instance_offset_bytes.load(Ordering::Relaxed) as u64;
            let instance_bytes = (instance_count as u64) * pipeline.instance_stride;
            pass.set_vertex_buffer(1, pipeline.instance_buffer.slice(instance_offset..(instance_offset + instance_bytes)));
            pass.draw(0..6, 0..instance_count);
        }
    }
}

pub struct SegmentedCharGlyphRenderer {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group: iced::wgpu::BindGroup,
    uniform_buffer: iced::wgpu::Buffer,
    uniform_stride: u64,
    uniform_capacity: u32,
    next_uniform: AtomicU32,
    quad_vertex_buffer: iced::wgpu::Buffer,
    instance_buffer: iced::wgpu::Buffer,
    instance_stride: u64,
    instance_capacity_per_primitive: u32,
    instance_slots: u32,
    instance_slot_stride: u64,
    next_instance_slot: AtomicU32,

    atlas_texture: iced::wgpu::Texture,
    atlas_view: iced::wgpu::TextureView,
    atlas_sampler: iced::wgpu::Sampler,
    atlas_key: Option<u64>,
    atlas_w: u32,
    atlas_h: u32,
}

impl SegmentedCharGlyphRenderer {
    fn update_atlas(&mut self, device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, key: u64, w: u32, h: u32, rgba: &[u8]) {
        if self.atlas_w != w || self.atlas_h != h {
            self.atlas_texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                label: Some("Segmented Glyph Atlas"),
                size: iced::wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: iced::wgpu::TextureDimension::D2,
                format: iced::wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: iced::wgpu::TextureUsages::TEXTURE_BINDING | iced::wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.atlas_view = self.atlas_texture.create_view(&iced::wgpu::TextureViewDescriptor::default());
            self.atlas_w = w;
            self.atlas_h = h;

            let bind_group_layout = self.pipeline.get_bind_group_layout(0);
            self.bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                label: Some("Segmented Glyph Bind Group"),
                layout: &bind_group_layout,
                entries: &[
                    iced::wgpu::BindGroupEntry {
                        binding: 0,
                        resource: iced::wgpu::BindingResource::Buffer(iced::wgpu::BufferBinding {
                            buffer: &self.uniform_buffer,
                            offset: 0,
                            size: NonZeroU64::new(std::mem::size_of::<GlyphUniforms>() as u64),
                        }),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 1,
                        resource: iced::wgpu::BindingResource::TextureView(&self.atlas_view),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 2,
                        resource: iced::wgpu::BindingResource::Sampler(&self.atlas_sampler),
                    },
                ],
            });
        }

        queue.write_texture(
            iced::wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: iced::wgpu::Origin3d::ZERO,
                aspect: iced::wgpu::TextureAspect::All,
            },
            rgba,
            iced::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            iced::wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        self.atlas_key = Some(key);
    }
}

impl shader::Pipeline for SegmentedCharGlyphRenderer {
    fn new(device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("Segmented Glyphs Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("fkey_glyphs_shader.wgsl").into()),
        });

        let uniform_size = std::mem::size_of::<GlyphUniforms>() as u64;
        let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        let uniform_stride = align_up(uniform_size, alignment);
        let uniform_capacity: u32 = 1024;
        let uniform_buffer_size = uniform_stride * (uniform_capacity as u64);

        let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("Segmented Glyphs Uniforms (Dynamic)"),
            size: uniform_buffer_size,
            usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let atlas_texture = device.create_texture(&iced::wgpu::TextureDescriptor {
            label: Some("Segmented Glyph Atlas (init)"),
            size: iced::wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: iced::wgpu::TextureDimension::D2,
            format: iced::wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: iced::wgpu::TextureUsages::TEXTURE_BINDING | iced::wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            iced::wgpu::TexelCopyTextureInfo {
                texture: &atlas_texture,
                mip_level: 0,
                origin: iced::wgpu::Origin3d::ZERO,
                aspect: iced::wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 0],
            iced::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            iced::wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let atlas_view = atlas_texture.create_view(&iced::wgpu::TextureViewDescriptor::default());

        let atlas_sampler = device.create_sampler(&iced::wgpu::SamplerDescriptor {
            label: Some("Segmented Glyph Atlas Sampler"),
            mag_filter: iced::wgpu::FilterMode::Nearest,
            min_filter: iced::wgpu::FilterMode::Nearest,
            mipmap_filter: iced::wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let quad: [QuadVertex; 6] = [
            QuadVertex {
                unit_pos: [0.0, 0.0],
                unit_uv: [0.0, 0.0],
            },
            QuadVertex {
                unit_pos: [1.0, 0.0],
                unit_uv: [1.0, 0.0],
            },
            QuadVertex {
                unit_pos: [0.0, 1.0],
                unit_uv: [0.0, 1.0],
            },
            QuadVertex {
                unit_pos: [0.0, 1.0],
                unit_uv: [0.0, 1.0],
            },
            QuadVertex {
                unit_pos: [1.0, 0.0],
                unit_uv: [1.0, 0.0],
            },
            QuadVertex {
                unit_pos: [1.0, 1.0],
                unit_uv: [1.0, 1.0],
            },
        ];
        let quad_vertex_buffer = device.create_buffer_init(&iced::wgpu::util::BufferInitDescriptor {
            label: Some("Segmented Glyph Quad"),
            contents: bytemuck::cast_slice(&quad),
            usage: iced::wgpu::BufferUsages::VERTEX,
        });

        let instance_stride = std::mem::size_of::<GlyphInstance>() as u64;
        // Fixed max instances per primitive (char segments only; small). Increase if needed.
        let instance_capacity_per_primitive: u32 = 64;
        let instance_slots: u32 = 1024;
        let instance_slot_stride = instance_stride * (instance_capacity_per_primitive as u64);
        let instance_buffer_size = instance_slot_stride * (instance_slots as u64);

        let instance_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("Segmented Glyph Instances (Ring)"),
            size: instance_buffer_size,
            usage: iced::wgpu::BufferUsages::VERTEX | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("Segmented Glyph Bind Group Layout"),
            entries: &[
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: iced::wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: iced::wgpu::BindingType::Buffer {
                        ty: iced::wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(uniform_size),
                    },
                    count: None,
                },
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: iced::wgpu::ShaderStages::FRAGMENT,
                    ty: iced::wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: iced::wgpu::TextureViewDimension::D2,
                        sample_type: iced::wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: iced::wgpu::ShaderStages::FRAGMENT,
                    ty: iced::wgpu::BindingType::Sampler(iced::wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
            label: Some("Segmented Glyph Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                iced::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: iced::wgpu::BindingResource::Buffer(iced::wgpu::BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: NonZeroU64::new(uniform_size),
                    }),
                },
                iced::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: iced::wgpu::BindingResource::TextureView(&atlas_view),
                },
                iced::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: iced::wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&iced::wgpu::PipelineLayoutDescriptor {
            label: Some("Segmented Glyph Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_buffers = [
            iced::wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<QuadVertex>() as u64,
                step_mode: iced::wgpu::VertexStepMode::Vertex,
                attributes: &[
                    iced::wgpu::VertexAttribute {
                        format: iced::wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    },
                    iced::wgpu::VertexAttribute {
                        format: iced::wgpu::VertexFormat::Float32x2,
                        offset: 8,
                        shader_location: 1,
                    },
                ],
            },
            iced::wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<GlyphInstance>() as u64,
                step_mode: iced::wgpu::VertexStepMode::Instance,
                attributes: &[
                    iced::wgpu::VertexAttribute {
                        format: iced::wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 2,
                    },
                    iced::wgpu::VertexAttribute {
                        format: iced::wgpu::VertexFormat::Float32x2,
                        offset: 8,
                        shader_location: 3,
                    },
                    iced::wgpu::VertexAttribute {
                        format: iced::wgpu::VertexFormat::Float32x4,
                        offset: 16,
                        shader_location: 4,
                    },
                    iced::wgpu::VertexAttribute {
                        format: iced::wgpu::VertexFormat::Float32x4,
                        offset: 32,
                        shader_location: 5,
                    },
                    iced::wgpu::VertexAttribute {
                        format: iced::wgpu::VertexFormat::Uint32,
                        offset: 48,
                        shader_location: 6,
                    },
                    iced::wgpu::VertexAttribute {
                        format: iced::wgpu::VertexFormat::Uint32,
                        offset: 52,
                        shader_location: 7,
                    },
                ],
            },
        ];

        let pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some("Segmented Glyph Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: iced::wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(iced::wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(iced::wgpu::ColorTargetState {
                    format,
                    blend: Some(iced::wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: iced::wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: iced::wgpu::PrimitiveState {
                topology: iced::wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: iced::wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group,
            uniform_buffer,
            uniform_stride,
            uniform_capacity,
            next_uniform: AtomicU32::new(0),
            quad_vertex_buffer,
            instance_buffer,
            instance_stride,
            instance_capacity_per_primitive,
            instance_slots,
            instance_slot_stride,
            next_instance_slot: AtomicU32::new(0),
            atlas_texture,
            atlas_view,
            atlas_sampler,
            atlas_key: None,
            atlas_w: 1,
            atlas_h: 1,
        }
    }
}

impl shader::Pipeline for SegmentedControlRenderer {
    fn new(device: &iced::wgpu::Device, _queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("Segmented Control Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("segmented_control_shader.wgsl").into()),
        });

        let uniform_size = std::mem::size_of::<SegmentedControlUniforms>() as u64;
        let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        let uniform_stride = align_up(uniform_size, alignment);
        // Large enough for typical UI; avoids "last write wins" when iced batches.
        let uniform_capacity: u32 = 1024;
        let uniform_buffer_size = uniform_stride * (uniform_capacity as u64);

        // Create uniform buffer (array) for dynamic offsets
        let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("Segmented Control Uniforms (Dynamic)"),
            size: uniform_buffer_size,
            usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("Segmented Control Bind Group Layout"),
            entries: &[iced::wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: iced::wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: iced::wgpu::BindingType::Buffer {
                    ty: iced::wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: NonZeroU64::new(uniform_size),
                },
                count: None,
            }],
        });

        // Create bind group
        let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
            label: Some("Segmented Control Bind Group"),
            layout: &bind_group_layout,
            entries: &[iced::wgpu::BindGroupEntry {
                binding: 0,
                resource: iced::wgpu::BindingResource::Buffer(iced::wgpu::BufferBinding {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: NonZeroU64::new(uniform_size),
                }),
            }],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&iced::wgpu::PipelineLayoutDescriptor {
            label: Some("Segmented Control Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some("Segmented Control Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: iced::wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(iced::wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(iced::wgpu::ColorTargetState {
                    format, // Use the format passed to the pipeline
                    blend: Some(iced::wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: iced::wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: iced::wgpu::PrimitiveState {
                topology: iced::wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: iced::wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group,
            uniform_buffer,
            uniform_stride,
            uniform_capacity,
            next_uniform: AtomicU32::new(0),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Canvas Text Overlay
// ═══════════════════════════════════════════════════════════════════════════

/// Optional char colors for Char segments (caret fg/bg)
#[derive(Clone, Debug, Default)]
pub struct CharColors {
    pub fg: Color,
    pub bg: Color,
}

/// Canvas program for drawing text over the shader background
struct TextOverlayProgram<T: Clone + PartialEq> {
    segments: Vec<Segment<T>>,
    segment_widths: Vec<f32>,
    selected_index: usize,
    font: Option<BitFont>,
    cache: Cache,
    hovered_index: Arc<AtomicU32>,
    /// Optional colors for Char segments (caret fg/bg with background fill)
    char_colors: Option<CharColors>,
    /// Whether Char segments are drawn by this Canvas overlay
    draw_char_segments: bool,
}

impl<T: Clone + PartialEq + Send + 'static> canvas::Program<SegmentedControlMessage<T>> for TextOverlayProgram<T> {
    type State = ();

    fn draw(&self, _state: &Self::State, renderer: &iced::Renderer, _theme: &Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let text_color = Color::from_rgb(0.85, 0.85, 0.85);
            let selected_text_color = Color::WHITE;
            // Drop shadow color (same style as icon shadows in tool panel)
            let shadow_color = Color::from_rgba(0.0, 0.0, 0.0, 0.5);
            let shadow_offset = (1.0, 1.0); // Subtle offset for text

            let char_w = self.font.as_ref().map(|f| f.size().width as f32).unwrap_or(8.0);
            let char_h = self.font.as_ref().map(|f| f.size().height as f32).unwrap_or(16.0);
            let pixel_size = 1.0; // Native size for text segments
            // Integer magnification for preview glyph: fit into PREVIEW_GLYPH_HEIGHT
            let preview_glyph_magnify = (PREVIEW_GLYPH_HEIGHT / char_h).floor().max(1.0);

            // Match shader content rect (and FKey toolbar): exclude shadow padding + border.
            let content_x = SHADOW_PADDING + BORDER_WIDTH;
            let content_y = SHADOW_PADDING + BORDER_WIDTH;
            let content_h = bounds.height - SHADOW_PADDING * 2.0 - BORDER_WIDTH * 2.0;

            // Draw text for each segment
            let mut x = content_x;
            for (idx, segment) in self.segments.iter().enumerate() {
                let width = self.segment_widths.get(idx).copied().unwrap_or(0.0);
                let is_selected = idx == self.selected_index;

                match &segment.content {
                    SegmentContent::Text(text) => {
                        let color = if is_selected { selected_text_color } else { text_color };
                        // Calculate text position (centered in segment)
                        let text_width = text.chars().count() as f32 * char_w;
                        let text_x = x + (width - text_width) / 2.0;
                        let text_y = (content_y + (content_h - char_h) / 2.0).floor();

                        // Draw text shadow first (offset down-right)
                        self.draw_text(
                            frame,
                            text_x + shadow_offset.0,
                            text_y + shadow_offset.1,
                            text,
                            shadow_color,
                            char_w,
                            pixel_size,
                        );

                        // Draw text
                        self.draw_text(frame, text_x, text_y, text, color, char_w, pixel_size);
                    }
                    SegmentContent::Char(ch) => {
                        if !self.draw_char_segments {
                            x += width;
                            continue;
                        }
                        // Preview glyph: integer magnification + pixel-aligned positioning
                        let preview_glyph_w = char_w * preview_glyph_magnify;
                        let preview_glyph_h = char_h * preview_glyph_magnify;

                        // Center the preview glyph in the segment (floor for crisp edges)
                        let glyph_x = (x + (width - preview_glyph_w) / 2.0).floor();
                        let glyph_y = (content_y + (content_h - preview_glyph_h) / 2.0).floor();

                        if let Some(ref char_colors) = self.char_colors {
                            // Draw background fill with caret bg color
                            frame.fill_rectangle(Point::new(glyph_x, glyph_y), Size::new(preview_glyph_w, preview_glyph_h), char_colors.bg);

                            // Draw preview glyph with caret fg color
                            self.draw_glyph(frame, glyph_x, glyph_y, *ch, char_colors.fg, preview_glyph_magnify);
                        } else {
                            // No char colors provided - use default colors with shadow
                            let color = if is_selected { selected_text_color } else { text_color };

                            // Draw shadow
                            self.draw_glyph(
                                frame,
                                glyph_x + shadow_offset.0,
                                glyph_y + shadow_offset.1,
                                *ch,
                                shadow_color,
                                preview_glyph_magnify,
                            );

                            // Draw preview glyph
                            self.draw_glyph(frame, glyph_x, glyph_y, *ch, color, preview_glyph_magnify);
                        }
                    }
                }

                x += width;
            }
        });

        vec![geometry]
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: Cursor,
    ) -> Option<iced::widget::Action<SegmentedControlMessage<T>>> {
        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let new_hover = cursor.position_in(bounds).and_then(|pos| segment_at_x(pos.x, &self.segment_widths));

                let new_raw = new_hover.map(|i| i as u32).unwrap_or(NO_HOVER);
                let old_raw = self.hovered_index.swap(new_raw, Ordering::Relaxed);

                if old_raw != new_raw {
                    // Text ändert sich nicht, aber das spart "stale" Frames bei manchen Backends.
                    self.cache.clear();
                    #[cfg(debug_assertions)]
                    {
                        eprintln!(
                            "[segmented_control] hover_change: old={:?} new={:?}",
                            if old_raw == NO_HOVER { None } else { Some(old_raw) },
                            new_hover
                        );
                    }
                    return Some(iced::widget::Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::CursorLeft) => {
                let old_raw = self.hovered_index.swap(NO_HOVER, Ordering::Relaxed);
                if old_raw != NO_HOVER {
                    self.cache.clear();
                    #[cfg(debug_assertions)]
                    {
                        eprintln!("[segmented_control] hover_clear (CursorLeft)");
                    }
                    return Some(iced::widget::Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(pos) = cursor.position_in(bounds) else {
                    return None;
                };

                let Some(idx) = segment_at_x(pos.x, &self.segment_widths) else {
                    return None;
                };

                self.cache.clear();

                // If clicking on a Char segment that is already selected, send CharClicked
                // to trigger the character picker popup
                let is_char_segment = matches!(&self.segments[idx].content, SegmentContent::Char(_));
                let is_already_selected = idx == self.selected_index;

                if is_char_segment && is_already_selected {
                    Some(iced::widget::Action::publish(SegmentedControlMessage::CharClicked(
                        self.segments[idx].value.clone(),
                    )))
                } else {
                    Some(iced::widget::Action::publish(SegmentedControlMessage::Selected(
                        self.segments[idx].value.clone(),
                    )))
                }
            }
            _ => None,
        }
    }

    fn mouse_interaction(&self, _state: &Self::State, bounds: Rectangle, cursor: Cursor) -> mouse::Interaction {
        if cursor.position_in(bounds).is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<T: Clone + PartialEq> TextOverlayProgram<T> {
    /// Draw text using the font
    fn draw_text(&self, frame: &mut Frame, x: f32, y: f32, text: &str, color: Color, char_w: f32, pixel_size: f32) {
        for (i, ch) in text.chars().enumerate() {
            self.draw_glyph(frame, x + i as f32 * char_w, y, ch, color, pixel_size);
        }
    }

    /// Draw a single glyph with horizontal run optimization
    fn draw_glyph(&self, frame: &mut Frame, x: f32, y: f32, ch: char, fg: Color, pixel_size: f32) {
        let Some(font) = &self.font else {
            return;
        };
        let Some(glyph) = font.glyph(ch) else {
            return;
        };

        for (row_idx, row) in glyph.bitmap.pixels.iter().enumerate() {
            let row_y = y + row_idx as f32 * pixel_size;
            let mut run_start: Option<usize> = None;

            for (col_idx, &pixel) in row.iter().enumerate() {
                if pixel {
                    if run_start.is_none() {
                        run_start = Some(col_idx);
                    }
                } else if let Some(start) = run_start {
                    let run_len = col_idx - start;
                    frame.fill_rectangle(
                        Point::new(x + start as f32 * pixel_size, row_y),
                        Size::new(run_len as f32 * pixel_size, pixel_size),
                        fg,
                    );
                    run_start = None;
                }
            }
            // Draw final run if row ends with pixels
            if let Some(start) = run_start {
                let run_len = row.len() - start;
                frame.fill_rectangle(
                    Point::new(x + start as f32 * pixel_size, row_y),
                    Size::new(run_len as f32 * pixel_size, pixel_size),
                    fg,
                );
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Multi-Select Canvas Text Overlay
// ═══════════════════════════════════════════════════════════════════════════

/// Canvas program for drawing text over the shader background in multi-select mode
struct MultiSelectTextOverlayProgram<T: Clone + PartialEq> {
    segments: Vec<Segment<T>>,
    segment_widths: Vec<f32>,
    selected_mask: u32,
    font: Option<BitFont>,
    cache: Cache,
    hovered_index: Arc<AtomicU32>,
    /// Optional colors for Char segments (caret fg/bg with background fill)
    char_colors: Option<CharColors>,
}

impl<T: Clone + PartialEq + Send + 'static> canvas::Program<SegmentedControlMessage<T>> for MultiSelectTextOverlayProgram<T> {
    type State = ();

    fn draw(&self, _state: &Self::State, renderer: &iced::Renderer, _theme: &Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let text_color = Color::from_rgb(0.85, 0.85, 0.85);
            let selected_text_color = Color::WHITE;
            let shadow_color = Color::from_rgba(0.0, 0.0, 0.0, 0.5);
            let shadow_offset = (1.0, 1.0);

            let char_w = self.font.as_ref().map(|f| f.size().width as f32).unwrap_or(8.0);
            let char_h = self.font.as_ref().map(|f| f.size().height as f32).unwrap_or(16.0);
            let pixel_size = 1.0;

            let content_x = SHADOW_PADDING + BORDER_WIDTH;
            let content_y = SHADOW_PADDING + BORDER_WIDTH;
            let content_h = bounds.height - SHADOW_PADDING * 2.0 - BORDER_WIDTH * 2.0;

            let mut x = content_x;
            for (idx, segment) in self.segments.iter().enumerate() {
                let width = self.segment_widths.get(idx).copied().unwrap_or(0.0);
                // Check selection via bitmask
                let is_selected = (self.selected_mask & (1u32 << idx)) != 0;

                match &segment.content {
                    SegmentContent::Text(text) => {
                        let color = if is_selected { selected_text_color } else { text_color };
                        let text_width = text.chars().count() as f32 * char_w;
                        let text_x = x + (width - text_width) / 2.0;
                        let text_y = (content_y + (content_h - char_h) / 2.0).floor();

                        // Shadow
                        self.draw_text(
                            frame,
                            text_x + shadow_offset.0,
                            text_y + shadow_offset.1,
                            text,
                            shadow_color,
                            char_w,
                            pixel_size,
                        );
                        // Text
                        self.draw_text(frame, text_x, text_y, text, color, char_w, pixel_size);
                    }
                    SegmentContent::Char(_) => {
                        // Char segments are drawn by GPU overlay, skip here
                    }
                }

                x += width;
            }
        });

        vec![geometry]
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: Cursor,
    ) -> Option<iced::widget::Action<SegmentedControlMessage<T>>> {
        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let new_hover = cursor.position_in(bounds).and_then(|pos| segment_at_x(pos.x, &self.segment_widths));
                let new_raw = new_hover.map(|i| i as u32).unwrap_or(NO_HOVER);
                let old_raw = self.hovered_index.swap(new_raw, Ordering::Relaxed);

                if old_raw != new_raw {
                    self.cache.clear();
                    return Some(iced::widget::Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::CursorLeft) => {
                let old_raw = self.hovered_index.swap(NO_HOVER, Ordering::Relaxed);
                if old_raw != NO_HOVER {
                    self.cache.clear();
                    return Some(iced::widget::Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(pos) = cursor.position_in(bounds) else {
                    return None;
                };

                let Some(idx) = segment_at_x(pos.x, &self.segment_widths) else {
                    return None;
                };

                self.cache.clear();

                // In multi-select mode, always toggle
                Some(iced::widget::Action::publish(SegmentedControlMessage::Toggled(
                    self.segments[idx].value.clone(),
                )))
            }
            _ => None,
        }
    }

    fn mouse_interaction(&self, _state: &Self::State, bounds: Rectangle, cursor: Cursor) -> mouse::Interaction {
        if cursor.position_in(bounds).is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<T: Clone + PartialEq> MultiSelectTextOverlayProgram<T> {
    fn draw_text(&self, frame: &mut Frame, x: f32, y: f32, text: &str, color: Color, char_w: f32, pixel_size: f32) {
        for (i, ch) in text.chars().enumerate() {
            self.draw_glyph(frame, x + i as f32 * char_w, y, ch, color, pixel_size);
        }
    }

    fn draw_glyph(&self, frame: &mut Frame, x: f32, y: f32, ch: char, fg: Color, pixel_size: f32) {
        let Some(font) = &self.font else {
            return;
        };
        let Some(glyph) = font.glyph(ch) else {
            return;
        };

        for (row_idx, row) in glyph.bitmap.pixels.iter().enumerate() {
            let row_y = y + row_idx as f32 * pixel_size;
            let mut run_start: Option<usize> = None;

            for (col_idx, &pixel) in row.iter().enumerate() {
                if pixel {
                    if run_start.is_none() {
                        run_start = Some(col_idx);
                    }
                } else if let Some(start) = run_start {
                    let run_len = col_idx - start;
                    frame.fill_rectangle(
                        Point::new(x + start as f32 * pixel_size, row_y),
                        Size::new(run_len as f32 * pixel_size, pixel_size),
                        fg,
                    );
                    run_start = None;
                }
            }
            if let Some(start) = run_start {
                let run_len = row.len() - start;
                frame.fill_rectangle(
                    Point::new(x + start as f32 * pixel_size, row_y),
                    Size::new(run_len as f32 * pixel_size, pixel_size),
                    fg,
                );
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════════════════════════

/// GPU-accelerated segmented control with shader background
pub struct ShaderSegmentedControl {
    hovered_index: Arc<AtomicU32>,
}

impl Default for ShaderSegmentedControl {
    fn default() -> Self {
        Self::new()
    }
}

impl ShaderSegmentedControl {
    pub fn new() -> Self {
        Self {
            hovered_index: Arc::new(AtomicU32::new(NO_HOVER)),
        }
    }

    /// Clear the text render cache (no-op, cache is created per view)
    #[allow(dead_code)]
    pub fn clear_cache(&mut self) {
        // Cache is owned by TextOverlayProgram now
    }

    /// Render the segmented control
    /// Returns an Element with usize-based messages that need to be mapped to the actual type
    pub fn view<T: Clone + PartialEq + Send + 'static>(
        &self,
        segments: Vec<Segment<T>>,
        selected: T,
        font: Option<BitFont>,
        theme: &Theme,
    ) -> Element<'_, SegmentedControlMessage<T>> {
        self.view_internal(segments, selected, font, theme, None)
    }

    /// Render the segmented control with char colors for Char segments
    /// The fg_color and bg_color are palette indices that will be used for the caret colors
    pub fn view_with_char_colors<T: Clone + PartialEq + Send + 'static>(
        &self,
        segments: Vec<Segment<T>>,
        selected: T,
        font: Option<BitFont>,
        theme: &Theme,
        fg_color: u32,
        bg_color: u32,
        palette: &Palette,
    ) -> Element<'_, SegmentedControlMessage<T>> {
        // Convert palette indices to iced::Color
        let (r, g, b) = palette.rgb(fg_color);
        let fg = Color::from_rgb8(r, g, b);
        let (r, g, b) = palette.rgb(bg_color);
        let bg = Color::from_rgb8(r, g, b);

        self.view_internal(segments, selected, font, theme, Some(CharColors { fg, bg }))
    }

    /// Render the segmented control in multi-select mode
    /// Multiple segments can be selected/toggled independently (like checkboxes)
    /// `selected_indices` is a slice of indices that are currently selected
    pub fn view_multi_select<T: Clone + PartialEq + Send + 'static>(
        &self,
        segments: Vec<Segment<T>>,
        selected_indices: &[usize],
        font: Option<BitFont>,
        theme: &Theme,
    ) -> Element<'_, SegmentedControlMessage<T>> {
        self.view_multi_select_internal(segments, selected_indices, font, theme, None)
    }

    /// Internal multi-select view implementation
    fn view_multi_select_internal<T: Clone + PartialEq + Send + 'static>(
        &self,
        segments: Vec<Segment<T>>,
        selected_indices: &[usize],
        font: Option<BitFont>,
        theme: &Theme,
        char_colors: Option<CharColors>,
    ) -> Element<'_, SegmentedControlMessage<T>> {
        // Calculate segment widths
        let segment_widths: Vec<f32> = segments.iter().map(|seg| calculate_segment_width(seg, &font)).collect();
        // Content width + border + shadow padding on both sides
        let content_width = segment_widths.iter().sum::<f32>();
        let total_width = content_width + BORDER_WIDTH * 2.0 + SHADOW_PADDING * 2.0;
        let total_height = SEGMENT_HEIGHT + SHADOW_PADDING * 2.0;

        // Build selected_mask from indices
        let mut selected_mask = 0u32;
        for &idx in selected_indices {
            if idx < MAX_SEGMENTS {
                selected_mask |= 1u32 << idx;
            }
        }

        // For text overlay highlighting, use the first selected index (or 0 if none)
        let first_selected_index = selected_indices.first().copied().unwrap_or(0);

        // Use the same main-area background as the editor canvas/preview areas.
        let bg_color = main_area_background(theme);

        // Clone segment values for message mapping
        let segment_values: Vec<T> = segments.iter().map(|s| s.value.clone()).collect();

        // Convert segments to usize-valued segments for the shader widget.
        let segments_idx: Vec<Segment<usize>> = segments
            .into_iter()
            .enumerate()
            .map(|(i, s)| Segment { content: s.content, value: i })
            .collect();

        // One-pass shader (background + glyphs)
        let shader_onepass: Element<'_, SegmentedControlMessage<usize>> = widget::shader(SegmentedOnePassProgram::<usize> {
            segment_widths: segment_widths.clone(),
            selected_mask,
            hovered_index: self.hovered_index.clone(),
            bg_color,
            segments: segments_idx,
            font,
            char_colors,
            selected_index: first_selected_index,
            multi_select: true,
        })
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .into();

        // Map shader messages from usize to T (use Toggled for multi-select)
        shader_onepass.map(move |msg| match msg {
            SegmentedControlMessage::Selected(idx) => SegmentedControlMessage::Toggled(segment_values[idx].clone()),
            SegmentedControlMessage::Toggled(idx) => SegmentedControlMessage::Toggled(segment_values[idx].clone()),
            SegmentedControlMessage::CharClicked(idx) => SegmentedControlMessage::CharClicked(segment_values[idx].clone()),
        })
    }

    /// Internal view implementation with optional char colors
    fn view_internal<T: Clone + PartialEq + Send + 'static>(
        &self,
        segments: Vec<Segment<T>>,
        selected: T,
        font: Option<BitFont>,
        theme: &Theme,
        char_colors: Option<CharColors>,
    ) -> Element<'_, SegmentedControlMessage<T>> {
        // Calculate segment widths
        let segment_widths: Vec<f32> = segments.iter().map(|seg| calculate_segment_width(seg, &font)).collect();
        // Content width + border + shadow padding on both sides
        let content_width = segment_widths.iter().sum::<f32>();
        let total_width = content_width + BORDER_WIDTH * 2.0 + SHADOW_PADDING * 2.0;
        let total_height = SEGMENT_HEIGHT + SHADOW_PADDING * 2.0;

        // Find selected index
        let selected_index = segments.iter().position(|seg| seg.value == selected).unwrap_or(0);
        // Convert to bitmask for shader (single selection = single bit)
        let selected_mask = 1u32 << selected_index;

        // Use the same main-area background as the editor canvas/preview areas.
        let bg_color = main_area_background(theme);

        // Clone segment values for message mapping
        let segment_values: Vec<T> = segments.iter().map(|s| s.value.clone()).collect();

        // Convert segments to usize-valued segments for the shader widget.
        let segments_idx: Vec<Segment<usize>> = segments
            .into_iter()
            .enumerate()
            .map(|(i, s)| Segment { content: s.content, value: i })
            .collect();

        // One-pass shader (background + glyphs)
        let shader_onepass: Element<'_, SegmentedControlMessage<usize>> = widget::shader(SegmentedOnePassProgram::<usize> {
            segment_widths: segment_widths.clone(),
            selected_mask,
            hovered_index: self.hovered_index.clone(),
            bg_color,
            segments: segments_idx,
            font,
            char_colors,
            selected_index,
            multi_select: false,
        })
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .into();

        shader_onepass.map(move |msg| match msg {
            SegmentedControlMessage::Selected(idx) => SegmentedControlMessage::Selected(segment_values[idx].clone()),
            SegmentedControlMessage::Toggled(idx) => SegmentedControlMessage::Toggled(segment_values[idx].clone()),
            SegmentedControlMessage::CharClicked(idx) => SegmentedControlMessage::CharClicked(segment_values[idx].clone()),
        })
    }
}

/// Magnification factor for Char segments (2x)
const CHAR_MAGNIFICATION: f32 = 2.0;

/// Calculate segment width - uses native font size (pixel_size = 1) for text,
/// and 2x magnification for Char segments
fn calculate_segment_width<T: Clone>(segment: &Segment<T>, font: &Option<BitFont>) -> f32 {
    let font_width = font.as_ref().map(|f| f.size().width as f32).unwrap_or(8.0);

    let content_width = match &segment.content {
        SegmentContent::Text(text) => text.chars().count() as f32 * font_width,
        SegmentContent::Char(_) => font_width * CHAR_MAGNIFICATION,
    };

    content_width + SEGMENT_PADDING_H * 2.0
}
