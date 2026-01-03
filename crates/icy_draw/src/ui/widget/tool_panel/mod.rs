//! Shared GPU-accelerated Tool Panel component
//!
//! A reusable toolbar component with:
//! - SVG icons rendered to textures with drop shadows
//! - Hover glow effects
//! - Selection highlighting
//! - Blend-over animation for tool state changes
//! - Dynamic layout with centering
//! - Scale factor aware

use icy_ui::{
    mouse,
    widget::shader::{self, Shader},
    Color, Element, Length, Rectangle,
};

use crate::ui::editor::ansi::constants::{TOOL_BLEND_ANIMATION_DURATION, TOOL_ICON_PADDING, TOOL_ICON_SIZE};

/// Size of each tool icon in logical pixels
const ICON_SIZE: f32 = TOOL_ICON_SIZE;
/// Padding between icons
const ICON_PADDING: f32 = TOOL_ICON_PADDING;
/// Animation duration for tool state transitions
const BLEND_ANIMATION_DURATION: f32 = TOOL_BLEND_ANIMATION_DURATION;
/// Maximum number of tool buttons supported by shader
const MAX_BUTTONS: usize = 16;

// ═══════════════════════════════════════════════════════════════════════════
// Public Types
// ═══════════════════════════════════════════════════════════════════════════

/// Messages from the tool panel
#[derive(Clone, Debug)]
pub enum ToolPanelMessage {
    /// Clicked on a tool slot
    ClickSlot(usize),
    /// Animation tick
    #[allow(dead_code)]
    Tick(f32),
}

// ═══════════════════════════════════════════════════════════════════════════
// Button Animation State
// ═══════════════════════════════════════════════════════════════════════════

/// Per-button animation state
#[derive(Clone, Debug)]
struct ButtonAnimationState {
    /// Previous tool index displayed (for blend-over)
    previous_tool: Option<usize>,
    /// Current tool index displayed
    current_tool: usize,
    /// Blend progress (0.0 = previous, 1.0 = current)
    blend_progress: f32,
    /// Whether blend animation is running
    animating: bool,
}

impl ButtonAnimationState {
    fn new(tool: usize) -> Self {
        Self {
            previous_tool: None,
            current_tool: tool,
            blend_progress: 1.0,
            animating: false,
        }
    }

    fn set_tool(&mut self, tool: usize) {
        if tool != self.current_tool {
            self.previous_tool = Some(self.current_tool);
            self.blend_progress = 0.0;
            self.animating = true;
        }
        self.current_tool = tool;
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

// ═══════════════════════════════════════════════════════════════════════════
// Generic Tool Panel
// ═══════════════════════════════════════════════════════════════════════════

/// Generic GPU-accelerated tool panel
pub struct GenericToolPanel {
    /// Number of tool slots
    num_slots: usize,
    /// Currently selected slot index
    selected_slot: usize,
    /// Per-button animation states
    button_states: Vec<ButtonAnimationState>,
    /// Hovered button index
    _hovered_button: Option<usize>,
    /// Time accumulator for effects
    time: f32,
}

impl GenericToolPanel {
    /// Create a new generic tool panel with a specified number of slots
    /// Uses the standard 4x4 atlas - caller should set slot displays to map to atlas indices
    pub fn new_with_slots(num_slots: usize) -> Self {
        // Initialize with 0 - will be overwritten by set_slot_display
        let button_states: Vec<_> = (0..num_slots).map(|_| ButtonAnimationState::new(0)).collect();

        Self {
            num_slots,
            selected_slot: 0,
            button_states,
            _hovered_button: None,
            time: 0.0,
        }
    }

    /// Get the currently selected slot index
    #[allow(dead_code)]
    pub fn selected_slot(&self) -> usize {
        self.selected_slot
    }

    /// Set the selected slot
    pub fn set_selected_slot(&mut self, slot: usize) {
        if slot < self.num_slots {
            self.selected_slot = slot;
        }
    }

    /// Update the display tool for a specific slot (for toggle tools)
    pub fn set_slot_display(&mut self, slot: usize, tool_index: usize) {
        if slot < self.button_states.len() {
            self.button_states[slot].set_tool(tool_index);
        }
    }

    /// Update animation state
    pub fn tick(&mut self, delta: f32) {
        self.time += delta;
        for state in &mut self.button_states {
            state.tick(delta);
        }
    }

    /// Handle a slot click - returns the clicked slot index
    #[allow(dead_code)]
    pub fn click_slot(&mut self, slot: usize) -> usize {
        if slot < self.num_slots {
            self.selected_slot = slot;
        }
        self.selected_slot
    }

    /// Render the tool panel
    pub fn view(&self, available_width: f32, bg_color: Color, icon_color: Color) -> Element<'_, ToolPanelMessage> {
        // Calculate columns based on available width
        let cols = ((available_width - ICON_PADDING) / (ICON_SIZE + ICON_PADDING)).floor() as usize;
        let cols = cols.max(1).min(self.num_slots);
        let rows = (self.num_slots + cols - 1) / cols;

        let total_width = available_width;
        let total_height = rows as f32 * (ICON_SIZE + ICON_PADDING) + ICON_PADDING;

        let bg_color_arr = [bg_color.r, bg_color.g, bg_color.b];
        let icon_color_arr = [icon_color.r, icon_color.g, icon_color.b];

        // Build button data for the shader
        let buttons: Vec<ButtonData> = self
            .button_states
            .iter()
            .enumerate()
            .map(|(slot, state)| {
                let is_selected = self.selected_slot == slot;

                ButtonData {
                    current_tool: state.current_tool,
                    blend_progress: state.blend_progress,
                    is_selected,
                }
            })
            .collect();

        Shader::new(ToolPanelProgram {
            buttons,
            time: self.time,
            cols,
            rows,
            num_buttons: self.num_slots,
            bg_color: bg_color_arr,
            icon_color: icon_color_arr,
        })
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .into()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Shader Types
// ═══════════════════════════════════════════════════════════════════════════

/// Data for a single button
#[derive(Debug, Clone)]
struct ButtonData {
    current_tool: usize,
    blend_progress: f32,
    is_selected: bool,
}

/// Shader program for rendering the tool panel
#[derive(Debug, Clone)]
struct ToolPanelProgram {
    buttons: Vec<ButtonData>,
    time: f32,
    cols: usize,
    rows: usize,
    num_buttons: usize,
    bg_color: [f32; 3],
    icon_color: [f32; 3],
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
            icon_color: self.icon_color,
        }
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: &icy_ui::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<icy_ui::widget::Action<ToolPanelMessage>> {
        let cols = self.cols;
        let rows = self.rows;
        let num_buttons = self.buttons.len();

        // Helper to get button slot from position
        let get_slot = |pos: icy_ui::Point, bounds: Rectangle| -> Option<usize> {
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
            if slot < num_buttons {
                Some(slot)
            } else {
                None
            }
        };

        match event {
            icy_ui::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let new_hover = cursor.position_in(bounds).and_then(|p| get_slot(p, bounds));

                if *state != new_hover {
                    *state = new_hover;
                    return Some(icy_ui::widget::Action::request_redraw());
                }
                None
            }
            icy_ui::Event::Mouse(mouse::Event::ButtonPressed {
                button: mouse::Button::Left, ..
            }) => {
                if let Some(slot) = cursor.position_in(bounds).and_then(|p| get_slot(p, bounds)) {
                    return Some(icy_ui::widget::Action::publish(ToolPanelMessage::ClickSlot(slot)));
                }
                None
            }
            _ => None,
        }
    }
}

/// Uniforms for the shader
/// WGSL layout (std140):
/// - vec3<f32> takes 16-byte slot (aligned to 16, padded to 16)
/// - array<vec4<f32>, N> aligned to 16
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ToolPanelUniforms {
    widget_size: [f32; 2], // offset 0, 8 bytes
    icon_size: f32,        // offset 8, 4 bytes
    icon_padding: f32,     // offset 12, 4 bytes
    time: f32,             // offset 16, 4 bytes
    cols: u32,             // offset 20, 4 bytes
    rows: u32,             // offset 24, 4 bytes
    num_buttons: u32,      // offset 28, 4 bytes
    bg_color: [f32; 4],    // offset 32, 16 bytes
    icon_color: [f32; 4],  // offset 48, 16 bytes (pad to 64 for array alignment)
    // Per-button data (packed) - offset 64
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
    icon_color: [f32; 3],
}

impl shader::Primitive for ToolPanelPrimitive {
    type Pipeline = ToolPanelRenderer;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        _device: &icy_ui::wgpu::Device,
        queue: &icy_ui::wgpu::Queue,
        bounds: &Rectangle,
        viewport: &icy_ui::advanced::graphics::Viewport,
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
                btn.current_tool as f32,
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
            bg_color: [self.bg_color[0], self.bg_color[1], self.bg_color[2], 1.0],
            icon_color: [self.icon_color[0], self.icon_color[1], self.icon_color[2], 1.0],
            button_data,
        };

        queue.write_buffer(&pipeline.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut icy_ui::wgpu::CommandEncoder, target: &icy_ui::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        let mut render_pass = encoder.begin_render_pass(&icy_ui::wgpu::RenderPassDescriptor {
            label: Some("Tool Panel Render Pass"),
            color_attachments: &[Some(icy_ui::wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: icy_ui::wgpu::Operations {
                    load: icy_ui::wgpu::LoadOp::Load,
                    store: icy_ui::wgpu::StoreOp::Store,
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

// ═══════════════════════════════════════════════════════════════════════════
// GPU Renderer
// ═══════════════════════════════════════════════════════════════════════════

/// GPU renderer/pipeline for the tool panel
pub struct ToolPanelRenderer {
    pipeline: icy_ui::wgpu::RenderPipeline,
    bind_group: icy_ui::wgpu::BindGroup,
    uniform_buffer: icy_ui::wgpu::Buffer,
    #[allow(dead_code)]
    icon_atlas: icy_ui::wgpu::Texture,
}

/// Standard icon atlas dimensions (4x4 grid for up to 16 icons)
const ATLAS_COLS: u32 = 4;
const ATLAS_ROWS: u32 = 4;
/// Icon size in atlas (2x display size for HiDPI)
const ATLAS_ICON_SIZE: u32 = (ICON_SIZE * 2.0) as u32;

impl shader::Pipeline for ToolPanelRenderer {
    fn new(device: &icy_ui::wgpu::Device, queue: &icy_ui::wgpu::Queue, format: icy_ui::wgpu::TextureFormat) -> Self {
        // Load and compile shader
        let shader = device.create_shader_module(icy_ui::wgpu::ShaderModuleDescriptor {
            label: Some("Tool Panel Shader"),
            source: icy_ui::wgpu::ShaderSource::Wgsl(include_str!("tool_panel_shader.wgsl").into()),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&icy_ui::wgpu::BufferDescriptor {
            label: Some("Tool Panel Uniform Buffer"),
            size: std::mem::size_of::<ToolPanelUniforms>() as u64,
            usage: icy_ui::wgpu::BufferUsages::UNIFORM | icy_ui::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create icon atlas with all standard tool icons
        let (icon_atlas, icon_atlas_view) = create_standard_icon_atlas(device, queue);

        // Create sampler
        let sampler = device.create_sampler(&icy_ui::wgpu::SamplerDescriptor {
            label: Some("Tool Icon Sampler"),
            mag_filter: icy_ui::wgpu::FilterMode::Linear,
            min_filter: icy_ui::wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&icy_ui::wgpu::BindGroupLayoutDescriptor {
            label: Some("Tool Panel Bind Group Layout"),
            entries: &[
                icy_ui::wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: icy_ui::wgpu::ShaderStages::VERTEX | icy_ui::wgpu::ShaderStages::FRAGMENT,
                    ty: icy_ui::wgpu::BindingType::Buffer {
                        ty: icy_ui::wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                icy_ui::wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: icy_ui::wgpu::ShaderStages::FRAGMENT,
                    ty: icy_ui::wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: icy_ui::wgpu::TextureViewDimension::D2,
                        sample_type: icy_ui::wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                icy_ui::wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: icy_ui::wgpu::ShaderStages::FRAGMENT,
                    ty: icy_ui::wgpu::BindingType::Sampler(icy_ui::wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create bind group
        let bind_group = device.create_bind_group(&icy_ui::wgpu::BindGroupDescriptor {
            label: Some("Tool Panel Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                icy_ui::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                icy_ui::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: icy_ui::wgpu::BindingResource::TextureView(&icon_atlas_view),
                },
                icy_ui::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: icy_ui::wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&icy_ui::wgpu::PipelineLayoutDescriptor {
            label: Some("Tool Panel Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&icy_ui::wgpu::RenderPipelineDescriptor {
            label: Some("Tool Panel Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: icy_ui::wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(icy_ui::wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(icy_ui::wgpu::ColorTargetState {
                    format,
                    blend: Some(icy_ui::wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: icy_ui::wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: icy_ui::wgpu::PrimitiveState {
                topology: icy_ui::wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: icy_ui::wgpu::MultisampleState::default(),
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

/// Standard tool icons in atlas order (matching ANSI editor)
/// These are all the icons that might be used in tool panels.
/// Index 0-15 in a 4x4 atlas.
const STANDARD_ICONS: &[&[u8]] = &[
    include_bytes!("../../../../data/icons/cursor.svg"),            // 0: Click
    include_bytes!("../../../../data/icons/select.svg"),            // 1: Select
    include_bytes!("../../../../data/icons/pencil.svg"),            // 2: Pencil
    include_bytes!("../../../../data/icons/line.svg"),              // 3: Line
    include_bytes!("../../../../data/icons/rectangle_outline.svg"), // 4: RectangleOutline
    include_bytes!("../../../../data/icons/rectangle_filled.svg"),  // 5: RectangleFilled
    include_bytes!("../../../../data/icons/ellipse_outline.svg"),   // 6: EllipseOutline
    include_bytes!("../../../../data/icons/ellipse_filled.svg"),    // 7: EllipseFilled
    include_bytes!("../../../../data/icons/fill.svg"),              // 8: Fill
    include_bytes!("../../../../data/icons/dropper.svg"),           // 9: Pipette
    include_bytes!("../../../../data/icons/font.svg"),              // 10: Font
    include_bytes!("../../../../data/icons/tag.svg"),               // 11: Tag
    include_bytes!("../../../../data/icons/cursor.svg"),            // 12: Placeholder (duplicate)
    include_bytes!("../../../../data/icons/cursor.svg"),            // 13: Placeholder (duplicate)
    include_bytes!("../../../../data/icons/cursor.svg"),            // 14: Placeholder (duplicate)
    include_bytes!("../../../../data/icons/cursor.svg"),            // 15: Placeholder (duplicate)
];

/// Create the standard icon atlas with all tool icons
fn create_standard_icon_atlas(device: &icy_ui::wgpu::Device, queue: &icy_ui::wgpu::Queue) -> (icy_ui::wgpu::Texture, icy_ui::wgpu::TextureView) {
    create_icon_atlas(device, queue, STANDARD_ICONS, ATLAS_COLS, ATLAS_ROWS, ATLAS_ICON_SIZE)
}

// ═══════════════════════════════════════════════════════════════════════════
// SVG Rendering utilities
// ═══════════════════════════════════════════════════════════════════════════

/// Render SVG data to RGBA pixels
pub fn render_svg_to_rgba(svg_data: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
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

/// Create an icon atlas texture from SVG data
pub fn create_icon_atlas(
    device: &icy_ui::wgpu::Device,
    queue: &icy_ui::wgpu::Queue,
    icon_data: &[&[u8]],
    atlas_cols: u32,
    atlas_rows: u32,
    icon_size: u32,
) -> (icy_ui::wgpu::Texture, icy_ui::wgpu::TextureView) {
    let atlas_width = atlas_cols * icon_size;
    let atlas_height = atlas_rows * icon_size;

    let mut atlas_data = vec![0u8; (atlas_width * atlas_height * 4) as usize];

    for (idx, svg_bytes) in icon_data.iter().enumerate() {
        let col = (idx as u32) % atlas_cols;
        let row = (idx as u32) / atlas_cols;

        if let Some(icon_rgba) = render_svg_to_rgba(svg_bytes, icon_size, icon_size) {
            // Copy icon into atlas at correct position
            let x_offset = col * icon_size;
            let y_offset = row * icon_size;

            for y in 0..icon_size {
                for x in 0..icon_size {
                    let src_idx = ((y * icon_size + x) * 4) as usize;
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

    let texture = device.create_texture(&icy_ui::wgpu::TextureDescriptor {
        label: Some("Tool Icon Atlas"),
        size: icy_ui::wgpu::Extent3d {
            width: atlas_width,
            height: atlas_height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: icy_ui::wgpu::TextureDimension::D2,
        format: icy_ui::wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: icy_ui::wgpu::TextureUsages::TEXTURE_BINDING | icy_ui::wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        icy_ui::wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: icy_ui::wgpu::Origin3d::ZERO,
            aspect: icy_ui::wgpu::TextureAspect::All,
        },
        &atlas_data,
        icy_ui::wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(atlas_width * 4),
            rows_per_image: Some(atlas_height),
        },
        icy_ui::wgpu::Extent3d {
            width: atlas_width,
            height: atlas_height,
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&icy_ui::wgpu::TextureViewDescriptor::default());
    (texture, view)
}
