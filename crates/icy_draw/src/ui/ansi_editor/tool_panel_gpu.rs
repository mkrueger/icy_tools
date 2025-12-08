//! GPU-accelerated Tool Panel with animations
//!
//! Features:
//! - SVG icons rendered to textures with drop shadows
//! - Hover glow effects
//! - Selection highlighting
//! - Blend-over animation for tool state changes (e.g., outline ↔ filled)
//! - Dynamic layout using cascada layout engine
//! - Scale factor aware

use iced::{
    Element, Length, Rectangle, mouse,
    widget::shader::{self, Shader},
};
use icy_engine_edit::tools::{TOOL_SLOTS, Tool, click_tool_slot, get_slot_display_tool};

use super::constants::{TOOL_ATLAS_COLS, TOOL_ATLAS_ROWS, TOOL_BLEND_ANIMATION_DURATION, TOOL_ICON_PADDING, TOOL_ICON_SIZE};

/// Size of each tool icon in logical pixels
const ICON_SIZE: f32 = TOOL_ICON_SIZE;
/// Padding between icons
const ICON_PADDING: f32 = TOOL_ICON_PADDING;
/// Animation duration for tool state transitions
const BLEND_ANIMATION_DURATION: f32 = TOOL_BLEND_ANIMATION_DURATION;
/// Maximum number of tool buttons
const MAX_BUTTONS: usize = 9;

/// Messages from the tool panel
#[derive(Clone, Debug)]
pub enum ToolPanelMessage {
    /// Clicked on a tool slot
    ClickSlot(usize),
    /// Animation tick
    Tick(f32),
}

/// Per-button animation state
#[derive(Clone, Debug)]
struct ButtonAnimationState {
    /// Previous tool displayed (for blend-over)
    previous_tool: Option<Tool>,
    /// Current tool displayed
    current_tool: Tool,
    /// Blend progress (0.0 = previous, 1.0 = current)
    blend_progress: f32,
    /// Whether blend animation is running
    animating: bool,
}

impl ButtonAnimationState {
    fn new(tool: Tool) -> Self {
        Self {
            previous_tool: None,
            current_tool: tool,
            blend_progress: 1.0,
            animating: false,
        }
    }

    fn set_tool(&mut self, tool: Tool) {
        if tool != self.current_tool {
            self.previous_tool = Some(self.current_tool);
            self.current_tool = tool;
            self.blend_progress = 0.0;
            self.animating = true;
        }
    }

    fn tick(&mut self, delta: f32) {
        if self.animating {
            self.blend_progress += delta / BLEND_ANIMATION_DURATION;
            if self.blend_progress >= 1.0 {
                self.blend_progress = 1.0;
                self.animating = false;
                self.previous_tool = None;
            }
        }
    }
}

/// Tool panel state
pub struct ToolPanel {
    /// Currently selected tool
    current_tool: Tool,
    /// Per-button animation states
    button_states: Vec<ButtonAnimationState>,
    /// Hovered button index
    hovered_button: Option<usize>,
    /// Time accumulator for effects
    time: f32,
}

impl Default for ToolPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolPanel {
    pub fn new() -> Self {
        // Initialize button states with their default tools
        let button_states: Vec<_> = (0..TOOL_SLOTS.len().min(MAX_BUTTONS))
            .map(|slot| {
                let tool = get_slot_display_tool(slot, Tool::Click);
                ButtonAnimationState::new(tool)
            })
            .collect();

        Self {
            current_tool: Tool::Click,
            button_states,
            hovered_button: None,
            time: 0.0,
        }
    }

    /// Get the current tool
    pub fn current_tool(&self) -> Tool {
        self.current_tool
    }

    /// Set the current tool
    pub fn set_tool(&mut self, tool: Tool) {
        self.current_tool = tool;
        self.update_button_states();
    }

    /// Check if any animation is running
    pub fn needs_animation(&self) -> bool {
        self.button_states.iter().any(|s| s.animating)
    }

    /// Update animation state
    pub fn tick(&mut self, delta: f32) {
        self.time += delta;
        for state in &mut self.button_states {
            state.tick(delta);
        }
    }

    /// Update button states based on current tool
    fn update_button_states(&mut self) {
        for (slot, state) in self.button_states.iter_mut().enumerate() {
            let display_tool = get_slot_display_tool(slot, self.current_tool);
            state.set_tool(display_tool);
        }
    }

    /// Update the tool panel state
    pub fn update(&mut self, message: ToolPanelMessage) -> iced::Task<ToolPanelMessage> {
        match message {
            ToolPanelMessage::ClickSlot(slot) => {
                self.current_tool = click_tool_slot(slot, self.current_tool);
                self.update_button_states();
            }
            ToolPanelMessage::Tick(delta) => {
                self.tick(delta);
            }
        }
        iced::Task::none()
    }

    /// Render the tool panel with the given available width and background color
    pub fn view_with_config(&self, available_width: f32, bg_color: iced::Color) -> Element<'_, ToolPanelMessage> {
        // Calculate how many columns fit in the available width
        // Total width = cols * ICON_SIZE + (cols + 1) * ICON_PADDING
        // So: cols = (available_width - ICON_PADDING) / (ICON_SIZE + ICON_PADDING)
        let cols = ((available_width - ICON_PADDING) / (ICON_SIZE + ICON_PADDING)).floor() as usize;
        let cols = cols.max(1).min(MAX_BUTTONS); // At least 1 column

        let num_buttons = self.button_states.len();
        let rows = (num_buttons + cols - 1) / cols; // Ceiling division

        // Use full available width to allow centering in shader
        let total_width = available_width;
        let total_height = rows as f32 * (ICON_SIZE + ICON_PADDING) + ICON_PADDING;

        let bg_color_arr = [bg_color.r, bg_color.g, bg_color.b];

        // Build button data for the shader
        let buttons: Vec<ButtonData> = self
            .button_states
            .iter()
            .enumerate()
            .map(|(slot, state)| {
                let is_selected = TOOL_SLOTS.get(slot).map_or(false, |tools| tools.contains(self.current_tool));
                let is_hovered = self.hovered_button == Some(slot);

                ButtonData {
                    current_tool: state.current_tool,
                    previous_tool: state.previous_tool,
                    blend_progress: state.blend_progress,
                    is_selected,
                    is_hovered,
                }
            })
            .collect();

        Shader::new(ToolPanelProgram {
            buttons,
            time: self.time,
            hovered_button: self.hovered_button,
            cols,
            rows,
            num_buttons,
            bg_color: bg_color_arr,
        })
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .into()
    }
}

/// Data for a single button
#[derive(Debug, Clone)]
struct ButtonData {
    current_tool: Tool,
    previous_tool: Option<Tool>,
    blend_progress: f32,
    is_selected: bool,
    is_hovered: bool,
}

/// Shader program for rendering the tool panel
#[derive(Debug, Clone)]
struct ToolPanelProgram {
    buttons: Vec<ButtonData>,
    time: f32,
    hovered_button: Option<usize>,
    cols: usize,
    rows: usize,
    num_buttons: usize,
    bg_color: [f32; 3],
}

impl shader::Program<ToolPanelMessage> for ToolPanelProgram {
    type State = Option<usize>; // Hovered button index
    type Primitive = ToolPanelPrimitive;

    fn draw(&self, state: &Self::State, _cursor: mouse::Cursor, _bounds: Rectangle) -> Self::Primitive {
        ToolPanelPrimitive {
            buttons: self.buttons.clone(),
            time: self.time,
            hovered_button: *state,
            cols: self.cols,
            rows: self.rows,
            num_buttons: self.num_buttons,
            bg_color: self.bg_color,
        }
    }

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<iced::widget::Action<ToolPanelMessage>> {
        let cols = self.cols;
        let rows = self.rows;
        let num_buttons = self.buttons.len();

        // Helper to get button slot from position
        let get_slot = |pos: iced::Point, bounds: Rectangle| -> Option<usize> {
            // Calculate center offset (same as shader)
            let content_width = cols as f32 * (ICON_SIZE + ICON_PADDING) + ICON_PADDING;
            let x_offset = (bounds.width - content_width) * 0.5;

            // Check if inside content area
            if pos.x < x_offset + ICON_PADDING || pos.y < ICON_PADDING {
                return None;
            }

            let x = pos.x - x_offset - ICON_PADDING;
            let y = pos.y - ICON_PADDING;

            let cell_size = ICON_SIZE + ICON_PADDING;
            let col = (x / cell_size) as usize;
            let row = (y / cell_size) as usize;

            // Check within grid
            if col >= cols || row >= rows {
                return None;
            }

            // Check if within the icon area (not in padding between icons)
            let x_in_cell = x - (col as f32 * cell_size);
            let y_in_cell = y - (row as f32 * cell_size);

            if x_in_cell > ICON_SIZE || y_in_cell > ICON_SIZE {
                return None;
            }

            let slot = row * cols + col;
            if slot < num_buttons { Some(slot) } else { None }
        };

        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let new_hover = cursor.position_in(bounds).and_then(|p| get_slot(p, bounds));

                if *state != new_hover {
                    *state = new_hover;
                    return Some(iced::widget::Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(slot) = cursor.position_in(bounds).and_then(|p| get_slot(p, bounds)) {
                    return Some(iced::widget::Action::publish(ToolPanelMessage::ClickSlot(slot)));
                }
                None
            }
            _ => None,
        }
    }
}

/// Uniforms for the shader
#[repr(C, align(16))]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ToolPanelUniforms {
    widget_size: [f32; 2],
    icon_size: f32,
    icon_padding: f32,
    time: f32,
    cols: u32,
    rows: u32,
    num_buttons: u32,
    // Background color from theme
    bg_color: [f32; 3],
    _padding2: f32,
    // Per-button data (packed)
    // Each button: [blend_progress, is_selected, is_hovered, current_tool_idx]
    button_data: [[f32; 4]; MAX_BUTTONS],
}

/// Shader primitive for GPU rendering
#[derive(Debug, Clone)]
struct ToolPanelPrimitive {
    buttons: Vec<ButtonData>,
    time: f32,
    hovered_button: Option<usize>,
    cols: usize,
    rows: usize,
    num_buttons: usize,
    bg_color: [f32; 3],
}

impl shader::Primitive for ToolPanelPrimitive {
    type Pipeline = ToolPanelRenderer;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        _device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        bounds: &Rectangle,
        viewport: &iced::advanced::graphics::Viewport,
    ) {
        let scale = viewport.scale_factor();

        // Pack button data
        let mut button_data = [[0.0f32; 4]; MAX_BUTTONS];
        for (i, btn) in self.buttons.iter().enumerate().take(MAX_BUTTONS) {
            let is_hovered = self.hovered_button == Some(i);
            button_data[i] = [
                btn.blend_progress,
                if btn.is_selected { 1.0 } else { 0.0 },
                if is_hovered { 1.0 } else { 0.0 },
                tool_to_index(btn.current_tool) as f32,
            ];
        }

        let uniforms = ToolPanelUniforms {
            widget_size: [bounds.width * scale, bounds.height * scale],
            icon_size: ICON_SIZE * scale,
            icon_padding: ICON_PADDING * scale,
            time: self.time,
            cols: self.cols as u32,
            rows: self.rows as u32,
            num_buttons: self.num_buttons as u32,
            bg_color: self.bg_color,
            _padding2: 0.0,
            button_data,
        };

        queue.write_buffer(&pipeline.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        let mut render_pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
            label: Some("Tool Panel Render Pass"),
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
            render_pass.set_scissor_rect(clip_bounds.x, clip_bounds.y, clip_bounds.width, clip_bounds.height);
            render_pass.set_viewport(
                clip_bounds.x as f32,
                clip_bounds.y as f32,
                clip_bounds.width as f32,
                clip_bounds.height as f32,
                0.0,
                1.0,
            );
            render_pass.set_pipeline(&pipeline.pipeline);
            render_pass.set_bind_group(0, &pipeline.bind_group, &[]);
            render_pass.draw(0..6, 0..1);
        }
    }
}

/// GPU renderer/pipeline for the tool panel
pub struct ToolPanelRenderer {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group: iced::wgpu::BindGroup,
    uniform_buffer: iced::wgpu::Buffer,
    #[allow(dead_code)]
    icon_atlas: iced::wgpu::Texture,
}

impl shader::Pipeline for ToolPanelRenderer {
    fn new(device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        // Load and compile shader
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("Tool Panel Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("tool_panel_shader.wgsl").into()),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("Tool Panel Uniform Buffer"),
            size: std::mem::size_of::<ToolPanelUniforms>() as u64,
            usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create icon atlas texture from SVGs
        let (icon_atlas, icon_atlas_view) = create_icon_atlas(device, queue);

        // Create sampler
        let sampler = device.create_sampler(&iced::wgpu::SamplerDescriptor {
            label: Some("Tool Icon Sampler"),
            mag_filter: iced::wgpu::FilterMode::Linear,
            min_filter: iced::wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("Tool Panel Bind Group Layout"),
            entries: &[
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: iced::wgpu::ShaderStages::VERTEX | iced::wgpu::ShaderStages::FRAGMENT,
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

        // Create bind group
        let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
            label: Some("Tool Panel Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                iced::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                iced::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: iced::wgpu::BindingResource::TextureView(&icon_atlas_view),
                },
                iced::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: iced::wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&iced::wgpu::PipelineLayoutDescriptor {
            label: Some("Tool Panel Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some("Tool Panel Pipeline"),
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
            icon_atlas,
        }
    }
}

/// Tool icons in atlas order
const TOOL_ICON_ORDER: &[Tool] = &[
    Tool::Click,
    Tool::Select,
    Tool::Pencil,
    Tool::Line,
    Tool::Brush,
    Tool::Erase,
    Tool::RectangleOutline,
    Tool::RectangleFilled,
    Tool::EllipseOutline,
    Tool::EllipseFilled,
    Tool::Fill,
    Tool::Pipette,
    Tool::Shifter,
    Tool::Font,
    Tool::Tag,
];

/// Map tool to atlas index
fn tool_to_index(tool: Tool) -> usize {
    TOOL_ICON_ORDER.iter().position(|&t| t == tool).unwrap_or(0)
}

/// Icon atlas dimensions (from constants)
/// Atlas icons are rendered at 2x display size for HiDPI sharpness
const ATLAS_ICON_SIZE: u32 = (TOOL_ICON_SIZE * 2.0) as u32;
const ATLAS_COLS: u32 = TOOL_ATLAS_COLS;
const ATLAS_ROWS: u32 = TOOL_ATLAS_ROWS;

/// Create icon atlas texture from SVG files
fn create_icon_atlas(device: &iced::wgpu::Device, queue: &iced::wgpu::Queue) -> (iced::wgpu::Texture, iced::wgpu::TextureView) {
    let atlas_width = ATLAS_COLS * ATLAS_ICON_SIZE;
    let atlas_height = ATLAS_ROWS * ATLAS_ICON_SIZE;

    let mut atlas_data = vec![0u8; (atlas_width * atlas_height * 4) as usize];

    // SVG data for each tool
    let svg_data: &[(&[u8], Tool)] = &[
        (include_bytes!("../../../data/icons/cursor.svg"), Tool::Click),
        (include_bytes!("../../../data/icons/select.svg"), Tool::Select),
        (include_bytes!("../../../data/icons/pencil.svg"), Tool::Pencil),
        (include_bytes!("../../../data/icons/line.svg"), Tool::Line),
        (include_bytes!("../../../data/icons/paint_brush.svg"), Tool::Brush),
        (include_bytes!("../../../data/icons/eraser.svg"), Tool::Erase),
        (include_bytes!("../../../data/icons/rectangle_outline.svg"), Tool::RectangleOutline),
        (include_bytes!("../../../data/icons/rectangle_filled.svg"), Tool::RectangleFilled),
        (include_bytes!("../../../data/icons/ellipse_outline.svg"), Tool::EllipseOutline),
        (include_bytes!("../../../data/icons/ellipse_filled.svg"), Tool::EllipseFilled),
        (include_bytes!("../../../data/icons/fill.svg"), Tool::Fill),
        (include_bytes!("../../../data/icons/dropper.svg"), Tool::Pipette),
        (include_bytes!("../../../data/icons/move.svg"), Tool::Shifter),
        (include_bytes!("../../../data/icons/font.svg"), Tool::Font),
        (include_bytes!("../../../data/icons/tag.svg"), Tool::Tag),
    ];

    for (svg_bytes, tool) in svg_data {
        let idx = tool_to_index(*tool);
        let col = (idx as u32) % ATLAS_COLS;
        let row = (idx as u32) / ATLAS_COLS;

        if let Some(icon_rgba) = render_svg_to_rgba(svg_bytes, ATLAS_ICON_SIZE, ATLAS_ICON_SIZE) {
            // Copy icon into atlas at correct position
            let x_offset = col * ATLAS_ICON_SIZE;
            let y_offset = row * ATLAS_ICON_SIZE;

            for y in 0..ATLAS_ICON_SIZE {
                for x in 0..ATLAS_ICON_SIZE {
                    let src_idx = ((y * ATLAS_ICON_SIZE + x) * 4) as usize;
                    let dst_idx = (((y_offset + y) * atlas_width + (x_offset + x)) * 4) as usize;

                    if src_idx + 3 < icon_rgba.len() && dst_idx + 3 < atlas_data.len() {
                        atlas_data[dst_idx] = icon_rgba[src_idx];
                        atlas_data[dst_idx + 1] = icon_rgba[src_idx + 1];
                        atlas_data[dst_idx + 2] = icon_rgba[src_idx + 2];
                        atlas_data[dst_idx + 3] = icon_rgba[src_idx + 3];
                    }
                }
            }
        }
    }

    let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
        label: Some("Tool Icon Atlas"),
        size: iced::wgpu::Extent3d {
            width: atlas_width,
            height: atlas_height,
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
            texture: &texture,
            mip_level: 0,
            origin: iced::wgpu::Origin3d::ZERO,
            aspect: iced::wgpu::TextureAspect::All,
        },
        &atlas_data,
        iced::wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(atlas_width * 4),
            rows_per_image: Some(atlas_height),
        },
        iced::wgpu::Extent3d {
            width: atlas_width,
            height: atlas_height,
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&iced::wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Render SVG data to RGBA pixels
fn render_svg_to_rgba(svg_data: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    let opt = usvg::Options::default();

    if let Ok(tree) = usvg::Tree::from_data(svg_data, &opt) {
        let size = tiny_skia::IntSize::from_wh(width, height)?;
        let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())?;

        // Calculate scale to fit
        let tree_size = tree.size();
        let scale_x = width as f32 / tree_size.width();
        let scale_y = height as f32 / tree_size.height();
        let scale = scale_x.min(scale_y);

        // Center the icon
        let offset_x = (width as f32 - tree_size.width() * scale) / 2.0;
        let offset_y = (height as f32 - tree_size.height() * scale) / 2.0;

        let transform = tiny_skia::Transform::from_scale(scale, scale).pre_translate(offset_x / scale, offset_y / scale);

        resvg::render(&tree, transform, &mut pixmap.as_mut());

        return Some(pixmap.take());
    }

    None
}
