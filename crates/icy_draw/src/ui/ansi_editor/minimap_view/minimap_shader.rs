//! Minimap shader implementation with sliding window texture support
//!
//! Uses 3 texture slices (matching Terminal's sliding window).

use std::collections::HashMap;
use std::sync::Arc;

use iced::Rectangle;
use iced::mouse;
use iced::widget::shader;
use parking_lot::Mutex;

use super::{MinimapMessage, SharedMinimapState, TextureSliceData};

/// Maximum number of texture slices (matches Terminal's sliding window)
const MAX_TEXTURE_SLICES: usize = 3;

/// Uniform data for the minimap shader (multi-texture version)
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
    /// Number of texture slices (1-10)
    num_slices: f32,
    /// Total image height across all slices
    total_image_height: f32,
    /// Heights of each slice in pixels (packed as 3 vec4s = 12 floats for 10 slices + 2 padding)
    slice_heights: [[f32; 4]; 3],
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
    /// Texture slices (up to MAX_TEXTURE_SLICES)
    pub slices: Vec<TextureSliceData>,
    /// Heights of each slice in pixels
    pub slice_heights: Vec<u32>,
    /// Width of the texture (same for all slices)
    pub texture_width: u32,
    /// Total rendered height across all slices
    pub total_rendered_height: u32,
    /// Unique instance ID
    pub instance_id: usize,
    /// Viewport overlay information
    pub viewport_info: ViewportInfo,
    /// Scroll offset (0.0 = top, 1.0 = bottom)
    pub scroll_offset: f32,
    /// Available height for rendering (to compute visible UV range)
    pub available_height: f32,
    /// Full content height in pixels (for proper viewport scaling)
    pub full_content_height: f32,
    /// Where the first slice starts in document Y coordinates
    pub first_slice_start_y: f32,
    /// Shared state for communicating bounds back to MinimapView
    pub shared_state: Arc<Mutex<SharedMinimapState>>,
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
        let tex_w = self.texture_width;
        let tex_h = self.total_rendered_height;
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
        // texture_uv is 0-1 over the rendered texture (which may be smaller than full buffer)
        let texture_uv_y = (scroll_uv_y + screen_y * visible_uv_height).clamp(0.0, 1.0);

        // Convert from texture space to full buffer space
        // render_ratio is how much of the full buffer we actually rendered
        let render_ratio = tex_h as f32 / self.full_content_height;
        let norm_y = (texture_uv_y * render_ratio).clamp(0.0, 1.0);
        let norm_x = (relative_x / bounds.width).clamp(0.0, 1.0);

        Some((norm_x, norm_y))
    }
}

impl shader::Program<MinimapMessage> for MinimapProgram {
    type State = MinimapState;
    type Primitive = MinimapPrimitive;

    fn draw(&self, _state: &Self::State, _cursor: mouse::Cursor, bounds: Rectangle) -> Self::Primitive {
        // Update shared state with current bounds (for next frame's rendering)
        {
            let mut shared = self.shared_state.lock();
            shared.available_width = bounds.width;
            shared.available_height = bounds.height;
        }

        MinimapPrimitive {
            slices: self.slices.clone(),
            slice_heights: self.slice_heights.clone(),
            texture_width: self.texture_width,
            total_rendered_height: self.total_rendered_height,
            instance_id: self.instance_id,
            viewport_info: self.viewport_info.clone(),
            scroll_offset: self.scroll_offset,
            available_height: bounds.height,
            full_content_height: self.full_content_height,
            first_slice_start_y: self.first_slice_start_y,
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
    /// Texture slices (up to MAX_TEXTURE_SLICES)
    pub slices: Vec<TextureSliceData>,
    /// Heights of each slice in pixels
    pub slice_heights: Vec<u32>,
    /// Width of the texture (same for all slices)
    pub texture_width: u32,
    /// Total rendered height across all slices
    pub total_rendered_height: u32,
    /// Unique instance ID
    pub instance_id: usize,
    /// Viewport overlay information
    pub viewport_info: ViewportInfo,
    /// Scroll offset (0.0 = top, 1.0 = bottom)
    pub scroll_offset: f32,
    /// Available height for rendering
    pub available_height: f32,
    /// Full content height in pixels
    pub full_content_height: f32,
    /// Where the first slice starts in document Y coordinates
    pub first_slice_start_y: f32,
}

impl MinimapPrimitive {
    /// Compute a hash of the texture data pointers to detect changes
    fn compute_data_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        for slice in &self.slices {
            // Use the Arc pointer address as a unique identifier
            let ptr = Arc::as_ptr(&slice.rgba_data) as usize;
            ptr.hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// Texture slice for GPU
struct TextureSlice {
    #[allow(dead_code)]
    texture: iced::wgpu::Texture,
    texture_view: iced::wgpu::TextureView,
    height: u32,
}

/// Per-instance GPU resources with texture slicing
struct InstanceResources {
    /// Texture slices (1-10 slices depending on content height)
    slices: Vec<TextureSlice>,
    /// Bind group for rendering (includes all texture slots)
    bind_group: iced::wgpu::BindGroup,
    /// Uniform buffer
    uniform_buffer: iced::wgpu::Buffer,
    /// Total texture dimensions for cache validation
    texture_size: (u32, u32),
    /// Number of slices for cache validation
    num_slices: usize,
    /// Individual slice sizes (width, height) for cache validation
    slice_sizes: Vec<(u32, u32)>,
    /// Hash of texture data pointers to detect when data changed
    texture_data_hash: u64,
}

/// The minimap shader renderer (GPU pipeline) with multi-texture support
pub struct MinimapShaderRenderer {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group_layout: iced::wgpu::BindGroupLayout,
    sampler: iced::wgpu::Sampler,
    /// 1x1 transparent texture for unused texture slots
    dummy_texture_view: iced::wgpu::TextureView,
    instances: HashMap<usize, InstanceResources>,
}

impl shader::Pipeline for MinimapShaderRenderer {
    fn new(device: &iced::wgpu::Device, _queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("Minimap Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("minimap.wgsl").into()),
        });

        // Create bind group layout with 10 texture slots + sampler + uniforms
        let mut entries = Vec::with_capacity(MAX_TEXTURE_SLICES + 2);

        // Add 10 texture bindings (0-9)
        for i in 0..MAX_TEXTURE_SLICES {
            entries.push(iced::wgpu::BindGroupLayoutEntry {
                binding: i as u32,
                visibility: iced::wgpu::ShaderStages::FRAGMENT,
                ty: iced::wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: iced::wgpu::TextureViewDimension::D2,
                    sample_type: iced::wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            });
        }

        // Sampler at binding 10
        entries.push(iced::wgpu::BindGroupLayoutEntry {
            binding: MAX_TEXTURE_SLICES as u32,
            visibility: iced::wgpu::ShaderStages::FRAGMENT,
            ty: iced::wgpu::BindingType::Sampler(iced::wgpu::SamplerBindingType::Filtering),
            count: None,
        });

        // Uniforms at binding 11
        entries.push(iced::wgpu::BindGroupLayoutEntry {
            binding: (MAX_TEXTURE_SLICES + 1) as u32,
            visibility: iced::wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: iced::wgpu::BindingType::Buffer {
                ty: iced::wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });

        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("Minimap Bind Group Layout"),
            entries: &entries,
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

        // Create 1x1 transparent dummy texture for unused slots
        let dummy_texture = device.create_texture(&iced::wgpu::TextureDescriptor {
            label: Some("Minimap Dummy Texture"),
            size: iced::wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: iced::wgpu::TextureDimension::D2,
            format: iced::wgpu::TextureFormat::Rgba8Unorm,
            usage: iced::wgpu::TextureUsages::TEXTURE_BINDING | iced::wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let dummy_texture_view = dummy_texture.create_view(&iced::wgpu::TextureViewDescriptor::default());

        MinimapShaderRenderer {
            pipeline,
            bind_group_layout,
            sampler,
            dummy_texture_view,
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
        let num_slices = self.slices.len().min(MAX_TEXTURE_SLICES);
        let tex_w = self.texture_width.max(1);
        let tex_h = self.total_rendered_height.max(1);

        // Check if we need to recreate resources
        let current_slice_sizes: Vec<(u32, u32)> = self.slices.iter()
            .take(MAX_TEXTURE_SLICES)
            .map(|s| (s.width, s.height))
            .collect();
        
        let needs_recreate = match pipeline.instances.get(&id) {
            Some(resources) => {
                resources.texture_size != (tex_w, tex_h) 
                || resources.num_slices != num_slices
                || resources.slice_sizes != current_slice_sizes
            },
            None => true,
        };

        if needs_recreate {
            // Create texture slices
            let mut slices = Vec::with_capacity(num_slices);

            for (i, slice_data) in self.slices.iter().enumerate().take(MAX_TEXTURE_SLICES) {
                let slice_w = slice_data.width.max(1);
                let slice_h = slice_data.height.max(1);

                let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                    label: Some(&format!("Minimap Texture {} Slice {}", id, i)),
                    size: iced::wgpu::Extent3d {
                        width: slice_w,
                        height: slice_h,
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

                slices.push(TextureSlice {
                    texture,
                    texture_view,
                    height: slice_h,
                });
            }

            // Create uniform buffer
            let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
                label: Some(&format!("Minimap Uniforms {}", id)),
                size: std::mem::size_of::<MinimapUniforms>() as u64,
                usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            // Create bind group entries for all 10 texture slots + sampler + uniforms
            let mut entries = Vec::with_capacity(MAX_TEXTURE_SLICES + 2);

            for i in 0..MAX_TEXTURE_SLICES {
                let texture_view = if i < slices.len() {
                    &slices[i].texture_view
                } else {
                    &pipeline.dummy_texture_view
                };
                entries.push(iced::wgpu::BindGroupEntry {
                    binding: i as u32,
                    resource: iced::wgpu::BindingResource::TextureView(texture_view),
                });
            }

            // Sampler at binding 10
            entries.push(iced::wgpu::BindGroupEntry {
                binding: MAX_TEXTURE_SLICES as u32,
                resource: iced::wgpu::BindingResource::Sampler(&pipeline.sampler),
            });

            // Uniforms at binding 11
            entries.push(iced::wgpu::BindGroupEntry {
                binding: (MAX_TEXTURE_SLICES + 1) as u32,
                resource: uniform_buffer.as_entire_binding(),
            });

            let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                label: Some(&format!("Minimap BindGroup {}", id)),
                layout: &pipeline.bind_group_layout,
                entries: &entries,
            });

            pipeline.instances.insert(
                id,
                InstanceResources {
                    slices,
                    bind_group,
                    uniform_buffer,
                    texture_size: (tex_w, tex_h),
                    num_slices,
                    slice_sizes: current_slice_sizes.clone(),
                    texture_data_hash: 0, // Will be updated below
                },
            );
        }

        let Some(resources) = pipeline.instances.get_mut(&id) else {
            return;
        };

        // Check if texture data has changed using pointer-based hash
        let current_hash = self.compute_data_hash();
        let needs_texture_upload = resources.texture_data_hash != current_hash;

        // Upload texture data only if changed
        if needs_texture_upload {
            for (i, slice_data) in self.slices.iter().enumerate().take(resources.slices.len()) {
                if !slice_data.rgba_data.is_empty() && i < resources.slices.len() {
                    let slice = &resources.slices[i];
                    let bytes_per_row = 4 * slice_data.width;

                    queue.write_texture(
                        iced::wgpu::TexelCopyTextureInfo {
                            texture: &slice.texture,
                            mip_level: 0,
                            origin: iced::wgpu::Origin3d::ZERO,
                            aspect: iced::wgpu::TextureAspect::All,
                        },
                        &slice_data.rgba_data,
                        iced::wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(bytes_per_row),
                            rows_per_image: Some(slice_data.height),
                        },
                        iced::wgpu::Extent3d {
                            width: slice_data.width,
                            height: slice_data.height,
                            depth_or_array_layers: 1,
                        },
                    );
                }
            }
            resources.texture_data_hash = current_hash;
        }

        // Update uniforms
        let show_viewport = if self.viewport_info.width > 0.0 && self.viewport_info.height > 0.0 {
            1.0
        } else {
            0.0
        };

        // Calculate visible UV range based on texture size, available height, and scroll
        let avail_h = self.available_height.max(1.0);
        let scale = bounds.width / tex_w as f32;
        let scaled_h = tex_h as f32 * scale;

        // How much of the texture is visible (in normalized UV coordinates)
        let visible_uv_height = (avail_h / scaled_h).min(1.0);
        let max_scroll_uv = (1.0 - visible_uv_height).max(0.0);
        let scroll_uv_y = self.scroll_offset * max_scroll_uv;

        let visible_uv_min_y = scroll_uv_y;
        let visible_uv_max_y = scroll_uv_y + visible_uv_height;

        // Convert viewport from full-buffer-space to texture-space
        // viewport_info is in full buffer coordinates (0-1 over full_content_height)
        // We need to convert to texture coordinates (0-1 over total_rendered_height)
        //
        // Example: full_content_height = 87232px, total_rendered_height = 80000px
        // render_ratio = 80000/87232 = 0.917
        // If viewport_info.y = 0.5 (50% of buffer = 43616px)
        // In texture space: 43616px / 80000px = 0.545
        // So: viewport_y_tex = viewport_info.y / render_ratio = 0.5 / 0.917 = 0.545
        let render_ratio = tex_h as f32 / self.full_content_height.max(1.0);
        let viewport_y_tex = self.viewport_info.y / render_ratio.max(0.001);
        let viewport_h_tex = self.viewport_info.height / render_ratio.max(0.001);

        // Pack slice heights into 3 vec4s
        // Like Terminal shader: slice_heights[0] = [h0, h1, h2, first_slice_start_y]
        let mut packed_heights = [[0.0f32; 4]; 3];
        for (i, &h) in self.slice_heights.iter().enumerate().take(MAX_TEXTURE_SLICES) {
            packed_heights[0][i] = h as f32;
        }
        // Store first_slice_start_y in the 4th element (w component) - same as Terminal
        packed_heights[0][3] = self.first_slice_start_y;

        let uniforms = MinimapUniforms {
            viewport_rect: [self.viewport_info.x, viewport_y_tex, self.viewport_info.width, viewport_h_tex],
            // Modern cyan accent color - vibrant but not overwhelming
            viewport_color: [0.2, 0.8, 0.9, 0.9],
            visible_uv_range: [visible_uv_min_y, visible_uv_max_y, 0.0, 0.0],
            border_thickness: 2.5,
            show_viewport,
            num_slices: num_slices as f32,
            total_image_height: tex_h as f32,
            slice_heights: packed_heights,
        };

        let uniform_bytes = unsafe { std::slice::from_raw_parts(&uniforms as *const MinimapUniforms as *const u8, std::mem::size_of::<MinimapUniforms>()) };
        queue.write_buffer(&resources.uniform_buffer, 0, uniform_bytes);
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        let Some(resources) = pipeline.instances.get(&self.instance_id) else {
            return;
        };

        // The viewport is just the visible area (clip_bounds)
        // Scrolling is handled in the shader via visible_uv_range uniforms
        let vp_x = clip_bounds.x as f32;
        let vp_y = clip_bounds.y as f32;
        let vp_w = clip_bounds.width as f32;
        let vp_h = clip_bounds.height as f32;

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

        if vp_w > 0.0 && vp_h > 0.0 {
            render_pass.set_scissor_rect(clip_bounds.x, clip_bounds.y, clip_bounds.width, clip_bounds.height);
            render_pass.set_viewport(vp_x, vp_y, vp_w, vp_h, 0.0, 1.0);
            render_pass.set_pipeline(&pipeline.pipeline);
            render_pass.set_bind_group(0, &resources.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }
    }
}
