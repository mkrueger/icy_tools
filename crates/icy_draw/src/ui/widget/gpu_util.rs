//! Small GPU-pipeline helpers shared by the widget renderers.
//!
//! All of icy_draw's `iced_wgpu`-backed widgets build essentially the same
//! render pipeline: a `vs_main` / `fs_main` shader pair, a triangle-list
//! topology, alpha blending, no depth/stencil and default multisample. The
//! only things that vary are the label, the shader module, the pipeline
//! layout and the vertex buffer layouts.
//!
//! [`build_alpha_blended_pipeline`] captures that boilerplate so each
//! widget only spells out what it actually customises.

use icy_ui::wgpu;

/// Build a render pipeline using icy_draw's standard widget defaults:
///
/// - vertex entry `vs_main`, fragment entry `fs_main`
/// - alpha blending against `format`, full color write mask
/// - triangle list, default primitive state, no depth/stencil
/// - default multisample, no multiview / pipeline cache
///
/// `vertex_buffers` may be empty for vertex-pulling pipelines.
pub fn build_alpha_blended_pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    vertex_buffers: &[wgpu::VertexBufferLayout<'_>],
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: vertex_buffers,
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}
