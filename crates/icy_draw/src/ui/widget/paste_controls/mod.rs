//! Paste Controls Widget
//!
//! A GPU-accelerated widget for paste mode controls with:
//! - Anchor (success/green) and Cancel (danger/red) buttons
//! - SVG icons with drop shadows
//! - Hover glow effects
//! - Consistent visual style with tool panel

use iced::{
    mouse,
    widget::shader::{self, Shader},
    Color, Element, Length, Rectangle,
};

use crate::ui::editor::ansi::constants::{TOOL_ICON_PADDING, TOOL_ICON_SIZE};

/// Size of each button in logical pixels
const ICON_SIZE: f32 = TOOL_ICON_SIZE;
/// Padding between buttons
const ICON_PADDING: f32 = TOOL_ICON_PADDING;
/// Number of buttons (Anchor + Cancel)
const NUM_BUTTONS: usize = 2;

// SVG icons embedded as bytes
const ANCHOR_SVG: &[u8] = include_bytes!("icons/anchor.svg");
const CANCEL_SVG: &[u8] = include_bytes!("icons/cancel.svg");

// ═══════════════════════════════════════════════════════════════════════════
// Public Types
// ═══════════════════════════════════════════════════════════════════════════

/// Messages from the paste controls
#[derive(Clone, Debug)]
pub enum PasteControlsMessage {
    /// Clicked Anchor button
    Anchor,
    /// Clicked Cancel button
    Cancel,
}

// ═══════════════════════════════════════════════════════════════════════════
// Paste Controls Widget
// ═══════════════════════════════════════════════════════════════════════════

/// GPU-accelerated paste controls panel
pub struct PasteControls {
    /// Time accumulator for effects
    time: f32,
}

impl Default for PasteControls {
    fn default() -> Self {
        Self::new()
    }
}

impl PasteControls {
    /// Create new paste controls
    pub fn new() -> Self {
        Self { time: 0.0 }
    }

    /// Render the paste controls
    pub fn view(&self, available_width: f32, bg_color: Color) -> Element<'_, PasteControlsMessage> {
        // Calculate layout - 2 buttons side by side or stacked
        let cols = ((available_width - ICON_PADDING) / (ICON_SIZE + ICON_PADDING)).floor() as usize;
        let cols = cols.max(1).min(NUM_BUTTONS);
        let rows = (NUM_BUTTONS + cols - 1) / cols;

        let total_width = available_width;
        let total_height = rows as f32 * (ICON_SIZE + ICON_PADDING) + ICON_PADDING;

        let bg_color_arr = [bg_color.r, bg_color.g, bg_color.b];

        Shader::new(PasteControlsProgram {
            time: self.time,
            cols,
            rows,
            bg_color: bg_color_arr,
        })
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .into()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Shader Types
// ═══════════════════════════════════════════════════════════════════════════

/// Shader program for rendering the paste controls
#[derive(Debug, Clone)]
struct PasteControlsProgram {
    time: f32,
    cols: usize,
    rows: usize,
    bg_color: [f32; 3],
}

impl shader::Program<PasteControlsMessage> for PasteControlsProgram {
    type State = Option<usize>; // Hovered button index (0 = Anchor, 1 = Cancel)
    type Primitive = PasteControlsPrimitive;

    fn draw(&self, state: &Self::State, _cursor: mouse::Cursor, _bounds: Rectangle) -> Self::Primitive {
        PasteControlsPrimitive {
            time: self.time,
            hovered_button: *state,
            cols: self.cols,
            rows: self.rows,
            bg_color: self.bg_color,
        }
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<iced::widget::Action<PasteControlsMessage>> {
        let cols = self.cols;
        let rows = self.rows;

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
            if slot < NUM_BUTTONS {
                Some(slot)
            } else {
                None
            }
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
                    let msg = match slot {
                        0 => PasteControlsMessage::Anchor,
                        1 => PasteControlsMessage::Cancel,
                        _ => return None,
                    };
                    return Some(iced::widget::Action::publish(msg));
                }
                None
            }
            _ => None,
        }
    }
}

/// Uniforms for the shader
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PasteControlsUniforms {
    widget_size: [f32; 2], // offset 0, 8 bytes
    icon_size: f32,        // offset 8, 4 bytes
    icon_padding: f32,     // offset 12, 4 bytes
    time: f32,             // offset 16, 4 bytes
    cols: u32,             // offset 20, 4 bytes
    rows: u32,             // offset 24, 4 bytes
    hovered_button: i32,   // offset 28, 4 bytes (-1 = none)
    bg_color: [f32; 4],    // offset 32, 16 bytes
}

/// Shader primitive for GPU rendering
#[derive(Debug, Clone)]
struct PasteControlsPrimitive {
    time: f32,
    hovered_button: Option<usize>,
    cols: usize,
    rows: usize,
    bg_color: [f32; 3],
}

impl shader::Primitive for PasteControlsPrimitive {
    type Pipeline = PasteControlsRenderer;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        _device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        bounds: &Rectangle,
        viewport: &iced::advanced::graphics::Viewport,
    ) {
        let scale = viewport.scale_factor();

        let uniforms = PasteControlsUniforms {
            widget_size: [bounds.width * scale, bounds.height * scale],
            icon_size: ICON_SIZE * scale,
            icon_padding: ICON_PADDING * scale,
            time: self.time,
            cols: self.cols as u32,
            rows: self.rows as u32,
            hovered_button: self.hovered_button.map(|i| i as i32).unwrap_or(-1),
            bg_color: [self.bg_color[0], self.bg_color[1], self.bg_color[2], 1.0],
        };

        queue.write_buffer(&pipeline.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        let mut render_pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
            label: Some("Paste Controls Render Pass"),
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
            render_pass.set_pipeline(&pipeline.render_pipeline);
            render_pass.set_bind_group(0, &pipeline.bind_group, &[]);
            render_pass.draw(0..6, 0..1);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GPU Pipeline
// ═══════════════════════════════════════════════════════════════════════════

/// Renderer for the paste controls shader
#[derive(Debug)]
pub struct PasteControlsRenderer {
    render_pipeline: iced::wgpu::RenderPipeline,
    uniform_buffer: iced::wgpu::Buffer,
    bind_group: iced::wgpu::BindGroup,
    _icon_texture: iced::wgpu::Texture,
}

impl shader::Pipeline for PasteControlsRenderer {
    fn new(device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        use iced::wgpu;

        // Create icon atlas texture (2x1 grid: Anchor, Cancel)
        let atlas_size = (ICON_SIZE as u32) * 2;
        let (icon_texture, icon_view) = create_icon_atlas(device, queue, atlas_size);

        // Create sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Paste Controls Icon Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Paste Controls Uniform Buffer"),
            size: std::mem::size_of::<PasteControlsUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Paste Controls Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Paste Controls Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&icon_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Paste Controls Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Paste Controls Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("paste_controls_shader.wgsl").into()),
        });

        // Create render pipeline
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Paste Controls Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            render_pipeline,
            uniform_buffer,
            bind_group,
            _icon_texture: icon_texture,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Icon Atlas Creation
// ═══════════════════════════════════════════════════════════════════════════

/// Create icon atlas texture (2x1 grid: Anchor, Cancel)
fn create_icon_atlas(device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, icon_size: u32) -> (iced::wgpu::Texture, iced::wgpu::TextureView) {
    use iced::wgpu;

    let atlas_width = icon_size * 2;
    let atlas_height = icon_size;

    // Create texture
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Paste Controls Icon Atlas"),
        size: wgpu::Extent3d {
            width: atlas_width,
            height: atlas_height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // Render icons to atlas
    let mut atlas_data = vec![0u8; (atlas_width * atlas_height * 4) as usize];

    // Render Anchor icon (slot 0) - green/success color
    if let Some(rgba) = render_svg_to_rgba_colored(ANCHOR_SVG, icon_size, icon_size, [0.4, 0.8, 0.4, 1.0]) {
        copy_icon_to_atlas(&mut atlas_data, &rgba, 0, icon_size, atlas_width);
    }

    // Render Cancel icon (slot 1) - red/danger color
    if let Some(rgba) = render_svg_to_rgba_colored(CANCEL_SVG, icon_size, icon_size, [0.9, 0.4, 0.4, 1.0]) {
        copy_icon_to_atlas(&mut atlas_data, &rgba, 1, icon_size, atlas_width);
    }

    // Upload to GPU
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &atlas_data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(atlas_width * 4),
            rows_per_image: Some(atlas_height),
        },
        wgpu::Extent3d {
            width: atlas_width,
            height: atlas_height,
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Copy icon data to atlas at specified slot
fn copy_icon_to_atlas(atlas: &mut [u8], icon: &[u8], slot: u32, icon_size: u32, atlas_width: u32) {
    let x_offset = slot * icon_size;
    for y in 0..icon_size {
        for x in 0..icon_size {
            let src_idx = ((y * icon_size + x) * 4) as usize;
            let dst_idx = ((y * atlas_width + x_offset + x) * 4) as usize;
            if src_idx + 4 <= icon.len() && dst_idx + 4 <= atlas.len() {
                atlas[dst_idx..dst_idx + 4].copy_from_slice(&icon[src_idx..src_idx + 4]);
            }
        }
    }
}

/// Render SVG to RGBA with a specific tint color
fn render_svg_to_rgba_colored(svg_data: &[u8], width: u32, height: u32, color: [f32; 4]) -> Option<Vec<u8>> {
    use resvg::tiny_skia::{Pixmap, Transform};
    use resvg::usvg::{Options, Tree};

    let opt = Options::default();
    let tree = Tree::from_data(svg_data, &opt).ok()?;

    let svg_size = tree.size();
    let scale_x = width as f32 / svg_size.width();
    let scale_y = height as f32 / svg_size.height();
    let scale = scale_x.min(scale_y);

    let offset_x = (width as f32 - svg_size.width() * scale) / 2.0;
    let offset_y = (height as f32 - svg_size.height() * scale) / 2.0;

    let transform = Transform::from_scale(scale, scale).post_translate(offset_x, offset_y);

    let mut pixmap = Pixmap::new(width, height)?;
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Replace color but keep alpha from original (use alpha as mask)
    let mut data = pixmap.take();
    for chunk in data.chunks_exact_mut(4) {
        let a = chunk[3] as f32 / 255.0;
        // Use the tint color, but use original alpha as mask
        chunk[0] = (color[0] * 255.0) as u8;
        chunk[1] = (color[1] * 255.0) as u8;
        chunk[2] = (color[2] * 255.0) as u8;
        chunk[3] = (a * color[3] * 255.0) as u8;
    }

    Some(data)
}
