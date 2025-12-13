//! GPU-accelerated F-Key Toolbar Component
//!
//! Renders F1-F12 function key slots with characters from the current font.
//! Uses WGSL shader for background (drop shadow, borders, hover highlights)
//! and Canvas overlay for text rendering, plus SVG arrows for navigation.

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use iced::{
    Color, Element, Length, Point, Rectangle, Size, Theme,
    mouse::{self, Cursor},
    widget::{
        self, Space,
        canvas::{self, Cache, Frame, Geometry},
        container, row, shader, svg,
    },
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
// Canvas Text Overlay
// ═══════════════════════════════════════════════════════════════════════════

struct TextOverlayProgram {
    fkeys: FKeySets,
    font: Option<BitFont>,
    palette: Palette,
    fg_color: u32,
    bg_color: u32,
    cache: Cache,
    hovered_slot: Arc<AtomicU32>,
    hover_type: Arc<AtomicU32>,
}

impl canvas::Program<FKeyToolbarMessage> for TextOverlayProgram {
    type State = ();

    fn draw(&self, _state: &Self::State, renderer: &iced::Renderer, _theme: &Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry> {
        // Colors
        let label_color = Color::from_rgba(0.55, 0.55, 0.58, 1.0);
        let label_hover_color = Color::from_rgba(0.85, 0.85, 0.88, 1.0);
        let shadow_color = Color::from_rgba(0.0, 0.0, 0.0, 0.5);

        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            // Get palette colors for characters
            let (fg_r, fg_g, fg_b) = self.palette.rgb(self.fg_color);
            let (bg_r, bg_g, bg_b) = self.palette.rgb(self.bg_color);
            let fg = Color::from_rgb8(fg_r, fg_g, fg_b);
            let bg = Color::from_rgb8(bg_r, bg_g, bg_b);

            let set_idx = self.fkeys.current_set();

            // Get hover state from atomics
            let hovered_slot = self.hovered_slot.load(Ordering::Relaxed);
            let hover_type = self.hover_type.load(Ordering::Relaxed);
            let hovered = HoverState::from_atomics(hovered_slot, hover_type);

            // Control area (excluding shadow padding)
            let control_height = bounds.height - SHADOW_PADDING * 2.0;

            // Calculate font scale
            let font_height = self.font.as_ref().map(|f| f.size().height as f32).unwrap_or(16.0);
            let scale = CHAR_DISPLAY_HEIGHT / font_height;
            let font_width = self.font.as_ref().map(|f| f.size().width as f32).unwrap_or(8.0);
            let label_char_w = font_width * scale * 0.6;

            // Content starts after shadow padding, border and left padding
            let content_start_x = SHADOW_PADDING + BORDER_WIDTH + LEFT_PADDING;

            // Center char display vertically within control bounds
            let char_display_y = SHADOW_PADDING + ((control_height - CHAR_DISPLAY_HEIGHT) / 2.0).floor();

            // Center labels vertically (they are smaller: scale * 0.6)
            let label_height = font_height * scale * 0.6;
            let label_y = SHADOW_PADDING + ((control_height - label_height) / 2.0).floor();

            // Draw each F-key slot
            for slot in 0..12usize {
                let slot_x = (content_start_x + slot as f32 * (SLOT_WIDTH + SLOT_SPACING)).floor();
                let char_x = (slot_x + LABEL_WIDTH).floor();
                let label_x = (slot_x - 2.0).floor();

                let is_label_hovered = matches!(hovered, HoverState::Slot(s, false) if s == slot);
                let is_char_hovered = matches!(hovered, HoverState::Slot(s, true) if s == slot);

                // Get character code
                let code = self.fkeys.code_at(set_idx, slot);
                let ch = char::from_u32(code as u32).unwrap_or(' ');

                // Draw label with drop shadow (01, 02, etc.)
                let current_label_color = if is_label_hovered { label_hover_color } else { label_color };
                // Shadow first
                self.draw_label(frame, label_x + 1.0, label_y + 1.0, slot, shadow_color, scale, label_char_w);
                // Then label
                self.draw_label(frame, label_x, label_y, slot, current_label_color, scale, label_char_w);

                // Draw character - brighten FG on hover (no backdrop, just FG color change)
                let char_fg = if is_char_hovered {
                    // Brighten the foreground color on hover
                    Color::from_rgb((fg.r * 1.3).min(1.0), (fg.g * 1.3).min(1.0), (fg.b * 1.3).min(1.0))
                } else {
                    fg
                };
                self.draw_glyph(frame, char_x, char_display_y, ch, char_fg, bg, scale);
            }

            // Navigation section - arrows are SVG in separate layer
            let nav_x = (content_start_x + 12.0 * (SLOT_WIDTH + SLOT_SPACING) + NAV_GAP).floor();

            // Set number
            let set_num = set_idx + 1;
            let num_str = format!("{}", set_num);
            let num_width = num_str.len() as f32 * label_char_w;
            let next_x = nav_x + NAV_SIZE + NAV_LABEL_SPACE;
            let space_between = next_x - (nav_x + NAV_SIZE);
            let num_x = nav_x + NAV_SIZE + (space_between - num_width) / 2.0;

            // Shadow first
            self.draw_set_number(frame, num_x + 1.0, label_y + 1.0, set_num, shadow_color, scale, label_char_w);
            // Then number
            self.draw_set_number(frame, num_x, label_y, set_num, label_color, scale, label_char_w);
        });

        vec![geometry]
    }

    fn update(&self, _state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: Cursor) -> Option<canvas::Action<FKeyToolbarMessage>> {
        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let (new_slot, new_type) = if let Some(pos) = cursor.position_in(bounds) {
                    compute_hover_state(pos, bounds)
                } else {
                    (NO_HOVER, 0)
                };

                let old_slot = self.hovered_slot.swap(new_slot, Ordering::Relaxed);
                let old_type = self.hover_type.swap(new_type, Ordering::Relaxed);

                if old_slot != new_slot || old_type != new_type {
                    self.cache.clear();
                    return Some(canvas::Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(cursor_pos) = cursor.position_in(bounds) else {
                    return None;
                };

                let (slot, hover_type) = compute_hover_state(cursor_pos, bounds);
                let state = HoverState::from_atomics(slot, hover_type);

                match state {
                    HoverState::Slot(idx, true) => Some(canvas::Action::publish(FKeyToolbarMessage::TypeFKey(idx))),
                    HoverState::Slot(idx, false) => Some(canvas::Action::publish(FKeyToolbarMessage::OpenCharSelector(idx))),
                    HoverState::NavPrev => Some(canvas::Action::publish(FKeyToolbarMessage::PrevSet)),
                    HoverState::NavNext => Some(canvas::Action::publish(FKeyToolbarMessage::NextSet)),
                    HoverState::None => None,
                }
            }
            iced::Event::Mouse(mouse::Event::CursorLeft) => {
                self.hovered_slot.store(NO_HOVER, Ordering::Relaxed);
                self.hover_type.store(0, Ordering::Relaxed);
                self.cache.clear();
                Some(canvas::Action::request_redraw())
            }
            _ => None,
        }
    }

    fn mouse_interaction(&self, _state: &Self::State, bounds: Rectangle, cursor: Cursor) -> mouse::Interaction {
        if let Some(pos) = cursor.position_in(bounds) {
            let (slot, hover_type) = compute_hover_state(pos, bounds);
            let state = HoverState::from_atomics(slot, hover_type);
            match state {
                HoverState::None => mouse::Interaction::default(),
                _ => mouse::Interaction::Pointer,
            }
        } else {
            mouse::Interaction::default()
        }
    }
}

impl TextOverlayProgram {
    /// Draw a single glyph from the font
    fn draw_glyph(&self, frame: &mut Frame, x: f32, y: f32, ch: char, fg: Color, bg: Color, scale: f32) {
        let x = x.floor();
        let y = y.floor();

        let Some(font) = &self.font else {
            frame.fill_rectangle(Point::new(x, y), Size::new(8.0 * scale, 16.0 * scale), bg);
            return;
        };

        let font_width = font.size().width as f32;
        let font_height = font.size().height as f32;
        let char_width = (font_width * scale).floor();
        let char_height = (font_height * scale).floor();
        let pixel_w = scale.floor().max(1.0);
        let pixel_h = scale.floor().max(1.0);

        // Fill background
        frame.fill_rectangle(Point::new(x, y), Size::new(char_width, char_height), bg);

        // Draw glyph pixels
        if let Some(glyph) = font.glyph(ch) {
            for (row_idx, row) in glyph.bitmap.pixels.iter().enumerate() {
                let row_y = y + (row_idx as f32 * pixel_h).floor();
                let mut run_start: Option<usize> = None;

                for (col_idx, &pixel) in row.iter().enumerate() {
                    if pixel {
                        if run_start.is_none() {
                            run_start = Some(col_idx);
                        }
                    } else if let Some(start) = run_start {
                        let run_len = col_idx - start;
                        frame.fill_rectangle(
                            Point::new(x + (start as f32 * pixel_w).floor(), row_y),
                            Size::new(run_len as f32 * pixel_w, pixel_h),
                            fg,
                        );
                        run_start = None;
                    }
                }
                if let Some(start) = run_start {
                    let run_len = row.len() - start;
                    frame.fill_rectangle(
                        Point::new(x + (start as f32 * pixel_w).floor(), row_y),
                        Size::new(run_len as f32 * pixel_w, pixel_h),
                        fg,
                    );
                }
            }
        }
    }

    /// Draw F-key label (01, 02, etc.)
    fn draw_label(&self, frame: &mut Frame, x: f32, y: f32, slot: usize, color: Color, scale: f32, char_w: f32) {
        let num = slot + 1;
        let label_chars: Vec<char> = if num < 10 {
            vec!['0', char::from_digit(num as u32, 10).unwrap_or('?')]
        } else if num == 10 {
            vec!['1', '0']
        } else if num == 11 {
            vec!['1', '1']
        } else {
            vec!['1', '2']
        };

        let label_scale = scale * 0.6;

        for (i, ch) in label_chars.iter().enumerate() {
            let glyph_y_offset = self.glyph_content_y_offset(*ch, label_scale);
            self.draw_glyph_no_bg(frame, x + i as f32 * char_w, y + glyph_y_offset, *ch, color, label_scale);
        }
    }

    /// Draw glyph without background (for labels)
    fn draw_glyph_no_bg(&self, frame: &mut Frame, x: f32, y: f32, ch: char, fg: Color, scale: f32) {
        let x = x.floor();
        let y = y.floor();

        let Some(font) = &self.font else {
            return;
        };

        let pixel_w = scale.floor().max(1.0);
        let pixel_h = scale.floor().max(1.0);

        if let Some(glyph) = font.glyph(ch) {
            for (row_idx, row) in glyph.bitmap.pixels.iter().enumerate() {
                let row_y = y + (row_idx as f32 * pixel_h).floor();
                let mut run_start: Option<usize> = None;

                for (col_idx, &pixel) in row.iter().enumerate() {
                    if pixel {
                        if run_start.is_none() {
                            run_start = Some(col_idx);
                        }
                    } else if let Some(start) = run_start {
                        let run_len = col_idx - start;
                        frame.fill_rectangle(
                            Point::new(x + (start as f32 * pixel_w).floor(), row_y),
                            Size::new(run_len as f32 * pixel_w, pixel_h),
                            fg,
                        );
                        run_start = None;
                    }
                }
                if let Some(start) = run_start {
                    let run_len = row.len() - start;
                    frame.fill_rectangle(
                        Point::new(x + (start as f32 * pixel_w).floor(), row_y),
                        Size::new(run_len as f32 * pixel_w, pixel_h),
                        fg,
                    );
                }
            }
        }
    }

    /// Compute Y offset to center glyph content vertically
    fn glyph_content_y_offset(&self, ch: char, scale: f32) -> f32 {
        let Some(font) = &self.font else {
            return 0.0;
        };
        let Some(glyph) = font.glyph(ch) else {
            return 0.0;
        };

        let font_height = font.size().height as f32;
        let char_height = (font_height * scale).floor();
        let pixel_h = scale.floor().max(1.0);

        let mut min_row: Option<usize> = None;
        let mut max_row: Option<usize> = None;

        for (row_idx, row) in glyph.bitmap.pixels.iter().enumerate() {
            if row.iter().any(|&p| p) {
                min_row = Some(min_row.map_or(row_idx, |m| m.min(row_idx)));
                max_row = Some(max_row.map_or(row_idx, |m| m.max(row_idx)));
            }
        }

        let (Some(min_row), Some(max_row)) = (min_row, max_row) else {
            return 0.0;
        };

        let used_height = ((max_row - min_row + 1) as f32) * pixel_h;
        let desired_top = ((char_height - used_height) / 2.0).floor();
        let current_top = (min_row as f32) * pixel_h;
        (desired_top - current_top).floor()
    }

    /// Draw set number
    fn draw_set_number(&self, frame: &mut Frame, x: f32, y: f32, set_num: usize, color: Color, scale: f32, char_w: f32) {
        let num_str = format!("{}", set_num);
        let num_scale = scale * 0.6;

        for (i, ch) in num_str.chars().enumerate() {
            let glyph_y_offset = self.glyph_content_y_offset(ch, num_scale);
            self.draw_glyph_no_bg(frame, x + i as f32 * char_w, y + glyph_y_offset, ch, color, num_scale);
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

        // Create text overlay
        let text_overlay: Element<'_, FKeyToolbarMessage> = widget::canvas(TextOverlayProgram {
            fkeys,
            font,
            palette,
            fg_color,
            bg_color,
            cache: Cache::new(),
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

        // Stack: shader background, text overlay, arrow overlay
        let toolbar_stack: Element<'_, FKeyToolbarMessage> = widget::stack![shader_bg, text_overlay, arrow_overlay]
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
