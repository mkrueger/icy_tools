//! GPU-accelerated Color Switcher with animations
//!
//! Features:
//! - Smooth swap animation between foreground/background colors
//! - Drop shadows for depth
//! - Swap icon rendering via shader
//! - Hover effects

use iced::{
    Element, Length, Rectangle, mouse,
    widget::shader::{self, Shader},
};
use icy_engine::{Palette, TextAttribute};
use std::time::Instant;

use super::constants::{
    COLOR_SWAP_ANIMATION_DURATION, COLOR_SWITCHER_RECT_SIZE, COLOR_SWITCHER_SHADOW_MARGIN, COLOR_SWITCHER_SIZE, COLOR_SWITCHER_SWAP_ICON_SIZE,
};

/// Size of the color switcher widget (re-export for external use)
pub const SWITCHER_SIZE: f32 = COLOR_SWITCHER_SIZE;

/// Messages from the color switcher
#[derive(Clone, Debug)]
pub enum ColorSwitcherMessage {
    /// Swap foreground and background colors (starts animation)
    SwapColors,
    /// Reset to default colors (white on black)
    ResetToDefault,
    /// Animation tick
    Tick(f32),
    /// Animation completed - now actually swap the colors
    AnimationComplete,
}

/// Uniforms for the shader (must match WGSL struct layout)
#[repr(C, align(16))]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ColorSwitcherUniforms {
    foreground_color: [f32; 4],
    background_color: [f32; 4],
    default_fg_color: [f32; 4], // DOS color 7 (light gray)
    swap_progress: f32,
    time: f32,
    widget_size: [f32; 2],
    hover_swap: f32,
    hover_default: f32,
    rect_size: f32,
    shadow_margin: f32,
}

/// Color switcher state
pub struct ColorSwitcher {
    /// Current foreground color index
    foreground: u32,
    /// Current background color index
    background: u32,
    /// Cached palette for rendering
    cached_palette: Palette,
    /// Animation progress (0.0 = normal, 1.0 = swapped)
    swap_progress: f32,
    /// Animation direction (true = swapping, false = returning)
    animating: bool,
    /// Animation completed, waiting for actual color swap
    pending_swap: bool,
    /// Time accumulator for animation
    animation_time: f32,
    /// Hover state for swap area
    hover_swap: bool,
    /// Hover state for default colors area
    hover_default: bool,
    /// Time for effects
    time: f32,
}

impl Default for ColorSwitcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ColorSwitcher {
    pub fn new() -> Self {
        Self {
            foreground: 7,
            background: 0,
            cached_palette: Palette::dos_default(),
            swap_progress: 0.0,
            animating: false,
            pending_swap: false,
            animation_time: 0.0,
            hover_swap: false,
            hover_default: false,
            time: 0.0,
        }
    }

    /// Update colors from attribute
    pub fn set_from_attribute(&mut self, attr: &TextAttribute) {
        self.foreground = attr.foreground();
        self.background = attr.background();
    }

    /// Get foreground color
    pub fn foreground(&self) -> u32 {
        self.foreground
    }

    /// Get background color
    pub fn background(&self) -> u32 {
        self.background
    }

    /// Sync palette from edit state
    pub fn sync_palette(&mut self, palette: &Palette) {
        self.cached_palette = palette.clone();
    }

    /// Start swap animation
    pub fn start_swap_animation(&mut self) {
        self.animating = true;
        self.animation_time = 0.0;
    }

    /// Check if animation is running
    pub fn is_animating(&self) -> bool {
        self.animating
    }

    /// Update animation state
    /// Returns true if animation just completed (time to swap colors)
    pub fn tick(&mut self, delta_seconds: f32) -> bool {
        self.time += delta_seconds;

        if self.animating {
            self.animation_time += delta_seconds;
            let progress = (self.animation_time / COLOR_SWAP_ANIMATION_DURATION).min(1.0);

            // Use smooth easing
            self.swap_progress = progress * progress * (3.0 - 2.0 * progress);

            if progress >= 1.0 {
                self.animating = false;
                self.pending_swap = true;
                // Keep swap_progress at 1.0 until confirm_swap() is called
                self.swap_progress = 1.0;
                return true; // Animation complete!
            }
        }
        false
    }

    /// Call this after the colors have been actually swapped
    /// Resets the visual state to normal (since colors are now swapped, progress 0 = correct display)
    pub fn confirm_swap(&mut self) {
        self.pending_swap = false;
        self.swap_progress = 0.0;
    }

    /// Render the color switcher
    pub fn view(&self, foreground: u32, background: u32) -> Element<'_, ColorSwitcherMessage> {
        let (fg_r, fg_g, fg_b) = self.cached_palette.rgb(foreground);
        let (bg_r, bg_g, bg_b) = self.cached_palette.rgb(background);
        // DOS color 7 is the default foreground (light gray)
        let (def_fg_r, def_fg_g, def_fg_b) = self.cached_palette.rgb(7);

        Shader::new(ColorSwitcherProgram {
            foreground_color: [fg_r as f32 / 255.0, fg_g as f32 / 255.0, fg_b as f32 / 255.0, 1.0],
            background_color: [bg_r as f32 / 255.0, bg_g as f32 / 255.0, bg_b as f32 / 255.0, 1.0],
            default_fg_color: [def_fg_r as f32 / 255.0, def_fg_g as f32 / 255.0, def_fg_b as f32 / 255.0, 1.0],
            swap_progress: self.swap_progress,
            time: self.time,
            animating: self.animating,
            hover_swap: self.hover_swap,
            hover_default: self.hover_default,
        })
        .width(Length::Fixed(SWITCHER_SIZE))
        .height(Length::Fixed(SWITCHER_SIZE))
        .into()
    }
}

/// Shader program for rendering the color switcher
#[derive(Debug, Clone)]
struct ColorSwitcherProgram {
    foreground_color: [f32; 4],
    background_color: [f32; 4],
    default_fg_color: [f32; 4],
    swap_progress: f32,
    time: f32,
    animating: bool,
    hover_swap: bool,
    hover_default: bool,
}

#[derive(Debug, Default, Clone, Copy)]
struct ColorSwitcherProgramState {
    last_redraw: Option<Instant>,
}

impl shader::Program<ColorSwitcherMessage> for ColorSwitcherProgram {
    type State = ColorSwitcherProgramState;
    type Primitive = ColorSwitcherPrimitive;

    fn draw(&self, _state: &Self::State, _cursor: mouse::Cursor, _bounds: Rectangle) -> Self::Primitive {
        ColorSwitcherPrimitive {
            foreground_color: self.foreground_color,
            background_color: self.background_color,
            default_fg_color: self.default_fg_color,
            swap_progress: self.swap_progress,
            time: self.time,
            hover_swap: self.hover_swap,
            hover_default: self.hover_default,
        }
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<iced::widget::Action<ColorSwitcherMessage>> {
        let size = SWITCHER_SIZE;
        let rect_size = size * 0.618;

        if let iced::Event::Window(iced::window::Event::RedrawRequested(now)) = event {
            if self.animating {
                let delta = state.last_redraw.map_or(0.0, |prev| now.saturating_duration_since(prev).as_secs_f32());
                state.last_redraw = Some(*now);
                return Some(iced::widget::Action::publish(ColorSwitcherMessage::Tick(delta)));
            } else {
                state.last_redraw = None;
            }
        }

        if let iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            if let Some(pos) = cursor.position_in(bounds) {
                // Check if clicked on swap area (top-right)
                if pos.x > rect_size && pos.y < rect_size {
                    return Some(iced::widget::Action::publish(ColorSwitcherMessage::SwapColors));
                }

                // Check if clicked on default colors area (bottom-left)
                if pos.x < rect_size && pos.y > rect_size {
                    return Some(iced::widget::Action::publish(ColorSwitcherMessage::ResetToDefault));
                }
            }
        }

        None
    }
}

/// Shader primitive for GPU rendering
#[derive(Debug, Clone)]
struct ColorSwitcherPrimitive {
    foreground_color: [f32; 4],
    background_color: [f32; 4],
    default_fg_color: [f32; 4],
    swap_progress: f32,
    time: f32,
    hover_swap: bool,
    hover_default: bool,
}

impl shader::Primitive for ColorSwitcherPrimitive {
    type Pipeline = ColorSwitcherRenderer;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        _device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        bounds: &Rectangle,
        viewport: &iced::advanced::graphics::Viewport,
    ) {
        let scale = viewport.scale_factor();
        let uniforms = ColorSwitcherUniforms {
            foreground_color: self.foreground_color,
            background_color: self.background_color,
            default_fg_color: self.default_fg_color,
            swap_progress: self.swap_progress,
            time: self.time,
            widget_size: [bounds.width * scale, bounds.height * scale],
            hover_swap: if self.hover_swap { 1.0 } else { 0.0 },
            hover_default: if self.hover_default { 1.0 } else { 0.0 },
            rect_size: COLOR_SWITCHER_RECT_SIZE * scale,
            shadow_margin: COLOR_SWITCHER_SHADOW_MARGIN * scale,
        };

        queue.write_buffer(&pipeline.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        let mut render_pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
            label: Some("Color Switcher Render Pass"),
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

        let vp_x = clip_bounds.x as f32;
        let vp_y = clip_bounds.y as f32;
        let vp_w = clip_bounds.width as f32;
        let vp_h = clip_bounds.height as f32;

        if clip_bounds.width > 0 && clip_bounds.height > 0 {
            render_pass.set_scissor_rect(clip_bounds.x, clip_bounds.y, clip_bounds.width, clip_bounds.height);
            render_pass.set_viewport(vp_x, vp_y, vp_w, vp_h, 0.0, 1.0);
            render_pass.set_pipeline(&pipeline.pipeline);
            render_pass.set_bind_group(0, &pipeline.bind_group, &[]);
            render_pass.draw(0..6, 0..1);
        }
    }
}

/// GPU renderer/pipeline for the color switcher
pub struct ColorSwitcherRenderer {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group: iced::wgpu::BindGroup,
    uniform_buffer: iced::wgpu::Buffer,
    #[allow(dead_code)]
    swap_icon_texture: iced::wgpu::Texture,
}

impl shader::Pipeline for ColorSwitcherRenderer {
    fn new(device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        // Load and compile shader
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("Color Switcher Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("color_switcher_shader.wgsl").into()),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("Color Switcher Uniform Buffer"),
            size: std::mem::size_of::<ColorSwitcherUniforms>() as u64,
            usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Load swap icon SVG and render to texture
        let (swap_icon_texture, swap_icon_view) = create_swap_icon_texture(device, queue);

        // Create sampler for the swap icon
        let sampler = device.create_sampler(&iced::wgpu::SamplerDescriptor {
            label: Some("Swap Icon Sampler"),
            mag_filter: iced::wgpu::FilterMode::Linear,
            min_filter: iced::wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create bind group layout with uniform + texture + sampler
        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("Color Switcher Bind Group Layout"),
            entries: &[
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: iced::wgpu::ShaderStages::FRAGMENT,
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
            label: Some("Color Switcher Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                iced::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                iced::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: iced::wgpu::BindingResource::TextureView(&swap_icon_view),
                },
                iced::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: iced::wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&iced::wgpu::PipelineLayoutDescriptor {
            label: Some("Color Switcher Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some("Color Switcher Pipeline"),
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
            swap_icon_texture,
        }
    }
}

/// Create swap icon texture from embedded SVG
fn create_swap_icon_texture(device: &iced::wgpu::Device, queue: &iced::wgpu::Queue) -> (iced::wgpu::Texture, iced::wgpu::TextureView) {
    // Render the SVG to RGBA pixels
    const ICON_SIZE: u32 = COLOR_SWITCHER_SWAP_ICON_SIZE;

    // Use resvg to render the SVG
    let svg_data = include_bytes!("../../../data/icons/swap.svg");
    let icon_rgba = render_svg_to_rgba(svg_data, ICON_SIZE, ICON_SIZE);

    let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
        label: Some("Swap Icon Texture"),
        size: iced::wgpu::Extent3d {
            width: ICON_SIZE,
            height: ICON_SIZE,
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
        &icon_rgba,
        iced::wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(ICON_SIZE * 4),
            rows_per_image: Some(ICON_SIZE),
        },
        iced::wgpu::Extent3d {
            width: ICON_SIZE,
            height: ICON_SIZE,
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&iced::wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Render SVG data to RGBA pixels
fn render_svg_to_rgba(svg_data: &[u8], width: u32, height: u32) -> Vec<u8> {
    // Try to parse and render the SVG using resvg
    let opt = usvg::Options::default();

    if let Ok(tree) = usvg::Tree::from_data(svg_data, &opt) {
        let size = tiny_skia::IntSize::from_wh(width, height).unwrap_or(tiny_skia::IntSize::from_wh(24, 24).unwrap());
        let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();

        // Calculate scale to fit
        let tree_size = tree.size();
        let scale_x = width as f32 / tree_size.width();
        let scale_y = height as f32 / tree_size.height();
        let scale = scale_x.min(scale_y);

        let transform = tiny_skia::Transform::from_scale(scale, scale);
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        return pixmap.take();
    }

    // Fallback: create a simple arrow pattern if SVG parsing fails
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            let dx = x as i32 - (width / 2) as i32;
            let dy = y as i32 - (height / 2) as i32;

            // Simple diagonal arrow
            let on_line = (dx + dy).abs() < 2 && dx.abs() < (width as i32 / 3) && dy.abs() < (height as i32 / 3);
            if on_line {
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = 255;
            }
        }
    }
    rgba
}
