//! GPU-accelerated Segmented Control (one-pass)
//!
//! Uses a WGSL shader to render both background and glyphs in a single pass.
//!
//! NOTE: Some fields are prepared for atlas caching.

#![allow(dead_code)]

use super::layout::{
    BORDER_WIDTH, CORNER_RADIUS, MAX_SEGMENTS, NO_HOVER, PREVIEW_GLYPH_HEIGHT, SEGMENT_FONT_SCALE, SEGMENT_HEIGHT, SEGMENT_PADDING_H, SHADOW_PADDING,
};
use crate::ui::editor::ansi::widget::layer_view::glyph_renderer::{build_glyph_atlas_rgba, cp437_index, font_key, GlyphInstance, QuadVertex, FLAG_DRAW_BG};
use iced::wgpu::util::DeviceExt;
use iced::{
    mouse::{self, Cursor},
    widget::{self, canvas, shader},
    Color, Element, Length, Rectangle, Theme,
};
use icy_engine::{BitFont, Palette};
use icy_engine_gui::theme::main_area_background;
use std::num::NonZeroU64;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
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
    text_color_selected: Color,
    text_color_unselected: Color,
    segments: Vec<Segment<T>>,
    font: Option<BitFont>,
    char_colors: Option<CharColors>,
    // For CharClicked in single-select: the current selected index
    selected_index: usize,
    multi_select: bool,
    render_text_labels: bool,
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

        let default_text = self.text_color_unselected;
        let selected_text = self.text_color_selected;
        let shadow = Color::from_rgba(0.0, 0.0, 0.0, 0.5);

        // Text glyph size in physical pixels (scaled + snapped)
        let text_glyph_w_px = (glyph_w * scale * SEGMENT_FONT_SCALE).round().max(1.0);
        let text_glyph_h_px = (glyph_h * scale * SEGMENT_FONT_SCALE).round().max(1.0);
        // Use rounding instead of floor to avoid a systematic upward bias when centering.
        let text_y_px = (content_y_px + ((content_h_px - text_glyph_h_px) * 0.5).round()).round();

        // Preview magnification for Char segments (integer in physical pixels)
        let target_preview_h_px = (PREVIEW_GLYPH_HEIGHT * scale).round().max(1.0);
        let preview_magnify_px = (target_preview_h_px / glyph_h.max(1.0)).floor().max(1.0);
        let preview_glyph_w_px = (glyph_w.max(1.0) * preview_magnify_px).round().max(1.0);
        let preview_glyph_h_px = (glyph_h.max(1.0) * preview_magnify_px).round().max(1.0);
        let preview_y_px = (content_y_px + ((content_h_px - preview_glyph_h_px) * 0.5).round()).round();

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
                    if !self.render_text_labels {
                        seg_start_x += seg_w;
                        continue;
                    }
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

                    // Char preview always uses char_colors (fixed colors from caret/palette)
                    if let Some(ref colors) = self.char_colors {
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
                        // Fallback: use default text color with shadow
                        let fg = default_text;
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
            text_color_selected: self.text_color_selected,
            text_color_unselected: self.text_color_unselected,
            segments: self.segments.clone(),
            font: self.font.clone(),
            char_colors: self.char_colors.clone(),
            selected_index: self.selected_index,
            multi_select: self.multi_select,
            render_text_labels: self.render_text_labels,
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
            iced::Event::Mouse(mouse::Event::ButtonPressed { button: mouse::Button::Left, .. }) => {
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
    text_color_selected: Color,
    text_color_unselected: Color,
    segments: Vec<Segment<T>>,
    font: Option<BitFont>,
    char_colors: Option<CharColors>,
    selected_index: usize,
    multi_select: bool,
    render_text_labels: bool,
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
            text_color_selected: self.text_color_selected,
            text_color_unselected: self.text_color_unselected,
            segments: self.segments.clone(),
            font: self.font.clone(),
            char_colors: self.char_colors.clone(),
            selected_index: self.selected_index,
            multi_select: self.multi_select,
            render_text_labels: self.render_text_labels,
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
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
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

    pub fn char(c: char) -> Self {
        Self::Char(c)
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

/// Optional char colors for Char segments (caret fg/bg)
#[derive(Clone, Debug, Default)]
pub struct CharColors {
    pub fg: Color,
    pub bg: Color,
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

    /// Clear internal caches (no-op; one-pass renderer has no per-widget cache)

    pub fn clear_cache(&mut self) {
        // Intentionally empty.
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
        let font = Some(resolve_font(font));

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

        // Text colors: match selected segment background (primary) vs. unselected.
        let text_color_selected = theme.extended_palette().primary.base.text;
        let text_color_unselected = theme.extended_palette().secondary.base.color;

        // Clone segment values for message mapping
        let segment_values: Vec<T> = segments.iter().map(|s| s.value.clone()).collect();

        // Keep segment contents for the TTF overlay (before `segments` gets moved).
        let overlay_segments: Vec<SegmentContent> = segments.iter().map(|s| s.content.clone()).collect();

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
            text_color_selected,
            text_color_unselected,
            segments: segments_idx,
            font,
            char_colors,
            selected_index: first_selected_index,
            multi_select: true,
            render_text_labels: false,
        })
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .into();

        let overlay: Element<'_, SegmentedControlMessage<usize>> = canvas(SegmentedTtfOverlay {
            segment_widths: segment_widths.clone(),
            segments: overlay_segments,
            selected_mask,
            hovered_index: self.hovered_index.clone(),
            text_color_selected,
            text_color_unselected,
        })
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .into();

        let stacked: Element<'_, SegmentedControlMessage<usize>> = iced::widget::stack![shader_onepass, overlay]
            .width(Length::Fixed(total_width))
            .height(Length::Fixed(total_height))
            .into();

        // Map shader messages from usize to T (use Toggled for multi-select)
        stacked.map(move |msg| match msg {
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
        let font = Some(resolve_font(font));

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

        // Text colors: match selected segment background (primary) vs. unselected.
        let text_color_selected = theme.extended_palette().primary.base.text;
        let text_color_unselected = theme.extended_palette().secondary.base.color;

        // Clone segment values for message mapping
        let segment_values: Vec<T> = segments.iter().map(|s| s.value.clone()).collect();

        // Keep segment contents for the TTF overlay (before `segments` gets moved).
        let overlay_segments: Vec<SegmentContent> = segments.iter().map(|s| s.content.clone()).collect();

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
            text_color_selected,
            text_color_unselected,
            segments: segments_idx,
            font,
            char_colors,
            selected_index,
            multi_select: false,
            render_text_labels: false,
        })
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .into();

        let overlay: Element<'_, SegmentedControlMessage<usize>> = canvas(SegmentedTtfOverlay {
            segment_widths: segment_widths.clone(),
            segments: overlay_segments,
            selected_mask,
            hovered_index: self.hovered_index.clone(),
            text_color_selected,
            text_color_unselected,
        })
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .into();

        let stacked: Element<'_, SegmentedControlMessage<usize>> = iced::widget::stack![shader_onepass, overlay]
            .width(Length::Fixed(total_width))
            .height(Length::Fixed(total_height))
            .into();

        stacked.map(move |msg| match msg {
            SegmentedControlMessage::Selected(idx) => SegmentedControlMessage::Selected(segment_values[idx].clone()),
            SegmentedControlMessage::Toggled(idx) => SegmentedControlMessage::Toggled(segment_values[idx].clone()),
            SegmentedControlMessage::CharClicked(idx) => SegmentedControlMessage::CharClicked(segment_values[idx].clone()),
        })
    }
}

#[derive(Clone, Debug)]
struct SegmentedTtfOverlay {
    segment_widths: Vec<f32>,
    segments: Vec<SegmentContent>,
    selected_mask: u32,
    hovered_index: Arc<AtomicU32>,
    text_color_selected: Color,
    text_color_unselected: Color,
}

impl canvas::Program<SegmentedControlMessage<usize>> for SegmentedTtfOverlay {
    type State = ();

    fn draw(&self, _state: &Self::State, renderer: &iced::Renderer, _theme: &Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let hovered_raw = self.hovered_index.load(Ordering::Relaxed);
        let hovered_idx = if hovered_raw == NO_HOVER { None } else { Some(hovered_raw as usize) };

        let content_x = SHADOW_PADDING + BORDER_WIDTH;
        let content_y = SHADOW_PADDING + BORDER_WIDTH;
        let content_h = (bounds.height - SHADOW_PADDING * 2.0 - BORDER_WIDTH * 2.0).max(1.0);

        let mut seg_x = content_x;
        for (idx, seg_w) in self.segment_widths.iter().copied().enumerate() {
            let Some(content) = self.segments.get(idx) else {
                seg_x += seg_w;
                continue;
            };

            if let SegmentContent::Text(text) = content {
                let is_selected = (self.selected_mask & (1u32 << idx)) != 0;
                let is_hovered = hovered_idx == Some(idx);

                let mut color = if is_selected { self.text_color_selected } else { self.text_color_unselected };
                if is_hovered {
                    color = Color::from_rgba((color.r * 1.08).min(1.0), (color.g * 1.08).min(1.0), (color.b * 1.08).min(1.0), color.a);
                }

                frame.fill_text(canvas::Text {
                    content: text.clone(),
                    position: iced::Point::new(seg_x + seg_w / 2.0, content_y + content_h / 2.0),
                    color,
                    size: 14.0.into(),
                    font: iced::Font::default(),
                    align_x: iced::alignment::Horizontal::Center.into(),
                    align_y: iced::alignment::Vertical::Center.into(),
                    ..Default::default()
                });
            }

            seg_x += seg_w;
        }

        vec![frame.into_geometry()]
    }
}

/// Magnification factor for Char segments (2x)
const CHAR_MAGNIFICATION: f32 = 2.0;

/// Calculate segment width - uses native font size (pixel_size = 1) for text,
/// and 2x magnification for Char segments
fn calculate_segment_width<T: Clone>(segment: &Segment<T>, font: &Option<BitFont>) -> f32 {
    let font_width = font.as_ref().map(|f| f.size().width as f32).unwrap_or(8.0) * SEGMENT_FONT_SCALE;

    let content_width = match &segment.content {
        SegmentContent::Text(text) => text.chars().count() as f32 * font_width,
        SegmentContent::Char(_) => font_width * CHAR_MAGNIFICATION,
    };

    content_width + SEGMENT_PADDING_H * 2.0
}

fn align_up(value: u64, alignment: u64) -> u64 {
    if alignment == 0 {
        return value;
    }
    ((value + alignment - 1) / alignment) * alignment
}

fn resolve_font(font: Option<BitFont>) -> BitFont {
    font.unwrap_or_default()
}
