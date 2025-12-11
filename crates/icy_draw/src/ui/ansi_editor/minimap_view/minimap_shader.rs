//! Minimap shader implementation
//!
//! A simplified GPU shader for rendering the minimap.
//! Unlike the full terminal shader, this doesn't need CRT effects,
//! zooming, selection, or keyboard handling.

use std::collections::HashMap;
use std::sync::Arc;

use iced::Rectangle;
use iced::mouse;
use iced::widget::shader;

use super::MinimapMessage;

/// Maximum texture dimension supported by most GPUs
const MAX_TEXTURE_DIMENSION: u32 = 8192;

/// Uniform data for the minimap shader
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct MinimapUniforms {
    /// Viewport rectangle (normalized 0-1 in texture space): x, y, width, height
    viewport_rect: [f32; 4],
    /// Viewport border color RGBA
    viewport_color: [f32; 4],
    /// Visible UV range (what part of texture is currently shown): min_y, max_y, unused, unused
    visible_uv_range: [f32; 4],
    /// Viewport border thickness in pixels
    border_thickness: f32,
    /// Whether to show viewport overlay
    show_viewport: f32,
    /// Padding for alignment
    _padding: [f32; 2],
}

/// Viewport information for the minimap overlay
#[derive(Clone, Debug, Default)]
pub struct ViewportInfo {
    /// Normalized X position of viewport (0.0-1.0)
    pub x: f32,
    /// Normalized Y position of viewport (0.0-1.0)
    pub y: f32,
    /// Normalized width of viewport (0.0-1.0)
    pub width: f32,
    /// Normalized height of viewport (0.0-1.0)
    pub height: f32,
}

/// The minimap shader program (high-level interface)
/// This implements shader::Program and creates MinimapPrimitive for rendering
#[derive(Debug, Clone)]
pub struct MinimapProgram {
    /// RGBA pixel data of the buffer (shared to avoid cloning)
    pub rgba_data: Arc<Vec<u8>>,
    /// Size of the texture (width, height)
    pub texture_size: (u32, u32),
    /// Unique instance ID
    pub instance_id: usize,
    /// Viewport overlay information
    pub viewport_info: ViewportInfo,
    /// Scroll offset (0.0 = top, 1.0 = bottom)
    pub scroll_offset: f32,
    /// Available height for rendering (to compute visible UV range)
    pub available_height: f32,
}

/// State for tracking mouse dragging in the minimap
#[derive(Debug, Clone, Default)]
pub struct MinimapState {
    /// Whether the left mouse button is currently pressed
    is_dragging: bool,
}

impl MinimapProgram {
    /// Helper to calculate normalized position from cursor position
    /// Takes absolute position and bounds, calculates relative position internally
    fn calculate_normalized_position(&self, absolute_pos: iced::Point, bounds: Rectangle) -> Option<(f32, f32)> {
        let (tex_w, tex_h) = self.texture_size;
        if tex_w == 0 || tex_h == 0 {
            return None;
        }

        // Calculate position relative to bounds (for mouse capture outside bounds)
        let relative_x = absolute_pos.x - bounds.x;
        let relative_y = absolute_pos.y - bounds.y;

        let scale = bounds.width / tex_w as f32;
        let scaled_h = tex_h as f32 * scale;

        // Calculate visible UV range (same logic as in prepare())
        let visible_uv_height = (bounds.height / scaled_h).min(1.0);
        let max_scroll_uv = (1.0 - visible_uv_height).max(0.0);
        let scroll_uv_y = self.scroll_offset * max_scroll_uv;

        // Position in screen space (0-1 of visible area) - can be outside 0-1 range
        let screen_y = relative_y / bounds.height;

        // Convert to texture UV space by mapping through visible range
        // Clamp to valid UV range (0-1)
        let norm_y = (scroll_uv_y + screen_y * visible_uv_height).clamp(0.0, 1.0);
        let norm_x = (relative_x / bounds.width).clamp(0.0, 1.0);

        Some((norm_x, norm_y))
    }
}

impl shader::Program<MinimapMessage> for MinimapProgram {
    type State = MinimapState;
    type Primitive = MinimapPrimitive;

    fn draw(&self, _state: &Self::State, _cursor: mouse::Cursor, bounds: Rectangle) -> Self::Primitive {
        MinimapPrimitive {
            rgba_data: Arc::clone(&self.rgba_data),
            texture_size: self.texture_size,
            instance_id: self.instance_id,
            viewport_info: self.viewport_info.clone(),
            scroll_offset: self.scroll_offset,
            available_height: bounds.height,
        }
    }

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<iced::widget::Action<MinimapMessage>> {
        match event {
            // Handle mouse button press - start dragging
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                // Use position_in for initial click - must be inside bounds
                if let Some(pos) = cursor.position_in(bounds) {
                    state.is_dragging = true;
                    // Convert to absolute position for the helper
                    let absolute_pos = iced::Point::new(pos.x + bounds.x, pos.y + bounds.y);
                    if let Some((norm_x, norm_y)) = self.calculate_normalized_position(absolute_pos, bounds) {
                        return Some(iced::widget::Action::publish(MinimapMessage::Click(norm_x, norm_y)));
                    }
                }
            }

            // Handle mouse button release - stop dragging
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.is_dragging = false;
            }

            // Handle cursor movement while dragging
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.is_dragging {
                    // Use cursor.position() for mouse capture effect - works even outside bounds
                    if let Some(pos) = cursor.position() {
                        if let Some((norm_x, norm_y)) = self.calculate_normalized_position(pos, bounds) {
                            return Some(iced::widget::Action::publish(MinimapMessage::Drag(norm_x, norm_y)));
                        }
                    }
                }
            }

            _ => {}
        }

        None
    }
}

/// The minimap shader primitive (low-level GPU rendering)
#[derive(Debug, Clone)]
pub struct MinimapPrimitive {
    /// RGBA pixel data of the buffer (shared to avoid cloning)
    pub rgba_data: Arc<Vec<u8>>,
    /// Size of the texture (width, height)
    pub texture_size: (u32, u32),
    /// Unique instance ID
    pub instance_id: usize,
    /// Viewport overlay information
    pub viewport_info: ViewportInfo,
    /// Scroll offset (0.0 = top, 1.0 = bottom)
    pub scroll_offset: f32,
    /// Available height for rendering
    pub available_height: f32,
}

/// Per-instance GPU resources
struct InstanceResources {
    texture: iced::wgpu::Texture,
    texture_view: iced::wgpu::TextureView,
    bind_group: iced::wgpu::BindGroup,
    uniform_buffer: iced::wgpu::Buffer,
    texture_size: (u32, u32),
}

/// The minimap shader renderer (GPU pipeline)
pub struct MinimapShaderRenderer {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group_layout: iced::wgpu::BindGroupLayout,
    sampler: iced::wgpu::Sampler,
    instances: HashMap<usize, InstanceResources>,
}

impl shader::Pipeline for MinimapShaderRenderer {
    fn new(device: &iced::wgpu::Device, _queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("Minimap Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("minimap.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("Minimap Bind Group Layout"),
            entries: &[
                // Texture
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: iced::wgpu::ShaderStages::FRAGMENT,
                    ty: iced::wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: iced::wgpu::TextureViewDimension::D2,
                        sample_type: iced::wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Sampler
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: iced::wgpu::ShaderStages::FRAGMENT,
                    ty: iced::wgpu::BindingType::Sampler(iced::wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Uniforms
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: iced::wgpu::ShaderStages::FRAGMENT,
                    ty: iced::wgpu::BindingType::Buffer {
                        ty: iced::wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&iced::wgpu::PipelineLayoutDescriptor {
            label: Some("Minimap Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some("Minimap Pipeline"),
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
                strip_index_format: None,
                front_face: iced::wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: iced::wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: iced::wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Use linear filtering for smooth minimap scaling
        let sampler = device.create_sampler(&iced::wgpu::SamplerDescriptor {
            label: Some("Minimap Sampler"),
            address_mode_u: iced::wgpu::AddressMode::ClampToEdge,
            address_mode_v: iced::wgpu::AddressMode::ClampToEdge,
            address_mode_w: iced::wgpu::AddressMode::ClampToEdge,
            mag_filter: iced::wgpu::FilterMode::Linear,
            min_filter: iced::wgpu::FilterMode::Linear,
            mipmap_filter: iced::wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        MinimapShaderRenderer {
            pipeline,
            bind_group_layout,
            sampler,
            instances: HashMap::new(),
        }
    }
}

impl shader::Primitive for MinimapPrimitive {
    type Pipeline = MinimapShaderRenderer;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        bounds: &iced::Rectangle,
        _viewport: &iced::advanced::graphics::Viewport,
    ) {
        let id = self.instance_id;
        let (w, h) = self.texture_size;
        let w = w.min(MAX_TEXTURE_DIMENSION).max(1);
        let h = h.min(MAX_TEXTURE_DIMENSION).max(1);

        // Get or create per-instance resources
        let resources = pipeline.instances.entry(id).or_insert_with(|| {
            let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
                label: Some(&format!("Minimap Uniforms {}", id)),
                size: std::mem::size_of::<MinimapUniforms>() as u64,
                usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                label: Some(&format!("Minimap Texture {}", id)),
                size: iced::wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: iced::wgpu::TextureDimension::D2,
                format: iced::wgpu::TextureFormat::Rgba8Unorm,
                usage: iced::wgpu::TextureUsages::TEXTURE_BINDING | iced::wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            let texture_view = texture.create_view(&iced::wgpu::TextureViewDescriptor::default());

            let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                label: Some(&format!("Minimap BindGroup {}", id)),
                layout: &pipeline.bind_group_layout,
                entries: &[
                    iced::wgpu::BindGroupEntry {
                        binding: 0,
                        resource: iced::wgpu::BindingResource::TextureView(&texture_view),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 1,
                        resource: iced::wgpu::BindingResource::Sampler(&pipeline.sampler),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                ],
            });

            InstanceResources {
                texture,
                texture_view,
                bind_group,
                uniform_buffer,
                texture_size: (w, h),
            }
        });

        // Recreate texture if size changed
        if resources.texture_size != (w, h) {
            let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                label: Some(&format!("Minimap Texture {}", id)),
                size: iced::wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: iced::wgpu::TextureDimension::D2,
                format: iced::wgpu::TextureFormat::Rgba8Unorm,
                usage: iced::wgpu::TextureUsages::TEXTURE_BINDING | iced::wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            let texture_view = texture.create_view(&iced::wgpu::TextureViewDescriptor::default());

            let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                label: Some(&format!("Minimap BindGroup {}", id)),
                layout: &pipeline.bind_group_layout,
                entries: &[
                    iced::wgpu::BindGroupEntry {
                        binding: 0,
                        resource: iced::wgpu::BindingResource::TextureView(&texture_view),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 1,
                        resource: iced::wgpu::BindingResource::Sampler(&pipeline.sampler),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 2,
                        resource: resources.uniform_buffer.as_entire_binding(),
                    },
                ],
            });

            resources.texture = texture;
            resources.texture_view = texture_view;
            resources.bind_group = bind_group;
            resources.texture_size = (w, h);
        }

        // Upload texture data
        if !self.rgba_data.is_empty() {
            let (orig_w, orig_h) = self.texture_size;
            let clamped_h = orig_h.min(MAX_TEXTURE_DIMENSION);
            let bytes_per_row = 4 * orig_w.min(MAX_TEXTURE_DIMENSION);
            let max_bytes = (bytes_per_row * clamped_h) as usize;
            let data = &self.rgba_data[..max_bytes.min(self.rgba_data.len())];

            queue.write_texture(
                iced::wgpu::TexelCopyTextureInfo {
                    texture: &resources.texture,
                    mip_level: 0,
                    origin: iced::wgpu::Origin3d::ZERO,
                    aspect: iced::wgpu::TextureAspect::All,
                },
                data,
                iced::wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(h),
                },
                iced::wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }

        // Update uniforms
        let show_viewport = if self.viewport_info.width > 0.0 && self.viewport_info.height > 0.0 {
            1.0
        } else {
            0.0
        };

        // Calculate visible UV range based on texture size, available height, and scroll
        let (tex_w, tex_h) = (w as f32, h as f32);
        let avail_h = self.available_height.max(1.0);
        let scale = bounds.width / tex_w;
        let scaled_h = tex_h * scale;

        // How much of the texture is visible (in normalized UV coordinates)
        let visible_uv_height = (avail_h / scaled_h).min(1.0);
        let max_scroll_uv = (1.0 - visible_uv_height).max(0.0);
        let scroll_uv_y = self.scroll_offset * max_scroll_uv;

        let visible_uv_min_y = scroll_uv_y;
        let visible_uv_max_y = scroll_uv_y + visible_uv_height;

        let uniforms = MinimapUniforms {
            viewport_rect: [self.viewport_info.x, self.viewport_info.y, self.viewport_info.width, self.viewport_info.height],
            // Modern cyan accent color - vibrant but not overwhelming
            viewport_color: [0.2, 0.8, 0.9, 0.9],
            visible_uv_range: [visible_uv_min_y, visible_uv_max_y, 0.0, 0.0],
            border_thickness: 2.5,
            show_viewport,
            _padding: [0.0; 2],
        };

        let uniform_bytes = unsafe { std::slice::from_raw_parts(&uniforms as *const MinimapUniforms as *const u8, std::mem::size_of::<MinimapUniforms>()) };
        queue.write_buffer(&resources.uniform_buffer, 0, uniform_bytes);
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        let Some(resources) = pipeline.instances.get(&self.instance_id) else {
            return;
        };

        // Calculate scaling to fill width (minimap fills available width)
        let (tex_w, tex_h) = resources.texture_size;
        let tex_w = tex_w.max(1) as f32;
        let tex_h = tex_h.max(1) as f32;

        let avail_w = clip_bounds.width.max(1) as f32;
        let avail_h = clip_bounds.height.max(1) as f32;

        // Scale to fill width - content may be taller than available space
        let scale = avail_w / tex_w;
        let scaled_w = avail_w;
        let scaled_h = tex_h * scale;

        // Calculate scroll offset if content is taller than available space
        let max_scroll = (scaled_h - avail_h).max(0.0);
        let scroll_y = self.scroll_offset * max_scroll;

        // Center horizontally (always), position vertically based on scroll
        let offset_x = 0.0;
        let offset_y = -scroll_y;

        let vp_x = clip_bounds.x as f32 + offset_x;
        let vp_y = clip_bounds.y as f32 + offset_y;

        let mut render_pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
            label: Some("Minimap Render Pass"),
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

        if scaled_w > 0.0 && scaled_h > 0.0 {
            render_pass.set_scissor_rect(clip_bounds.x, clip_bounds.y, clip_bounds.width, clip_bounds.height);
            render_pass.set_viewport(vp_x, vp_y, scaled_w, scaled_h, 0.0, 1.0);
            render_pass.set_pipeline(&pipeline.pipeline);
            render_pass.set_bind_group(0, &resources.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }
    }
}
