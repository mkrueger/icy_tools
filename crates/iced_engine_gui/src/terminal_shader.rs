use crate::{Blink, Message, MonitorSettings, MonitorType, Terminal, now_ms};
use iced::widget::shader;
use iced::{Element, Rectangle, mouse};
use icy_engine::TextPane;

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
    _pad: [f32; 4], // padding -> total floats: 12 (48 bytes)
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
}

impl shader::Primitive for TerminalShader {
    type Renderer = TerminalShaderRenderer;

    fn initialize(&self, device: &iced::wgpu::Device, _queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self::Renderer {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("Terminal CRT Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("shaders/crt.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("Terminal Shader Bind Group Layout"),
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
            label: Some("Terminal Shader Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some("Terminal Shader Pipeline"),
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
            label: Some("Terminal Texture Sampler"),
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
            label: Some("Terminal Shader Uniforms"),
            size: std::mem::size_of::<CRTUniforms>() as u64,
            usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let monitor_color_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("Monitor Color Buffer"),
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
        if renderer.texture.is_none() || !self.terminal_rgba.is_empty() {
            let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                label: Some("Terminal Texture"),
                size: iced::wgpu::Extent3d {
                    width: self.terminal_size.0.max(1),
                    height: self.terminal_size.1.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: iced::wgpu::TextureDimension::D2,
                format: iced::wgpu::TextureFormat::Rgba8Unorm,
                usage: iced::wgpu::TextureUsages::TEXTURE_BINDING | iced::wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            if !self.terminal_rgba.is_empty() {
                use iced::wgpu::util::DeviceExt;
                let temp_texture = device.create_texture_with_data(
                    queue,
                    &iced::wgpu::TextureDescriptor {
                        label: Some("Terminal Texture Data"),
                        size: iced::wgpu::Extent3d {
                            width: self.terminal_size.0.max(1),
                            height: self.terminal_size.1.max(1),
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: iced::wgpu::TextureDimension::D2,
                        format: iced::wgpu::TextureFormat::Rgba8Unorm,
                        usage: iced::wgpu::TextureUsages::TEXTURE_BINDING | iced::wgpu::TextureUsages::COPY_SRC,
                        view_formats: &[],
                    },
                    iced::wgpu::util::TextureDataOrder::LayerMajor,
                    &self.terminal_rgba,
                );

                let mut encoder = device.create_command_encoder(&iced::wgpu::CommandEncoderDescriptor {
                    label: Some("Terminal Texture Copy"),
                });

                encoder.copy_texture_to_texture(
                    temp_texture.as_image_copy(),
                    texture.as_image_copy(),
                    iced::wgpu::Extent3d {
                        width: self.terminal_size.0.max(1),
                        height: self.terminal_size.1.max(1),
                        depth_or_array_layers: 1,
                    },
                );

                queue.submit(Some(encoder.finish()));
            }

            let texture_view = texture.create_view(&iced::wgpu::TextureViewDescriptor::default());

            let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                label: Some("Terminal Shader Bind Group"),
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

        let uniform_data = CRTUniforms {
            time: now_ms() as f32 / 1000.0,
            brightness: brightness_mul,
            contrast: contrast_mul,
            gamma: gamma_val,
            saturation: saturation_mul,
            monitor_type: self.monitor_settings.monitor_type.to_index() as f32,
            resolution: [scaled_w, scaled_h],
            _pad: [0.0; 4],
        };

        let uniform_bytes = unsafe { std::slice::from_raw_parts(&uniform_data as *const CRTUniforms as *const u8, std::mem::size_of::<CRTUniforms>()) };
        queue.write_buffer(&renderer.uniform_buffer, 0, uniform_bytes);

        let color_bytes = unsafe { std::slice::from_raw_parts(monitor_color.as_ptr() as *const u8, std::mem::size_of::<[f32; 4]>()) };
        queue.write_buffer(&renderer.monitor_color_buffer, 0, color_bytes);
    }

    fn render(&self, renderer: &Self::Renderer, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        encoder.push_debug_group("Terminal CRT Shader Render");

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
        }
    }
}

impl<'a> shader::Program<Message> for CRTShaderProgram<'a> {
    type State = CRTShaderState;
    type Primitive = TerminalShader;

    fn draw(&self, _state: &Self::State, _cursor: mouse::Cursor, bounds: Rectangle) -> Self::Primitive {
        let mut rgba_data = Vec::new();
        let size;

        // Local variables to allow caret overlay after lock is released
        let mut caret_pos_opt = None;
        let mut font_w = 0usize;
        let mut font_h = 0usize;

        if let Ok(edit_state) = self.term.edit_state.try_lock() {
            let buffer = edit_state.get_buffer();

            // Capture caret & font metrics
            caret_pos_opt = Some(edit_state.get_caret().get_position());
            if let Some(font) = buffer.get_font(0) {
                font_w = font.size.width as usize;
                font_h = font.size.height as usize;
            }

            let rect = icy_engine::Rectangle {
                start: icy_engine::Position::new(0, 0),
                size: icy_engine::Size::new(buffer.get_width(), buffer.get_height()),
            };

            // Pass blink_on to actually animate ANSI blinking attributes
            let (img_size, data) = buffer.render_to_rgba(rect, _state.character_blink.is_on());
            size = (img_size.width as u32, img_size.height as u32);
            rgba_data = data;
        } else {
            size = (bounds.width as u32, bounds.height as u32);
        }

        // Caret overlay only if we have the metrics & want it visible this phase
        if _state.caret_blink.is_on() {
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

    fn update(&self, state: &mut Self::State, _event: &iced::Event, _bounds: Rectangle, _cursor: mouse::Cursor) -> Option<iced::widget::Action<Message>> {
        let mut needs_redraw = false;
        let now = crate::Blink::now_ms();

        // Update caret blink
        if state.caret_blink.update(now) {
            needs_redraw = true;
        }

        // Update character blink
        if state.character_blink.update(now) {
            needs_redraw = true;
        }

        if needs_redraw { Some(iced::widget::Action::request_redraw()) } else { None }
    }

    fn mouse_interaction(&self, _state: &Self::State, _bounds: Rectangle, _cursor: mouse::Cursor) -> mouse::Interaction {
        mouse::Interaction::default()
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
