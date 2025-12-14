//! Shared GPU Glyph Renderer
//!
//! Provides a reusable GPU-accelerated glyph rendering system based on a
//! 16x16 CP437 glyph atlas. Used by FKey-Toolbar and SegmentedControl for
//! crisp, pixel-perfect character rendering.

use codepages::tables::CP437_TO_UNICODE;
use iced::wgpu::util::DeviceExt;
use iced::{Color, Rectangle};
use icy_engine::BitFont;

// ═══════════════════════════════════════════════════════════════════════════
// Uniform and Instance Structures
// ═══════════════════════════════════════════════════════════════════════════

/// Uniforms for the glyph shader
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GlyphUniforms {
    pub clip_size: [f32; 2],
    pub atlas_size: [f32; 2],
    pub glyph_size: [f32; 2],
    pub _pad: [f32; 2],
}

unsafe impl bytemuck::Pod for GlyphUniforms {}
unsafe impl bytemuck::Zeroable for GlyphUniforms {}

/// Quad vertex for instanced glyph rendering
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct QuadVertex {
    pub unit_pos: [f32; 2],
    pub unit_uv: [f32; 2],
}

unsafe impl bytemuck::Pod for QuadVertex {}
unsafe impl bytemuck::Zeroable for QuadVertex {}

/// Per-instance data for a single glyph
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GlyphInstance {
    /// Position in clip-space pixels (top-left)
    pub pos: [f32; 2],
    /// Size in pixels (width, height)
    pub size: [f32; 2],
    /// Foreground color (RGBA)
    pub fg: [f32; 4],
    /// Background color (RGBA)
    pub bg: [f32; 4],
    /// Glyph index (0-255 for CP437)
    pub glyph: u32,
    /// Flags: bit 1 = draw bg, bit 2 = bg only, bit 3 = left arrow, bit 4 = right arrow
    pub flags: u32,
    pub _pad: [u32; 2],
}

unsafe impl bytemuck::Pod for GlyphInstance {}
unsafe impl bytemuck::Zeroable for GlyphInstance {}

// Flag constants
pub const FLAG_DRAW_BG: u32 = 1;
pub const FLAG_BG_ONLY: u32 = 2;
pub const FLAG_ARROW_LEFT: u32 = 4;
pub const FLAG_ARROW_RIGHT: u32 = 8;

// ═══════════════════════════════════════════════════════════════════════════
// Atlas Generation
// ═══════════════════════════════════════════════════════════════════════════

/// Generate a unique key for the given font for atlas caching
pub fn font_key(font: &BitFont) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    font.name().hash(&mut hasher);
    let size = font.size();
    size.width.hash(&mut hasher);
    size.height.hash(&mut hasher);
    font.is_default().hash(&mut hasher);
    hasher.finish()
}

/// Build a 16x16 glyph atlas texture (256 glyphs) from a BitFont.
/// Returns (atlas_width, atlas_height, rgba_data).
pub fn build_glyph_atlas_rgba(font: &BitFont) -> (u32, u32, Vec<u8>) {
    let size = font.size();
    let gw = size.width.max(1) as u32;
    let gh = size.height.max(1) as u32;
    let atlas_w = gw * 16;
    let atlas_h = gh * 16;
    let mut rgba = vec![0u8; (atlas_w * atlas_h * 4) as usize];

    for code in 0u32..256u32 {
        // Try both CP437 slot and Unicode lookup
        let slot_ch = char::from_u32(code).unwrap_or(' ');
        let unicode_ch = CP437_TO_UNICODE.get(code as usize).copied().unwrap_or(' ');
        let col = (code % 16) as u32;
        let row = (code / 16) as u32;
        let base_x = col * gw;
        let base_y = row * gh;

        if let Some(glyph) = font.glyph(slot_ch).or_else(|| font.glyph(unicode_ch)) {
            for y in 0..gh as usize {
                let dst_y = base_y as usize + y;
                if dst_y >= atlas_h as usize {
                    continue;
                }
                let src_row = glyph.bitmap.pixels.get(y);
                for x in 0..gw as usize {
                    let dst_x = base_x as usize + x;
                    if dst_x >= atlas_w as usize {
                        continue;
                    }
                    let on = src_row.and_then(|r| r.get(x)).copied().unwrap_or(false);
                    let idx = ((dst_y * atlas_w as usize + dst_x) * 4) as usize;
                    rgba[idx] = 255;
                    rgba[idx + 1] = 255;
                    rgba[idx + 2] = 255;
                    rgba[idx + 3] = if on { 255 } else { 0 };
                }
            }
        }
    }

    (atlas_w, atlas_h, rgba)
}

/// Convert a Unicode char to CP437 index for atlas lookup
pub fn cp437_index(ch: char) -> u32 {
    if (ch as u32) <= 0xFF {
        return ch as u32;
    }
    CP437_TO_UNICODE.iter().position(|&c| c == ch).map(|idx| idx as u32).unwrap_or(b'?' as u32)
}

// ═══════════════════════════════════════════════════════════════════════════
// GPU Renderer
// ═══════════════════════════════════════════════════════════════════════════

/// Shared GPU glyph renderer with atlas caching
pub struct GlyphRenderer {
    pub pipeline: iced::wgpu::RenderPipeline,
    pub bind_group: iced::wgpu::BindGroup,
    pub uniform_buffer: iced::wgpu::Buffer,
    pub quad_vertex_buffer: iced::wgpu::Buffer,
    pub instance_buffer: iced::wgpu::Buffer,
    pub instance_count: u32,

    pub atlas_texture: iced::wgpu::Texture,
    pub atlas_view: iced::wgpu::TextureView,
    pub atlas_sampler: iced::wgpu::Sampler,
    pub atlas_key: Option<u64>,
    pub atlas_w: u32,
    pub atlas_h: u32,
}

impl GlyphRenderer {
    /// Create a new glyph renderer
    pub fn new(device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("Glyph Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("fkey_glyphs_shader.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("Glyph Uniforms"),
            size: std::mem::size_of::<GlyphUniforms>() as u64,
            usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create a tiny default atlas (will be replaced on first prepare)
        let atlas_texture = device.create_texture(&iced::wgpu::TextureDescriptor {
            label: Some("Glyph Atlas (init)"),
            size: iced::wgpu::Extent3d {
                width: 1,
                height: 1,
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
                texture: &atlas_texture,
                mip_level: 0,
                origin: iced::wgpu::Origin3d::ZERO,
                aspect: iced::wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 0],
            iced::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            iced::wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let atlas_view = atlas_texture.create_view(&iced::wgpu::TextureViewDescriptor::default());

        let atlas_sampler = device.create_sampler(&iced::wgpu::SamplerDescriptor {
            label: Some("Glyph Atlas Sampler"),
            mag_filter: iced::wgpu::FilterMode::Nearest,
            min_filter: iced::wgpu::FilterMode::Nearest,
            mipmap_filter: iced::wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Quad vertex buffer (two triangles)
        let quad: [QuadVertex; 6] = [
            QuadVertex {
                unit_pos: [0.0, 0.0],
                unit_uv: [0.0, 0.0],
            },
            QuadVertex {
                unit_pos: [1.0, 0.0],
                unit_uv: [1.0, 0.0],
            },
            QuadVertex {
                unit_pos: [0.0, 1.0],
                unit_uv: [0.0, 1.0],
            },
            QuadVertex {
                unit_pos: [0.0, 1.0],
                unit_uv: [0.0, 1.0],
            },
            QuadVertex {
                unit_pos: [1.0, 0.0],
                unit_uv: [1.0, 0.0],
            },
            QuadVertex {
                unit_pos: [1.0, 1.0],
                unit_uv: [1.0, 1.0],
            },
        ];
        let quad_vertex_buffer = device.create_buffer_init(&iced::wgpu::util::BufferInitDescriptor {
            label: Some("Glyph Quad"),
            contents: bytemuck::cast_slice(&quad),
            usage: iced::wgpu::BufferUsages::VERTEX,
        });

        // Instance buffer
        let instance_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
            label: Some("Glyph Instances"),
            size: (std::mem::size_of::<GlyphInstance>() * 128) as u64,
            usage: iced::wgpu::BufferUsages::VERTEX | iced::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("Glyph Bind Group Layout"),
            entries: &[
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: iced::wgpu::ShaderStages::VERTEX_FRAGMENT,
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

        let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
            label: Some("Glyph Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                iced::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                iced::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: iced::wgpu::BindingResource::TextureView(&atlas_view),
                },
                iced::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: iced::wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&iced::wgpu::PipelineLayoutDescriptor {
            label: Some("Glyph Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_buffers = [
            iced::wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<QuadVertex>() as u64,
                step_mode: iced::wgpu::VertexStepMode::Vertex,
                attributes: &[
                    iced::wgpu::VertexAttribute {
                        format: iced::wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    },
                    iced::wgpu::VertexAttribute {
                        format: iced::wgpu::VertexFormat::Float32x2,
                        offset: 8,
                        shader_location: 1,
                    },
                ],
            },
            iced::wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<GlyphInstance>() as u64,
                step_mode: iced::wgpu::VertexStepMode::Instance,
                attributes: &[
                    iced::wgpu::VertexAttribute {
                        format: iced::wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 2,
                    },
                    iced::wgpu::VertexAttribute {
                        format: iced::wgpu::VertexFormat::Float32x2,
                        offset: 8,
                        shader_location: 3,
                    },
                    iced::wgpu::VertexAttribute {
                        format: iced::wgpu::VertexFormat::Float32x4,
                        offset: 16,
                        shader_location: 4,
                    },
                    iced::wgpu::VertexAttribute {
                        format: iced::wgpu::VertexFormat::Float32x4,
                        offset: 32,
                        shader_location: 5,
                    },
                    iced::wgpu::VertexAttribute {
                        format: iced::wgpu::VertexFormat::Uint32,
                        offset: 48,
                        shader_location: 6,
                    },
                    iced::wgpu::VertexAttribute {
                        format: iced::wgpu::VertexFormat::Uint32,
                        offset: 52,
                        shader_location: 7,
                    },
                ],
            },
        ];

        let pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some("Glyph Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: iced::wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &vertex_buffers,
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
            quad_vertex_buffer,
            instance_buffer,
            instance_count: 0,
            atlas_texture,
            atlas_view,
            atlas_sampler,
            atlas_key: None,
            atlas_w: 1,
            atlas_h: 1,
        }
    }

    /// Update the atlas texture if the font has changed
    pub fn update_atlas(&mut self, device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, key: u64, w: u32, h: u32, rgba: &[u8]) {
        if self.atlas_w != w || self.atlas_h != h {
            self.atlas_texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                label: Some("Glyph Atlas"),
                size: iced::wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: iced::wgpu::TextureDimension::D2,
                format: iced::wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: iced::wgpu::TextureUsages::TEXTURE_BINDING | iced::wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.atlas_view = self.atlas_texture.create_view(&iced::wgpu::TextureViewDescriptor::default());
            self.atlas_w = w;
            self.atlas_h = h;

            // Rebuild bind group to reference the new view
            let bind_group_layout = self.pipeline.get_bind_group_layout(0);
            self.bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                label: Some("Glyph Bind Group"),
                layout: &bind_group_layout,
                entries: &[
                    iced::wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.uniform_buffer.as_entire_binding(),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 1,
                        resource: iced::wgpu::BindingResource::TextureView(&self.atlas_view),
                    },
                    iced::wgpu::BindGroupEntry {
                        binding: 2,
                        resource: iced::wgpu::BindingResource::Sampler(&self.atlas_sampler),
                    },
                ],
            });
        }

        queue.write_texture(
            iced::wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: iced::wgpu::Origin3d::ZERO,
                aspect: iced::wgpu::TextureAspect::All,
            },
            rgba,
            iced::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            iced::wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        self.atlas_key = Some(key);
    }

    /// Upload glyph instances to the GPU
    pub fn upload_instances(&mut self, queue: &iced::wgpu::Queue, instances: &[GlyphInstance]) {
        let count = instances
            .len()
            .min((self.instance_buffer.size() as usize) / std::mem::size_of::<GlyphInstance>());
        self.instance_count = count as u32;
        if count == 0 {
            return;
        }
        let bytes = bytemuck::cast_slice(&instances[..count]);
        queue.write_buffer(&self.instance_buffer, 0, bytes);
    }

    /// Update uniforms
    pub fn update_uniforms(&self, queue: &iced::wgpu::Queue, clip_size: [f32; 2], glyph_size: [f32; 2]) {
        let uniforms = GlyphUniforms {
            clip_size,
            atlas_size: [self.atlas_w as f32, self.atlas_h as f32],
            glyph_size,
            _pad: [0.0, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    /// Render all uploaded instances
    pub fn render(&self, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        if self.instance_count == 0 {
            return;
        }

        let mut pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
            label: Some("Glyph Render Pass"),
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

        pass.set_scissor_rect(clip_bounds.x, clip_bounds.y, clip_bounds.width, clip_bounds.height);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.draw(0..6, 0..self.instance_count);
    }

    /// Check if atlas needs update for a given font
    pub fn needs_atlas_update(&self, font: &BitFont) -> bool {
        self.atlas_key != Some(font_key(font))
    }

    /// Ensure atlas is up-to-date for the given font
    pub fn ensure_atlas(&mut self, device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, font: &BitFont) {
        let key = font_key(font);
        if self.atlas_key != Some(key) {
            let (aw, ah, rgba) = build_glyph_atlas_rgba(font);
            self.update_atlas(device, queue, key, aw, ah, &rgba);
        }
    }
}
