//! Small GPU helpers shared by the widget renderers.
//!
//! All of icy_draw's `iced_wgpu`-backed widgets build essentially the same
//! render pipeline and most share the same bind-group shape (uniform +
//! texture + sampler). This module captures both as small functions so each
//! widget only spells out what it actually customises.

use icy_ui::wgpu;
use std::num::NonZeroU64;

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

/// Configuration for [`build_uniform_texture_sampler_layout`].
///
/// The resulting layout has three entries:
/// - binding 0: uniform buffer
/// - binding 1: 2D filterable float texture (fragment-only)
/// - binding 2: filtering sampler (fragment-only)
pub struct UniformTextureSamplerLayout<'a> {
    pub label: &'a str,
    /// Shader stages that see the uniform buffer (binding 0).
    pub uniform_visibility: wgpu::ShaderStages,
    /// Whether the uniform binding uses dynamic offsets.
    pub uniform_dynamic_offset: bool,
    /// Required when `uniform_dynamic_offset` is true; ignored otherwise.
    pub uniform_min_binding_size: Option<NonZeroU64>,
}

/// Build the standard widget bind-group layout: uniform + 2D texture + sampler.
///
/// Used by `tool_panel`, `paste_controls`, `color_switcher`, `layer_view`,
/// `segmented_control` and `fkey_toolbar`. The minimap widget uses a different
/// binding order and is intentionally not routed through here.
pub fn build_uniform_texture_sampler_layout(device: &wgpu::Device, opts: UniformTextureSamplerLayout<'_>) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(opts.label),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: opts.uniform_visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: opts.uniform_dynamic_offset,
                    min_binding_size: if opts.uniform_dynamic_offset { opts.uniform_min_binding_size } else { None },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}
