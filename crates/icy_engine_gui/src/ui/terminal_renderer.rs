use std::num::NonZeroU32;

use egui::{Rect};
use wgpu::{Device, Queue, TextureFormat, RenderPass, Buffer};
use icy_engine::{Buffer as TextBuffer, Color};

#[derive(Debug)]
pub struct TerminalGlyphAtlas {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub size: (u32, u32),
}

pub struct WgpuTerminalRenderer {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    vertex_buf: Buffer,
    index_buf: Buffer,
    index_count: u32,
    uniform_buf: Buffer,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    // position in pixels relative to terminal rect origin
    pos: [f32; 2],
    // uv (0..1)
    uv: [f32; 2],
    // fg color RGBA u8 -> packed
    color: [u8; 4],
}

unsafe impl bytemuck::Zeroable for Vertex {}
unsafe impl bytemuck::Pod for Vertex {}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Uniforms {
    // transform to NDC
    // [ sx,  0, tx,   0
    //   0, sy, ty,   0 ]
    transform: [f32; 4],
    atlas_size: [f32; 2],
}
unsafe impl bytemuck::Zeroable for Uniforms {}
unsafe impl bytemuck::Pod for Uniforms {}

impl WgpuTerminalRenderer {
    pub fn new(device: &Device, surface_format: TextureFormat, atlas: &TerminalGlyphAtlas) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terminal_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("terminal_shader.wgsl").into()),
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terminal_bind_group_layout"),
            entries: &[
                // Uniforms
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
                // Atlas texture
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
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terminal_uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terminal_bind_group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&atlas.sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terminal_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // pos
                wgpu::VertexAttribute {
                    shader_location: 0,
                    offset: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // uv
                wgpu::VertexAttribute {
                    shader_location: 1,
                    offset: 8,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color (unorm8x4)
                wgpu::VertexAttribute {
                    shader_location: 2,
                    offset: 16,
                    format: wgpu::VertexFormat::Unorm8x4,
                },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terminal_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[vertex_layout],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // Empty buffers – will be replaced per frame
        let vertex_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terminal_vertex_buf"),
            size: 1, // resized dynamically
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let index_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terminal_index_buf"),
            size: 1,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            layout,
            bind_group,
            vertex_buf,
            index_buf,
            index_count: 0,
            uniform_buf,
        }
    }

    pub fn update_geometry(
        &mut self,
        device: &Device,
        queue: &Queue,
        buf: &TextBuffer,
        viewport_rect: Rect,
        font_dims: (f32, f32),
        scale: (f32, f32),
        first_line: f32,
        first_col: f32,
    ) {
        // Build vertices (simple: each cell becomes a quad)
        let fw = font_dims.0 * scale.0;
        let fh = font_dims.1 * scale.1;

        let mut vertices = Vec::<Vertex>::new();
        let mut indices = Vec::<u32>::new();
        let mut idx_base: u32 = 0;

        let visible_lines = (viewport_rect.height() / fh).ceil() as usize;
        let visible_cols = (viewport_rect.width() / fw).ceil() as usize;

        for row in 0..visible_lines {
            let y_buf = row + first_line.floor() as usize;
            if y_buf >= buf.get_height() { break; }
            for col in 0..visible_cols {
                let x_buf = col + first_col.floor() as usize;
                if x_buf >= buf.get_width() { break; }
                let ch = buf.get_char((x_buf as i32, y_buf as i32));
                let glyph_uv = compute_uv(ch.ch);
                let fg = ch.attribute.get_foreground();
                let (r,g,b) = fg.get_rgb();
                let color = [r, g, b, 255u8];

                let x0 = viewport_rect.left() + col as f32 * fw;
                let y0 = viewport_rect.top() + row as f32 * fh;
                let x1 = x0 + fw;
                let y1 = y0 + fh;

                // Four vertices
                vertices.push(Vertex { pos:[x0,y0], uv:[glyph_uv.0, glyph_uv.1], color });
                vertices.push(Vertex { pos:[x1,y0], uv:[glyph_uv.2, glyph_uv.1], color });
                vertices.push(Vertex { pos:[x1,y1], uv:[glyph_uv.2, glyph_uv.3], color });
                vertices.push(Vertex { pos:[x0,y1], uv:[glyph_uv.0, glyph_uv.3], color });

                indices.extend_from_slice(&[idx_base, idx_base+1, idx_base+2, idx_base, idx_base+2, idx_base+3]);
                idx_base += 4;
            }
        }

        self.index_count = indices.len() as u32;

        if !vertices.is_empty() {
            let vb_size = (vertices.len() * std::mem::size_of::<Vertex>()) as u64;
            let ib_size = (indices.len() * std::mem::size_of::<u32>()) as u64;

            // Recreate buffers sized exactly – simpler than partial updates.
            self.vertex_buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("terminal_vertex_buf_dyn"),
                size: vb_size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.index_buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("terminal_index_buf_dyn"),
                size: ib_size,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            queue.write_buffer(&self.vertex_buf, 0, bytemuck::cast_slice(&vertices));
            queue.write_buffer(&self.index_buf, 0, bytemuck::cast_slice(&indices));
        }

        // Uniforms: convert pixel space to NDC (-1..1)
        let sx = 2.0 / viewport_rect.width();
        let sy = 2.0 / viewport_rect.height();
        let tx = -1.0 - viewport_rect.left() * sx;
        let ty = -1.0 - viewport_rect.top() * sy;
        let uniforms = Uniforms {
            transform: [sx, sy, tx, ty],
            atlas_size: [1.0, 1.0],
        };
        queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&uniforms));
    }

    pub fn render(&self, render_pass: &mut RenderPass) {
        if self.index_count == 0 { return; }
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buf.slice(..));
        render_pass.set_index_buffer(self.index_buf.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

// Very simplified UV calculator placeholder.
// Real implementation should consult font atlas layout.
fn compute_uv(_ch: char) -> (f32, f32, f32, f32) {
    // Stub: full atlas = one glyph placeholder
    (0.0, 0.0, 1.0, 1.0)
}