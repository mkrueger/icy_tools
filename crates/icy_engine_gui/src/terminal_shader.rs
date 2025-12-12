//! Terminal shader with multi-texture slicing support
//!
//! A GPU shader for rendering the terminal that supports very tall content
//! (up to 80,000 pixels) by splitting into multiple texture slices.
//! Each slice is max 8000px tall, with up to 10 slices supported.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use crate::tile_cache::MAX_TEXTURE_SLICES;
use crate::{MonitorSettings, PENDING_INSTANCE_REMOVALS, RenderInfo, TextureSliceData, now_ms, set_scale_factor};
use iced::Rectangle;
use iced::widget::shader;

/// Maximum texture dimension supported by most GPUs
const MAX_TEXTURE_DIMENSION: u32 = 8192;

/// Uniform data for the CRT shader (multi-texture version with slicing)
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct CRTUniforms {
    time: f32,
    brightness: f32,
    contrast: f32,
    gamma: f32,
    saturation: f32,
    monitor_type: f32,
    resolution: [f32; 2],

    curvature_x: f32,
    curvature_y: f32,
    enable_curvature: f32,

    scanline_thickness: f32,
    scanline_sharpness: f32,
    scanline_phase: f32,
    enable_scanlines: f32,

    noise_level: f32,
    sync_wobble: f32,
    enable_noise: f32,

    bloom_threshold: f32,
    bloom_radius: f32,
    bloom_intensity: f32,
    enable_bloom: f32,

    _padding: [f32; 2], // Padding to align background_color to 16 bytes
    background_color: [f32; 4],

    // Slicing uniforms
    num_slices: f32,
    total_image_height: f32,
    scroll_offset_y: f32,
    visible_height: f32,
    // x=slice0 height, y=slice1 height, z=slice2 height, w=first_slice_start_y
    slice_heights: [f32; 4],
}

/// The terminal shader program (high-level interface)
#[derive(Debug, Clone)]
pub struct TerminalShader {
    /// Texture slices (up to MAX_TEXTURE_SLICES)
    pub slices: Vec<TextureSliceData>,
    /// Heights of each slice in pixels
    pub slice_heights: Vec<u32>,
    /// Width of the texture (same for all slices)
    pub texture_width: u32,
    /// Total content height (full document)
    pub total_content_height: f32,
    /// Store the monitor settings for CRT effects
    pub monitor_settings: MonitorSettings,
    /// Unique instance ID
    pub instance_id: u64,
    /// Zoom level (1.0 = 100%)
    pub zoom: f32,
    /// Shared render info for mouse mapping
    pub render_info: Arc<RwLock<RenderInfo>>,
    /// Font dimensions for mouse mapping
    pub font_width: f32,
    pub font_height: f32,
    pub scan_lines: bool,
    /// Background color for out-of-bounds areas
    pub background_color: [f32; 4],
    /// Scroll offset in pixels
    pub scroll_offset_y: f32,
    /// Visible height in pixels
    pub visible_height: f32,
    /// Where the first slice starts in document Y coordinates
    pub first_slice_start_y: f32,
}

/// Texture slice for GPU
#[allow(dead_code)]
struct TextureSlice {
    texture: iced::wgpu::Texture,
    texture_view: iced::wgpu::TextureView,
    height: u32,
}

/// Per-instance GPU resources with texture slicing
struct InstanceResources {
    /// Texture slices (1-10 slices depending on content height)
    _slices: Vec<TextureSlice>,
    /// Bind group for rendering (includes all texture slots)
    bind_group: iced::wgpu::BindGroup,
    /// Uniform buffer
    uniform_buffer: iced::wgpu::Buffer,
    /// Monitor color buffer
    monitor_color_buffer: iced::wgpu::Buffer,
    /// Texture width for cache validation
    texture_width: u32,
    /// Total texture height for cache validation
    total_height: u32,
    /// Number of slices for cache validation
    num_slices: usize,
    /// Hash of texture data pointers to detect when data changed
    texture_data_hash: u64,
}

/// The terminal shader renderer (GPU pipeline) with multi-texture support
pub struct TerminalShaderRenderer {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group_layout: iced::wgpu::BindGroupLayout,
    sampler: iced::wgpu::Sampler,
    /// 1x1 transparent texture for unused texture slots
    dummy_texture_view: iced::wgpu::TextureView,
    instances: HashMap<u64, InstanceResources>,
}

static RENDERER_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static mut FILTER_MODE: iced::wgpu::FilterMode = iced::wgpu::FilterMode::Linear;

impl shader::Pipeline for TerminalShaderRenderer {
    fn new(device: &iced::wgpu::Device, _queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let renderer_id = RENDERER_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let filter_mode = iced::wgpu::FilterMode::Linear;
        unsafe { FILTER_MODE = filter_mode };

        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some(&format!("Terminal CRT Shader {}", renderer_id)),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("shaders/crt.wgsl").into()),
        });

        // Create bind group layout with 10 texture slots + sampler + uniforms + monitor_color
        let mut entries = Vec::with_capacity(MAX_TEXTURE_SLICES + 3);

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

        // Monitor color at binding 12
        entries.push(iced::wgpu::BindGroupLayoutEntry {
            binding: (MAX_TEXTURE_SLICES + 2) as u32,
            visibility: iced::wgpu::ShaderStages::FRAGMENT,
            ty: iced::wgpu::BindingType::Buffer {
                ty: iced::wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });

        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("Terminal Shader Bind Group Layout {}", renderer_id)),
            entries: &entries,
        });

        let pipeline_layout = device.create_pipeline_layout(&iced::wgpu::PipelineLayoutDescriptor {
            label: Some(&format!("Terminal Shader Pipeline Layout {}", renderer_id)),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some(&format!("Terminal Shader Pipeline {}", renderer_id)),
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

        let sampler = device.create_sampler(&iced::wgpu::SamplerDescriptor {
            label: Some(&format!("Terminal Texture Sampler {}", renderer_id)),
            address_mode_u: iced::wgpu::AddressMode::ClampToEdge,
            address_mode_v: iced::wgpu::AddressMode::ClampToEdge,
            address_mode_w: iced::wgpu::AddressMode::ClampToEdge,
            mag_filter: filter_mode,
            min_filter: filter_mode,
            mipmap_filter: iced::wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create 1x1 dummy texture for unused slots
        let dummy_texture = device.create_texture(&iced::wgpu::TextureDescriptor {
            label: Some("Terminal Dummy Texture"),
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

        TerminalShaderRenderer {
            pipeline,
            bind_group_layout,
            sampler,
            dummy_texture_view,
            instances: HashMap::new(),
        }
    }
}

impl TerminalShader {
    /// Compute a hash of the texture data pointers to detect changes
    fn compute_data_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        for slice in &self.slices {
            let ptr = Arc::as_ptr(&slice.rgba_data) as usize;
            ptr.hash(&mut hasher);
        }
        hasher.finish()
    }
}

impl shader::Primitive for TerminalShader {
    type Pipeline = TerminalShaderRenderer;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        _bounds: &iced::Rectangle,
        _viewport: &iced::advanced::graphics::Viewport,
    ) {
        set_scale_factor(_viewport.scale_factor() as f32);

        // Check if we need to recreate the sampler due to filter mode change
        let desired_filter = if self.monitor_settings.use_bilinear_filtering {
            iced::wgpu::FilterMode::Linear
        } else {
            iced::wgpu::FilterMode::Nearest
        };
        if desired_filter != unsafe { FILTER_MODE } {
            unsafe { FILTER_MODE = desired_filter };
            pipeline.sampler = device.create_sampler(&iced::wgpu::SamplerDescriptor {
                label: Some("Terminal Texture Sampler"),
                address_mode_u: iced::wgpu::AddressMode::ClampToEdge,
                address_mode_v: iced::wgpu::AddressMode::ClampToEdge,
                address_mode_w: iced::wgpu::AddressMode::ClampToEdge,
                mag_filter: desired_filter,
                min_filter: desired_filter,
                mipmap_filter: iced::wgpu::FilterMode::Nearest,
                ..Default::default()
            });
            // Clear all instances to force bind group recreation
            pipeline.instances.clear();
        }

        // Remove pending instances
        {
            let mut pending = PENDING_INSTANCE_REMOVALS.lock();
            for id in pending.drain(..) {
                pipeline.instances.remove(&id);
            }
        }

        let id = self.instance_id;
        let data_hash = self.compute_data_hash();
        let num_slices = self.slices.len();
        let texture_width = self.texture_width.min(MAX_TEXTURE_DIMENSION);
        let total_height = self.total_content_height as u32;

        // Check if we need to recreate resources
        let needs_recreate = match pipeline.instances.get(&id) {
            None => true,
            Some(resources) => {
                resources.texture_data_hash != data_hash
                    || resources.num_slices != num_slices
                    || resources.texture_width != texture_width
                    || resources.total_height != total_height
            }
        };

        if needs_recreate {
            // Create new slice textures
            let mut gpu_slices = Vec::with_capacity(num_slices);

            for (i, slice_data) in self.slices.iter().enumerate() {
                let w = slice_data.width.min(MAX_TEXTURE_DIMENSION);
                let h = slice_data.height.min(MAX_TEXTURE_DIMENSION);

                let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                    label: Some(&format!("Terminal Slice {} Instance {}", i, id)),
                    size: iced::wgpu::Extent3d {
                        width: w.max(1),
                        height: h.max(1),
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

                // Upload texture data
                if !slice_data.rgba_data.is_empty() {
                    queue.write_texture(
                        iced::wgpu::TexelCopyTextureInfo {
                            texture: &texture,
                            mip_level: 0,
                            origin: iced::wgpu::Origin3d::ZERO,
                            aspect: iced::wgpu::TextureAspect::All,
                        },
                        &slice_data.rgba_data,
                        iced::wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * w),
                            rows_per_image: Some(h),
                        },
                        iced::wgpu::Extent3d {
                            width: w.max(1),
                            height: h.max(1),
                            depth_or_array_layers: 1,
                        },
                    );
                }

                gpu_slices.push(TextureSlice {
                    texture,
                    texture_view,
                    height: h,
                });
            }

            // Create uniform buffer
            let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
                label: Some(&format!("Terminal Uniforms Instance {}", id)),
                size: std::mem::size_of::<CRTUniforms>() as u64,
                usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let monitor_color_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
                label: Some(&format!("Monitor Color Instance {}", id)),
                size: std::mem::size_of::<[f32; 4]>() as u64,
                usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            // Build bind group entries
            let mut bind_entries: Vec<iced::wgpu::BindGroupEntry> = Vec::with_capacity(MAX_TEXTURE_SLICES + 3);

            // Add texture bindings (0-9), using dummy for unused slots
            for i in 0..MAX_TEXTURE_SLICES {
                let view = if i < gpu_slices.len() {
                    &gpu_slices[i].texture_view
                } else {
                    &pipeline.dummy_texture_view
                };
                bind_entries.push(iced::wgpu::BindGroupEntry {
                    binding: i as u32,
                    resource: iced::wgpu::BindingResource::TextureView(view),
                });
            }

            // Sampler at binding 10
            bind_entries.push(iced::wgpu::BindGroupEntry {
                binding: MAX_TEXTURE_SLICES as u32,
                resource: iced::wgpu::BindingResource::Sampler(&pipeline.sampler),
            });

            // Uniforms at binding 11
            bind_entries.push(iced::wgpu::BindGroupEntry {
                binding: (MAX_TEXTURE_SLICES + 1) as u32,
                resource: uniform_buffer.as_entire_binding(),
            });

            // Monitor color at binding 12
            bind_entries.push(iced::wgpu::BindGroupEntry {
                binding: (MAX_TEXTURE_SLICES + 2) as u32,
                resource: monitor_color_buffer.as_entire_binding(),
            });

            let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                label: Some(&format!("Terminal BindGroup Instance {}", id)),
                layout: &pipeline.bind_group_layout,
                entries: &bind_entries,
            });

            pipeline.instances.insert(
                id,
                InstanceResources {
                    _slices: gpu_slices,
                    bind_group,
                    uniform_buffer,
                    monitor_color_buffer,
                    texture_width,
                    total_height,
                    num_slices,
                    texture_data_hash: data_hash,
                },
            );
        }

        // Update uniforms every frame
        let Some(resources) = pipeline.instances.get(&id) else {
            return;
        };

        // Calculate display size
        let term_w = self.texture_width.max(1) as f32;
        let term_h = self.visible_height.max(1.0);
        let avail_w = _bounds.width.max(1.0);
        let avail_h = _bounds.height.max(1.0);
        let use_int = self.monitor_settings.use_integer_scaling;
        let final_scale = self.monitor_settings.scaling_mode.compute_zoom(term_w, term_h, avail_w, avail_h, use_int);
        let scaled_w = term_w * final_scale;
        let scaled_h = term_h * final_scale;

        // Pack slice heights into array: [slice0, slice1, slice2, first_slice_start_y]
        let mut slice_heights = [0.0f32; 4];
        for (i, &height) in self.slice_heights.iter().enumerate() {
            if i < 3 {
                slice_heights[i] = height as f32;
            }
        }
        slice_heights[3] = self.first_slice_start_y;

        let monitor_color = match self.monitor_settings.monitor_type {
            crate::MonitorType::Color => [1.0, 1.0, 1.0, 1.0],
            crate::MonitorType::Grayscale => [1.0, 1.0, 1.0, 1.0],
            crate::MonitorType::Amber => [1.0, 0.7, 0.0, 1.0],
            crate::MonitorType::Green => [0.0, 1.0, 0.2, 1.0],
            crate::MonitorType::Apple2 => [0.2, 1.0, 0.4, 1.0],
            crate::MonitorType::Futuristic => [0.0, 0.8, 1.0, 1.0],
            crate::MonitorType::CustomMonochrome => {
                let (r, g, b) = self.monitor_settings.custom_monitor_color.rgb();
                [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
            }
        };

        let uniform_data = CRTUniforms {
            time: now_ms() as f32 / 1000.0,
            brightness: self.monitor_settings.brightness / 100.0,
            contrast: self.monitor_settings.contrast / 100.0,
            gamma: self.monitor_settings.gamma,
            saturation: self.monitor_settings.saturation / 100.0,
            monitor_type: self.monitor_settings.monitor_type.to_index() as f32,
            resolution: [scaled_w, scaled_h],
            curvature_x: if self.monitor_settings.use_curvature {
                (100.0 - self.monitor_settings.curvature_x) / 10.0
            } else {
                0.0
            },
            curvature_y: if self.monitor_settings.use_curvature {
                (100.0 - self.monitor_settings.curvature_y) / 10.0
            } else {
                0.0
            },
            enable_curvature: if self.monitor_settings.use_curvature { 1.0 } else { 0.0 },
            scanline_thickness: if self.monitor_settings.use_scanlines {
                self.monitor_settings.scanline_thickness
            } else {
                0.5
            },
            scanline_sharpness: if self.monitor_settings.use_scanlines {
                self.monitor_settings.scanline_sharpness
            } else {
                0.5
            },
            scanline_phase: if self.monitor_settings.use_scanlines {
                self.monitor_settings.scanline_phase
            } else {
                0.0
            },
            enable_scanlines: if self.monitor_settings.use_scanlines { 1.0 } else { 0.0 },
            noise_level: if self.monitor_settings.use_noise {
                (self.monitor_settings.noise_level / 100.0).clamp(0.0, 1.0)
            } else {
                0.0
            },
            sync_wobble: if self.monitor_settings.use_noise {
                (self.monitor_settings.sync_wobble / 100.0).clamp(0.0, 1.0)
            } else {
                0.0
            },
            enable_noise: if self.monitor_settings.use_noise { 1.0 } else { 0.0 },
            bloom_threshold: if self.monitor_settings.use_bloom {
                (self.monitor_settings.bloom_threshold / 100.0).clamp(0.0, 1.0)
            } else {
                1.0
            },
            bloom_radius: if self.monitor_settings.use_bloom {
                (self.monitor_settings.bloom_radius / 10.0).max(0.5)
            } else {
                0.0
            },
            bloom_intensity: if self.monitor_settings.use_bloom {
                (self.monitor_settings.glow_strength / 50.0).max(0.0)
            } else {
                0.0
            },
            enable_bloom: if self.monitor_settings.use_bloom { 1.0 } else { 0.0 },
            _padding: [0.0; 2],
            background_color: self.background_color,
            num_slices: self.slices.len() as f32,
            total_image_height: self.total_content_height,
            scroll_offset_y: self.scroll_offset_y,
            visible_height: self.visible_height,
            slice_heights,
        };

        let uniform_bytes = unsafe { std::slice::from_raw_parts(&uniform_data as *const CRTUniforms as *const u8, std::mem::size_of::<CRTUniforms>()) };
        queue.write_buffer(&resources.uniform_buffer, 0, uniform_bytes);

        let color_bytes = unsafe { std::slice::from_raw_parts(monitor_color.as_ptr() as *const u8, std::mem::size_of::<[f32; 4]>()) };
        queue.write_buffer(&resources.monitor_color_buffer, 0, color_bytes);
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        encoder.push_debug_group(&format!("Terminal Instance {} Render", self.instance_id));

        let Some(resources) = pipeline.instances.get(&self.instance_id) else {
            encoder.pop_debug_group();
            return;
        };

        // GPU dimension limits
        const MAX_VIEWPORT_DIM: f32 = 8192.0;

        // Use visible dimensions for rendering, clamped to GPU limits
        let term_w = (self.texture_width.max(1) as f32).min(MAX_VIEWPORT_DIM);
        let term_h = self.visible_height.max(1.0).min(MAX_VIEWPORT_DIM);

        let avail_w = clip_bounds.width.max(1) as f32;
        let avail_h = clip_bounds.height.max(1) as f32;

        let use_int = self.monitor_settings.use_integer_scaling;
        let display_scale = self.monitor_settings.scaling_mode.compute_zoom(term_w, term_h, avail_w, avail_h, use_int);

        // Clamp scaled dimensions to available space to prevent negative offsets
        let scaled_w = (term_w * display_scale).min(avail_w);
        let scaled_h = (term_h * display_scale).min(avail_h);

        let offset_x = ((avail_w - scaled_w) / 2.0).max(0.0);
        let offset_y = ((avail_h - scaled_h) / 2.0).max(0.0);

        let (vp_x, vp_y, vp_w, vp_h) = if use_int {
            (
                clip_bounds.x as f32 + offset_x.round(),
                clip_bounds.y as f32 + offset_y.round(),
                scaled_w.round(),
                scaled_h.round(),
            )
        } else {
            (clip_bounds.x as f32 + offset_x, clip_bounds.y as f32 + offset_y, scaled_w, scaled_h)
        };

        // Update shared render info for mouse mapping
        {
            let mut info = self.render_info.write();
            info.display_scale = display_scale;
            info.viewport_x = if use_int { offset_x.round() } else { offset_x };
            info.viewport_y = if use_int { offset_y.round() } else { offset_y };
            info.viewport_width = vp_w;
            info.viewport_height = vp_h;
            info.terminal_width = term_w;
            info.terminal_height = term_h;
            info.font_width = self.font_width;
            info.font_height = self.font_height;
            info.scan_lines = self.scan_lines;
            info.bounds_x = clip_bounds.x as f32;
            info.bounds_y = clip_bounds.y as f32;
            info.bounds_width = avail_w;
            info.bounds_height = avail_h;
        }

        let mut render_pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
            label: Some("Terminal Shader Render Pass"),
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

        let scissor_x = clip_bounds.x;
        let scissor_y = clip_bounds.y;
        let scissor_width = clip_bounds.width;
        let scissor_height = clip_bounds.height;

        // Final clamp of viewport dimensions (should already be clamped, but ensure safety)
        let vp_w = vp_w.min(MAX_VIEWPORT_DIM);
        let vp_h = vp_h.min(MAX_VIEWPORT_DIM);

        if scissor_width > 0 && scissor_height > 0 && vp_w > 0.0 && vp_h > 0.0 {
            render_pass.set_scissor_rect(scissor_x, scissor_y, scissor_width, scissor_height);
            render_pass.set_viewport(vp_x, vp_y, vp_w, vp_h, 0.0, 1.0);
            render_pass.set_pipeline(&pipeline.pipeline);
            render_pass.set_bind_group(0, &resources.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        drop(render_pass);
        encoder.pop_debug_group();
    }
}
