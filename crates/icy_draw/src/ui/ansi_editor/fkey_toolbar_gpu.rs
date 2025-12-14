//! GPU-accelerated F-Key Toolbar Component
//!
//! Renders F1-F12 function key slots with characters from the current font.
//! Uses WGSL shader for background (drop shadow, borders, hover highlights)
//! and Canvas overlay for text rendering, plus SVG arrows for navigation.

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use codepages::tables::CP437_TO_UNICODE;
use iced::wgpu::util::DeviceExt;
use iced::{
    Color, Element, Length, Point, Rectangle, Size, Theme,
    mouse::{self, Cursor},
    widget::{self, Space, container, row, shader, svg},
};
use icy_engine::{BitFont, Palette};
use icy_engine_gui::theme::main_area_background;

use crate::ui::FKeySets;

// SVG icons for navigation arrows
const ARROW_LEFT_SVG: &[u8] = include_bytes!("../../../data/icons/arrow_left.svg");
const ARROW_RIGHT_SVG: &[u8] = include_bytes!("../../../data/icons/arrow_right.svg");

// ═══════════════════════════════════════════════════════════════════════════
// Layout Constants (matching fkey_toolbar.rs)
// ═══════════════════════════════════════════════════════════════════════════

/// Character display height (32px = 2x font height)
const CHAR_DISPLAY_HEIGHT: f32 = 32.0;

/// Width per F-key slot (label + char)
const SLOT_WIDTH: f32 = 40.0;

/// Label width (01, 02, etc. - 2 chars)
const LABEL_WIDTH: f32 = 20.0;

/// Spacing between slots
const SLOT_SPACING: f32 = 4.0;

/// Nav button size
const NAV_SIZE: f32 = 28.0;

/// Space between nav arrows for label
const NAV_LABEL_SPACE: f32 = 16.0;

/// Gap before nav section
const NAV_GAP: f32 = 10.0;

/// Corner radius for rounded rectangles
const CORNER_RADIUS: f32 = 6.0;

/// Border width
const BORDER_WIDTH: f32 = 1.0;

/// Extra padding around the control for drop shadow
const SHADOW_PADDING: f32 = 6.0;

/// Toolbar height - slightly taller than SegmentedControl for character display
const TOOLBAR_HEIGHT: f32 = 36.0;

/// Left padding before content
const LEFT_PADDING: f32 = 8.0;

/// No hover marker
const NO_HOVER: u32 = 0xFFFF_FFFF;

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

/// Hover state: which element is currently hovered
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoverState {
    #[default]
    None,
    /// Slot hover (slot_index, is_on_char)
    Slot(usize, bool),
    /// Hover over previous-set navigation arrow
    NavPrev,
    /// Hover over next-set navigation arrow
    NavNext,
}

impl HoverState {
    fn to_uniforms(&self) -> (u32, u32) {
        match self {
            HoverState::None => (NO_HOVER, 0),
            HoverState::Slot(idx, is_char) => (*idx as u32, if *is_char { 1 } else { 0 }),
            HoverState::NavPrev => (NO_HOVER, 2),
            HoverState::NavNext => (NO_HOVER, 3),
        }
    }

    fn from_atomics(slot: u32, hover_type: u32) -> Self {
        if hover_type == 2 {
            HoverState::NavPrev
        } else if hover_type == 3 {
            HoverState::NavNext
        } else if slot != NO_HOVER {
            HoverState::Slot(slot as usize, hover_type == 1)
        } else {
            HoverState::None
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GPU Shader Types
// ═══════════════════════════════════════════════════════════════════════════

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
        }
    }

    fn update(&self, _state: &mut Self::State, _event: &iced::Event, _bounds: Rectangle, _cursor: Cursor) -> Option<iced::widget::Action<FKeyToolbarMessage>> {
        None
    }

    fn mouse_interaction(&self, _state: &Self::State, _bounds: Rectangle, _cursor: Cursor) -> mouse::Interaction {
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
        let nav_start_x = content_start_x + 12.0 * (SLOT_WIDTH + SLOT_SPACING) * scale + NAV_GAP * scale;

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

        queue.write_buffer(&pipeline.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
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
            pass.set_bind_group(0, &pipeline.bind_group, &[]);
            pass.draw(0..6, 0..1);
        }
    }
}

/// GPU renderer for the F-Key toolbar background
pub struct FKeyToolbarRenderer {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group: iced::wgpu::BindGroup,
    uniform_buffer: iced::wgpu::Buffer,
}

impl shader::Pipeline for FKeyToolbarRenderer {
    fn new(device: &iced::wgpu::Device, _queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("FKey Toolbar Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("fkey_toolbar_shader.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("FKey Toolbar Uniforms"),
            size: std::mem::size_of::<FKeyToolbarUniforms>() as u64,
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
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
            label: Some("FKey Toolbar Bind Group"),
            layout: &bind_group_layout,
            entries: &[iced::wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
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
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Glyph Atlas Overlay (GPU)
// ═══════════════════════════════════════════════════════════════════════════

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct FKeyGlyphUniforms {
    clip_size: [f32; 2],
    atlas_size: [f32; 2],
    glyph_size: [f32; 2],
    _pad: [f32; 2],
}

unsafe impl bytemuck::Pod for FKeyGlyphUniforms {}
unsafe impl bytemuck::Zeroable for FKeyGlyphUniforms {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct QuadVertex {
    unit_pos: [f32; 2],
    unit_uv: [f32; 2],
}

unsafe impl bytemuck::Pod for QuadVertex {}
unsafe impl bytemuck::Zeroable for QuadVertex {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct GlyphInstance {
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

fn font_key(font: &BitFont) -> u64 {
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

fn build_glyph_atlas_rgba(font: &BitFont) -> (u32, u32, Vec<u8>) {
    let size = font.size();
    let gw = size.width.max(1) as u32;
    let gh = size.height.max(1) as u32;
    let atlas_w = gw * 16;
    let atlas_h = gh * 16;
    println!("Building glyph atlas: {}x{} (glyph {}x{})", atlas_w, atlas_h, gw, gh);
    let mut rgba = vec![0u8; (atlas_w * atlas_h * 4) as usize];

    for code in 0u32..256u32 {
        // Map CP437 code to Unicode char for font lookup
        let ch = CP437_TO_UNICODE.get(code as usize).copied().unwrap_or(' ');
        let col = (code % 16) as u32;
        let row = (code / 16) as u32;
        let base_x = col * gw;
        let base_y = row * gh;

        // default transparent
        if let Some(glyph) = font.glyph(ch) {
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

fn cp437_index(ch: char) -> u32 {
    if (ch as u32) <= 0xFF {
        return ch as u32;
    }

    CP437_TO_UNICODE.iter().position(|&c| c == ch).map(|idx| idx as u32).unwrap_or(b'?' as u32)
}

#[derive(Clone)]
pub struct FKeyGlyphProgram {
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
pub struct FKeyGlyphPrimitive {
    pub bounds: Rectangle,
    pub fkeys: FKeySets,
    pub font: Option<BitFont>,
    pub palette: Palette,
    pub fg_color: u32,
    pub bg_color: u32,
    pub hovered_slot: u32,
    pub hover_type: u32,
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
        queue.write_buffer(&pipeline.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        // Build instances (positions are clip-local pixels)
        let (fg_r, fg_g, fg_b) = self.palette.rgb(self.fg_color);
        let (bg_r, bg_g, bg_b) = self.palette.rgb(self.bg_color);
        let fg = Color::from_rgb8(fg_r, fg_g, fg_b);
        let bg = Color::from_rgb8(bg_r, bg_g, bg_b);

        let hovered = HoverState::from_atomics(self.hovered_slot, self.hover_type);
        let set_idx = self.fkeys.current_set();

        const FLAG_DRAW_BG: u32 = 1;
        const FLAG_BG_ONLY: u32 = 2;

        let control_height = bounds.height - SHADOW_PADDING * 2.0;
        let font_height = self.font.as_ref().map(|f| f.size().height as f32).unwrap_or(16.0);
        let font_width = self.font.as_ref().map(|f| f.size().width as f32).unwrap_or(8.0);

        // Integer magnification for crisp pixel rendering
        // For chars: fit into CHAR_DISPLAY_HEIGHT (32px) with integer scale
        let max_char_magnify = (CHAR_DISPLAY_HEIGHT / font_height).floor().max(1.0);
        // For labels: ~60% of char size, also integer
        let max_label_magnify = (max_char_magnify * 0.6).floor().max(1.0);

        // Effective sizes in logical pixels (pre-scale)
        let char_render_w = font_width * max_char_magnify;
        let char_render_h = font_height * max_char_magnify;
        let label_render_w = font_width * max_label_magnify;
        let label_render_h = font_height * max_label_magnify;

        // Spacing uses the scaled label width
        let label_char_w = label_render_w;

        let content_start_x = SHADOW_PADDING + BORDER_WIDTH + LEFT_PADDING;
        let char_display_y = SHADOW_PADDING + ((control_height - char_render_h) / 2.0).floor();
        let label_y = SHADOW_PADDING + ((control_height - label_render_h) / 2.0).floor();

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
            let slot_x = (content_start_x + slot as f32 * (SLOT_WIDTH + SLOT_SPACING)).floor();
            let char_x = (slot_x + LABEL_WIDTH).floor();
            let label_x = (slot_x - 2.0).floor();

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

                let label_w = (label_render_w * scale).floor();
                let label_h = (label_render_h * scale).floor();

                // Shadow
                instances.push(GlyphInstance {
                    pos: [((x + 1.0) * scale).floor(), ((y + 1.0) * scale).floor()],
                    size: [label_w, label_h],
                    fg: [shadow_color.r, shadow_color.g, shadow_color.b, shadow_color.a],
                    bg: [0.0, 0.0, 0.0, 0.0],
                    glyph,
                    flags: 0,
                    _pad: [0, 0],
                });

                // Foreground
                instances.push(GlyphInstance {
                    pos: [(x * scale).floor(), (y * scale).floor()],
                    size: [label_w, label_h],
                    fg: [label_color.r, label_color.g, label_color.b, label_color.a],
                    bg: [0.0, 0.0, 0.0, 0.0],
                    glyph,
                    flags: 0,
                    _pad: [0, 0],
                });
            }

            // Char background (full cell)
            instances.push(GlyphInstance {
                pos: [(char_x * scale).floor(), (char_display_y * scale).floor()],
                size: [(char_render_w * scale).floor(), (char_render_h * scale).floor()],
                fg: [0.0, 0.0, 0.0, 0.0],
                bg: [bg.r, bg.g, bg.b, 1.0],
                glyph: 0,
                flags: FLAG_BG_ONLY,
                _pad: [0, 0],
            });

            // Char glyph (crisp, integer magnification)
            // code_at returns CP437 code directly - use as atlas index
            let code = self.fkeys.code_at(set_idx, slot);
            let glyph = (code as u32) & 0xFF;
            let char_fg = if is_char_hovered {
                Color::from_rgb((fg.r * 1.3).min(1.0), (fg.g * 1.3).min(1.0), (fg.b * 1.3).min(1.0))
            } else {
                fg
            };

            instances.push(GlyphInstance {
                pos: [(char_x * scale).floor(), (char_display_y * scale).floor()],
                size: [(char_render_w * scale).floor(), (char_render_h * scale).floor()],
                fg: [char_fg.r, char_fg.g, char_fg.b, 1.0],
                bg: [0.0, 0.0, 0.0, 0.0],
                glyph,
                flags: 0,
                _pad: [0, 0],
            });
        }

        // Set number between arrows (no bg), with shadow
        let nav_x = (content_start_x + 12.0 * (SLOT_WIDTH + SLOT_SPACING) + NAV_GAP).floor();
        let set_num = set_idx + 1;
        let num_str = set_num.to_string();
        let num_width = num_str.len() as f32 * label_char_w;
        let next_x = nav_x + NAV_SIZE + NAV_LABEL_SPACE;
        let space_between = next_x - (nav_x + NAV_SIZE);
        let num_x = nav_x + NAV_SIZE + (space_between - num_width) / 2.0;

        let label_color = Color::from_rgba(0.55, 0.55, 0.58, 1.0);
        let shadow_color = Color::from_rgba(0.0, 0.0, 0.0, 0.5);

        for (i, ch) in num_str.chars().enumerate() {
            let digit = ch.to_digit(10).unwrap_or(0);
            let y_off = glyph_y_offset(digit);
            let x = num_x + i as f32 * label_char_w;
            let y = label_y + y_off;
            let glyph = ch as u32;

            let label_w = (label_render_w * scale).floor();
            let label_h = (label_render_h * scale).floor();

            // Shadow
            instances.push(GlyphInstance {
                pos: [((x + 1.0) * scale).floor(), ((y + 1.0) * scale).floor()],
                size: [label_w, label_h],
                fg: [shadow_color.r, shadow_color.g, shadow_color.b, shadow_color.a],
                bg: [0.0, 0.0, 0.0, 0.0],
                glyph,
                flags: 0,
                _pad: [0, 0],
            });
            // Foreground
            instances.push(GlyphInstance {
                pos: [(x * scale).floor(), (y * scale).floor()],
                size: [label_w, label_h],
                fg: [label_color.r, label_color.g, label_color.b, label_color.a],
                bg: [0.0, 0.0, 0.0, 0.0],
                glyph,
                flags: 0,
                _pad: [0, 0],
            });
        }

        pipeline.upload_instances(queue, &instances);

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
            pass.set_bind_group(0, &pipeline.bind_group, &[]);
            pass.set_vertex_buffer(0, pipeline.quad_vertex_buffer.slice(..));
            pass.set_vertex_buffer(1, pipeline.instance_buffer.slice(..));
            pass.draw(0..6, 0..pipeline.instance_count);
        }
    }
}

pub struct FKeyGlyphRenderer {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group: iced::wgpu::BindGroup,
    uniform_buffer: iced::wgpu::Buffer,
    quad_vertex_buffer: iced::wgpu::Buffer,
    instance_buffer: iced::wgpu::Buffer,
    instance_count: u32,

    atlas_texture: iced::wgpu::Texture,
    atlas_view: iced::wgpu::TextureView,
    atlas_sampler: iced::wgpu::Sampler,
    atlas_key: Option<u64>,
    atlas_w: u32,
    atlas_h: u32,
}

impl FKeyGlyphRenderer {
    fn update_atlas(&mut self, device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, key: u64, w: u32, h: u32, rgba: &[u8]) {
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
                        resource: self.uniform_buffer.as_entire_binding(),
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

    fn upload_instances(&mut self, queue: &iced::wgpu::Queue, instances: &[GlyphInstance]) {
        let count = instances
            .len()
            .min((self.instance_buffer.size() as usize) / std::mem::size_of::<GlyphInstance>());
        self.instance_count = count as u32;
        if count == 0 {
            return;
        }
        let bytes = bytemuck::cast_slice(&instances[..count]);
        queue.write_buffer(&self.instance_buffer, 0, bytes);
    }
}

impl shader::Pipeline for FKeyGlyphRenderer {
    fn new(device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("FKey Glyphs Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("fkey_glyphs_shader.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("FKey Glyphs Uniforms"),
            size: std::mem::size_of::<FKeyGlyphUniforms>() as u64,
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

        // Instance buffer
        let instance_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("FKey Glyph Instances"),
            size: (std::mem::size_of::<GlyphInstance>() * 128) as u64,
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
                        has_dynamic_offset: false,
                        min_binding_size: None,
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
                    resource: uniform_buffer.as_entire_binding(),
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
            quad_vertex_buffer,
            instance_buffer,
            instance_count: 0,
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

/// Compute hover state from cursor position
fn compute_hover_state(pos: Point, bounds: Rectangle) -> (u32, u32) {
    let content_start_x = SHADOW_PADDING + BORDER_WIDTH + LEFT_PADDING;
    let control_height = bounds.height - SHADOW_PADDING * 2.0;

    // Check F-key slots
    for slot in 0..12usize {
        let slot_x = content_start_x + slot as f32 * (SLOT_WIDTH + SLOT_SPACING);
        let char_x = slot_x + LABEL_WIDTH;

        if pos.x >= slot_x && pos.x < slot_x + SLOT_WIDTH && pos.y >= SHADOW_PADDING && pos.y < SHADOW_PADDING + control_height {
            let is_on_char = pos.x >= char_x;
            return (slot as u32, if is_on_char { 1 } else { 0 });
        }
    }

    // Check nav buttons
    let nav_x = content_start_x + 12.0 * (SLOT_WIDTH + SLOT_SPACING) + NAV_GAP;
    let nav_y = SHADOW_PADDING + (control_height - NAV_SIZE) / 2.0;
    let next_x = nav_x + NAV_SIZE + NAV_LABEL_SPACE;

    if pos.x >= nav_x && pos.x < nav_x + NAV_SIZE && pos.y >= nav_y && pos.y < nav_y + NAV_SIZE {
        return (NO_HOVER, 2); // NavPrev
    }

    if pos.x >= next_x && pos.x < next_x + NAV_SIZE && pos.y >= nav_y && pos.y < nav_y + NAV_SIZE {
        return (NO_HOVER, 3); // NavNext
    }

    (NO_HOVER, 0)
}

// ═══════════════════════════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════════════════════════

/// GPU-accelerated F-Key Toolbar with shader background
pub struct ShaderFKeyToolbar {
    hovered_slot: Arc<AtomicU32>,
    hover_type: Arc<AtomicU32>,
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
        // Calculate total dimensions
        let content_width = 12.0 * SLOT_WIDTH + 11.0 * SLOT_SPACING + NAV_GAP + NAV_SIZE * 2.0 + 32.0;
        let total_width = content_width + SHADOW_PADDING * 2.0 + BORDER_WIDTH * 2.0 + LEFT_PADDING;
        let total_height = TOOLBAR_HEIGHT + SHADOW_PADDING * 2.0;

        let bg_color_themed = main_area_background(theme);

        // Create shader background
        let shader_bg: Element<'_, FKeyToolbarMessage> = widget::shader(FKeyToolbarProgram {
            bg_color: bg_color_themed,
            hovered_slot: self.hovered_slot.clone(),
            hover_type: self.hover_type.clone(),
        })
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .into();

        // Create glyph overlay (atlas)
        let glyph_overlay: Element<'_, FKeyToolbarMessage> = widget::shader(FKeyGlyphProgram {
            fkeys,
            font,
            palette,
            fg_color,
            bg_color,
            hovered_slot: self.hovered_slot.clone(),
            hover_type: self.hover_type.clone(),
        })
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .into();

        // Calculate nav arrow positions
        let content_start_x = SHADOW_PADDING + BORDER_WIDTH + LEFT_PADDING;
        let nav_x = content_start_x + 12.0 * (SLOT_WIDTH + SLOT_SPACING) + NAV_GAP;
        let next_x = nav_x + NAV_SIZE + NAV_LABEL_SPACE;
        let arrow_size = NAV_SIZE; // SVG arrow size matches nav button size

        // Get current hover state for arrow colors
        let hover_type = self.hover_type.load(Ordering::Relaxed);
        let is_prev_hovered = hover_type == 2;
        let is_next_hovered = hover_type == 3;

        // Arrow colors: dim gray normally, bright white on hover
        let arrow_normal = Color::from_rgb(0.55, 0.55, 0.58);
        let arrow_hover = Color::WHITE;

        // Create SVG arrow overlay with hover-dependent colors
        let left_arrow = svg(svg::Handle::from_memory(ARROW_LEFT_SVG))
            .width(Length::Fixed(arrow_size))
            .height(Length::Fixed(arrow_size))
            .style(move |_theme, _status| svg::Style {
                color: Some(if is_prev_hovered { arrow_hover } else { arrow_normal }),
            });

        let right_arrow = svg(svg::Handle::from_memory(ARROW_RIGHT_SVG))
            .width(Length::Fixed(arrow_size))
            .height(Length::Fixed(arrow_size))
            .style(move |_theme, _status| svg::Style {
                color: Some(if is_next_hovered { arrow_hover } else { arrow_normal }),
            });

        // Space before first arrow (arrow_size now equals NAV_SIZE, so offset is 0)
        let space_before = nav_x;
        // Space between arrows
        let space_between = next_x - nav_x - NAV_SIZE;

        let arrow_overlay: Element<'_, FKeyToolbarMessage> = container(row![
            Space::new().width(Length::Fixed(space_before)),
            left_arrow,
            Space::new().width(Length::Fixed(space_between)),
            right_arrow,
        ])
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .center_y(Length::Fixed(total_height))
        .into();

        // Stack: shader background, glyph overlay, arrow overlay
        let toolbar_stack: Element<'_, FKeyToolbarMessage> = widget::stack![shader_bg, glyph_overlay, arrow_overlay]
            .width(Length::Fixed(total_width))
            .height(Length::Fixed(total_height))
            .into();

        // Wrap in container to center horizontally in parent
        container(toolbar_stack)
            .width(Length::Fill)
            .height(Length::Fixed(total_height))
            .center_x(Length::Fill)
            .into()
    }
}
