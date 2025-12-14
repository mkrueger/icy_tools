//! GPU-accelerated Segmented Control with Shader Background
//!
//! Uses a WGSL shader for the background (shadows, glow, selection highlights)
//! and a Canvas overlay for text rendering with the BitFont.

use iced::{
    Color, Element, Length, Point, Rectangle, Size, Theme,
    mouse::{self, Cursor},
    widget::{
        self,
        canvas::{self, Cache, Frame, Geometry},
        shader,
    },
};
use icy_engine::BitFont;
use icy_engine_gui::theme::main_area_background;
use std::sync::Once;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

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

/// Constants for segmented control layout
const SEGMENT_PADDING_H: f32 = 12.0;
const SEGMENT_HEIGHT: f32 = 32.0;
const CORNER_RADIUS: f32 = 6.0;
const BORDER_WIDTH: f32 = 1.0;
const SHADOW_PADDING: f32 = 6.0;
const NO_HOVER: u32 = 0xFFFF_FFFF;

/// Maximum number of segments supported
const MAX_SEGMENTS: usize = 8;

/// Messages from the segmented control
#[derive(Clone, Debug)]
pub enum SegmentedControlMessage<T> {
    /// Selected a segment
    Selected(T),
    /// Clicked on a char segment (for popup)
    CharClicked(T),
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
    /// Selected segment index
    selected_segment: u32,
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
    pub selected_index: usize,
    pub hovered_index: Arc<AtomicU32>,
    pub bg_color: Color,
    pub total_width: f32,
}

impl SegmentedControlProgram {
    /// Get segment index at cursor position
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

    fn draw(&self, _state: &Self::State, _cursor: Cursor, bounds: Rectangle) -> Self::Primitive {
        let hovered_raw = self.hovered_index.load(Ordering::Relaxed);
        let hovered_index = if hovered_raw == NO_HOVER { None } else { Some(hovered_raw as usize) };

        SegmentedControlPrimitive {
            bounds,
            segment_widths: self.segment_widths.clone(),
            selected_index: self.selected_index,
            hovered_index,
            bg_color: self.bg_color,
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
    pub bounds: Rectangle,
    pub segment_widths: Vec<f32>,
    pub selected_index: usize,
    pub hovered_index: Option<usize>,
    pub bg_color: Color,
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
                    "[segmented_control] origin=({:.1},{:.1}) size=({:.1},{:.1}) num_segments={} selected={} hovered={:?} widths={:?}",
                    origin_x,
                    origin_y,
                    size_w,
                    size_h,
                    self.segment_widths.len(),
                    self.selected_index,
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
            selected_segment: self.selected_index as u32,
            hovered_segment: self.hovered_index.map(|i| i as u32).unwrap_or(0xFFFFFFFF),
            _flags: 0,
            corner_radius: CORNER_RADIUS * scale,
            time: 0.0,
            _padding: [0.0, 0.0],
            bg_color: [self.bg_color.r, self.bg_color.g, self.bg_color.b, self.bg_color.a],
            segment_widths,
        };

        queue.write_buffer(&pipeline.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
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
            pass.set_bind_group(0, &pipeline.bind_group, &[]);
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
}

impl shader::Pipeline for SegmentedControlRenderer {
    fn new(device: &iced::wgpu::Device, _queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("Segmented Control Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("segmented_control_shader.wgsl").into()),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("Segmented Control Uniforms"),
            size: std::mem::size_of::<SegmentedControlUniforms>() as u64,
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
                    has_dynamic_offset: false,
                    min_binding_size: None,
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
                resource: uniform_buffer.as_entire_binding(),
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
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Canvas Text Overlay
// ═══════════════════════════════════════════════════════════════════════════

/// Canvas program for drawing text over the shader background
struct TextOverlayProgram<T: Clone + PartialEq> {
    segments: Vec<Segment<T>>,
    segment_widths: Vec<f32>,
    selected_index: usize,
    font: Option<BitFont>,
    cache: Cache,
    hovered_index: Arc<AtomicU32>,
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
            let pixel_size = 1.0; // Native size

            // Content area starts after shadow padding and border
            let content_x = SHADOW_PADDING + BORDER_WIDTH;
            let content_y = SHADOW_PADDING + BORDER_WIDTH;
            let content_h = bounds.height - SHADOW_PADDING * 2.0 - BORDER_WIDTH * 2.0;

            // Draw text for each segment
            let mut x = content_x;
            for (idx, segment) in self.segments.iter().enumerate() {
                let width = self.segment_widths.get(idx).copied().unwrap_or(0.0);
                let is_selected = idx == self.selected_index;
                let color = if is_selected { selected_text_color } else { text_color };

                // Calculate text position (centered in segment)
                let text_width = match &segment.content {
                    SegmentContent::Text(text) => text.chars().count() as f32 * char_w,
                    SegmentContent::Char(_) => char_w,
                };
                let text_x = x + (width - text_width) / 2.0;
                let text_y = content_y + (content_h - char_h) / 2.0;

                // Draw text shadow first (offset down-right)
                match &segment.content {
                    SegmentContent::Text(text) => {
                        self.draw_text(
                            frame,
                            text_x + shadow_offset.0,
                            text_y + shadow_offset.1,
                            text,
                            shadow_color,
                            char_w,
                            pixel_size,
                        );
                    }
                    SegmentContent::Char(ch) => {
                        self.draw_glyph(frame, text_x + shadow_offset.0, text_y + shadow_offset.1, *ch, shadow_color, pixel_size);
                    }
                }

                // Draw text
                match &segment.content {
                    SegmentContent::Text(text) => {
                        self.draw_text(frame, text_x, text_y, text, color, char_w, pixel_size);
                    }
                    SegmentContent::Char(ch) => {
                        self.draw_glyph(frame, text_x, text_y, *ch, color, pixel_size);
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
                let new_hover = cursor.position_in(bounds).and_then(|pos| self.segment_at_x(pos.x, bounds));

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

                let Some(idx) = self.segment_at_x(pos.x, bounds) else {
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
    fn segment_at_x(&self, x: f32, bounds: Rectangle) -> Option<usize> {
        // Content area starts after shadow padding and border
        let content_x = SHADOW_PADDING + BORDER_WIDTH;
        let local_x = x - content_x;
        if local_x < 0.0 {
            return None;
        }

        let mut seg_x = 0.0;
        for (idx, &width) in self.segment_widths.iter().enumerate() {
            if local_x >= seg_x && local_x < seg_x + width {
                return Some(idx);
            }
            seg_x += width;
        }

        None
    }

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
        // Calculate segment widths
        let segment_widths: Vec<f32> = segments.iter().map(|seg| calculate_segment_width(seg, &font)).collect();
        // Content width + border + shadow padding on both sides
        let content_width = segment_widths.iter().sum::<f32>();
        let total_width = content_width + BORDER_WIDTH * 2.0 + SHADOW_PADDING * 2.0;
        let total_height = SEGMENT_HEIGHT + SHADOW_PADDING * 2.0;

        // Find selected index
        let selected_index = segments.iter().position(|seg| seg.value == selected).unwrap_or(0);

        // Use the same main-area background as the editor canvas/preview areas.
        let bg_color = main_area_background(theme);

        // Clone segment values for message mapping
        let segment_values: Vec<T> = segments.iter().map(|s| s.value.clone()).collect();

        // Create the shader background
        let shader_bg: Element<'_, SegmentedControlMessage<usize>> = widget::shader(SegmentedControlProgram {
            segment_widths: segment_widths.clone(),
            selected_index,
            hovered_index: self.hovered_index.clone(),
            bg_color,
            total_width,
        })
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .into();

        // Map shader messages from usize to T
        let shader_bg: Element<'_, SegmentedControlMessage<T>> = shader_bg.map(move |msg| match msg {
            SegmentedControlMessage::Selected(idx) => SegmentedControlMessage::Selected(segment_values[idx].clone()),
            SegmentedControlMessage::CharClicked(idx) => SegmentedControlMessage::CharClicked(segment_values[idx].clone()),
        });

        // Create the text overlay canvas with owned data
        let text_overlay: Element<'_, SegmentedControlMessage<T>> = widget::canvas(TextOverlayProgram::<T> {
            segments,
            segment_widths,
            selected_index,
            font,
            cache: Cache::new(),
            hovered_index: self.hovered_index.clone(),
        })
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .into();

        // Stack the text overlay on top of the shader background
        widget::stack![shader_bg, text_overlay]
            .width(Length::Fixed(total_width))
            .height(Length::Fixed(total_height))
            .into()
    }
}

/// Calculate segment width - uses native font size (pixel_size = 1)
fn calculate_segment_width<T: Clone>(segment: &Segment<T>, font: &Option<BitFont>) -> f32 {
    let font_width = font.as_ref().map(|f| f.size().width as f32).unwrap_or(8.0);
    let char_w = font_width; // Native size, no scaling

    let content_width = match &segment.content {
        SegmentContent::Text(text) => text.chars().count() as f32 * char_w,
        SegmentContent::Char(_) => char_w,
    };

    content_width + SEGMENT_PADDING_H * 2.0
}
