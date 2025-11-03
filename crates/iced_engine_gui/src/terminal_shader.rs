use crate::{Blink, Message, MonitorSettings, Terminal, now_ms};
use iced::widget::shader;
use iced::{Element, Rectangle, mouse};
use icy_engine::{Position, Selection, TextPane};

static mut SCALE_FACTOR: f32 = 1.0;

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
    terminal_rgba: Vec<u8>,
    terminal_size: (u32, u32),
    // Store the monitor settings for CRT effects
    monitor_settings: MonitorSettings,
}

impl TerminalShader {
    pub fn new(monitor_settings: MonitorSettings) -> Self {
        Self {
            terminal_rgba: Vec::new(),
            terminal_size: (800, 600),
            monitor_settings,
        }
    }
}

// Renderer struct for GPU resources
#[derive(Debug)]
pub struct TerminalShaderRenderer {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group_layout: iced::wgpu::BindGroupLayout,
    bind_group: Option<iced::wgpu::BindGroup>,
    texture: Option<iced::wgpu::Texture>,
    texture_view: Option<iced::wgpu::TextureView>,
    sampler: iced::wgpu::Sampler,
    uniform_buffer: iced::wgpu::Buffer,
    monitor_color_buffer: iced::wgpu::Buffer,
    texture_size: (u32, u32), // NEW: track current texture dimensions
    renderer_id: u64,
}

static RENDERER_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl shader::Primitive for TerminalShader {
    type Renderer = TerminalShaderRenderer;

    fn initialize(&self, device: &iced::wgpu::Device, _queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self::Renderer {
        let renderer_id = RENDERER_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some(&format!("Terminal CRT Shader {}", renderer_id)),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("shaders/crt.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("Terminal Shader Bind Group Layout {}", renderer_id)),
            entries: &[
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

        // Choose sampler filtering based on pixel-perfect preference
        let want_pixel_perfect = self.monitor_settings.use_pixel_perfect_scaling;
        let sampler = device.create_sampler(&iced::wgpu::SamplerDescriptor {
            label: Some(&format!("Terminal Texture Sampler {}", renderer_id)),
            address_mode_u: iced::wgpu::AddressMode::ClampToEdge,
            address_mode_v: iced::wgpu::AddressMode::ClampToEdge,
            address_mode_w: iced::wgpu::AddressMode::ClampToEdge,
            mag_filter: if want_pixel_perfect {
                iced::wgpu::FilterMode::Nearest
            } else {
                iced::wgpu::FilterMode::Linear
            },
            min_filter: if want_pixel_perfect {
                iced::wgpu::FilterMode::Nearest
            } else {
                iced::wgpu::FilterMode::Linear
            },
            mipmap_filter: iced::wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some(&format!("Terminal Shader Uniforms {}", renderer_id)),
            size: std::mem::size_of::<CRTUniforms>() as u64,
            usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let monitor_color_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some(&format!("Monitor Color Buffer {}", renderer_id)),
            size: std::mem::size_of::<[f32; 4]>() as u64,
            usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        TerminalShaderRenderer {
            pipeline,
            bind_group_layout,
            bind_group: None,
            texture: None,
            texture_view: None,
            sampler,
            uniform_buffer,
            monitor_color_buffer,
            texture_size: (0, 0),
            renderer_id,
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
        unsafe {
            SCALE_FACTOR = _viewport.scale_factor();
        }

        // Only (re)create texture if size changed or not yet allocated
        let (w, h) = self.terminal_size;
        let need_new_texture = renderer.texture.is_none() || renderer.texture_size.0 != w || renderer.texture_size.1 != h;

        if need_new_texture {
            let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                label: Some(&format!("Terminal Texture {}", renderer.renderer_id)),
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
                label: Some(&format!("Terminal Shader Bind Group {}", renderer.renderer_id)),
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
                        resource: renderer.uniform_buffer.as_entire_binding(),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 3,
                        resource: renderer.monitor_color_buffer.as_entire_binding(),
                    },
                ],
            });

            renderer.texture = Some(texture);
            renderer.texture_view = Some(texture_view);
            renderer.bind_group = Some(bind_group);
            renderer.texture_size = (w, h);
        }

        // Upload new pixel data only if we have something
        if !self.terminal_rgba.is_empty() {
            if let Some(texture) = &renderer.texture {
                queue.write_texture(
                    iced::wgpu::TexelCopyTextureInfo {
                        texture,
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

        let brightness_mul = self.monitor_settings.brightness / 100.0; // 100 -> 1.0
        let contrast_mul = self.monitor_settings.contrast / 100.0; // 100 -> 1.0
        let gamma_val = self.monitor_settings.gamma;
        let saturation_mul = self.monitor_settings.saturation / 100.0; // 100 -> 1.0

        // Curvature values (only active if enabled)
        let use_curv = self.monitor_settings.use_curvature;
        let curv_x = if use_curv { (100.0 - self.monitor_settings.curvature_x) / 10.0 } else { 0.0 };
        let curv_y = if use_curv { (100.0 - self.monitor_settings.curvature_y) / 10.0 } else { 0.0 };
        let enable_curvature = if use_curv { 1.0 } else { 0.0 };

        // Scanline values (only active if enabled)
        let use_scan = self.monitor_settings.use_scanlines;
        let scanline_thickness = if use_scan { self.monitor_settings.scanline_thickness } else { 0.5 };
        let scanline_sharpness = if use_scan { self.monitor_settings.scanline_sharpness } else { 0.5 };
        let scanline_phase = if use_scan { self.monitor_settings.scanline_phase } else { 0.0 };
        let enable_scanlines = if use_scan { 1.0 } else { 0.0 };

        // Noise values (only active if enabled)
        let use_noise = self.monitor_settings.use_noise;
        // Assuming UI noise_level 0..100
        let nl = if use_noise {
            (self.monitor_settings.noise_level / 100.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let enable_noise = if use_noise { 1.0 } else { 0.0 };

        // Bloom - scale UI values (0-100) to shader-appropriate ranges
        let use_bloom = self.monitor_settings.use_bloom;
        let bloom_threshold = if use_bloom {
            // UI: 0-100, where lower = more bloom
            // Shader expects: 0-1, where lower = more bloom
            (self.monitor_settings.bloom_threshold / 100.0).clamp(0.0, 1.0)
        } else {
            1.0 // Threshold of 1.0 = no pixels pass
        };

        let bloom_radius = if use_bloom {
            // UI: 0-100, but shader expects pixels (typically 1-10)
            // Scale down to reasonable pixel radius
            (self.monitor_settings.bloom_radius / 10.0).max(0.5)
        } else {
            0.0
        };

        let bloom_intensity = if use_bloom {
            // UI: 0-100, shader expects multiplier (typically 0.1-2.0)
            // Scale to reasonable intensity range
            (self.monitor_settings.glow_strength / 50.0).max(0.0)
        } else {
            0.0
        };

        let enable_bloom = if use_bloom { 1.0 } else { 0.0 };

        let uniform_data = CRTUniforms {
            time: now_ms() as f32 / 1000.0,
            brightness: brightness_mul,
            contrast: contrast_mul,
            gamma: gamma_val,
            saturation: saturation_mul,
            monitor_type: self.monitor_settings.monitor_type.to_index() as f32,
            resolution: [scaled_w, scaled_h],

            curvature_x: curv_x,
            curvature_y: curv_y,
            enable_curvature,

            scanline_thickness,
            scanline_sharpness,
            scanline_phase,
            enable_scanlines,

            noise_level: nl,
            sync_wobble: if self.monitor_settings.use_noise {
                (self.monitor_settings.sync_wobble / 100.0).clamp(0.0, 1.0)
            } else {
                0.0
            },
            enable_noise,

            bloom_threshold,
            bloom_radius,
            bloom_intensity,
            enable_bloom,
        };

        let uniform_bytes = unsafe { std::slice::from_raw_parts(&uniform_data as *const CRTUniforms as *const u8, std::mem::size_of::<CRTUniforms>()) };
        queue.write_buffer(&renderer.uniform_buffer, 0, uniform_bytes);

        let color_bytes = unsafe { std::slice::from_raw_parts(monitor_color.as_ptr() as *const u8, std::mem::size_of::<[f32; 4]>()) };
        queue.write_buffer(&renderer.monitor_color_buffer, 0, color_bytes);
    }

    fn render(&self, renderer: &Self::Renderer, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        encoder.push_debug_group(&format!("Terminal CRT Shader Render {}", renderer.renderer_id));

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

        // If pixel-perfect, snap to integer pixels
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

        // Calculate scissor rect ensuring it's within clip_bounds
        let sc_x = vp_x.max(0.0).floor() as u32;
        let sc_y = vp_y.max(0.0).floor() as u32;
        let sc_w = vp_w.max(1.0).floor() as u32;
        let sc_h = vp_h.max(1.0).floor() as u32;

        // Clamp scissor rect to clip_bounds to avoid out-of-bounds
        let scissor_x = sc_x.min(clip_bounds.x + clip_bounds.width);
        let scissor_y = sc_y.min(clip_bounds.y + clip_bounds.height);
        let scissor_width = sc_w.min((clip_bounds.x + clip_bounds.width).saturating_sub(scissor_x));
        let scissor_height = sc_h.min((clip_bounds.y + clip_bounds.height).saturating_sub(scissor_y));

        // Only set scissor and viewport if we have valid dimensions
        if scissor_width > 0 && scissor_height > 0 && vp_w > 0.0 && vp_h > 0.0 {
            render_pass.set_scissor_rect(scissor_x, scissor_y, scissor_width, scissor_height);
            render_pass.set_viewport(vp_x, vp_y, vp_w, vp_h, 0.0, 1.0);

            render_pass.set_pipeline(&renderer.pipeline);
            if let Some(bind_group) = &renderer.bind_group {
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.draw(0..3, 0..1);
            }
        }

        drop(render_pass);
        encoder.pop_debug_group();
    }
}
// Program wrapper that renders the terminal and creates the shader
pub struct CRTShaderProgram<'a> {
    term: &'a Terminal,
    monitor_settings: MonitorSettings,
}

impl<'a> CRTShaderProgram<'a> {
    pub fn new(term: &'a Terminal, monitor_settings: MonitorSettings) -> Self {
        Self { term, monitor_settings }
    }
}

pub struct CRTShaderState {
    caret_blink: crate::Blink,
    character_blink: crate::Blink,

    // Mouse/selection tracking
    dragging: bool,
    drag_anchor: Option<Position>,
    last_drag_position: Option<Position>,
    shift_pressed_during_selection: bool,

    // Modifier tracking
    alt_pressed: bool,
    shift_pressed: bool,

    // Hover tracking
    hovered_cell: Option<Position>,
    hovered_link: Option<String>,
    hovered_rip_field: bool,

    last_rendered_size: Option<(u32, u32)>,
}

impl CRTShaderState {
    pub fn reset_caret(&mut self) {
        self.caret_blink.reset();
    }
}

impl Default for CRTShaderState {
    fn default() -> Self {
        Self {
            caret_blink: Blink::new((1000.0 / 1.875) as u128 / 2),
            character_blink: Blink::new((1000.0 / 1.8) as u128),
            dragging: false,
            drag_anchor: None,
            last_drag_position: None,
            shift_pressed_during_selection: false,
            alt_pressed: false,
            shift_pressed: false,
            hovered_cell: None,
            hovered_link: None,
            hovered_rip_field: false,
            last_rendered_size: None,
        }
    }
}

impl<'a> shader::Program<Message> for CRTShaderProgram<'a> {
    type State = CRTShaderState;
    type Primitive = TerminalShader;

    fn draw(&self, state: &Self::State, _cursor: mouse::Cursor, _bounds: Rectangle) -> Self::Primitive {
        let mut rgba_data = Vec::new();
        let size;

        // Local variables to allow caret overlay after lock is released
        let mut caret_pos_opt = None;
        let mut caret_visible = false;
        let mut font_w = 0usize;
        let mut font_h = 0usize;
        let no_scrollback;
        if let Ok(edit_state) = self.term.edit_state.try_lock() {
            no_scrollback = edit_state.scrollback_offset == 0;
            let buffer = edit_state.get_display_buffer();

            // Capture caret & font metrics
            caret_pos_opt = Some(edit_state.get_caret().get_position());
            caret_visible = edit_state.get_caret().is_visible();
            if let Some(font) = buffer.get_font(0) {
                font_w = font.size.width as usize;
                font_h = font.size.height as usize;
            }

            let rect = icy_engine::Rectangle {
                start: icy_engine::Position::new(0, 0),
                size: icy_engine::Size::new(buffer.get_width(), buffer.get_height()),
            };
            let (fg, rg) = edit_state.get_buffer().buffer_type.get_selection_colors();

            // Pass blink_on to actually animate ANSI blinking attributes
            let (img_size, data) = buffer.render_to_rgba(&icy_engine::RenderOptions {
                rect,
                blink_on: state.character_blink.is_on(),
                selection: edit_state.get_selection(),
                selection_fg: Some(fg),
                selection_bg: Some(rg),
            });
            size = (img_size.width as u32, img_size.height as u32);
            rgba_data = data;
        } else {
            // IMPORTANT: Use a consistent fallback size instead of bounds
            // This prevents size oscillation when the lock fails
            if let Some(last_size) = state.last_rendered_size {
                size = last_size;
            } else {
                // Initial fallback - use standard terminal size
                size = (640, 400); // 80x25 with 8x16 font
            }
            no_scrollback = true;
        }

        // Caret overlay only if we have the metrics & want it visible this phase
        if state.caret_blink.is_on() && no_scrollback && caret_visible {
            if let Some(caret_pos) = caret_pos_opt {
                if font_w > 0 && font_h > 0 && size.0 > 0 && size.1 > 0 {
                    let line_bytes = (size.0 as usize) * 4;

                    let cell_x = caret_pos.x;
                    let cell_y = caret_pos.y;
                    if cell_x >= 0 && cell_y >= 0 {
                        let px_x = (cell_x as usize) * font_w;
                        let px_y = (cell_y as usize) * font_h;

                        if px_x + font_w <= size.0 as usize && px_y + font_h <= size.1 as usize {
                            // ===== DOS-like inverted caret =====
                            let style = CaretStyle::FullBlock; // Change this to control caret size
                            let caret_rows = style.rows(font_h);
                            let start_row = font_h - caret_rows;

                            // Invert the colors in the caret area
                            for row in start_row..font_h {
                                let row_offset = (px_y + row) * line_bytes + px_x * 4;
                                let slice = &mut rgba_data[row_offset..row_offset + font_w * 4];

                                // XOR-style color inversion for each pixel
                                for p in slice.chunks_exact_mut(4) {
                                    // Invert RGB components, preserve alpha
                                    p[0] = 255 - p[0]; // Invert Red
                                    p[1] = 255 - p[1]; // Invert Green
                                    p[2] = 255 - p[2]; // Invert Blue
                                    // p[3] unchanged (keep original alpha)
                                }
                            }
                        }
                    }
                }
            }
        }

        TerminalShader {
            terminal_rgba: rgba_data,
            terminal_size: size,
            monitor_settings: self.monitor_settings.clone(),
        }
    }

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<iced::widget::Action<Message>> {
        let mut needs_redraw = false;
        let now = crate::Blink::now_ms();

        // Update blink timers
        if state.caret_blink.update(now) {
            needs_redraw = true;
        }
        if state.character_blink.update(now) {
            needs_redraw = true;
        }

        // Track the actual rendered size to detect real changes
        // Only update if we successfully get the lock
        if let Ok(edit_state) = self.term.edit_state.try_lock() {
            let buffer = edit_state.get_display_buffer();
            if let Some(font) = buffer.get_font(0) {
                let font_w = font.size.width as u32;
                let font_h = font.size.height as u32;
                let current_size = (buffer.get_width() as u32 * font_w, buffer.get_height() as u32 * font_h);

                // Only trigger redraw if size actually changed
                if state.last_rendered_size != Some(current_size) {
                    state.last_rendered_size = Some(current_size);
                    needs_redraw = true;
                }
            }
        }
        // If we can't get the lock, keep the last known size to prevent oscillation

        // ...rest of the update method stays the same...
        // Track modifier keys
        if let iced::Event::Keyboard(kbd_event) = event {
            match kbd_event {
                iced::keyboard::Event::ModifiersChanged(mods) => {
                    state.alt_pressed = mods.alt();
                    state.shift_pressed = mods.shift();
                }
                _ => {}
            }
        }

        // Handle mouse events
        if let iced::Event::Mouse(mouse_event) = event {
            match mouse_event {
                mouse::Event::CursorMoved { .. } => {
                    if let Some(position) = cursor.position() {
                        let cell_pos = map_mouse_to_cell(self.term, &self.monitor_settings, bounds, position.x, position.y);
                        state.hovered_cell = cell_pos;

                        // Check for hyperlinks
                        if let Some(cell) = cell_pos {
                            if let Ok(edit_state) = self.term.edit_state.try_lock() {
                                let buffer = edit_state.get_display_buffer();

                                // Check hyperlinks
                                let mut found_link = None;
                                for hyperlink in buffer.layers[0].hyperlinks() {
                                    if buffer.is_position_in_range(cell, hyperlink.position, hyperlink.length) {
                                        found_link = Some(hyperlink.get_url(buffer));
                                        break;
                                    }
                                }

                                if state.hovered_link != found_link {
                                    state.hovered_link = found_link;
                                    needs_redraw = true;
                                }

                                // TODO: Check RIP fields when available
                                // if self.term.use_rip {
                                //     check RIP mouse fields
                                // }
                            }
                        } else {
                            if state.hovered_link.is_some() {
                                state.hovered_link = None;
                                needs_redraw = true;
                            }
                        }

                        // Handle dragging for selection
                        if state.dragging {
                            if let Some(cell) = cell_pos {
                                if state.last_drag_position != Some(cell) {
                                    state.last_drag_position = Some(cell);
                                    if let Ok(mut edit_state) = self.term.edit_state.try_lock() {
                                        // Update selection
                                        if let Some(mut sel) = edit_state.get_selection().clone() {
                                            if !sel.locked {
                                                sel.lead = cell;
                                                sel.shape = if state.alt_pressed {
                                                    icy_engine::Shape::Rectangle
                                                } else {
                                                    icy_engine::Shape::Lines
                                                };
                                                let _ = edit_state.set_selection(sel);
                                            }
                                        }
                                    }
                                    needs_redraw = true;
                                }
                            }
                        }
                    }
                }

                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if let Some(position) = cursor.position() {
                        if let Some(cell) = map_mouse_to_cell(self.term, &self.monitor_settings, bounds, position.x, position.y) {
                            // Check if clicking on a hyperlink
                            if let Some(url) = &state.hovered_link {
                                return Some(iced::widget::Action::publish(Message::OpenLink(url.clone())));
                            }

                            // TODO: Handle RIP field clicks
                            // if self.term.use_rip && state.hovered_rip_field {
                            //     handle RIP command
                            // }

                            // Start selection
                            if let Ok(mut edit_state) = self.term.edit_state.try_lock() {
                                // Clear existing selection unless shift is held
                                if !state.shift_pressed {
                                    let _ = edit_state.clear_selection();
                                }

                                // Create new selection
                                let mut sel = Selection::new(cell);
                                sel.shape = if state.alt_pressed {
                                    icy_engine::Shape::Rectangle
                                } else {
                                    icy_engine::Shape::Lines
                                };
                                sel.locked = false;
                                let _ = edit_state.set_selection(sel);

                                state.dragging = true;
                                state.drag_anchor = Some(cell);
                                state.last_drag_position = Some(cell);
                                needs_redraw = true;
                            }
                        } else {
                            // Clicked outside terminal area - clear selection
                            if let Ok(mut edit_state) = self.term.edit_state.try_lock() {
                                let _ = edit_state.clear_selection();
                                needs_redraw = true;
                            }
                        }
                    }
                }

                mouse::Event::ButtonReleased(mouse::Button::Left) => {
                    if state.dragging {
                        state.dragging = false;
                        state.shift_pressed_during_selection = state.shift_pressed;

                        // Lock the selection
                        if let Ok(mut edit_state) = self.term.edit_state.try_lock() {
                            if let Some(mut sel) = edit_state.get_selection().clone() {
                                sel.locked = true;
                                let _ = edit_state.set_selection(sel);
                            }
                        }

                        state.drag_anchor = None;
                        state.last_drag_position = None;
                        needs_redraw = true;
                    }
                }

                mouse::Event::ButtonPressed(mouse::Button::Middle) => {
                    // Middle click to copy (if you want this feature)
                    return Some(iced::widget::Action::publish(Message::Copy));
                }

                mouse::Event::WheelScrolled { delta } => {
                    match delta {
                        mouse::ScrollDelta::Lines { y, .. } => {
                            let lines = -(*y as i32); // Negative for natural scrolling
                            return Some(iced::widget::Action::publish(Message::Scroll(lines)));
                        }
                        mouse::ScrollDelta::Pixels { y, .. } => {
                            let lines = -((*y / 20.0) as i32); // Convert pixels to lines
                            if lines != 0 {
                                return Some(iced::widget::Action::publish(Message::Scroll(lines)));
                            }
                        }
                    }
                }

                _ => {}
            }
        }

        if needs_redraw { Some(iced::widget::Action::request_redraw()) } else { None }
    }

    fn mouse_interaction(&self, state: &Self::State, _bounds: Rectangle, _cursor: mouse::Cursor) -> mouse::Interaction {
        if state.hovered_link.is_some() || state.hovered_rip_field {
            mouse::Interaction::Pointer
        } else if state.dragging {
            mouse::Interaction::Crosshair
        } else if state.hovered_cell.is_some() {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::default()
        }
    }
}

// Helper function to create shader with terminal and monitor settings
pub fn create_crt_shader<'a>(term: &'a Terminal, monitor_settings: MonitorSettings) -> Element<'a, Message> {
    // Let the parent wrapper decide sizing; shader can just be Fill.
    shader(CRTShaderProgram::new(term, monitor_settings))
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .into()
}

#[derive(Clone, Copy)]
pub enum CaretStyle {
    FullBlock,
    HalfBlock,
    QuarterBlock,
    Underline,
}

impl CaretStyle {
    fn rows(self, font_h: usize) -> usize {
        match self {
            CaretStyle::FullBlock => font_h,
            CaretStyle::HalfBlock => (font_h / 2).max(1),
            CaretStyle::QuarterBlock => (font_h / 4).max(1),
            CaretStyle::Underline => 2.min(font_h),
        }
    }
}

fn map_mouse_to_cell(
    term: &Terminal,
    monitor: &MonitorSettings,
    logical_bounds: Rectangle, // bounds passed to update()/draw() (logical coordinates)
    logical_mx: f32,           // mouse x in logical space
    logical_my: f32,           // mouse y in logical space
) -> Option<Position> {
    // 1. Obtain scale factor (logical -> physical)
    // Prefer querying window; fallback to stored SCALE_FACTOR if needed.
    let scale_factor = unsafe { SCALE_FACTOR };

    // 2. Promote logical coordinates to physical pixels
    let phys_bounds_x: f32 = logical_bounds.x * scale_factor;
    let phys_bounds_y = logical_bounds.y * scale_factor;
    let phys_bounds_w = logical_bounds.width * scale_factor;
    let phys_bounds_h = logical_bounds.height * scale_factor;

    let phys_mx = logical_mx * scale_factor;
    let phys_my = logical_my * scale_factor;

    // 3. Lock edit state & obtain font + buffer size (already in pixel units)
    let edit = term.edit_state.try_lock().ok()?;
    let buffer = edit.get_display_buffer();
    let font = buffer.get_font(0)?;
    let font_w = font.size.width as f32;
    let font_h = font.size.height as f32;
    if font_w <= 0.0 || font_h <= 0.0 {
        return None;
    }

    let term_px_w = buffer.get_width() as f32 * font_w;
    let term_px_h = buffer.get_height() as f32 * font_h;
    if term_px_w <= 0.0 || term_px_h <= 0.0 {
        return None;
    }

    // 4. Aspect-fit scale in PHYSICAL space (match render())
    let avail_w = phys_bounds_w.max(1.0);
    let avail_h = phys_bounds_h.max(1.0);
    let uniform_scale = (avail_w / term_px_w).min(avail_h / term_px_h);

    let use_pp = monitor.use_pixel_perfect_scaling;
    let display_scale = if use_pp { uniform_scale.floor().max(1.0) } else { uniform_scale };

    let scaled_w = term_px_w * display_scale;
    let scaled_h = term_px_h * display_scale;

    // 5. Center terminal inside physical bounds (same as render())
    let offset_x = phys_bounds_x + (avail_w - scaled_w) / 2.0;
    let offset_y = phys_bounds_y + (avail_h - scaled_h) / 2.0;

    // 6. Pixel-perfect rounding (only position & size used for viewport clipping)
    let (vp_x, vp_y, vp_w, vp_h) = if use_pp {
        (offset_x.round(), offset_y.round(), scaled_w.round(), scaled_h.round())
    } else {
        (offset_x, offset_y, scaled_w, scaled_h)
    };

    // 7. Hit test in physical viewport
    if phys_mx < vp_x || phys_my < vp_y || phys_mx >= vp_x + vp_w || phys_my >= vp_y + vp_h {
        return None;
    }

    // 8. Undo scaling using display_scale, not viewport width ratios
    let local_px_x = (phys_mx - vp_x) / display_scale;
    let local_px_y = (phys_my - vp_y) / display_scale;

    // 9. Convert to cell indices
    let cx = (local_px_x / font_w).floor() as i32;
    let cy = (local_px_y / font_h).floor() as i32;

    if cx < 0 || cy < 0 || cx >= buffer.get_width() as i32 || cy >= buffer.get_height() as i32 {
        return None;
    }

    Some(Position::new(cx, cy))
}
