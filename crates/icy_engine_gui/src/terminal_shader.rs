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
    // Packed slice heights (up to 10) + first_slice_start_y
    // slice_heights[0] = [h0, h1, h2, first_slice_start_y]
    // slice_heights[1] = [h3, h4, h5, h6]
    // slice_heights[2] = [h7, h8, h9, 0]
    slice_heights: [[f32; 4]; 3],

    // X-axis scrolling uniforms (for zoom/pan)
    scroll_offset_x: f32,
    visible_width: f32,
    texture_width: f32,
    _x_padding: f32, // Padding for alignment

    // Caret uniforms (rendered in shader to avoid texture cache invalidation)
    /// Caret position in pixels (x, y) relative to viewport
    caret_pos: [f32; 2],
    /// Caret size in pixels (width, height)
    caret_size: [f32; 2],
    /// Caret visibility (1.0 = visible, 0.0 = hidden for blinking)
    caret_visible: f32,
    /// Caret mode: 0 = Bar, 1 = Block, 2 = Underline
    caret_mode: f32,
    /// Padding for 16-byte alignment
    _caret_padding: [f32; 2],

    // Marker uniforms (raster grid and guide crosshair)
    /// Raster grid spacing in pixels (cell_width, cell_height), (0,0) = disabled
    raster_spacing: [f32; 2],
    /// Padding to align raster_color to 16-byte boundary (vec4 requires 16-byte alignment)
    _raster_spacing_padding: [f32; 2],
    /// Raster grid color (RGBA)
    raster_color: [f32; 4],
    /// Raster grid alpha (0.0 - 1.0)
    raster_alpha: f32,
    /// Raster grid enabled (1.0 = enabled, 0.0 = disabled)
    raster_enabled: f32,
    /// Padding for 16-byte alignment
    _raster_padding: [f32; 2],

    /// Guide crosshair position in pixels (x, y), negative = disabled
    guide_pos: [f32; 2],
    /// Padding to align guide_color to 16-byte boundary (vec4 requires 16-byte alignment)
    _guide_pos_padding: [f32; 2],
    /// Guide crosshair color (RGBA)
    guide_color: [f32; 4],
    /// Guide crosshair alpha (0.0 - 1.0)
    guide_alpha: f32,
    /// Guide crosshair enabled (1.0 = enabled, 0.0 = disabled)
    guide_enabled: f32,
    /// Padding for 16-byte alignment
    _marker_padding: [f32; 2],

    // Reference image uniforms
    /// Reference image enabled (1.0 = enabled, 0.0 = disabled)
    ref_image_enabled: f32,
    /// Reference image alpha/opacity (0.0 - 1.0)
    ref_image_alpha: f32,
    /// Reference image mode: 0 = Stretch, 1 = Original, 2 = Tile
    ref_image_mode: f32,
    /// Padding for alignment
    _ref_padding: f32,
    /// Reference image offset in pixels (x, y)
    ref_image_offset: [f32; 2],
    /// Reference image scale factor (x, y)
    ref_image_scale: [f32; 2],
    /// Reference image size in pixels (width, height)
    ref_image_size: [f32; 2],
    /// Padding for 16-byte alignment
    _ref_padding2: [f32; 2],

    // Layer bounds uniforms (for showing current layer border)
    /// Layer bounds rectangle in pixels (x, y, x+width, y+height) in document space
    layer_rect: [f32; 4],
    /// Layer bounds border color (RGBA)
    layer_color: [f32; 4],
    /// Layer bounds enabled (1.0 = enabled, 0.0 = disabled)
    layer_enabled: f32,
    /// Padding for 16-byte alignment (must match WGSL struct size)
    _layer_padding: [f32; 3],

    // Selection uniforms (for highlighting selected area)
    /// Selection rectangle in pixels (x, y, x+width, y+height) in document space
    selection_rect: [f32; 4],
    /// Selection border color (RGBA) - white for normal, green for add, red for subtract
    selection_color: [f32; 4],
    /// Selection enabled (1.0 = enabled, 0.0 = disabled)
    selection_enabled: f32,
    /// Selection mask enabled (1.0 = use texture mask, 0.0 = use rectangle only)
    selection_mask_enabled: f32,
    /// Padding for 16-byte alignment
    _selection_padding: [f32; 2],

    // Brush/Pencil preview uniforms (tool hover preview)
    /// Preview rectangle in pixels (x, y, x+width, y+height) in document space
    brush_preview_rect: [f32; 4],
    /// Preview enabled (1.0 = enabled, 0.0 = disabled)
    brush_preview_enabled: f32,
    /// Padding to match WGSL alignment.
    ///
    /// WGSL has `_brush_preview_padding: vec3<f32>`, which is **16-byte aligned** in
    /// uniform buffers. Because `brush_preview_enabled` is only 4 bytes, WGSL inserts
    /// 12 bytes of implicit padding before the vec3. We must mirror that here or all
    /// following fields (incl. `terminal_rect`) are read at the wrong offsets.
    _brush_preview_padding0: [f32; 3],
    /// The WGSL vec3 consumes 16 bytes in uniforms, so we store 4 floats.
    _brush_preview_padding: [f32; 4],

    // Font dimensions for selection mask sampling
    /// Font width in pixels
    font_width: f32,
    /// Font height in pixels  
    font_height: f32,
    /// Selection mask size in cells (width, height)
    selection_mask_size: [f32; 2],

    // Tool overlay mask (Moebius-style alpha preview)
    /// (mask_width_in_cells, mask_height_in_cells, cell_height_scale, enabled)
    tool_overlay_params: [f32; 4],

    // Terminal area within the full viewport (for rendering selection outside document bounds)
    /// Terminal rect in normalized UV coordinates (start_x, start_y, width, height)
    terminal_rect: [f32; 4],
}

#[cfg(test)]
mod tests {
    use super::CRTUniforms;

    #[test]
    fn crt_uniforms_size_matches_shader_expectations() {
        // Keep in sync with `crates/icy_engine_gui/src/shaders/crt.wgsl` (`Uniforms`).
        assert_eq!(std::mem::align_of::<CRTUniforms>(), 16);
        assert_eq!(std::mem::size_of::<CRTUniforms>(), 560);
    }
}

/// The terminal shader program (high-level interface)
#[derive(Debug, Clone)]
pub struct TerminalShader {
    /// Texture slices for blink_off state
    pub slices_blink_off: Vec<TextureSliceData>,
    /// Texture slices for blink_on state
    pub slices_blink_on: Vec<TextureSliceData>,
    /// Heights of each slice in pixels (same for both blink states)
    pub slice_heights: Vec<u32>,
    /// Width of the texture (same for all slices)
    pub texture_width: u32,
    /// Total content height (full document)
    pub total_content_height: f32,
    /// Store the monitor settings for CRT effects
    pub monitor_settings: Arc<MonitorSettings>,
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
    /// Scroll offset in pixels (Y axis)
    pub scroll_offset_y: f32,
    /// Visible height in pixels
    pub visible_height: f32,
    /// Where the first slice starts in document Y coordinates
    pub first_slice_start_y: f32,
    /// Scroll offset in pixels (X axis)
    pub scroll_offset_x: f32,
    /// Visible width in pixels
    pub visible_width: f32,

    // Caret rendering (in shader)
    /// Caret position in pixels (x, y) relative to viewport
    pub caret_pos: [f32; 2],
    /// Caret size in pixels (width, height)
    pub caret_size: [f32; 2],
    /// Caret visibility (for blinking)
    pub caret_visible: bool,
    /// Caret mode: 0 = Bar, 1 = Block, 2 = Underline
    pub caret_mode: u8,
    /// Current blink state (for selecting which bind group to use)
    pub blink_on: bool,

    // Marker rendering (raster grid and guide crosshair)
    /// Raster grid spacing in pixels (cell_width, cell_height), None = disabled
    pub raster_spacing: Option<(f32, f32)>,
    /// Raster grid color (RGBA)
    pub raster_color: [f32; 4],
    /// Raster grid alpha (0.0 - 1.0)
    pub raster_alpha: f32,

    /// Guide crosshair position in pixels (x, y), None = disabled
    pub guide_pos: Option<(f32, f32)>,
    /// Guide crosshair color (RGBA)
    pub guide_color: [f32; 4],
    /// Guide crosshair alpha (0.0 - 1.0)
    pub guide_alpha: f32,

    // Reference image rendering
    /// Reference image texture data (RGBA bytes), None = no image
    pub reference_image_data: Option<(Vec<u8>, u32, u32)>, // (data, width, height)
    /// Reference image enabled
    pub reference_image_enabled: bool,
    /// Reference image alpha/opacity (0.0 - 1.0)
    pub reference_image_alpha: f32,
    /// Reference image mode: 0 = Stretch, 1 = Original, 2 = Tile
    pub reference_image_mode: u8,
    /// Reference image offset in pixels (x, y)
    pub reference_image_offset: [f32; 2],
    /// Reference image scale factor
    pub reference_image_scale: f32,

    // Layer bounds rendering
    /// Layer bounds rectangle in pixels (x, y, x+width, y+height) in document space, None = disabled
    pub layer_rect: Option<[f32; 4]>,
    /// Layer bounds border color (RGBA) - yellow for normal, white for preview
    pub layer_color: [f32; 4],
    /// Whether to show layer bounds
    pub show_layer_bounds: bool,

    // Selection rendering
    /// Selection rectangle in pixels (x, y, x+width, y+height) in document space, None = disabled
    pub selection_rect: Option<[f32; 4]>,
    /// Selection border color (RGBA) - white for normal, green for add, red for subtract
    /// Default is white [1.0, 1.0, 1.0, 1.0]
    pub selection_color: [f32; 4],

    // Selection mask rendering (for complex non-rectangular selections)
    /// Selection mask texture data (RGBA bytes), None = no mask
    /// Each pixel represents one character cell: white = selected, black = not selected
    pub selection_mask_data: Option<(Vec<u8>, u32, u32)>, // (data, width_in_cells, height_in_cells)
    /// Font dimensions for selection mask sampling (font_width, font_height in pixels)
    pub font_dimensions: Option<(f32, f32)>,

    // Tool overlay mask rendering (Moebius-like alpha preview)
    /// Tool overlay mask texture data (RGBA bytes), None = no overlay
    pub tool_overlay_mask_data: Option<(Vec<u8>, u32, u32)>, // (data, width_in_cells, height_in_cells)
    /// Cell height scale for tool overlay sampling (1.0 normal, 0.5 half-block)
    pub tool_overlay_cell_height_scale: f32,

    // Brush/Pencil preview rendering
    /// Preview rectangle in pixels (x, y, x+width, y+height) in document space, None = disabled
    pub brush_preview_rect: Option<[f32; 4]>,
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
    /// Texture slices for blink_off state (slots 0-2)
    _slices_blink_off: Vec<TextureSlice>,
    /// Texture slices for blink_on state (slots 3-5)  
    _slices_blink_on: Vec<TextureSlice>,
    /// Bind group for blink_off state
    bind_group_blink_off: iced::wgpu::BindGroup,
    /// Bind group for blink_on state
    bind_group_blink_on: iced::wgpu::BindGroup,
    /// Uniform buffer (shared between both states)
    uniform_buffer: iced::wgpu::Buffer,
    /// Monitor color buffer (shared between both states)
    monitor_color_buffer: iced::wgpu::Buffer,
    /// Texture width for cache validation
    texture_width: u32,
    /// Total texture height for cache validation
    total_height: u32,
    /// Number of slices for cache validation
    num_slices: usize,
    /// Hash of texture data pointers for blink_on=false
    texture_data_hash_blink_off: u64,
    /// Hash of texture data pointers for blink_on=true
    texture_data_hash_blink_on: u64,
    /// Reference image texture (optional)
    reference_image_texture: Option<TextureSlice>,
    /// Hash of reference image data for cache validation
    reference_image_hash: u64,
    /// Selection mask texture (optional)
    selection_mask_texture: Option<TextureSlice>,
    /// Hash of selection mask data for cache validation
    selection_mask_hash: u64,

    /// Tool overlay mask texture (optional)
    tool_overlay_mask_texture: Option<TextureSlice>,
    /// Hash of tool overlay mask data for cache validation
    tool_overlay_mask_hash: u64,

    /// Physical pixel viewport for this widget instance (x, y, width, height).
    /// We must render into the widget bounds viewport and only use `clip_bounds`
    /// for the scissor rect. Using `clip_bounds` as viewport would rescale the
    /// whole terminal and can distort aspect ratio.
    viewport_px: [f32; 4],
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
static DEBUG_RENDER_INFO_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static DEBUG_RENDER_PASS_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
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

        // Create bind group layout with slices + sampler + uniforms + monitor_color + reference_image + selection_mask + tool_overlay_mask
        let mut entries = Vec::with_capacity(MAX_TEXTURE_SLICES + 5);

        // Add 3 texture bindings (0-2)
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

        // Sampler at binding 3
        entries.push(iced::wgpu::BindGroupLayoutEntry {
            binding: MAX_TEXTURE_SLICES as u32,
            visibility: iced::wgpu::ShaderStages::FRAGMENT,
            ty: iced::wgpu::BindingType::Sampler(iced::wgpu::SamplerBindingType::Filtering),
            count: None,
        });

        // Uniforms at binding 4
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

        // Monitor color at binding 5
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

        // Reference image texture at binding 6
        entries.push(iced::wgpu::BindGroupLayoutEntry {
            binding: (MAX_TEXTURE_SLICES + 3) as u32,
            visibility: iced::wgpu::ShaderStages::FRAGMENT,
            ty: iced::wgpu::BindingType::Texture {
                multisampled: false,
                view_dimension: iced::wgpu::TextureViewDimension::D2,
                sample_type: iced::wgpu::TextureSampleType::Float { filterable: true },
            },
            count: None,
        });

        // Selection mask texture at binding 7
        entries.push(iced::wgpu::BindGroupLayoutEntry {
            binding: (MAX_TEXTURE_SLICES + 4) as u32,
            visibility: iced::wgpu::ShaderStages::FRAGMENT,
            ty: iced::wgpu::BindingType::Texture {
                multisampled: false,
                view_dimension: iced::wgpu::TextureViewDimension::D2,
                sample_type: iced::wgpu::TextureSampleType::Float { filterable: true },
            },
            count: None,
        });

        // Tool overlay mask texture at binding 8
        entries.push(iced::wgpu::BindGroupLayoutEntry {
            binding: (MAX_TEXTURE_SLICES + 5) as u32,
            visibility: iced::wgpu::ShaderStages::FRAGMENT,
            ty: iced::wgpu::BindingType::Texture {
                multisampled: false,
                view_dimension: iced::wgpu::TextureViewDimension::D2,
                sample_type: iced::wgpu::TextureSampleType::Float { filterable: true },
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
    /// Compute a hash of the texture data pointers for a slice set
    fn compute_data_hash(slices: &[TextureSliceData]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        for slice in slices {
            let ptr = Arc::as_ptr(&slice.rgba_data) as usize;
            ptr.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Compute a hash for reference image data
    fn compute_ref_image_hash(data: &Option<(Vec<u8>, u32, u32)>) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        match data {
            Some((bytes, w, h)) => {
                // Use data pointer and dimensions for hash
                bytes.as_ptr().hash(&mut hasher);
                bytes.len().hash(&mut hasher);
                w.hash(&mut hasher);
                h.hash(&mut hasher);
            }
            None => {
                0u64.hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    /// Compute a hash for selection mask data
    fn compute_selection_mask_hash(data: &Option<(Vec<u8>, u32, u32)>) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        match data {
            Some((bytes, w, h)) => {
                // Use data content for hash (selection changes frequently)
                bytes.hash(&mut hasher);
                w.hash(&mut hasher);
                h.hash(&mut hasher);
            }
            None => {
                0u64.hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    /// Compute a hash for tool overlay mask data
    fn compute_tool_overlay_mask_hash(data: &Option<(Vec<u8>, u32, u32)>) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        match data {
            Some((bytes, w, h)) => {
                // Overlay changes frequently while dragging
                bytes.hash(&mut hasher);
                w.hash(&mut hasher);
                h.hash(&mut hasher);
            }
            None => {
                0u64.hash(&mut hasher);
            }
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
        bounds: &iced::Rectangle,
        _viewport: &iced::advanced::graphics::Viewport,
    ) {
        let scale_factor = _viewport.scale_factor() as f32;
        set_scale_factor(scale_factor);

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
        let hash_blink_off = Self::compute_data_hash(&self.slices_blink_off);
        let hash_blink_on = Self::compute_data_hash(&self.slices_blink_on);
        let num_slices = self.slices_blink_off.len(); // Both should have same count
        let texture_width = self.texture_width.min(MAX_TEXTURE_DIMENSION);
        let total_height = self.total_content_height as u32;

        // Check if we need to recreate resources for either blink state
        let needs_recreate = match pipeline.instances.get(&id) {
            None => true,
            Some(resources) => {
                // Recreate if any structural parameter changed or if either blink state data changed
                let hash_off_changed = resources.texture_data_hash_blink_off != hash_blink_off;
                let hash_on_changed = resources.texture_data_hash_blink_on != hash_blink_on;
                let slices_changed = resources.num_slices != num_slices;
                let width_changed = resources.texture_width != texture_width;
                let height_changed = resources.total_height != total_height;

                hash_off_changed || hash_on_changed || slices_changed || width_changed || height_changed
            }
        };

        if needs_recreate {
            // Helper function to create GPU slices from texture data
            let create_gpu_slices = |slices: &[TextureSliceData], label_prefix: &str| -> Vec<TextureSlice> {
                let mut gpu_slices = Vec::with_capacity(slices.len());
                for (i, slice_data) in slices.iter().enumerate() {
                    let w = slice_data.width.min(MAX_TEXTURE_DIMENSION);
                    let h = slice_data.height.min(MAX_TEXTURE_DIMENSION);

                    let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                        label: Some(&format!("Terminal {} Slice {} Instance {}", label_prefix, i, id)),
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
                gpu_slices
            };

            // Create GPU slices for both blink states
            let gpu_slices_blink_off = create_gpu_slices(&self.slices_blink_off, "BlinkOff");
            let gpu_slices_blink_on = create_gpu_slices(&self.slices_blink_on, "BlinkOn");

            // Create shared uniform buffer
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

            // Helper to create bind group for a set of GPU slices
            let create_bind_group = |gpu_slices: &[TextureSlice],
                                     ref_image_view: &iced::wgpu::TextureView,
                                     sel_mask_view: &iced::wgpu::TextureView,
                                     tool_mask_view: &iced::wgpu::TextureView,
                                     label: &str|
             -> iced::wgpu::BindGroup {
                let mut bind_entries: Vec<iced::wgpu::BindGroupEntry> = Vec::with_capacity(MAX_TEXTURE_SLICES + 6);

                // Add texture bindings (0-2), using dummy for unused slots
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

                // Sampler at binding 3
                bind_entries.push(iced::wgpu::BindGroupEntry {
                    binding: MAX_TEXTURE_SLICES as u32,
                    resource: iced::wgpu::BindingResource::Sampler(&pipeline.sampler),
                });

                // Uniforms at binding 4
                bind_entries.push(iced::wgpu::BindGroupEntry {
                    binding: (MAX_TEXTURE_SLICES + 1) as u32,
                    resource: uniform_buffer.as_entire_binding(),
                });

                // Monitor color at binding 5
                bind_entries.push(iced::wgpu::BindGroupEntry {
                    binding: (MAX_TEXTURE_SLICES + 2) as u32,
                    resource: monitor_color_buffer.as_entire_binding(),
                });

                // Reference image at binding 6
                bind_entries.push(iced::wgpu::BindGroupEntry {
                    binding: (MAX_TEXTURE_SLICES + 3) as u32,
                    resource: iced::wgpu::BindingResource::TextureView(ref_image_view),
                });

                // Selection mask at binding 7
                bind_entries.push(iced::wgpu::BindGroupEntry {
                    binding: (MAX_TEXTURE_SLICES + 4) as u32,
                    resource: iced::wgpu::BindingResource::TextureView(sel_mask_view),
                });

                // Tool overlay mask at binding 8
                bind_entries.push(iced::wgpu::BindGroupEntry {
                    binding: (MAX_TEXTURE_SLICES + 5) as u32,
                    resource: iced::wgpu::BindingResource::TextureView(tool_mask_view),
                });

                device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                    label: Some(&format!("Terminal BindGroup {} Instance {}", label, id)),
                    layout: &pipeline.bind_group_layout,
                    entries: &bind_entries,
                })
            };

            // Create bind groups for both blink states (using dummy for reference image and selection mask initially)
            let bind_group_blink_off = create_bind_group(
                &gpu_slices_blink_off,
                &pipeline.dummy_texture_view,
                &pipeline.dummy_texture_view,
                &pipeline.dummy_texture_view,
                "BlinkOff",
            );
            let bind_group_blink_on = create_bind_group(
                &gpu_slices_blink_on,
                &pipeline.dummy_texture_view,
                &pipeline.dummy_texture_view,
                &pipeline.dummy_texture_view,
                "BlinkOn",
            );

            pipeline.instances.insert(
                id,
                InstanceResources {
                    _slices_blink_off: gpu_slices_blink_off,
                    _slices_blink_on: gpu_slices_blink_on,
                    bind_group_blink_off,
                    bind_group_blink_on,
                    uniform_buffer,
                    monitor_color_buffer,
                    texture_width,
                    total_height,
                    num_slices,
                    texture_data_hash_blink_off: hash_blink_off,
                    texture_data_hash_blink_on: hash_blink_on,
                    reference_image_texture: None,
                    reference_image_hash: 0,
                    selection_mask_texture: None,
                    selection_mask_hash: 0,
                    tool_overlay_mask_texture: None,
                    tool_overlay_mask_hash: 0,
                    viewport_px: [0.0, 0.0, 1.0, 1.0],
                },
            );
        }

        // Check if reference image changed and needs update
        let ref_image_hash = Self::compute_ref_image_hash(&self.reference_image_data);
        let needs_ref_image_update = match pipeline.instances.get(&id) {
            Some(resources) => resources.reference_image_hash != ref_image_hash,
            None => false,
        };

        if needs_ref_image_update {
            if let Some(resources) = pipeline.instances.get_mut(&id) {
                // Create or update reference image texture
                if let Some((data, width, height)) = &self.reference_image_data {
                    let w = (*width).max(1).min(MAX_TEXTURE_DIMENSION);
                    let h = (*height).max(1).min(MAX_TEXTURE_DIMENSION);

                    let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                        label: Some(&format!("Reference Image Instance {}", id)),
                        size: iced::wgpu::Extent3d {
                            width: w,
                            height: h,
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

                    // Write image data to texture
                    if !data.is_empty() {
                        queue.write_texture(
                            iced::wgpu::TexelCopyTextureInfo {
                                texture: &texture,
                                mip_level: 0,
                                origin: iced::wgpu::Origin3d::ZERO,
                                aspect: iced::wgpu::TextureAspect::All,
                            },
                            data,
                            iced::wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(4 * w),
                                rows_per_image: Some(h),
                            },
                            iced::wgpu::Extent3d {
                                width: w,
                                height: h,
                                depth_or_array_layers: 1,
                            },
                        );
                    }

                    resources.reference_image_texture = Some(TextureSlice {
                        texture,
                        texture_view,
                        height: h,
                    });
                } else {
                    resources.reference_image_texture = None;
                }

                // Helper to create bind group
                let create_bind_group = |gpu_slices: &[TextureSlice],
                                         ref_view: &iced::wgpu::TextureView,
                                         sel_mask_view: &iced::wgpu::TextureView,
                                         tool_mask_view: &iced::wgpu::TextureView,
                                         label: &str|
                 -> iced::wgpu::BindGroup {
                    let mut bind_entries: Vec<iced::wgpu::BindGroupEntry> = Vec::with_capacity(MAX_TEXTURE_SLICES + 6);

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

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: MAX_TEXTURE_SLICES as u32,
                        resource: iced::wgpu::BindingResource::Sampler(&pipeline.sampler),
                    });

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 1) as u32,
                        resource: resources.uniform_buffer.as_entire_binding(),
                    });

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 2) as u32,
                        resource: resources.monitor_color_buffer.as_entire_binding(),
                    });

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 3) as u32,
                        resource: iced::wgpu::BindingResource::TextureView(ref_view),
                    });

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 4) as u32,
                        resource: iced::wgpu::BindingResource::TextureView(sel_mask_view),
                    });

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 5) as u32,
                        resource: iced::wgpu::BindingResource::TextureView(tool_mask_view),
                    });

                    device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                        label: Some(&format!("Terminal BindGroup {} Instance {}", label, id)),
                        layout: &pipeline.bind_group_layout,
                        entries: &bind_entries,
                    })
                };

                // Recreate bind groups with new reference image
                let ref_view = resources
                    .reference_image_texture
                    .as_ref()
                    .map(|t| &t.texture_view)
                    .unwrap_or(&pipeline.dummy_texture_view);
                let sel_mask_view = resources
                    .selection_mask_texture
                    .as_ref()
                    .map(|t| &t.texture_view)
                    .unwrap_or(&pipeline.dummy_texture_view);
                let tool_mask_view = resources
                    .tool_overlay_mask_texture
                    .as_ref()
                    .map(|t| &t.texture_view)
                    .unwrap_or(&pipeline.dummy_texture_view);
                resources.bind_group_blink_off = create_bind_group(&resources._slices_blink_off, ref_view, sel_mask_view, tool_mask_view, "BlinkOff");
                resources.bind_group_blink_on = create_bind_group(&resources._slices_blink_on, ref_view, sel_mask_view, tool_mask_view, "BlinkOn");
                resources.reference_image_hash = ref_image_hash;
            }
        }

        // Check if selection mask changed and needs update
        let sel_mask_hash = Self::compute_selection_mask_hash(&self.selection_mask_data);
        let needs_sel_mask_update = match pipeline.instances.get(&id) {
            Some(resources) => resources.selection_mask_hash != sel_mask_hash,
            None => false,
        };

        if needs_sel_mask_update {
            if let Some(resources) = pipeline.instances.get_mut(&id) {
                // Create or update selection mask texture
                if let Some((data, width, height)) = &self.selection_mask_data {
                    let w = (*width).max(1).min(MAX_TEXTURE_DIMENSION);
                    let h = (*height).max(1).min(MAX_TEXTURE_DIMENSION);

                    let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                        label: Some(&format!("Selection Mask Instance {}", id)),
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

                    let texture_view = texture.create_view(&iced::wgpu::TextureViewDescriptor::default());

                    // Write mask data to texture
                    if !data.is_empty() {
                        queue.write_texture(
                            iced::wgpu::TexelCopyTextureInfo {
                                texture: &texture,
                                mip_level: 0,
                                origin: iced::wgpu::Origin3d::ZERO,
                                aspect: iced::wgpu::TextureAspect::All,
                            },
                            data,
                            iced::wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(4 * w),
                                rows_per_image: Some(h),
                            },
                            iced::wgpu::Extent3d {
                                width: w,
                                height: h,
                                depth_or_array_layers: 1,
                            },
                        );
                    }

                    resources.selection_mask_texture = Some(TextureSlice {
                        texture,
                        texture_view,
                        height: h,
                    });
                } else {
                    resources.selection_mask_texture = None;
                }

                // Helper to create bind group
                let create_bind_group = |gpu_slices: &[TextureSlice],
                                         ref_view: &iced::wgpu::TextureView,
                                         sel_mask_view: &iced::wgpu::TextureView,
                                         tool_mask_view: &iced::wgpu::TextureView,
                                         label: &str|
                 -> iced::wgpu::BindGroup {
                    let mut bind_entries: Vec<iced::wgpu::BindGroupEntry> = Vec::with_capacity(MAX_TEXTURE_SLICES + 6);

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

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: MAX_TEXTURE_SLICES as u32,
                        resource: iced::wgpu::BindingResource::Sampler(&pipeline.sampler),
                    });

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 1) as u32,
                        resource: resources.uniform_buffer.as_entire_binding(),
                    });

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 2) as u32,
                        resource: resources.monitor_color_buffer.as_entire_binding(),
                    });

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 3) as u32,
                        resource: iced::wgpu::BindingResource::TextureView(ref_view),
                    });

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 4) as u32,
                        resource: iced::wgpu::BindingResource::TextureView(sel_mask_view),
                    });

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 5) as u32,
                        resource: iced::wgpu::BindingResource::TextureView(tool_mask_view),
                    });

                    device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                        label: Some(&format!("Terminal BindGroup {} Instance {}", label, id)),
                        layout: &pipeline.bind_group_layout,
                        entries: &bind_entries,
                    })
                };

                // Recreate bind groups with new selection mask
                let ref_view = resources
                    .reference_image_texture
                    .as_ref()
                    .map(|t| &t.texture_view)
                    .unwrap_or(&pipeline.dummy_texture_view);
                let sel_mask_view = resources
                    .selection_mask_texture
                    .as_ref()
                    .map(|t| &t.texture_view)
                    .unwrap_or(&pipeline.dummy_texture_view);
                let tool_mask_view = resources
                    .tool_overlay_mask_texture
                    .as_ref()
                    .map(|t| &t.texture_view)
                    .unwrap_or(&pipeline.dummy_texture_view);
                resources.bind_group_blink_off = create_bind_group(&resources._slices_blink_off, ref_view, sel_mask_view, tool_mask_view, "BlinkOff");
                resources.bind_group_blink_on = create_bind_group(&resources._slices_blink_on, ref_view, sel_mask_view, tool_mask_view, "BlinkOn");
                resources.selection_mask_hash = sel_mask_hash;
            }
        }

        // Check if tool overlay mask changed and needs update
        let tool_mask_hash = Self::compute_tool_overlay_mask_hash(&self.tool_overlay_mask_data);
        let needs_tool_mask_update = match pipeline.instances.get(&id) {
            Some(resources) => resources.tool_overlay_mask_hash != tool_mask_hash,
            None => false,
        };

        if needs_tool_mask_update {
            if let Some(resources) = pipeline.instances.get_mut(&id) {
                // Create or update tool overlay mask texture
                if let Some((data, width, height)) = &self.tool_overlay_mask_data {
                    let w = (*width).max(1).min(MAX_TEXTURE_DIMENSION);
                    let h = (*height).max(1).min(MAX_TEXTURE_DIMENSION);

                    let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                        label: Some(&format!("Tool Overlay Mask Instance {}", id)),
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

                    let texture_view = texture.create_view(&iced::wgpu::TextureViewDescriptor::default());

                    // Write mask data to texture
                    if !data.is_empty() {
                        queue.write_texture(
                            iced::wgpu::TexelCopyTextureInfo {
                                texture: &texture,
                                mip_level: 0,
                                origin: iced::wgpu::Origin3d::ZERO,
                                aspect: iced::wgpu::TextureAspect::All,
                            },
                            data,
                            iced::wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(4 * w),
                                rows_per_image: Some(h),
                            },
                            iced::wgpu::Extent3d {
                                width: w,
                                height: h,
                                depth_or_array_layers: 1,
                            },
                        );
                    }

                    resources.tool_overlay_mask_texture = Some(TextureSlice {
                        texture,
                        texture_view,
                        height: h,
                    });
                } else {
                    resources.tool_overlay_mask_texture = None;
                }

                // Helper to create bind group
                let create_bind_group = |gpu_slices: &[TextureSlice],
                                         ref_view: &iced::wgpu::TextureView,
                                         sel_mask_view: &iced::wgpu::TextureView,
                                         tool_mask_view: &iced::wgpu::TextureView,
                                         label: &str|
                 -> iced::wgpu::BindGroup {
                    let mut bind_entries: Vec<iced::wgpu::BindGroupEntry> = Vec::with_capacity(MAX_TEXTURE_SLICES + 6);

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

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: MAX_TEXTURE_SLICES as u32,
                        resource: iced::wgpu::BindingResource::Sampler(&pipeline.sampler),
                    });

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 1) as u32,
                        resource: resources.uniform_buffer.as_entire_binding(),
                    });

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 2) as u32,
                        resource: resources.monitor_color_buffer.as_entire_binding(),
                    });

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 3) as u32,
                        resource: iced::wgpu::BindingResource::TextureView(ref_view),
                    });

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 4) as u32,
                        resource: iced::wgpu::BindingResource::TextureView(sel_mask_view),
                    });

                    bind_entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 5) as u32,
                        resource: iced::wgpu::BindingResource::TextureView(tool_mask_view),
                    });

                    device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                        label: Some(&format!("Terminal BindGroup {} Instance {}", label, id)),
                        layout: &pipeline.bind_group_layout,
                        entries: &bind_entries,
                    })
                };

                // Recreate bind groups with new tool overlay mask
                let ref_view = resources
                    .reference_image_texture
                    .as_ref()
                    .map(|t| &t.texture_view)
                    .unwrap_or(&pipeline.dummy_texture_view);
                let sel_mask_view = resources
                    .selection_mask_texture
                    .as_ref()
                    .map(|t| &t.texture_view)
                    .unwrap_or(&pipeline.dummy_texture_view);
                let tool_mask_view = resources
                    .tool_overlay_mask_texture
                    .as_ref()
                    .map(|t| &t.texture_view)
                    .unwrap_or(&pipeline.dummy_texture_view);

                resources.bind_group_blink_off = create_bind_group(&resources._slices_blink_off, ref_view, sel_mask_view, tool_mask_view, "BlinkOff");
                resources.bind_group_blink_on = create_bind_group(&resources._slices_blink_on, ref_view, sel_mask_view, tool_mask_view, "BlinkOn");
                resources.tool_overlay_mask_hash = tool_mask_hash;
            }
        }

        // Update per-frame physical viewport for this instance.
        // `render()` only receives `clip_bounds` (scissor), so we persist the
        // real widget bounds here to prevent viewport-based scaling.
        if let Some(resources) = pipeline.instances.get_mut(&id) {
            let vp_x = (bounds.x * scale_factor).round();
            let vp_y = (bounds.y * scale_factor).round();
            let vp_w = (bounds.width * scale_factor).round().max(1.0);
            let vp_h = (bounds.height * scale_factor).round().max(1.0);
            resources.viewport_px = [vp_x, vp_y, vp_w, vp_h];
        }

        // Update uniforms every frame
        let Some(resources) = pipeline.instances.get(&id) else {
            return;
        };

        // Calculate display size
        // IMPORTANT: The CRT shader maps `uv.x` over `visible_width` (document pixels).
        // Using the full texture width here would make auto-scale and `terminal_area`
        // inconsistent and can lead to perceived stretching.
        let term_w = self.visible_width.max(1.0);
        let term_h = self.visible_height.max(1.0);
        let avail_w = bounds.width.max(1.0);
        let avail_h = bounds.height.max(1.0);
        let use_int = self.monitor_settings.use_integer_scaling;
        let final_scale = self.monitor_settings.scaling_mode.compute_zoom(term_w, term_h, avail_w, avail_h, use_int);
        let scaled_w = (term_w * final_scale).min(avail_w);
        let scaled_h = (term_h * final_scale).min(avail_h);

        // terminal_rect is consumed by WGSL as (start_x, start_y, width, height) in normalized 0-1 coords.
        let offset_x = ((avail_w - scaled_w) / 2.0).max(0.0);
        let offset_y = ((avail_h - scaled_h) / 2.0).max(0.0);
        let start_x = offset_x / avail_w;
        let start_y = offset_y / avail_h;
        let width_n = scaled_w / avail_w;
        let height_n = scaled_h / avail_h;
        let terminal_rect = [start_x, start_y, width_n, height_n];

        // Mouse events are handled in widget-local logical coordinates.
        // Keep RenderInfo in the same space (do NOT use clip/scissor rectangles).
        {
            let mut info = self.render_info.write();
            info.display_scale = final_scale;
            info.viewport_x = if use_int { offset_x.round() } else { offset_x };
            info.viewport_y = if use_int { offset_y.round() } else { offset_y };
            info.viewport_width = scaled_w;
            info.viewport_height = scaled_h;
            info.terminal_width = term_w;
            info.terminal_height = term_h;
            info.font_width = self.font_width;
            info.font_height = self.font_height;
            info.scan_lines = self.scan_lines;
            info.bounds_x = 0.0;
            info.bounds_y = 0.0;
            info.bounds_width = avail_w;
            info.bounds_height = avail_h;
        }

        // Pack slice heights into 3 vec4s (matches WGSL packing)
        // slice_heights[0] = [h0, h1, h2, first_slice_start_y]
        // slice_heights[1] = [h3, h4, h5, h6]
        // slice_heights[2] = [h7, h8, h9, 0]
        let mut slice_heights = [[0.0f32; 4]; 3];
        for (i, &height) in self.slice_heights.iter().enumerate().take(MAX_TEXTURE_SLICES) {
            match i {
                0 => slice_heights[0][0] = height as f32,
                1 => slice_heights[0][1] = height as f32,
                2 => slice_heights[0][2] = height as f32,
                3 => slice_heights[1][0] = height as f32,
                4 => slice_heights[1][1] = height as f32,
                5 => slice_heights[1][2] = height as f32,
                6 => slice_heights[1][3] = height as f32,
                7 => slice_heights[2][0] = height as f32,
                8 => slice_heights[2][1] = height as f32,
                9 => slice_heights[2][2] = height as f32,
                _ => {}
            }
        }
        slice_heights[0][3] = self.first_slice_start_y;

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
            num_slices: self.slices_blink_off.len() as f32, // Both have same count
            total_image_height: self.total_content_height,
            scroll_offset_y: self.scroll_offset_y,
            visible_height: self.visible_height,
            slice_heights,
            // X-axis scrolling uniforms
            scroll_offset_x: self.scroll_offset_x,
            visible_width: self.visible_width,
            texture_width: self.texture_width as f32,
            _x_padding: 0.0,
            // Caret uniforms
            caret_pos: self.caret_pos,
            caret_size: self.caret_size,
            caret_visible: if self.caret_visible { 1.0 } else { 0.0 },
            caret_mode: self.caret_mode as f32,
            _caret_padding: [0.0; 2],

            // Marker uniforms
            raster_spacing: self.raster_spacing.map_or([0.0, 0.0], |(w, h)| [w, h]),
            _raster_spacing_padding: [0.0; 2],
            raster_color: self.raster_color,
            raster_alpha: self.raster_alpha,
            raster_enabled: if self.raster_spacing.is_some() { 1.0 } else { 0.0 },
            _raster_padding: [0.0; 2],

            guide_pos: self.guide_pos.map_or([-1.0, -1.0], |(x, y)| [x, y]),
            _guide_pos_padding: [0.0; 2],
            guide_color: self.guide_color,
            guide_alpha: self.guide_alpha,
            guide_enabled: if self.guide_pos.is_some() { 1.0 } else { 0.0 },
            _marker_padding: [0.0; 2],

            // Reference image uniforms
            ref_image_enabled: if self.reference_image_enabled && self.reference_image_data.is_some() {
                1.0
            } else {
                0.0
            },
            ref_image_alpha: self.reference_image_alpha,
            ref_image_mode: self.reference_image_mode as f32,
            _ref_padding: 0.0,
            ref_image_offset: self.reference_image_offset,
            ref_image_scale: [self.reference_image_scale, self.reference_image_scale],
            ref_image_size: self.reference_image_data.as_ref().map_or([1.0, 1.0], |(_, w, h)| [*w as f32, *h as f32]),
            _ref_padding2: [0.0; 2],

            // Layer bounds uniforms
            layer_rect: self.layer_rect.unwrap_or([0.0, 0.0, 0.0, 0.0]),
            layer_color: self.layer_color,
            layer_enabled: if self.show_layer_bounds { 1.0 } else { 0.0 },
            _layer_padding: [0.0; 3],

            // Selection uniforms
            selection_rect: self.selection_rect.unwrap_or([0.0, 0.0, 0.0, 0.0]),
            selection_color: self.selection_color,
            selection_enabled: if self.selection_rect.is_some() || self.selection_mask_data.is_some() {
                1.0
            } else {
                0.0
            },
            selection_mask_enabled: if self.selection_mask_data.is_some() { 1.0 } else { 0.0 },
            _selection_padding: [0.0; 2],

            // Brush/Pencil preview uniforms
            brush_preview_rect: self.brush_preview_rect.unwrap_or([0.0, 0.0, 0.0, 0.0]),
            brush_preview_enabled: if self.brush_preview_rect.is_some() { 1.0 } else { 0.0 },
            _brush_preview_padding0: [0.0; 3],
            _brush_preview_padding: [0.0; 4],

            // Font dimensions for selection mask
            font_width: self.font_dimensions.map(|(w, _)| w).unwrap_or(8.0),
            font_height: self.font_dimensions.map(|(_, h)| h).unwrap_or(16.0),
            selection_mask_size: if let Some((_, w, h)) = &self.selection_mask_data {
                [*w as f32, *h as f32]
            } else {
                [1.0, 1.0]
            },

            tool_overlay_params: if let Some((_, w, h)) = &self.tool_overlay_mask_data {
                [*w as f32, *h as f32, self.tool_overlay_cell_height_scale, 1.0]
            } else {
                [1.0, 1.0, 1.0, 0.0]
            },

            terminal_rect,
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

        // IMPORTANT:
        // Use the widget bounds as viewport and `clip_bounds` only as scissor.
        // If we used `clip_bounds` as viewport, we'd rescale the whole terminal
        // whenever it is clipped, which can lead to perceived stretching.
        let (mut vp_x, mut vp_y, mut vp_w, mut vp_h) = (0.0f32, 0.0f32, 1.0f32, 1.0f32);
        if let Some(resources) = pipeline.instances.get(&self.instance_id) {
            vp_x = resources.viewport_px[0];
            vp_y = resources.viewport_px[1];
            vp_w = resources.viewport_px[2];
            vp_h = resources.viewport_px[3];
        }

        // NOTE: RenderInfo is updated in `prepare()` in widget-local logical coordinates.
        // Do not overwrite it here with `clip_bounds`-derived values.

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
            // Select bind group based on current blink state
            let bind_group = if self.blink_on {
                &resources.bind_group_blink_on
            } else {
                &resources.bind_group_blink_off
            };
            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        drop(render_pass);
        encoder.pop_debug_group();
    }
}
