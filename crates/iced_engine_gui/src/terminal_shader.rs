use std::collections::HashMap;

use crate::{CRTShaderState, MonitorSettings, PENDING_INSTANCE_REMOVALS, Terminal, now_ms, set_scale_factor};
use iced::Rectangle;
use iced::widget::shader;
use icy_engine::{Caret, CaretShape};

#[repr(C)]
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
}

// Define your shader primitive - store rendered data, not references
#[derive(Debug, Clone)]
pub struct TerminalShader {
    // Store the rendered terminal as RGBA data
    pub terminal_rgba: Vec<u8>,
    pub terminal_size: (u32, u32),
    // Store the monitor settings for CRT effects
    pub monitor_settings: MonitorSettings,
    pub instance_id: u64,
}

// Add per-instance resources struct
struct InstanceResources {
    texture: iced::wgpu::Texture,
    texture_view: iced::wgpu::TextureView,
    bind_group: iced::wgpu::BindGroup,
    uniform_buffer: iced::wgpu::Buffer,
    monitor_color_buffer: iced::wgpu::Buffer,
    texture_size: (u32, u32),
}
// Renderer struct for GPU resources
pub struct TerminalShaderRenderer {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group_layout: iced::wgpu::BindGroupLayout,
    sampler: iced::wgpu::Sampler,
    instances: HashMap<u64, InstanceResources>,
}

static RENDERER_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static mut FILTER_MODE: iced::wgpu::FilterMode = iced::wgpu::FilterMode::Linear;

impl shader::Primitive for TerminalShader {
    type Renderer = TerminalShaderRenderer;

    fn initialize(&self, device: &iced::wgpu::Device, _queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self::Renderer {
        let renderer_id = RENDERER_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // ...existing shader and pipeline creation code unchanged...
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some(&format!("Terminal CRT Shader {}", renderer_id)),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("shaders/crt.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("Terminal Shader Bind Group Layout {}", renderer_id)),
            entries: &[
                // ...existing entries unchanged...
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
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: iced::wgpu::ShaderStages::FRAGMENT,
                    ty: iced::wgpu::BindingType::Sampler(iced::wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
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
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 3,
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

        let filter_mode = if self.monitor_settings.use_bilinear_filtering {
            iced::wgpu::FilterMode::Linear
        } else {
            iced::wgpu::FilterMode::Nearest
        };
        unsafe { FILTER_MODE = filter_mode };
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

        TerminalShaderRenderer {
            pipeline,
            bind_group_layout,
            sampler,
            instances: HashMap::new(), // NEW: empty map of instances
        }
    }

    fn prepare(
        &self,
        renderer: &mut Self::Renderer,
        device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        bounds: &iced::Rectangle,
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
            // Recreate sampler if filter mode changed
            // We need to track the current filter mode in the renderer
            // For now, recreate it every frame (small overhead but ensures correctness)
            let new_sampler = device.create_sampler(&iced::wgpu::SamplerDescriptor {
                label: Some("Terminal Texture Sampler"),
                address_mode_u: iced::wgpu::AddressMode::ClampToEdge,
                address_mode_v: iced::wgpu::AddressMode::ClampToEdge,
                address_mode_w: iced::wgpu::AddressMode::ClampToEdge,
                mag_filter: desired_filter,
                min_filter: desired_filter,
                mipmap_filter: iced::wgpu::FilterMode::Nearest,
                ..Default::default()
            });

            renderer.sampler = new_sampler;
            // Update ALL existing instances' bind groups with the new sampler
            for (_instance_id, resources) in renderer.instances.iter_mut() {
                let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                    label: Some(&format!("Terminal BindGroup Instance {}", _instance_id)),
                    layout: &renderer.bind_group_layout,
                    entries: &[
                        iced::wgpu::BindGroupEntry {
                            binding: 0,
                            resource: iced::wgpu::BindingResource::TextureView(&resources.texture_view),
                        },
                        iced::wgpu::BindGroupEntry {
                            binding: 1,
                            resource: iced::wgpu::BindingResource::Sampler(&renderer.sampler), // Use new sampler
                        },
                        iced::wgpu::BindGroupEntry {
                            binding: 2,
                            resource: resources.uniform_buffer.as_entire_binding(),
                        },
                        iced::wgpu::BindGroupEntry {
                            binding: 3,
                            resource: resources.monitor_color_buffer.as_entire_binding(),
                        },
                    ],
                });
                resources.bind_group = bind_group;
            }
        }

        if let Ok(mut pending) = PENDING_INSTANCE_REMOVALS.lock() {
            for id in pending.drain(..) {
                renderer.instances.remove(&id);
            }
        }

        let id = self.instance_id;
        let (w, h) = self.terminal_size;

        // Get or create per-instance resources
        let resources = renderer.instances.entry(id).or_insert_with(|| {
            // Create per-instance resources
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

            let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                label: Some(&format!("Terminal Texture Instance {}", id)),
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

            let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                label: Some(&format!("Terminal BindGroup Instance {}", id)),
                layout: &renderer.bind_group_layout,
                entries: &[
                    iced::wgpu::BindGroupEntry {
                        binding: 0,
                        resource: iced::wgpu::BindingResource::TextureView(&texture_view),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 1,
                        resource: iced::wgpu::BindingResource::Sampler(&renderer.sampler),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 3,
                        resource: monitor_color_buffer.as_entire_binding(),
                    },
                ],
            });

            InstanceResources {
                texture,
                texture_view,
                bind_group,
                uniform_buffer,
                monitor_color_buffer,
                texture_size: (w, h),
            }
        });

        // Recreate texture if size changed
        if resources.texture_size != (w, h) {
            let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                label: Some(&format!("Terminal Texture Instance {}", id)),
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

            let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                label: Some(&format!("Terminal BindGroup Instance {}", id)),
                layout: &renderer.bind_group_layout,
                entries: &[
                    iced::wgpu::BindGroupEntry {
                        binding: 0,
                        resource: iced::wgpu::BindingResource::TextureView(&texture_view),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 1,
                        resource: iced::wgpu::BindingResource::Sampler(&renderer.sampler),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 2,
                        resource: resources.uniform_buffer.as_entire_binding(),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 3,
                        resource: resources.monitor_color_buffer.as_entire_binding(),
                    },
                ],
            });

            resources.texture = texture;
            resources.texture_view = texture_view;
            resources.bind_group = bind_group;
            resources.texture_size = (w, h);
        }

        // Upload texture data for this instance
        if !self.terminal_rgba.is_empty() {
            queue.write_texture(
                iced::wgpu::TexelCopyTextureInfo {
                    texture: &resources.texture,
                    mip_level: 0,
                    origin: iced::wgpu::Origin3d::ZERO,
                    aspect: iced::wgpu::TextureAspect::All,
                },
                &self.terminal_rgba,
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

        // Aspect ratio fit
        let term_w = self.terminal_size.0.max(1) as f32;
        let term_h = self.terminal_size.1.max(1) as f32;
        let avail_w = bounds.width.max(1.0);
        let avail_h = bounds.height.max(1.0);
        let uniform_scale = (avail_w / term_w).min(avail_h / term_h);

        let use_pp = self.monitor_settings.use_pixel_perfect_scaling;
        let int_scale = if use_pp { uniform_scale.floor().max(1.0) } else { uniform_scale };
        let display_scale = if use_pp { int_scale } else { uniform_scale };

        let scaled_w = term_w * display_scale;
        let scaled_h = term_h * display_scale;

        // ...rest of uniform data setup unchanged...
        let monitor_color = match self.monitor_settings.monitor_type {
            crate::MonitorType::Color => [1.0, 1.0, 1.0, 1.0],
            crate::MonitorType::Grayscale => [1.0, 1.0, 1.0, 1.0],
            crate::MonitorType::Amber => [1.0, 0.7, 0.0, 1.0],
            crate::MonitorType::Green => [0.0, 1.0, 0.2, 1.0],
            crate::MonitorType::Apple2 => [0.2, 1.0, 0.4, 1.0],
            crate::MonitorType::Futuristic => [0.0, 0.8, 1.0, 1.0],
            crate::MonitorType::CustomMonochrome => {
                let (r, g, b) = self.monitor_settings.custom_monitor_color.get_rgb();
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
        };

        // Write to this instance's uniform buffers
        let uniform_bytes = unsafe { std::slice::from_raw_parts(&uniform_data as *const CRTUniforms as *const u8, std::mem::size_of::<CRTUniforms>()) };
        queue.write_buffer(&resources.uniform_buffer, 0, uniform_bytes);

        let color_bytes = unsafe { std::slice::from_raw_parts(monitor_color.as_ptr() as *const u8, std::mem::size_of::<[f32; 4]>()) };
        queue.write_buffer(&resources.monitor_color_buffer, 0, color_bytes);
    }

    fn render(&self, renderer: &Self::Renderer, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        encoder.push_debug_group(&format!("Terminal Instance {} Render", self.instance_id));

        // Get this instance's resources
        let Some(resources) = renderer.instances.get(&self.instance_id) else {
            encoder.pop_debug_group();
            return;
        };

        // ...rest of render code unchanged except using instance bind_group...
        let term_w = self.terminal_size.0.max(1) as f32;
        let term_h = self.terminal_size.1.max(1) as f32;
        let avail_w = clip_bounds.width.max(1) as f32;
        let avail_h = clip_bounds.height.max(1) as f32;

        let uniform_scale = (avail_w / term_w).min(avail_h / term_h);
        let use_pp = self.monitor_settings.use_pixel_perfect_scaling;
        let display_scale = if use_pp { uniform_scale.floor().max(1.0) } else { uniform_scale };

        let scaled_w = term_w * display_scale;
        let scaled_h = term_h * display_scale;

        let offset_x = clip_bounds.x as f32 + (avail_w - scaled_w) / 2.0;
        let offset_y = clip_bounds.y as f32 + (avail_h - scaled_h) / 2.0;

        let (vp_x, vp_y, vp_w, vp_h) = if use_pp {
            (offset_x.round(), offset_y.round(), scaled_w.round(), scaled_h.round())
        } else {
            (offset_x, offset_y, scaled_w, scaled_h)
        };

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

        let sc_x = vp_x.max(0.0).floor() as u32;
        let sc_y = vp_y.max(0.0).floor() as u32;
        let sc_w = vp_w.max(1.0).floor() as u32;
        let sc_h = vp_h.max(1.0).floor() as u32;

        let scissor_x = sc_x.min(clip_bounds.x + clip_bounds.width);
        let scissor_y = sc_y.min(clip_bounds.y + clip_bounds.height);
        let scissor_width = sc_w.min((clip_bounds.x + clip_bounds.width).saturating_sub(scissor_x));
        let scissor_height = sc_h.min((clip_bounds.y + clip_bounds.height).saturating_sub(scissor_y));

        if scissor_width > 0 && scissor_height > 0 && vp_w > 0.0 && vp_h > 0.0 {
            render_pass.set_scissor_rect(scissor_x, scissor_y, scissor_width, scissor_height);
            render_pass.set_viewport(vp_x, vp_y, vp_w, vp_h, 0.0, 1.0);
            render_pass.set_pipeline(&renderer.pipeline);
            render_pass.set_bind_group(0, &resources.bind_group, &[]); // Use instance-specific bind group
            render_pass.draw(0..3, 0..1);
        }

        drop(render_pass);
        encoder.pop_debug_group();
    }
}

// Program wrapper that renders the terminal and creates the shader
pub struct CRTShaderProgram<'a> {
    pub term: &'a Terminal,
    pub monitor_settings: MonitorSettings,
}

impl<'a> CRTShaderProgram<'a> {
    pub fn new(term: &'a Terminal, monitor_settings: MonitorSettings) -> Self {
        Self { term, monitor_settings }
    }

    pub fn draw_caret(&self, caret: &Caret, state: &CRTShaderState, rgba_data: &mut Vec<u8>, size: (u32, u32), font_w: usize, font_h: usize) {
        // Check both the caret's is_blinking property and the blink timer state
        let should_draw = caret.is_visible() && (!caret.is_blinking || state.caret_blink.is_on());

        if should_draw && self.term.has_focus {
            let caret_pos = caret.get_position();
            if font_w > 0 && font_h > 0 && size.0 > 0 && size.1 > 0 {
                let line_bytes = (size.0 as usize) * 4;
                let cell_x = caret_pos.x;
                let cell_y = caret_pos.y;
                if cell_x >= 0 && cell_y >= 0 {
                    let px_x = (cell_x as usize) * font_w;
                    let px_y = (cell_y as usize) * font_h;
                    if px_x + font_w <= size.0 as usize && px_y + font_h <= size.1 as usize {
                        match caret.shape() {
                            CaretShape::Bar => {
                                // Draw a vertical bar on the left edge of the character cell
                                let bar_width = 2.min(font_w); // 2 pixels wide or font width if smaller
                                for row in 0..font_h {
                                    let row_offset = (px_y + row) * line_bytes + px_x * 4;
                                    let slice = &mut rgba_data[row_offset..row_offset + bar_width * 4];
                                    for p in slice.chunks_exact_mut(4) {
                                        p[0] = 255 - p[0];
                                        p[1] = 255 - p[1];
                                        p[2] = 255 - p[2];
                                    }
                                }
                            }
                            CaretShape::Block => {
                                for row in 0..font_h {
                                    let row_offset = (px_y + row) * line_bytes + px_x * 4;
                                    let slice = &mut rgba_data[row_offset..row_offset + font_w * 4];
                                    for p in slice.chunks_exact_mut(4) {
                                        p[0] = 255 - p[0];
                                        p[1] = 255 - p[1];
                                        p[2] = 255 - p[2];
                                    }
                                }
                            }
                            CaretShape::Underline => {
                                let start_row = font_h - 2;
                                for row in start_row..font_h {
                                    let row_offset = (px_y + row) * line_bytes + px_x * 4;
                                    let slice = &mut rgba_data[row_offset..row_offset + font_w * 4];
                                    for p in slice.chunks_exact_mut(4) {
                                        p[0] = 255 - p[0];
                                        p[1] = 255 - p[1];
                                        p[2] = 255 - p[2];
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
