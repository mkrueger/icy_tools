//! GPU-accelerated F-Key Toolbar Component
//!
//! Renders F1-F12 function key slots with characters from the current font.
//! Uses WGSL shader for background (drop shadow, borders, hover highlights)
//! and glyph atlas shader for text rendering (labels, chars, arrows).

use std::num::NonZeroU64;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use codepages::tables::CP437_TO_UNICODE;
use iced::wgpu::util::DeviceExt;
use iced::{
    Color, Element, Length, Point, Rectangle, Theme,
    mouse::{self, Cursor},
    widget::{self, container, shader},
};
use icy_engine::{BitFont, Palette};
use icy_engine_gui::theme::main_area_background;

use super::fkey_layout::{
    ARROW_SIZE, BORDER_WIDTH, CORNER_RADIUS, FKeyLayout, HoverState, LABEL_HEIGHT, LABEL_WIDTH, LEFT_PADDING, NAV_GAP, NAV_NEXT_SHIFT_X, NAV_NUM_SHIFT_X,
    NAV_SIZE, NO_HOVER, SET_NUM_ICON_GAP, SHADOW_PADDING, SLOT_CHAR_HEIGHT, SLOT_SPACING, SLOT_WIDTH,
};
use crate::ui::FKeySets;

fn align_up(value: u64, alignment: u64) -> u64 {
    if alignment == 0 {
        return value;
    }
    ((value + alignment - 1) / alignment) * alignment
}

// ═══════════════════════════════════════════════════════════════════════════
// Messages
// ═══════════════════════════════════════════════════════════════════════════

/// Messages from the F-key toolbar (same as Canvas version for drop-in replacement)
#[derive(Clone, Debug)]
pub enum FKeyToolbarMessage {
    /// Click on F-key slot to type character
    TypeFKey(usize),
    /// Click on F-key label to open character selector popup
    OpenCharSelector(usize),
    /// Navigate to previous F-key set
    PrevSet,
    /// Navigate to next F-key set
    NextSet,
}

// ═══════════════════════════════════════════════════════════════════════════
// GPU Shader Types
// ═══════════════════════════════════════════════════════════════════════════

// Flag for fkey background instance (must match shader)
const FLAG_FKEY_BG: u32 = 16;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct FKeyToolbarUniforms {
    widget_origin: [f32; 2],
    widget_size: [f32; 2],
    num_slots: u32,
    hovered_slot: u32,
    hover_type: u32,
    _flags: u32,
    corner_radius: f32,
    time: f32,
    slot_width: f32,
    slot_spacing: f32,
    bg_color: [f32; 4],
    content_start_x: f32,
    label_width: f32,
    nav_start_x: f32,
    nav_size: f32,
}

unsafe impl bytemuck::Pod for FKeyToolbarUniforms {}
unsafe impl bytemuck::Zeroable for FKeyToolbarUniforms {}

/// The shader program for the F-Key toolbar background
#[derive(Clone)]
pub struct FKeyToolbarProgram {
    pub bg_color: Color,
    pub hovered_slot: Arc<AtomicU32>,
    pub hover_type: Arc<AtomicU32>,
    pub nav_label_space_bits: Arc<AtomicU32>,
}

impl shader::Program<FKeyToolbarMessage> for FKeyToolbarProgram {
    type State = ();
    type Primitive = FKeyToolbarPrimitive;

    fn draw(&self, _state: &Self::State, _cursor: Cursor, bounds: Rectangle) -> Self::Primitive {
        FKeyToolbarPrimitive {
            bounds,
            bg_color: self.bg_color,
            hovered_slot: self.hovered_slot.load(Ordering::Relaxed),
            hover_type: self.hover_type.load(Ordering::Relaxed),
            uniform_offset_bytes: Arc::new(AtomicU32::new(0)),
        }
    }

    fn update(&self, _state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: Cursor) -> Option<iced::widget::Action<FKeyToolbarMessage>> {
        let nav_label_space = f32::from_bits(self.nav_label_space_bits.load(Ordering::Relaxed)).max(0.0);
        // Handle mouse movement for hover state
        if let Some(pos) = cursor.position_in(bounds) {
            let (slot, hover_type) = compute_hover_state(pos, bounds, nav_label_space);
            self.hovered_slot.store(slot, Ordering::Relaxed);
            self.hover_type.store(hover_type, Ordering::Relaxed);
        } else {
            // Mouse outside bounds - clear hover
            self.hovered_slot.store(NO_HOVER, Ordering::Relaxed);
            self.hover_type.store(0, Ordering::Relaxed);
        }

        // Handle mouse clicks
        if let iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            if let Some(pos) = cursor.position_in(bounds) {
                let (slot, hover_type) = compute_hover_state(pos, bounds, nav_label_space);

                // Click on slot
                if slot != NO_HOVER {
                    let is_on_char = hover_type == 1;
                    if is_on_char {
                        // Click on character area - type the F-key
                        return Some(iced::widget::Action::publish(FKeyToolbarMessage::TypeFKey(slot as usize)));
                    } else {
                        // Click on label area - open character selector
                        return Some(iced::widget::Action::publish(FKeyToolbarMessage::OpenCharSelector(slot as usize)));
                    }
                }

                // Click on nav buttons
                if hover_type == 2 {
                    return Some(iced::widget::Action::publish(FKeyToolbarMessage::PrevSet));
                }
                if hover_type == 3 {
                    return Some(iced::widget::Action::publish(FKeyToolbarMessage::NextSet));
                }
            }
        }

        None
    }

    fn mouse_interaction(&self, _state: &Self::State, bounds: Rectangle, cursor: Cursor) -> mouse::Interaction {
        if let Some(pos) = cursor.position_in(bounds) {
            let nav_label_space = f32::from_bits(self.nav_label_space_bits.load(Ordering::Relaxed)).max(0.0);
            let (slot, hover_type) = compute_hover_state(pos, bounds, nav_label_space);
            // Show pointer cursor over clickable elements
            if slot != NO_HOVER || hover_type == 2 || hover_type == 3 {
                return mouse::Interaction::Pointer;
            }
        }
        mouse::Interaction::default()
    }
}

/// Primitive for rendering
#[derive(Clone, Debug)]
pub struct FKeyToolbarPrimitive {
    pub bounds: Rectangle,
    pub bg_color: Color,
    pub hovered_slot: u32,
    pub hover_type: u32,
    /// Dynamic uniform offset (bytes) into the shared uniform buffer.
    uniform_offset_bytes: Arc<AtomicU32>,
}

impl shader::Primitive for FKeyToolbarPrimitive {
    type Pipeline = FKeyToolbarRenderer;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        _device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        bounds: &Rectangle,
        viewport: &iced::advanced::graphics::Viewport,
    ) {
        let scale = viewport.scale_factor();
        let origin_x = (bounds.x * scale).round();
        let origin_y = (bounds.y * scale).round();
        let size_w = (bounds.width * scale).round().max(1.0);
        let size_h = (bounds.height * scale).round().max(1.0);

        let content_start_x = (SHADOW_PADDING + BORDER_WIDTH + LEFT_PADDING) * scale;
        // IMPORTANT: 12 slots, but only 11 spacings.
        let slots_width = (12.0 * SLOT_WIDTH + 11.0 * SLOT_SPACING) * scale;
        let nav_start_x = content_start_x + slots_width + NAV_GAP * scale;

        let uniforms = FKeyToolbarUniforms {
            widget_origin: [origin_x, origin_y],
            widget_size: [size_w, size_h],
            num_slots: 12,
            hovered_slot: self.hovered_slot,
            hover_type: self.hover_type,
            _flags: 0,
            corner_radius: CORNER_RADIUS * scale,
            time: 0.0,
            slot_width: SLOT_WIDTH * scale,
            slot_spacing: SLOT_SPACING * scale,
            bg_color: [self.bg_color.r, self.bg_color.g, self.bg_color.b, self.bg_color.a],
            content_start_x,
            label_width: LABEL_WIDTH * scale,
            nav_start_x,
            nav_size: NAV_SIZE * scale,
        };

        let slot = pipeline.next_uniform.fetch_add(1, Ordering::Relaxed) % pipeline.uniform_capacity;
        let offset = (slot as u64) * pipeline.uniform_stride;
        self.uniform_offset_bytes.store(offset as u32, Ordering::Relaxed);
        queue.write_buffer(&pipeline.uniform_buffer, offset, bytemuck::bytes_of(&uniforms));
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        let mut pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
            label: Some("FKey Toolbar Render Pass"),
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

/// GPU renderer for the F-Key toolbar background
pub struct FKeyToolbarRenderer {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group: iced::wgpu::BindGroup,
    uniform_buffer: iced::wgpu::Buffer,
    uniform_stride: u64,
    uniform_capacity: u32,
    next_uniform: AtomicU32,
}

impl shader::Pipeline for FKeyToolbarRenderer {
    fn new(device: &iced::wgpu::Device, _queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("FKey Toolbar Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("fkey_toolbar_shader.wgsl").into()),
        });

        let uniform_size = std::mem::size_of::<FKeyToolbarUniforms>() as u64;
        let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        let uniform_stride = align_up(uniform_size, alignment);
        let uniform_capacity: u32 = 1024;
        let uniform_buffer_size = uniform_stride * (uniform_capacity as u64);

        let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("FKey Toolbar Uniforms (Dynamic)"),
            size: uniform_buffer_size,
            usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("FKey Toolbar Bind Group Layout"),
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

        let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
            label: Some("FKey Toolbar Bind Group"),
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

        let pipeline_layout = device.create_pipeline_layout(&iced::wgpu::PipelineLayoutDescriptor {
            label: Some("FKey Toolbar Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some("FKey Toolbar Pipeline"),
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
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Glyph Atlas Overlay (GPU)
// ═══════════════════════════════════════════════════════════════════════════

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FKeyGlyphUniforms {
    clip_size: [f32; 2],
    atlas_size: [f32; 2],
    glyph_size: [f32; 2],
    _pad: [f32; 2],
}

unsafe impl bytemuck::Pod for FKeyGlyphUniforms {}
unsafe impl bytemuck::Zeroable for FKeyGlyphUniforms {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct QuadVertex {
    unit_pos: [f32; 2],
    unit_uv: [f32; 2],
}

unsafe impl bytemuck::Pod for QuadVertex {}
unsafe impl bytemuck::Zeroable for QuadVertex {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct GlyphInstance {
    pos: [f32; 2],
    size: [f32; 2],
    fg: [f32; 4],
    bg: [f32; 4],
    glyph: u32,
    flags: u32,
    _pad: [u32; 2],
}

unsafe impl bytemuck::Pod for GlyphInstance {}
unsafe impl bytemuck::Zeroable for GlyphInstance {}

pub(crate) fn font_key(font: &BitFont) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    font.name().hash(&mut hasher);
    let size = font.size();
    size.width.hash(&mut hasher);
    size.height.hash(&mut hasher);
    // This is intentionally a heuristic key; enough to avoid constant re-uploads.
    font.is_default().hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn build_glyph_atlas_rgba(font: &BitFont) -> (u32, u32, Vec<u8>) {
    let size = font.size();
    let gw = size.width.max(1) as u32;
    let gh = size.height.max(1) as u32;
    let atlas_w = gw * 16;
    let atlas_h = gh * 16;
    let mut rgba = vec![0u8; (atlas_w * atlas_h * 4) as usize];

    for code in 0u32..256u32 {
        // Fonts in the wild differ:
        // - some label glyphs by 0..255 "codepoint" slots (CP/ANSI index)
        // - others label glyphs by Unicode (e.g. box-drawing U+250C)
        // We build the atlas by CP437 index, but try both lookup strategies.
        let slot_ch = char::from_u32(code).unwrap_or(' ');
        let unicode_ch = CP437_TO_UNICODE.get(code as usize).copied().unwrap_or(' ');
        let col = (code % 16) as u32;
        let row = (code / 16) as u32;
        let base_x = col * gw;
        let base_y = row * gh;

        // default transparent
        if let Some(glyph) = font.glyph(slot_ch).or_else(|| font.glyph(unicode_ch)) {
            for y in 0..gh as usize {
                let dst_y = base_y as usize + y;
                if dst_y >= atlas_h as usize {
                    continue;
                }
                let src_row = glyph.bitmap.pixels.get(y);
                for x in 0..gw as usize {
                    let dst_x = base_x as usize + x;
                    if dst_x >= atlas_w as usize {
                        continue;
                    }
                    let on = src_row.and_then(|r| r.get(x)).copied().unwrap_or(false);
                    let idx = ((dst_y * atlas_w as usize + dst_x) * 4) as usize;
                    rgba[idx + 0] = 255;
                    rgba[idx + 1] = 255;
                    rgba[idx + 2] = 255;
                    rgba[idx + 3] = if on { 255 } else { 0 };
                }
            }
        }
    }

    (atlas_w, atlas_h, rgba)
}

pub(crate) fn cp437_index(ch: char) -> u32 {
    if (ch as u32) <= 0xFF {
        return ch as u32;
    }

    CP437_TO_UNICODE.iter().position(|&c| c == ch).map(|idx| idx as u32).unwrap_or(b'?' as u32)
}

#[derive(Clone)]
pub(crate) struct FKeyGlyphProgram {
    pub fkeys: FKeySets,
    pub font: Option<BitFont>,
    pub palette: Palette,
    pub fg_color: u32,
    pub bg_color: u32,
    pub hovered_slot: Arc<AtomicU32>,
    pub hover_type: Arc<AtomicU32>,
}

impl shader::Program<FKeyToolbarMessage> for FKeyGlyphProgram {
    type State = ();
    type Primitive = FKeyGlyphPrimitive;

    fn draw(&self, _state: &Self::State, _cursor: Cursor, bounds: Rectangle) -> Self::Primitive {
        FKeyGlyphPrimitive {
            bounds,
            fkeys: self.fkeys.clone(),
            font: self.font.clone(),
            palette: self.palette.clone(),
            fg_color: self.fg_color,
            bg_color: self.bg_color,
            hovered_slot: self.hovered_slot.load(Ordering::Relaxed),
            hover_type: self.hover_type.load(Ordering::Relaxed),
            uniform_offset_bytes: Arc::new(AtomicU32::new(0)),
            instance_offset_bytes: Arc::new(AtomicU32::new(0)),
            instance_count: Arc::new(AtomicU32::new(0)),
        }
    }

    fn update(&self, _state: &mut Self::State, _event: &iced::Event, _bounds: Rectangle, _cursor: Cursor) -> Option<iced::widget::Action<FKeyToolbarMessage>> {
        // Interaction is handled by the background shader program.
        None
    }

    fn mouse_interaction(&self, _state: &Self::State, _bounds: Rectangle, _cursor: Cursor) -> mouse::Interaction {
        mouse::Interaction::default()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FKeyGlyphPrimitive {
    pub bounds: Rectangle,
    pub fkeys: FKeySets,
    pub font: Option<BitFont>,
    pub palette: Palette,
    pub fg_color: u32,
    pub bg_color: u32,
    pub hovered_slot: u32,
    pub hover_type: u32,
    /// Dynamic uniform offset (bytes) into the shared uniform buffer.
    uniform_offset_bytes: Arc<AtomicU32>,
    /// Byte offset into the shared instance ring buffer.
    instance_offset_bytes: Arc<AtomicU32>,
    /// Number of instances to draw for this primitive.
    instance_count: Arc<AtomicU32>,
}

impl shader::Primitive for FKeyGlyphPrimitive {
    type Pipeline = FKeyGlyphRenderer;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        bounds: &Rectangle,
        viewport: &iced::advanced::graphics::Viewport,
    ) {
        let scale = viewport.scale_factor();

        // Clip is provided via the render() scissor/viewport. We compute positions in that clip-local coordinate space.
        let origin_x = (bounds.x * scale).round();
        let origin_y = (bounds.y * scale).round();
        let size_w = (bounds.width * scale).round().max(1.0);
        let size_h = (bounds.height * scale).round().max(1.0);

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
            (8.0, 16.0)
        };

        // Update uniforms
        let uniforms = FKeyGlyphUniforms {
            clip_size: [size_w, size_h],
            atlas_size: [pipeline.atlas_w as f32, pipeline.atlas_h as f32],
            glyph_size: [glyph_w, glyph_h],
            _pad: [0.0, 0.0],
        };
        let uniform_slot = pipeline.next_uniform.fetch_add(1, Ordering::Relaxed) % pipeline.uniform_capacity;
        let uniform_offset = (uniform_slot as u64) * pipeline.uniform_stride;
        self.uniform_offset_bytes.store(uniform_offset as u32, Ordering::Relaxed);
        queue.write_buffer(&pipeline.uniform_buffer, uniform_offset, bytemuck::bytes_of(&uniforms));

        // Build instances (positions are clip-local pixels)
        let (fg_r, fg_g, fg_b) = self.palette.rgb(self.fg_color);
        let (bg_r, bg_g, bg_b) = self.palette.rgb(self.bg_color);
        let fg = Color::from_rgb8(fg_r, fg_g, fg_b);
        let bg = Color::from_rgb8(bg_r, bg_g, bg_b);

        let hovered = HoverState::from_uniforms(self.hovered_slot, self.hover_type);
        let set_idx = self.fkeys.current_set();

        const FLAG_DRAW_BG: u32 = 1;
        const FLAG_BG_ONLY: u32 = 2;

        // HiDPI-safe integer magnification in *physical pixels*.
        // Otherwise, a logical 2x magnify at scale 1.25 becomes 2.5x physical and produces "fragments".
        let control_height_px = ((bounds.height - SHADOW_PADDING * 2.0) * scale).round().max(1.0);
        let font_height = glyph_h.max(1.0);
        let font_width = glyph_w.max(1.0);

        let target_slot_char_h_px = (SLOT_CHAR_HEIGHT * scale).round().max(1.0);
        let slot_char_magnify = (target_slot_char_h_px / font_height).floor().max(1.0);
        let target_label_h_px = LABEL_HEIGHT * scale;
        let max_label_magnify = (target_label_h_px / font_height).floor().max(1.0);

        // Effective sizes in physical pixels
        let slot_char_w = (font_width * slot_char_magnify).round().max(1.0);
        let slot_char_h = (font_height * slot_char_magnify).round().max(1.0);
        let label_render_w = (font_width * max_label_magnify).round().max(1.0);
        let label_render_h = (font_height * max_label_magnify).round().max(1.0);
        let label_char_w = label_render_w;

        let content_start_x = ((SHADOW_PADDING + BORDER_WIDTH + LEFT_PADDING) * scale).floor();
        let slot_char_y = (SHADOW_PADDING * scale).floor() + ((control_height_px - slot_char_h) / 2.0).floor();
        let label_y = (SHADOW_PADDING * scale).floor() + ((control_height_px - label_render_h) / 2.0).floor();

        // Small cache for digit y-offsets (0..9) for label scale.
        let mut digit_offset: [f32; 10] = [0.0; 10];
        let mut digit_offset_set: [bool; 10] = [false; 10];

        let mut instances: Vec<GlyphInstance> = Vec::with_capacity(96);

        // Helper: compute y-offset using font bitmap bounds (only for digits)
        let mut glyph_y_offset = |digit: u32| -> f32 {
            let d = digit as usize;
            if d < 10 && digit_offset_set[d] {
                return digit_offset[d];
            }
            let ch = char::from_digit(digit, 10).unwrap_or('0');
            let Some(font) = &self.font else {
                return 0.0;
            };
            let Some(glyph) = font.glyph(ch) else {
                return 0.0;
            };

            let char_height = label_render_h;
            let pixel_h = max_label_magnify;

            let mut min_row: Option<usize> = None;
            let mut max_row: Option<usize> = None;
            for (row_idx, row) in glyph.bitmap.pixels.iter().enumerate() {
                if row.iter().any(|&p| p) {
                    min_row = Some(min_row.map_or(row_idx, |m| m.min(row_idx)));
                    max_row = Some(max_row.map_or(row_idx, |m| m.max(row_idx)));
                }
            }

            let off = if let (Some(min_row), Some(max_row)) = (min_row, max_row) {
                let used_height = ((max_row - min_row + 1) as f32) * pixel_h;
                let desired_top = ((char_height - used_height) / 2.0).floor();
                let current_top = (min_row as f32) * pixel_h;
                (desired_top - current_top).floor()
            } else {
                0.0
            };

            if d < 10 {
                digit_offset[d] = off;
                digit_offset_set[d] = true;
            }
            off
        };

        // Draw each slot: 2-digit label (no bg) + char (with bg)
        for slot in 0..12usize {
            let slot_x = (content_start_x + slot as f32 * ((SLOT_WIDTH + SLOT_SPACING) * scale)).floor();
            let char_x = (slot_x + (LABEL_WIDTH * scale)).floor();
            let label_x = (slot_x - (2.0 * scale)).floor();

            let is_label_hovered = matches!(hovered, HoverState::Slot(s, false) if s == slot);
            let is_char_hovered = matches!(hovered, HoverState::Slot(s, true) if s == slot);

            // Label colors
            let label_color = if is_label_hovered {
                Color::from_rgba(0.85, 0.85, 0.88, 1.0)
            } else {
                Color::from_rgba(0.55, 0.55, 0.58, 1.0)
            };
            let shadow_color = Color::from_rgba(0.0, 0.0, 0.0, 0.5);

            // Two-digit label chars
            let num = slot + 1;
            let (d1, d2) = if num < 10 {
                (0u32, num as u32)
            } else if num == 10 {
                (1u32, 0u32)
            } else if num == 11 {
                (1u32, 1u32)
            } else {
                (1u32, 2u32)
            };

            for (i, &d) in [d1, d2].iter().enumerate() {
                let glyph = char::from_digit(d, 10).unwrap_or('0') as u32;
                let y_off = glyph_y_offset(d);
                let x = label_x + i as f32 * label_char_w;
                let y = label_y + y_off;

                let label_w = label_render_w.floor().max(1.0);
                let label_h = label_render_h.floor().max(1.0);

                // Shadow
                instances.push(GlyphInstance {
                    pos: [(x + 1.0).floor(), (y + 1.0).floor()],
                    size: [label_w, label_h],
                    fg: [shadow_color.r, shadow_color.g, shadow_color.b, shadow_color.a],
                    bg: [0.0, 0.0, 0.0, 0.0],
                    glyph,
                    flags: 0,
                    _pad: [0, 0],
                });

                // Foreground
                instances.push(GlyphInstance {
                    pos: [x.floor(), y.floor()],
                    size: [label_w, label_h],
                    fg: [label_color.r, label_color.g, label_color.b, label_color.a],
                    bg: [0.0, 0.0, 0.0, 0.0],
                    glyph,
                    flags: 0,
                    _pad: [0, 0],
                });
            }

            // Slot char background (full cell)
            instances.push(GlyphInstance {
                pos: [char_x.floor(), slot_char_y.floor()],
                size: [slot_char_w.floor(), slot_char_h.floor()],
                fg: [0.0, 0.0, 0.0, 0.0],
                bg: [bg.r, bg.g, bg.b, 1.0],
                glyph: 0,
                flags: FLAG_BG_ONLY,
                _pad: [0, 0],
            });

            // Slot char glyph (crisp, integer magnification)
            // code_at returns CP437 code directly - use as atlas index
            let code = self.fkeys.code_at(set_idx, slot);
            let glyph = (code as u32) & 0xFF;
            let char_fg = if is_char_hovered {
                Color::from_rgb((fg.r * 1.3).min(1.0), (fg.g * 1.3).min(1.0), (fg.b * 1.3).min(1.0))
            } else {
                fg
            };

            instances.push(GlyphInstance {
                pos: [char_x.floor(), slot_char_y.floor()],
                size: [slot_char_w.floor(), slot_char_h.floor()],
                fg: [char_fg.r, char_fg.g, char_fg.b, 1.0],
                bg: [0.0, 0.0, 0.0, 0.0],
                glyph,
                flags: 0,
                _pad: [0, 0],
            });
        }

        // Set number between arrows (no bg), with shadow (physical pixel coordinates)
        let slot_width_px = (SLOT_WIDTH * scale).round().max(1.0);
        let slot_spacing_px = (SLOT_SPACING * scale).round().max(0.0);
        let nav_gap_px = (NAV_GAP * scale).round().max(0.0);
        let nav_size_px = (NAV_SIZE * scale).round().max(1.0);
        let arrow_size_px = (ARROW_SIZE * scale).round().max(1.0);
        let set_num_icon_gap_px = (SET_NUM_ICON_GAP * scale).round().max(0.0);
        let nav_num_shift_x_px = (NAV_NUM_SHIFT_X * scale).round();
        let nav_next_shift_x_px = (NAV_NEXT_SHIFT_X * scale).round();

        // nav_x: after 12 slots (each SLOT_WIDTH wide, with SLOT_SPACING between them)
        let slots_width_px = 12.0 * slot_width_px + 11.0 * slot_spacing_px;
        let nav_x = (content_start_x + slots_width_px + nav_gap_px).floor();
        let set_num = set_idx + 1;
        let num_str = set_num.to_string();
        let num_width = num_str.len() as f32 * label_char_w;

        // Fixed spacing: always layout for a 2-character number.
        // Keep the perceived gap between arrow icon and digits stable (~SET_NUM_ICON_GAP).
        let icon_side_gap = (nav_size_px - arrow_size_px) / 2.0;
        let num_padding: f32 = (set_num_icon_gap_px - icon_side_gap).max(0.0);
        let num_field_width = 2.0 * label_char_w;
        let label_space = num_field_width + 2.0 * num_padding;

        let next_x = nav_x + nav_size_px + label_space + nav_next_shift_x_px;
        // Center the actual number inside the 2-character field.
        let num_x = nav_x + nav_size_px + num_padding + (num_field_width - num_width) / 2.0 + nav_num_shift_x_px;

        let label_color = Color::from_rgba(0.55, 0.55, 0.58, 1.0);
        let label_hover_color = Color::from_rgba(0.85, 0.85, 0.88, 1.0);
        let shadow_color = Color::from_rgba(0.0, 0.0, 0.0, 0.5);

        // Check hover state for nav arrows
        let is_prev_hovered = matches!(hovered, HoverState::NavPrev);
        let is_next_hovered = matches!(hovered, HoverState::NavNext);

        // Navigation arrows - rendered as triangles via shader flags
        // flags bit 2 = left arrow, bit 3 = right arrow
        const FLAG_ARROW_LEFT: u32 = 4;
        const FLAG_ARROW_RIGHT: u32 = 8;

        let arrow_w = arrow_size_px.floor().max(1.0);
        let arrow_h = arrow_size_px.floor().max(1.0);

        // Center arrows vertically and horizontally within nav button area
        let arrow_y = (SHADOW_PADDING * scale).floor() + ((control_height_px - arrow_size_px) / 2.0).floor();

        // Left arrow (◄)
        let left_arrow_x = nav_x + ((nav_size_px - arrow_size_px) / 2.0).floor();
        let left_arrow_color = if is_prev_hovered { label_hover_color } else { label_color };

        // Shadow (smaller offset for small icons)
        instances.push(GlyphInstance {
            pos: [(left_arrow_x + 1.0).floor(), (arrow_y + 1.0).floor()],
            size: [arrow_w, arrow_h],
            fg: [shadow_color.r, shadow_color.g, shadow_color.b, shadow_color.a],
            bg: [0.0, 0.0, 0.0, 0.0],
            glyph: 0,
            flags: FLAG_ARROW_LEFT,
            _pad: [0, 0],
        });
        // Foreground
        instances.push(GlyphInstance {
            pos: [left_arrow_x.floor(), arrow_y.floor()],
            size: [arrow_w, arrow_h],
            fg: [left_arrow_color.r, left_arrow_color.g, left_arrow_color.b, 1.0],
            bg: [0.0, 0.0, 0.0, 0.0],
            glyph: 0,
            flags: FLAG_ARROW_LEFT,
            _pad: [0, 0],
        });

        // Right arrow (►)
        let right_arrow_x = next_x + ((nav_size_px - arrow_size_px) / 2.0).floor();
        let right_arrow_color = if is_next_hovered { label_hover_color } else { label_color };

        // Shadow (smaller offset for small icons)
        instances.push(GlyphInstance {
            pos: [(right_arrow_x + 1.0).floor(), (arrow_y + 1.0).floor()],
            size: [arrow_w, arrow_h],
            fg: [shadow_color.r, shadow_color.g, shadow_color.b, shadow_color.a],
            bg: [0.0, 0.0, 0.0, 0.0],
            glyph: 0,
            flags: FLAG_ARROW_RIGHT,
            _pad: [0, 0],
        });
        // Foreground
        instances.push(GlyphInstance {
            pos: [right_arrow_x.floor(), arrow_y.floor()],
            size: [arrow_w, arrow_h],
            fg: [right_arrow_color.r, right_arrow_color.g, right_arrow_color.b, 1.0],
            bg: [0.0, 0.0, 0.0, 0.0],
            glyph: 0,
            flags: FLAG_ARROW_RIGHT,
            _pad: [0, 0],
        });

        // Set number digits
        for (i, ch) in num_str.chars().enumerate() {
            let digit = ch.to_digit(10).unwrap_or(0);
            let y_off = glyph_y_offset(digit);
            let x = num_x + i as f32 * label_char_w;
            let y = label_y + y_off;
            let glyph = ch as u32;

            let label_w = label_render_w.floor().max(1.0);
            let label_h = label_render_h.floor().max(1.0);

            // Shadow
            instances.push(GlyphInstance {
                pos: [(x + 1.0).floor(), (y + 1.0).floor()],
                size: [label_w, label_h],
                fg: [shadow_color.r, shadow_color.g, shadow_color.b, shadow_color.a],
                bg: [0.0, 0.0, 0.0, 0.0],
                glyph,
                flags: 0,
                _pad: [0, 0],
            });
            // Foreground
            instances.push(GlyphInstance {
                pos: [x.floor(), y.floor()],
                size: [label_w, label_h],
                fg: [label_color.r, label_color.g, label_color.b, label_color.a],
                bg: [0.0, 0.0, 0.0, 0.0],
                glyph,
                flags: 0,
                _pad: [0, 0],
            });
        }

        // Upload instances into a fixed-size ring slot to avoid last-write-wins.
        let slot = pipeline.next_instance_slot.fetch_add(1, Ordering::Relaxed) % pipeline.instance_slots;
        let instance_offset = (slot as u64) * pipeline.instance_slot_stride;
        self.instance_offset_bytes.store(instance_offset as u32, Ordering::Relaxed);

        let max_instances = pipeline.instance_capacity_per_primitive as usize;
        let count = instances.len().min(max_instances);
        self.instance_count.store(count as u32, Ordering::Relaxed);
        if count > 0 {
            let bytes = bytemuck::cast_slice(&instances[..count]);
            queue.write_buffer(&pipeline.instance_buffer, instance_offset, bytes);
        }

        // suppress unused warnings (origin not used directly - viewport uses clip rect)
        let _ = (origin_x, origin_y);
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        let mut pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
            label: Some("FKey Glyphs Render Pass"),
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
            let instance_count = self.instance_count.load(Ordering::Relaxed);
            if instance_count == 0 {
                return;
            }
            let instance_offset = self.instance_offset_bytes.load(Ordering::Relaxed) as u64;
            let instance_bytes = (instance_count as u64) * pipeline.instance_stride;
            pass.set_vertex_buffer(1, pipeline.instance_buffer.slice(instance_offset..(instance_offset + instance_bytes)));
            pass.draw(0..6, 0..instance_count);
        }
    }
}

pub(crate) struct FKeyGlyphRenderer {
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

impl FKeyGlyphRenderer {
    pub(crate) fn update_atlas(&mut self, device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, key: u64, w: u32, h: u32, rgba: &[u8]) {
        if self.atlas_w != w || self.atlas_h != h {
            self.atlas_texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                label: Some("FKey Glyph Atlas"),
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

            // Rebuild bind group to reference the new view
            // NOTE: layout is identical, so we can reuse the pipeline's bind-group layout by recreating it locally.
            // This is cheap and only happens on atlas resize.
            let bind_group_layout = self.pipeline.get_bind_group_layout(0);
            self.bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                label: Some("FKey Glyphs Bind Group"),
                layout: &bind_group_layout,
                entries: &[
                    iced::wgpu::BindGroupEntry {
                        binding: 0,
                        resource: iced::wgpu::BindingResource::Buffer(iced::wgpu::BufferBinding {
                            buffer: &self.uniform_buffer,
                            offset: 0,
                            size: NonZeroU64::new(std::mem::size_of::<FKeyGlyphUniforms>() as u64),
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

impl shader::Pipeline for FKeyGlyphRenderer {
    fn new(device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("FKey Glyphs Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("fkey_glyphs_shader.wgsl").into()),
        });

        let uniform_size = std::mem::size_of::<FKeyGlyphUniforms>() as u64;
        let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        let uniform_stride = align_up(uniform_size, alignment);
        let uniform_capacity: u32 = 1024;
        let uniform_buffer_size = uniform_stride * (uniform_capacity as u64);

        let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("FKey Glyphs Uniforms (Dynamic)"),
            size: uniform_buffer_size,
            usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create a tiny default atlas (will be replaced on first prepare)
        let atlas_texture = device.create_texture(&iced::wgpu::TextureDescriptor {
            label: Some("FKey Glyph Atlas (init)"),
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
            label: Some("FKey Glyph Atlas Sampler"),
            mag_filter: iced::wgpu::FilterMode::Nearest,
            min_filter: iced::wgpu::FilterMode::Nearest,
            mipmap_filter: iced::wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Quad vertex buffer (two triangles)
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
            label: Some("FKey Glyph Quad"),
            contents: bytemuck::cast_slice(&quad),
            usage: iced::wgpu::BufferUsages::VERTEX,
        });

        let instance_stride = std::mem::size_of::<GlyphInstance>() as u64;
        // Enough for all quads in one toolbar (labels+chars+nav). Increase if needed.
        let instance_capacity_per_primitive: u32 = 512;
        let instance_slots: u32 = 256;
        let instance_slot_stride = instance_stride * (instance_capacity_per_primitive as u64);
        let instance_buffer_size = instance_slot_stride * (instance_slots as u64);

        let instance_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("FKey Glyph Instances (Ring)"),
            size: instance_buffer_size,
            usage: iced::wgpu::BufferUsages::VERTEX | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layout: uniforms + texture + sampler
        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("FKey Glyphs Bind Group Layout"),
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
            label: Some("FKey Glyphs Bind Group"),
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
            label: Some("FKey Glyphs Pipeline Layout"),
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
            label: Some("FKey Glyphs Pipeline"),
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

// ═══════════════════════════════════════════════════════════════════════════
// One-Pass Combined Renderer (Background + Glyphs)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct FKeyOnePassUniforms {
    clip_size: [f32; 2],
    atlas_size: [f32; 2],
    glyph_size: [f32; 2],
    _pad0: [f32; 2],
    num_slots: u32,
    hovered_slot: u32,
    hover_type: u32,
    _pad1: u32,
    corner_radius: f32,
    _pad2: [f32; 3],
    bg_color: [f32; 4],
    content_start_x: f32,
    slot_width: f32,
    slot_spacing: f32,
    label_width: f32,
}

unsafe impl bytemuck::Pod for FKeyOnePassUniforms {}
unsafe impl bytemuck::Zeroable for FKeyOnePassUniforms {}

#[derive(Clone)]
struct FKeyOnePassProgram {
    fkeys: FKeySets,
    font: Option<BitFont>,
    palette: Palette,
    fg_color: u32,
    bg_color: u32,
    bg_color_themed: Color,
    hovered_slot: Arc<AtomicU32>,
    hover_type: Arc<AtomicU32>,
    nav_label_space_bits: Arc<AtomicU32>,
}

impl shader::Program<FKeyToolbarMessage> for FKeyOnePassProgram {
    type State = ();
    type Primitive = FKeyOnePassPrimitive;

    fn draw(&self, _state: &Self::State, _cursor: Cursor, _bounds: Rectangle) -> Self::Primitive {
        FKeyOnePassPrimitive {
            fkeys: self.fkeys.clone(),
            font: self.font.clone(),
            palette: self.palette.clone(),
            fg_color: self.fg_color,
            bg_color: self.bg_color,
            bg_color_themed: self.bg_color_themed,
            hovered_slot: self.hovered_slot.clone(),
            hover_type: self.hover_type.clone(),
            viewport_x: Arc::new(AtomicU32::new(0)),
            viewport_y: Arc::new(AtomicU32::new(0)),
            viewport_w: Arc::new(AtomicU32::new(0)),
            viewport_h: Arc::new(AtomicU32::new(0)),
            uniform_offset_bytes: Arc::new(AtomicU32::new(0)),
            instance_offset_bytes: Arc::new(AtomicU32::new(0)),
            instance_count: Arc::new(AtomicU32::new(0)),
        }
    }

    fn update(&self, _state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: Cursor) -> Option<iced::widget::Action<FKeyToolbarMessage>> {
        let nav_label_space = f32::from_bits(self.nav_label_space_bits.load(Ordering::Relaxed)).max(0.0);
        // Handle mouse movement for hover state
        if let Some(pos) = cursor.position_in(bounds) {
            let (slot, hover_type) = compute_hover_state(pos, bounds, nav_label_space);
            self.hovered_slot.store(slot, Ordering::Relaxed);
            self.hover_type.store(hover_type, Ordering::Relaxed);
        } else {
            self.hovered_slot.store(NO_HOVER, Ordering::Relaxed);
            self.hover_type.store(0, Ordering::Relaxed);
        }

        // Handle mouse clicks
        if let iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            if let Some(pos) = cursor.position_in(bounds) {
                let (slot, hover_type) = compute_hover_state(pos, bounds, nav_label_space);

                if slot != NO_HOVER {
                    let is_on_char = hover_type == 1;
                    if is_on_char {
                        return Some(iced::widget::Action::publish(FKeyToolbarMessage::TypeFKey(slot as usize)));
                    } else {
                        return Some(iced::widget::Action::publish(FKeyToolbarMessage::OpenCharSelector(slot as usize)));
                    }
                }

                if hover_type == 2 {
                    return Some(iced::widget::Action::publish(FKeyToolbarMessage::PrevSet));
                }
                if hover_type == 3 {
                    return Some(iced::widget::Action::publish(FKeyToolbarMessage::NextSet));
                }
            }
        }

        None
    }

    fn mouse_interaction(&self, _state: &Self::State, bounds: Rectangle, cursor: Cursor) -> mouse::Interaction {
        if let Some(pos) = cursor.position_in(bounds) {
            let nav_label_space = f32::from_bits(self.nav_label_space_bits.load(Ordering::Relaxed)).max(0.0);
            let (slot, hover_type) = compute_hover_state(pos, bounds, nav_label_space);
            if slot != NO_HOVER || hover_type == 2 || hover_type == 3 {
                return mouse::Interaction::Pointer;
            }
        }
        mouse::Interaction::default()
    }
}

#[derive(Clone, Debug)]
struct FKeyOnePassPrimitive {
    fkeys: FKeySets,
    font: Option<BitFont>,
    palette: Palette,
    fg_color: u32,
    bg_color: u32,
    bg_color_themed: Color,
    hovered_slot: Arc<AtomicU32>,
    hover_type: Arc<AtomicU32>,
    viewport_x: Arc<AtomicU32>,
    viewport_y: Arc<AtomicU32>,
    viewport_w: Arc<AtomicU32>,
    viewport_h: Arc<AtomicU32>,
    uniform_offset_bytes: Arc<AtomicU32>,
    instance_offset_bytes: Arc<AtomicU32>,
    instance_count: Arc<AtomicU32>,
}

impl shader::Primitive for FKeyOnePassPrimitive {
    type Pipeline = FKeyOnePassRenderer;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        bounds: &Rectangle,
        viewport: &iced::advanced::graphics::Viewport,
    ) {
        let scale = viewport.scale_factor();
        let origin_x = (bounds.x * scale).round().max(0.0);
        let origin_y = (bounds.y * scale).round().max(0.0);
        let size_w = (bounds.width * scale).round().max(1.0);
        let size_h = (bounds.height * scale).round().max(1.0);

        self.viewport_x.store(origin_x as u32, Ordering::Relaxed);
        self.viewport_y.store(origin_y as u32, Ordering::Relaxed);
        self.viewport_w.store(size_w as u32, Ordering::Relaxed);
        self.viewport_h.store(size_h as u32, Ordering::Relaxed);

        let hovered_slot = self.hovered_slot.load(Ordering::Relaxed);
        let hover_type = self.hover_type.load(Ordering::Relaxed);

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

        let content_start_x = ((SHADOW_PADDING + BORDER_WIDTH + LEFT_PADDING) * scale).floor();

        let uniforms = FKeyOnePassUniforms {
            clip_size: [size_w, size_h],
            atlas_size: [pipeline.atlas_w as f32, pipeline.atlas_h as f32],
            glyph_size: [glyph_w, glyph_h],
            _pad0: [0.0, 0.0],
            num_slots: 12,
            hovered_slot,
            hover_type,
            _pad1: 0,
            corner_radius: CORNER_RADIUS * scale,
            _pad2: [0.0, 0.0, 0.0],
            bg_color: [self.bg_color_themed.r, self.bg_color_themed.g, self.bg_color_themed.b, self.bg_color_themed.a],
            content_start_x,
            slot_width: SLOT_WIDTH * scale,
            slot_spacing: SLOT_SPACING * scale,
            label_width: LABEL_WIDTH * scale,
        };

        let uniform_slot = pipeline.next_uniform.fetch_add(1, Ordering::Relaxed) % pipeline.uniform_capacity;
        let uniform_offset = (uniform_slot as u64) * pipeline.uniform_stride;
        self.uniform_offset_bytes.store(uniform_offset as u32, Ordering::Relaxed);
        queue.write_buffer(&pipeline.uniform_buffer, uniform_offset, bytemuck::bytes_of(&uniforms));

        // Build instances - first background, then glyphs
        let mut instances: Vec<GlyphInstance> = Vec::with_capacity(128);

        // Background instance covers the entire widget
        instances.push(GlyphInstance {
            pos: [0.0, 0.0],
            size: [size_w, size_h],
            fg: [0.0, 0.0, 0.0, 0.0],
            bg: [0.0, 0.0, 0.0, 0.0],
            glyph: 0,
            flags: FLAG_FKEY_BG,
            _pad: [0, 0],
        });

        // Build glyph instances (same logic as FKeyGlyphPrimitive)
        let (fg_r, fg_g, fg_b) = self.palette.rgb(self.fg_color);
        let (bg_r, bg_g, bg_b) = self.palette.rgb(self.bg_color);
        let fg = Color::from_rgb8(fg_r, fg_g, fg_b);
        let bg = Color::from_rgb8(bg_r, bg_g, bg_b);

        let hovered = HoverState::from_uniforms(hovered_slot, hover_type);
        let set_idx = self.fkeys.current_set();

        const FLAG_DRAW_BG: u32 = 1;
        const FLAG_BG_ONLY: u32 = 2;

        let control_height_px = ((bounds.height - SHADOW_PADDING * 2.0) * scale).round().max(1.0);
        let font_height = glyph_h.max(1.0);
        let font_width = glyph_w.max(1.0);

        let target_slot_char_h_px = (SLOT_CHAR_HEIGHT * scale).round().max(1.0);
        let slot_char_magnify = (target_slot_char_h_px / font_height).floor().max(1.0);
        let target_label_h_px = LABEL_HEIGHT * scale;
        let max_label_magnify = (target_label_h_px / font_height).floor().max(1.0);

        let slot_char_w = (font_width * slot_char_magnify).round().max(1.0);
        let slot_char_h = (font_height * slot_char_magnify).round().max(1.0);
        let label_render_w = (font_width * max_label_magnify).round().max(1.0);
        let label_render_h = (font_height * max_label_magnify).round().max(1.0);
        let label_char_w = label_render_w;

        let slot_char_y = (SHADOW_PADDING * scale).floor() + ((control_height_px - slot_char_h) / 2.0).floor();
        let label_y = (SHADOW_PADDING * scale).floor() + ((control_height_px - label_render_h) / 2.0).floor();

        let mut digit_offset: [f32; 10] = [0.0; 10];
        let mut digit_offset_set: [bool; 10] = [false; 10];

        let mut glyph_y_offset = |digit: u32, font_opt: &Option<BitFont>| -> f32 {
            let d = digit as usize;
            if d < 10 && digit_offset_set[d] {
                return digit_offset[d];
            }
            let ch = char::from_digit(digit, 10).unwrap_or('0');
            let Some(font) = font_opt else {
                return 0.0;
            };
            let Some(glyph) = font.glyph(ch) else {
                return 0.0;
            };

            let char_height = label_render_h;
            let pixel_h = max_label_magnify;

            let mut min_row: Option<usize> = None;
            let mut max_row: Option<usize> = None;
            for (row_idx, row) in glyph.bitmap.pixels.iter().enumerate() {
                if row.iter().any(|&p| p) {
                    min_row = Some(min_row.map_or(row_idx, |m| m.min(row_idx)));
                    max_row = Some(max_row.map_or(row_idx, |m| m.max(row_idx)));
                }
            }

            let off = if let (Some(min_row), Some(max_row)) = (min_row, max_row) {
                let used_height = ((max_row - min_row + 1) as f32) * pixel_h;
                let desired_top = ((char_height - used_height) / 2.0).floor();
                let current_top = (min_row as f32) * pixel_h;
                (desired_top - current_top).floor()
            } else {
                0.0
            };

            if d < 10 {
                digit_offset[d] = off;
                digit_offset_set[d] = true;
            }
            off
        };

        // Draw each slot
        for slot in 0..12usize {
            let slot_x = (content_start_x + slot as f32 * ((SLOT_WIDTH + SLOT_SPACING) * scale)).floor();
            let char_x = (slot_x + (LABEL_WIDTH * scale)).floor();
            let label_x = (slot_x - (2.0 * scale)).floor();

            let is_label_hovered = matches!(hovered, HoverState::Slot(s, false) if s == slot);
            let is_char_hovered = matches!(hovered, HoverState::Slot(s, true) if s == slot);

            let label_color = if is_label_hovered {
                Color::from_rgba(0.85, 0.85, 0.88, 1.0)
            } else {
                Color::from_rgba(0.55, 0.55, 0.58, 1.0)
            };
            let shadow_color = Color::from_rgba(0.0, 0.0, 0.0, 0.5);

            let num = slot + 1;
            let (d1, d2) = if num < 10 {
                (0u32, num as u32)
            } else if num == 10 {
                (1u32, 0u32)
            } else if num == 11 {
                (1u32, 1u32)
            } else {
                (1u32, 2u32)
            };

            for (i, &d) in [d1, d2].iter().enumerate() {
                let glyph = char::from_digit(d, 10).unwrap_or('0') as u32;
                let y_off = glyph_y_offset(d, &self.font);
                let x = label_x + i as f32 * label_char_w;
                let y = label_y + y_off;

                let label_w = label_render_w.floor().max(1.0);
                let label_h = label_render_h.floor().max(1.0);

                instances.push(GlyphInstance {
                    pos: [(x + 1.0).floor(), (y + 1.0).floor()],
                    size: [label_w, label_h],
                    fg: [shadow_color.r, shadow_color.g, shadow_color.b, shadow_color.a],
                    bg: [0.0, 0.0, 0.0, 0.0],
                    glyph,
                    flags: 0,
                    _pad: [0, 0],
                });

                instances.push(GlyphInstance {
                    pos: [x.floor(), y.floor()],
                    size: [label_w, label_h],
                    fg: [label_color.r, label_color.g, label_color.b, label_color.a],
                    bg: [0.0, 0.0, 0.0, 0.0],
                    glyph,
                    flags: 0,
                    _pad: [0, 0],
                });
            }

            instances.push(GlyphInstance {
                pos: [char_x.floor(), slot_char_y.floor()],
                size: [slot_char_w.floor(), slot_char_h.floor()],
                fg: [0.0, 0.0, 0.0, 0.0],
                bg: [bg.r, bg.g, bg.b, 1.0],
                glyph: 0,
                flags: FLAG_BG_ONLY,
                _pad: [0, 0],
            });

            let code = self.fkeys.code_at(set_idx, slot);
            let glyph = (code as u32) & 0xFF;
            let char_fg = if is_char_hovered {
                Color::from_rgb((fg.r * 1.3).min(1.0), (fg.g * 1.3).min(1.0), (fg.b * 1.3).min(1.0))
            } else {
                fg
            };

            instances.push(GlyphInstance {
                pos: [char_x.floor(), slot_char_y.floor()],
                size: [slot_char_w.floor(), slot_char_h.floor()],
                fg: [char_fg.r, char_fg.g, char_fg.b, 1.0],
                bg: [0.0, 0.0, 0.0, 0.0],
                glyph,
                flags: 0,
                _pad: [0, 0],
            });
        }

        // Navigation arrows and set number
        let slot_width_px = (SLOT_WIDTH * scale).round().max(1.0);
        let slot_spacing_px = (SLOT_SPACING * scale).round().max(0.0);
        let nav_gap_px = (NAV_GAP * scale).round().max(0.0);
        let nav_size_px = (NAV_SIZE * scale).round().max(1.0);
        let arrow_size_px = (ARROW_SIZE * scale).round().max(1.0);
        let set_num_icon_gap_px = (SET_NUM_ICON_GAP * scale).round().max(0.0);
        let nav_num_shift_x_px = (NAV_NUM_SHIFT_X * scale).round();
        let nav_next_shift_x_px = (NAV_NEXT_SHIFT_X * scale).round();

        let slots_width_px = 12.0 * slot_width_px + 11.0 * slot_spacing_px;
        let nav_x = (content_start_x + slots_width_px + nav_gap_px).floor();
        let set_num = set_idx + 1;
        let num_str = set_num.to_string();
        let num_width = num_str.len() as f32 * label_char_w;

        let icon_side_gap = (nav_size_px - arrow_size_px) / 2.0;
        let num_padding: f32 = (set_num_icon_gap_px - icon_side_gap).max(0.0);
        let num_field_width = 2.0 * label_char_w;
        let label_space = num_field_width + 2.0 * num_padding;

        let next_x = nav_x + nav_size_px + label_space + nav_next_shift_x_px;
        let num_x = nav_x + nav_size_px + num_padding + (num_field_width - num_width) / 2.0 + nav_num_shift_x_px;

        let label_color = Color::from_rgba(0.55, 0.55, 0.58, 1.0);
        let label_hover_color = Color::from_rgba(0.85, 0.85, 0.88, 1.0);
        let shadow_color = Color::from_rgba(0.0, 0.0, 0.0, 0.5);

        let is_prev_hovered = matches!(hovered, HoverState::NavPrev);
        let is_next_hovered = matches!(hovered, HoverState::NavNext);

        const FLAG_ARROW_LEFT: u32 = 4;
        const FLAG_ARROW_RIGHT: u32 = 8;

        let arrow_w = arrow_size_px.floor().max(1.0);
        let arrow_h = arrow_size_px.floor().max(1.0);
        let arrow_y = (SHADOW_PADDING * scale).floor() + ((control_height_px - arrow_size_px) / 2.0).floor();

        let left_arrow_x = nav_x + ((nav_size_px - arrow_size_px) / 2.0).floor();
        let left_arrow_color = if is_prev_hovered { label_hover_color } else { label_color };

        instances.push(GlyphInstance {
            pos: [(left_arrow_x + 1.0).floor(), (arrow_y + 1.0).floor()],
            size: [arrow_w, arrow_h],
            fg: [shadow_color.r, shadow_color.g, shadow_color.b, shadow_color.a],
            bg: [0.0, 0.0, 0.0, 0.0],
            glyph: 0,
            flags: FLAG_ARROW_LEFT,
            _pad: [0, 0],
        });
        instances.push(GlyphInstance {
            pos: [left_arrow_x.floor(), arrow_y.floor()],
            size: [arrow_w, arrow_h],
            fg: [left_arrow_color.r, left_arrow_color.g, left_arrow_color.b, 1.0],
            bg: [0.0, 0.0, 0.0, 0.0],
            glyph: 0,
            flags: FLAG_ARROW_LEFT,
            _pad: [0, 0],
        });

        let right_arrow_x = next_x + ((nav_size_px - arrow_size_px) / 2.0).floor();
        let right_arrow_color = if is_next_hovered { label_hover_color } else { label_color };

        instances.push(GlyphInstance {
            pos: [(right_arrow_x + 1.0).floor(), (arrow_y + 1.0).floor()],
            size: [arrow_w, arrow_h],
            fg: [shadow_color.r, shadow_color.g, shadow_color.b, shadow_color.a],
            bg: [0.0, 0.0, 0.0, 0.0],
            glyph: 0,
            flags: FLAG_ARROW_RIGHT,
            _pad: [0, 0],
        });
        instances.push(GlyphInstance {
            pos: [right_arrow_x.floor(), arrow_y.floor()],
            size: [arrow_w, arrow_h],
            fg: [right_arrow_color.r, right_arrow_color.g, right_arrow_color.b, 1.0],
            bg: [0.0, 0.0, 0.0, 0.0],
            glyph: 0,
            flags: FLAG_ARROW_RIGHT,
            _pad: [0, 0],
        });

        for (i, ch) in num_str.chars().enumerate() {
            let digit = ch.to_digit(10).unwrap_or(0);
            let y_off = glyph_y_offset(digit, &self.font);
            let x = num_x + i as f32 * label_char_w;
            let y = label_y + y_off;
            let glyph = ch as u32;

            let label_w = label_render_w.floor().max(1.0);
            let label_h = label_render_h.floor().max(1.0);

            instances.push(GlyphInstance {
                pos: [(x + 1.0).floor(), (y + 1.0).floor()],
                size: [label_w, label_h],
                fg: [shadow_color.r, shadow_color.g, shadow_color.b, shadow_color.a],
                bg: [0.0, 0.0, 0.0, 0.0],
                glyph,
                flags: 0,
                _pad: [0, 0],
            });
            instances.push(GlyphInstance {
                pos: [x.floor(), y.floor()],
                size: [label_w, label_h],
                fg: [label_color.r, label_color.g, label_color.b, label_color.a],
                bg: [0.0, 0.0, 0.0, 0.0],
                glyph,
                flags: 0,
                _pad: [0, 0],
            });
        }

        // Upload instances
        let slot = pipeline.next_instance_slot.fetch_add(1, Ordering::Relaxed) % pipeline.instance_slots;
        let instance_offset = (slot as u64) * pipeline.instance_slot_stride;
        self.instance_offset_bytes.store(instance_offset as u32, Ordering::Relaxed);

        let max_instances = pipeline.instance_capacity_per_primitive as usize;
        let count = instances.len().min(max_instances);
        self.instance_count.store(count as u32, Ordering::Relaxed);
        if count > 0 {
            let bytes = bytemuck::cast_slice(&instances[..count]);
            queue.write_buffer(&pipeline.instance_buffer, instance_offset, bytes);
        }
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        let instance_count = self.instance_count.load(Ordering::Relaxed);
        if instance_count == 0 {
            return;
        }

        let mut pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
            label: Some("FKey OnePass Render Pass"),
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

pub struct FKeyOnePassRenderer {
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

impl FKeyOnePassRenderer {
    fn update_atlas(&mut self, device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, key: u64, w: u32, h: u32, rgba: &[u8]) {
        if self.atlas_w != w || self.atlas_h != h {
            self.atlas_texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                label: Some("FKey OnePass Glyph Atlas"),
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
                label: Some("FKey OnePass Bind Group"),
                layout: &bind_group_layout,
                entries: &[
                    iced::wgpu::BindGroupEntry {
                        binding: 0,
                        resource: iced::wgpu::BindingResource::Buffer(iced::wgpu::BufferBinding {
                            buffer: &self.uniform_buffer,
                            offset: 0,
                            size: NonZeroU64::new(std::mem::size_of::<FKeyOnePassUniforms>() as u64),
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

impl shader::Pipeline for FKeyOnePassRenderer {
    fn new(device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("FKey OnePass Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("fkey_toolbar_onepass_shader.wgsl").into()),
        });

        let uniform_size = std::mem::size_of::<FKeyOnePassUniforms>() as u64;
        let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        let uniform_stride = align_up(uniform_size, alignment);
        let uniform_capacity: u32 = 1024;
        let uniform_buffer_size = uniform_stride * (uniform_capacity as u64);

        let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("FKey OnePass Uniforms (Dynamic)"),
            size: uniform_buffer_size,
            usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let atlas_texture = device.create_texture(&iced::wgpu::TextureDescriptor {
            label: Some("FKey OnePass Glyph Atlas (init)"),
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
            label: Some("FKey OnePass Atlas Sampler"),
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
            label: Some("FKey OnePass Quad"),
            contents: bytemuck::cast_slice(&quad),
            usage: iced::wgpu::BufferUsages::VERTEX,
        });

        let instance_stride = std::mem::size_of::<GlyphInstance>() as u64;
        let instance_capacity_per_primitive: u32 = 512;
        let instance_slots: u32 = 256;
        let instance_slot_stride = instance_stride * (instance_capacity_per_primitive as u64);
        let instance_buffer_size = instance_slot_stride * (instance_slots as u64);

        let instance_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("FKey OnePass Instances (Ring)"),
            size: instance_buffer_size,
            usage: iced::wgpu::BufferUsages::VERTEX | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("FKey OnePass Bind Group Layout"),
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
            label: Some("FKey OnePass Bind Group"),
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
            label: Some("FKey OnePass Pipeline Layout"),
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
            label: Some("FKey OnePass Pipeline"),
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

// ═══════════════════════════════════════════════════════════════════════════
// Helper Functions
// ═══════════════════════════════════════════════════════════════════════════

/// Compute hover state from cursor position using FKeyLayout.
///
/// The `nav_label_space` parameter allows overriding the computed label space
/// (used when we know the exact font dimensions). Pass 0.0 to use default.
fn compute_hover_state(pos: Point, _bounds: Rectangle, nav_label_space: f32) -> (u32, u32) {
    // Use default font for hit testing (the exact font doesn't matter much for hit areas)
    let mut layout = FKeyLayout::default_font();

    // Override nav_label_space if provided
    if nav_label_space > 0.0 {
        layout.nav_label_space = nav_label_space;
        // Recalculate next_nav_x with the new label space
        layout.next_nav_x = layout.nav_x + NAV_SIZE + nav_label_space + NAV_NEXT_SHIFT_X;
    }

    layout.hit_test_uniforms(pos)
}

// ═══════════════════════════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════════════════════════

/// GPU-accelerated F-Key Toolbar with shader background
pub struct ShaderFKeyToolbar {
    hovered_slot: Arc<AtomicU32>,
    hover_type: Arc<AtomicU32>,
    nav_label_space_bits: Arc<AtomicU32>,
}

impl Default for ShaderFKeyToolbar {
    fn default() -> Self {
        Self::new()
    }
}

impl ShaderFKeyToolbar {
    pub fn new() -> Self {
        Self {
            hovered_slot: Arc::new(AtomicU32::new(NO_HOVER)),
            hover_type: Arc::new(AtomicU32::new(0)),
            nav_label_space_bits: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Clear the render cache
    pub fn clear_cache(&mut self) {
        // Cache is owned per-view
    }

    /// Render the toolbar
    pub fn view(
        &self,
        fkeys: FKeySets,
        font: Option<BitFont>,
        palette: Palette,
        fg_color: u32,
        bg_color: u32,
        theme: &Theme,
    ) -> Element<'_, FKeyToolbarMessage> {
        // Use centralized layout calculations
        let (font_w, font_h) = font
            .as_ref()
            .map(|f| (f.size().width.max(1) as f32, f.size().height.max(1) as f32))
            .unwrap_or((8.0, 16.0));
        let layout = FKeyLayout::new(font_w, font_h);

        // Share the exact label-space with hit-testing.
        self.nav_label_space_bits.store(layout.nav_label_space.to_bits(), Ordering::Relaxed);

        let bg_color_themed = main_area_background(theme);

        // Create one-pass shader (background + glyphs combined)
        let onepass: Element<'_, FKeyToolbarMessage> = widget::shader(FKeyOnePassProgram {
            fkeys,
            font,
            palette,
            fg_color,
            bg_color,
            bg_color_themed,
            hovered_slot: self.hovered_slot.clone(),
            hover_type: self.hover_type.clone(),
            nav_label_space_bits: self.nav_label_space_bits.clone(),
        })
        .width(Length::Fixed(layout.total_width))
        .height(Length::Fixed(layout.total_height))
        .into();

        // Wrap in container to center horizontally in parent
        container(onepass)
            .width(Length::Fill)
            .height(Length::Fixed(layout.total_height))
            .center_x(Length::Fill)
            .into()
    }
}
