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

/// Description of a 2D / 2D-array texture that should be clamped to the
/// device's reported limits before allocation.
///
/// All shader-backed widgets in icy_draw used to allocate textures using
/// hard-coded sizes. On the wgpu GLES downlevel path
/// `max_texture_dimension_2d` can be as low as 2048 and
/// `max_texture_array_layers` as low as 256; allocating beyond that panics
/// inside wgpu. Routing every texture creation through
/// [`create_clamped_texture`] makes the widgets degrade gracefully on
/// constrained backends instead of crashing.
pub struct ClampedTextureDescriptor<'a> {
    pub label: &'a str,
    pub width: u32,
    pub height: u32,
    /// 1 for plain 2D textures, >1 for 2D arrays.
    pub depth_or_array_layers: u32,
    pub format: wgpu::TextureFormat,
    pub usage: wgpu::TextureUsages,
}

/// Outcome of a clamped texture allocation. The `width`, `height` and
/// `layers` fields reflect the **actual** dimensions used after clamping;
/// callers that index into the texture should respect them.
#[allow(dead_code)]
pub struct ClampedTexture {
    pub texture: wgpu::Texture,
    pub width: u32,
    pub height: u32,
    pub layers: u32,
}

/// Create a 2D / 2D-array texture, clamping each dimension to the device
/// limits reported by `device.limits()`. A clamp event is logged at
/// `warn` level so it shows up in user-supplied diagnostics.
///
/// All values are clamped to at least 1 to avoid zero-sized texture
/// descriptors.
pub fn create_clamped_texture(device: &wgpu::Device, desc: ClampedTextureDescriptor<'_>) -> ClampedTexture {
    let limits = device.limits();
    let (width, height, layers) = clamp_texture_size(
        desc.label,
        desc.width,
        desc.height,
        desc.depth_or_array_layers,
        limits.max_texture_dimension_2d,
        limits.max_texture_array_layers,
    );

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(desc.label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: desc.format,
        usage: desc.usage,
        view_formats: &[],
    });

    ClampedTexture {
        texture,
        width,
        height,
        layers,
    }
}

/// Pure clamp logic. Returns `(width, height, layers)` after clamping each
/// dimension to at least 1 and at most the supplied device limits, and
/// emits a single `warn` log line when any clamp actually fired.
///
/// Extracted so the clamp policy can be unit-tested without spinning up a
/// real `wgpu::Device`.
fn clamp_texture_size(label: &str, width: u32, height: u32, layers: u32, max_dim: u32, max_layers: u32) -> (u32, u32, u32) {
    let max_dim = max_dim.max(1);
    let max_layers = max_layers.max(1);

    let req_w = width.max(1);
    let req_h = height.max(1);
    let req_layers = layers.max(1);

    let w = req_w.min(max_dim);
    let h = req_h.min(max_dim);
    let l = req_layers.min(max_layers);

    if w != req_w || h != req_h || l != req_layers {
        log::warn!(
            "{}: texture clamped to device limits ({}x{}x{} -> {}x{}x{}; max_dim={}, max_layers={})",
            label,
            req_w,
            req_h,
            req_layers,
            w,
            h,
            l,
            max_dim,
            max_layers,
        );
    }

    (w, h, l)
}

#[cfg(test)]
mod tests {
    use super::clamp_texture_size;

    #[test]
    fn no_clamp_when_within_limits() {
        assert_eq!(clamp_texture_size("t", 1024, 512, 4, 4096, 256), (1024, 512, 4));
    }

    #[test]
    fn clamps_width_height_to_max_dim() {
        assert_eq!(clamp_texture_size("t", 8192, 4096, 1, 2048, 256), (2048, 2048, 1));
    }

    #[test]
    fn clamps_layers_to_max_layers() {
        assert_eq!(clamp_texture_size("t", 256, 256, 1024, 8192, 256), (256, 256, 256));
    }

    #[test]
    fn promotes_zero_to_one() {
        assert_eq!(clamp_texture_size("t", 0, 0, 0, 4096, 256), (1, 1, 1));
    }

    #[test]
    fn promotes_zero_limits_to_one() {
        // A backend reporting `0` for a limit shouldn't crash us either.
        assert_eq!(clamp_texture_size("t", 100, 100, 4, 0, 0), (1, 1, 1));
    }
}
