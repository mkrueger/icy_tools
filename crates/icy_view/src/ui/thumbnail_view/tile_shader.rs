//! Shader-based tile grid rendering
//!
//! Uses wgpu textures to render thumbnails efficiently in a grid layout.
//! Supports very tall images (up to 80,000px) by splitting into multiple texture slices.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use iced::Rectangle;
use iced::widget::shader;

// ============================================================================
// Texture Slicing Constants
// ============================================================================

/// Maximum height per texture slice (GPU limit is typically 8192, we use 8000 for safety)
pub const MAX_SLICE_HEIGHT: u32 = 8000;

/// Maximum number of texture slices per tile (10 slices * 8000px = 80,000px max height)
pub const MAX_TEXTURE_SLICES: usize = 10;

// ============================================================================
// Tile Geometry Constants (the three base values everything derives from)
// ============================================================================

/// Width of the image content area inside a tile
pub const TILE_IMAGE_WIDTH: f32 = 320.0;

/// Border width for tiles
pub const TILE_BORDER_WIDTH: f32 = 2.0;

/// Padding inside the tile border (between border and image)
pub const TILE_INNER_PADDING: f32 = 4.0;

/// Maximum viewport stripe height for multi-pass rendering (in logical pixels)
/// Set to 4000 to ensure we stay under 8192px even at 2x DPI scaling
pub const MAX_VIEWPORT_STRIPE_HEIGHT: f32 = 4000.0;

/// Maximum display height for a tile image (removed - we now support full height via multi-pass)
/// Kept for reference: pub const MAX_TILE_IMAGE_HEIGHT: f32 = 4000.0;

// ============================================================================
// Derived Tile Geometry (calculated from base values)
// ============================================================================

/// Total padding on each side of the image (border + inner padding)
pub const TILE_PADDING: f32 = TILE_BORDER_WIDTH + TILE_INNER_PADDING;

/// Full tile width including borders and padding
pub const TILE_WIDTH: f32 = TILE_IMAGE_WIDTH + TILE_PADDING * 2.0;

/// Spacing between tiles in the grid
pub const TILE_SPACING: f32 = 16.0;

// ============================================================================
// Tile Visual Styling Constants
// ============================================================================

/// Corner radius for tile borders
pub const TILE_CORNER_RADIUS: f32 = 8.0;

/// Drop shadow offset X
pub const SHADOW_OFFSET_X: f32 = 3.0;

/// Drop shadow offset Y  
pub const SHADOW_OFFSET_Y: f32 = 4.0;

/// Drop shadow blur radius (visual approximation)
pub const SHADOW_BLUR_RADIUS: f32 = 6.0;

/// Drop shadow opacity (0.0 - 1.0)
pub const SHADOW_OPACITY: f32 = 0.35;

// ============================================================================
// Tile Data Structures
// ============================================================================

/// A single tile's texture data
#[derive(Debug, Clone)]
pub struct TileTexture {
    /// Unique ID for this tile
    pub id: u64,
    /// RGBA pixel data (Arc for cheap cloning)
    pub rgba_data: Arc<Vec<u8>>,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Label RGBA data (separate texture for GPU rendering)
    pub label_rgba: Option<Arc<Vec<u8>>>,
    /// Label raw texture dimensions (width, height) - for texture creation
    pub label_raw_size: (u32, u32),
    /// Label display dimensions (width, height) - scaled to fit tile
    pub label_size: (u32, u32),
    /// Position in the grid (x, y)
    pub position: (f32, f32),
    /// Display size (width, height) - full tile including label area
    pub display_size: (f32, f32),
    /// Image height (without label area)
    pub image_height: f32,
    /// Is this tile selected?
    pub is_selected: bool,
    /// Is this tile hovered?
    pub is_hovered: bool,
}

/// Shader primitive for rendering a tile grid
#[derive(Debug, Clone)]
pub struct TileGridShader {
    /// All tiles to render
    pub tiles: Vec<TileTexture>,
    /// Scroll offset
    pub scroll_y: f32,
    /// Viewport height
    pub viewport_height: f32,
}

/// Uniforms for the tile shader (v4 - multi-texture slicing)
/// All rectangles are in pixel coordinates with 0,0 = top-left
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TileUniforms {
    /// Total tile size including shadow area (width, height)
    tile_size: [f32; 2],
    /// Padding for alignment
    _pad0: [f32; 2],
    /// Content rectangle (tile without shadow): x, y, width, height
    content_rect: [f32; 4],
    /// Image rectangle: x, y, width, height
    image_rect: [f32; 4],
    /// Label rectangle: x, y, width, height
    label_rect: [f32; 4],
    /// Is selected (0.0 or 1.0)
    is_selected: f32,
    /// Is hovered (0.0 or 1.0)
    is_hovered: f32,
    /// Border radius
    border_radius: f32,
    /// Border width
    border_width: f32,
    /// Shadow blur
    shadow_blur: f32,
    /// Shadow opacity
    shadow_opacity: f32,
    /// Number of texture slices (1-10)
    num_slices: f32,
    /// Total image height in pixels (source texture height)
    total_image_height: f32,
    /// Viewport Y offset for viewport slicing (0 for first stripe, stripe_height for second, etc.)
    /// This tells the shader which vertical portion of the tile is being rendered
    viewport_y_offset: f32,
    /// Implicit alignment padding (3 floats to align next vec4 to 16-byte boundary)
    _pad_align: [f32; 3],
    /// Padding for alignment (must be vec4 = 16 bytes to align slice_heights array in WGSL)
    _pad1: [f32; 4],
    /// Extra padding (vec4 = 16 bytes for WGSL alignment)
    _pad2: [f32; 4],
    /// Heights of each slice in pixels (packed as 3 vec4s = 12 floats for 10 slices + 2 padding)
    slice_heights: [[f32; 4]; 3],
    /// Label texture size (width, height, 0, 0) - 0,0 if no label
    label_texture_size: [f32; 4],
}

impl TileUniforms {
    /// Create uniforms for a tile with pre-computed rectangles and slice info
    /// `viewport_y_offset` specifies which vertical portion of the tile is being rendered (in pixels)
    /// `stripe_height` is the height of this rendering stripe (for viewport sizing)
    fn new(tile: &TileTexture, slice_heights: &[u32], viewport_y_offset: f32, stripe_height: f32) -> Self {
        let shadow_extra_x = SHADOW_OFFSET_X + SHADOW_BLUR_RADIUS;
        let shadow_extra_y = SHADOW_OFFSET_Y + SHADOW_BLUR_RADIUS;

        // Total size including shadow - use stripe height for this pass
        let total_width = tile.display_size.0 + shadow_extra_x;
        let total_height = stripe_height + shadow_extra_y;

        // Content rect (tile without shadow) - positioned at top-left
        let content_x = 0.0;
        let content_y = 0.0;
        let content_w = tile.display_size.0;
        let content_h = tile.display_size.1;

        // Image rect - inside content with padding
        let padding = TILE_BORDER_WIDTH + TILE_INNER_PADDING;
        let image_x = padding;
        let image_y = padding;
        let image_w = content_w - padding * 2.0;

        // Image height from the tile (actual rendered image height, not display height)
        let image_h = tile.image_height;

        // Label rect - below image with separator
        let label_x = padding;
        let label_y = image_y + image_h + TILE_INNER_PADDING;
        let label_w = image_w;
        let label_h = tile.label_size.1 as f32;

        // Pack slice heights into 3 vec4s (12 floats for 10 slices + 2 padding)
        let mut packed_heights = [[0.0f32; 4]; 3];
        for (i, &h) in slice_heights.iter().enumerate().take(MAX_TEXTURE_SLICES) {
            packed_heights[i / 4][i % 4] = h as f32;
        }

        Self {
            tile_size: [total_width, total_height],
            _pad0: [0.0, 0.0],
            content_rect: [content_x, content_y, content_w, content_h],
            image_rect: [image_x, image_y, image_w, image_h],
            label_rect: [label_x, label_y, label_w, label_h],
            is_selected: if tile.is_selected { 1.0 } else { 0.0 },
            is_hovered: if tile.is_hovered { 1.0 } else { 0.0 },
            border_radius: TILE_CORNER_RADIUS,
            border_width: TILE_BORDER_WIDTH,
            shadow_blur: SHADOW_BLUR_RADIUS,
            shadow_opacity: SHADOW_OPACITY,
            num_slices: slice_heights.len().min(MAX_TEXTURE_SLICES) as f32,
            total_image_height: tile.height as f32,
            viewport_y_offset,
            _pad_align: [0.0, 0.0, 0.0],
            _pad1: [0.0, 0.0, 0.0, 0.0],
            _pad2: [0.0, 0.0, 0.0, 0.0],
            slice_heights: packed_heights,
            label_texture_size: [tile.label_size.0 as f32, tile.label_size.1 as f32, 0.0, 0.0],
        }
    }
}

/// Texture slice for a single portion of a tall image
struct TextureSlice {
    #[allow(dead_code)]
    texture: iced::wgpu::Texture,
    texture_view: iced::wgpu::TextureView,
    height: u32,
}

/// Shared texture resources - now supports multiple slices for tall images
/// NOTE: Labels are NOT stored here because multiple tiles can share the same image
/// but have different labels (e.g., all folders use the same icon but different names)
#[allow(dead_code)]
struct SharedTextureResources {
    /// Texture slices (1-10 slices depending on image height)
    slices: Vec<TextureSlice>,
    /// Original image dimensions
    texture_size: (u32, u32),
}

/// A single viewport stripe for multi-pass rendering of tall tiles
struct StripeResources {
    bind_group: iced::wgpu::BindGroup,
    uniform_buffer: iced::wgpu::Buffer,
    /// Y offset in display pixels for this stripe
    y_offset: f32,
    /// Height of this stripe in display pixels
    height: f32,
}

/// Per-tile GPU resources (unique to each tile for position/hover/selection state)
#[allow(dead_code)]
struct TileResources {
    /// Key to look up shared texture resources
    texture_key: usize,
    /// Stripes for multi-pass rendering (1 for small tiles, multiple for tall tiles)
    stripes: Vec<StripeResources>,
}

// ============================================================================
// Label Rendering Structures
// ============================================================================

/// Uniforms for the label shader (must match tile_label.wgsl)
/// WGSL alignment rules:
/// - vec2<f32> has 8-byte alignment
/// - vec4<f32> has 16-byte alignment  
/// - Struct must be 16-byte aligned
/// Total size: 48 bytes
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LabelUniforms {
    /// Viewport size (width, height in pixels)
    viewport_size: [f32; 2],
    /// Padding for alignment
    _pad0: [f32; 2],
    /// Label texture dimensions (raw texture size)
    texture_size: [f32; 2],
    /// Display dimensions (scaled to fit tile width)
    display_size: [f32; 2],
    /// Packed: is_hovered, 0, 0, 0 (using vec4 to avoid vec3 alignment issues)
    hover_and_pad: [f32; 4],
}

/// Resources for rendering a tile's label
/// Each tile has its own label texture (not shared) because tiles sharing
/// the same image can have different labels (e.g., folders)
#[allow(dead_code)]
struct LabelResources {
    /// Label texture (owned by this tile)
    texture: iced::wgpu::Texture,
    /// Label texture view
    texture_view: iced::wgpu::TextureView,
    /// Bind group for rendering
    bind_group: iced::wgpu::BindGroup,
    /// Uniform buffer
    uniform_buffer: iced::wgpu::Buffer,
    /// Display Y position (below image)
    display_y: f32,
    /// Display width
    display_width: f32,
    /// Display height
    display_height: f32,
}

/// Renderer for the tile grid shader
pub struct TileGridShaderRenderer {
    // Main tile pipeline
    pipeline: iced::wgpu::RenderPipeline,
    bind_group_layout: iced::wgpu::BindGroupLayout,
    sampler: iced::wgpu::Sampler,
    /// 1x1 transparent texture for unused texture slots
    dummy_texture_view: iced::wgpu::TextureView,
    /// Per-tile resources (unique to each tile)
    tiles: HashMap<u64, TileResources>,
    /// Shared texture resources keyed by Arc pointer address
    shared_textures: HashMap<usize, SharedTextureResources>,

    // Label rendering pipeline
    label_pipeline: iced::wgpu::RenderPipeline,
    label_bind_group_layout: iced::wgpu::BindGroupLayout,
    /// Per-tile label resources
    label_resources: HashMap<u64, LabelResources>,
}

static TILE_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generate a unique tile ID
pub fn new_tile_id() -> u64 {
    TILE_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

impl shader::Pipeline for TileGridShaderRenderer {
    fn new(device: &iced::wgpu::Device, _queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        // Load shader from external WGSL file for better syntax highlighting and maintainability
        let shader_source = include_str!("tile_grid.wgsl");

        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("Tile Grid Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Create bind group layout with 10 texture slots + sampler + uniforms + label texture
        let mut entries = Vec::with_capacity(MAX_TEXTURE_SLICES + 3);

        // Add 10 texture bindings (0-9)
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

        // Sampler at binding 10
        entries.push(iced::wgpu::BindGroupLayoutEntry {
            binding: MAX_TEXTURE_SLICES as u32,
            visibility: iced::wgpu::ShaderStages::FRAGMENT,
            ty: iced::wgpu::BindingType::Sampler(iced::wgpu::SamplerBindingType::Filtering),
            count: None,
        });

        // Uniforms at binding 11
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

        // Label texture at binding 12
        entries.push(iced::wgpu::BindGroupLayoutEntry {
            binding: (MAX_TEXTURE_SLICES + 2) as u32,
            visibility: iced::wgpu::ShaderStages::FRAGMENT,
            ty: iced::wgpu::BindingType::Texture {
                multisampled: false,
                view_dimension: iced::wgpu::TextureViewDimension::D2,
                sample_type: iced::wgpu::TextureSampleType::Float { filterable: true },
            },
            count: None,
        });

        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("Tile Grid Bind Group Layout"),
            entries: &entries,
        });

        let pipeline_layout = device.create_pipeline_layout(&iced::wgpu::PipelineLayoutDescriptor {
            label: Some("Tile Grid Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some("Tile Grid Pipeline"),
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
            label: Some("Tile Grid Sampler"),
            address_mode_u: iced::wgpu::AddressMode::ClampToEdge,
            address_mode_v: iced::wgpu::AddressMode::ClampToEdge,
            address_mode_w: iced::wgpu::AddressMode::ClampToEdge,
            mag_filter: iced::wgpu::FilterMode::Linear,
            min_filter: iced::wgpu::FilterMode::Linear,
            mipmap_filter: iced::wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create 1x1 transparent dummy texture for unused slots
        let dummy_texture = device.create_texture(&iced::wgpu::TextureDescriptor {
            label: Some("Dummy Texture"),
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

        // ====================================================================
        // Label Pipeline Setup
        // ====================================================================
        let label_shader_source = include_str!("tile_label.wgsl");
        let label_shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("Label Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(label_shader_source.into()),
        });

        let label_bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("Label Bind Group Layout"),
            entries: &[
                // Label texture at binding 0
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
                // Sampler at binding 1
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: iced::wgpu::ShaderStages::FRAGMENT,
                    ty: iced::wgpu::BindingType::Sampler(iced::wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Uniforms at binding 2
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: iced::wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: iced::wgpu::BindingType::Buffer {
                        ty: iced::wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let label_pipeline_layout = device.create_pipeline_layout(&iced::wgpu::PipelineLayoutDescriptor {
            label: Some("Label Pipeline Layout"),
            bind_group_layouts: &[&label_bind_group_layout],
            push_constant_ranges: &[],
        });

        let label_pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some("Label Pipeline"),
            layout: Some(&label_pipeline_layout),
            vertex: iced::wgpu::VertexState {
                module: &label_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(iced::wgpu::FragmentState {
                module: &label_shader,
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

        TileGridShaderRenderer {
            pipeline,
            bind_group_layout,
            sampler,
            dummy_texture_view,
            tiles: HashMap::new(),
            shared_textures: HashMap::new(),
            label_pipeline,
            label_bind_group_layout,
            label_resources: HashMap::new(),
        }
    }
}

impl shader::Primitive for TileGridShader {
    type Pipeline = TileGridShaderRenderer;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        _bounds: &Rectangle,
        viewport: &iced::advanced::graphics::Viewport,
    ) {
        // Set scale factor for HiDPI/Retina displays on first prepare
        icy_engine_gui::set_scale_factor(viewport.scale_factor() as f32);

        // Track which shared textures are still in use
        let mut used_texture_keys: std::collections::HashSet<usize> = std::collections::HashSet::new();

        // Update or create textures for each tile
        for tile in &self.tiles {
            // Skip empty tiles
            if tile.width == 0 || tile.height == 0 || tile.rgba_data.is_empty() {
                continue;
            }

            // Validate data size matches expected texture size
            let expected_size = (4 * tile.width * tile.height) as usize;
            if tile.rgba_data.len() != expected_size {
                log::warn!(
                    "Tile {} has mismatched data size: expected {} bytes ({}x{}x4), got {} bytes. Skipping.",
                    tile.id,
                    expected_size,
                    tile.width,
                    tile.height,
                    tile.rgba_data.len()
                );
                continue;
            }

            // Use Arc pointer as key for shared texture lookup
            let texture_key = Arc::as_ptr(&tile.rgba_data) as usize;
            used_texture_keys.insert(texture_key);

            // Create shared texture slices if they don't exist
            if !pipeline.shared_textures.contains_key(&texture_key) {
                let texture_start = Instant::now();

                // Calculate how many slices we need
                let total_height = tile.height;
                let num_slices = ((total_height as usize + MAX_SLICE_HEIGHT as usize - 1) / MAX_SLICE_HEIGHT as usize).min(MAX_TEXTURE_SLICES);

                if num_slices > 1 {
                    log::info!("[TileShader] Creating {} slices for tile {}x{}", num_slices, tile.width, tile.height);
                }

                let mut slices = Vec::with_capacity(num_slices);
                let mut y_offset = 0u32;

                log::debug!(
                    "[TileShader] Creating texture slices for tile {} ({}x{}) with key {:x} num: {num_slices}",
                    tile.id,
                    tile.width,
                    tile.height,
                    texture_key
                );
                for slice_idx in 0..num_slices {
                    let remaining_height = total_height.saturating_sub(y_offset);
                    let slice_height = remaining_height.min(MAX_SLICE_HEIGHT);

                    if slice_height == 0 {
                        break;
                    }

                    let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                        label: Some(&format!("Texture Slice {:x}_{}", texture_key, slice_idx)),
                        size: iced::wgpu::Extent3d {
                            width: tile.width,
                            height: slice_height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: iced::wgpu::TextureDimension::D2,
                        format: iced::wgpu::TextureFormat::Rgba8Unorm,
                        usage: iced::wgpu::TextureUsages::TEXTURE_BINDING | iced::wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });

                    // Upload slice data
                    let bytes_per_row = 4 * tile.width;
                    let slice_start = (y_offset * bytes_per_row) as usize;
                    let slice_end = ((y_offset + slice_height) * bytes_per_row) as usize;
                    let slice_data = &tile.rgba_data[slice_start..slice_end];

                    queue.write_texture(
                        iced::wgpu::TexelCopyTextureInfo {
                            texture: &texture,
                            mip_level: 0,
                            origin: iced::wgpu::Origin3d::ZERO,
                            aspect: iced::wgpu::TextureAspect::All,
                        },
                        slice_data,
                        iced::wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(bytes_per_row),
                            rows_per_image: Some(slice_height),
                        },
                        iced::wgpu::Extent3d {
                            width: tile.width,
                            height: slice_height,
                            depth_or_array_layers: 1,
                        },
                    );

                    let texture_view = texture.create_view(&iced::wgpu::TextureViewDescriptor::default());

                    slices.push(TextureSlice {
                        texture,
                        texture_view,
                        height: slice_height,
                    });

                    y_offset += slice_height;
                }

                log::debug!(
                    "[TIMING] Texture creation for tile {} ({}x{}): {:?}",
                    tile.id,
                    tile.width,
                    tile.height,
                    texture_start.elapsed()
                );

                // NOTE: Labels are NOT created here in shared_textures!
                // Each tile has its own label, even if they share the same image.
                // Labels are created per-tile in label_resources below.

                pipeline.shared_textures.insert(
                    texture_key,
                    SharedTextureResources {
                        slices,
                        texture_size: (tile.width, tile.height),
                    },
                );
            }

            // Check if per-tile resources need to be created or updated
            let needs_recreate = match pipeline.tiles.get(&tile.id) {
                Some(resources) => resources.texture_key != texture_key,
                None => true,
            };

            // Get the shared texture
            let shared_texture = pipeline.shared_textures.get(&texture_key).unwrap();
            let slice_heights: Vec<u32> = shared_texture.slices.iter().map(|s| s.height).collect();

            // Calculate how many viewport stripes we need for this tile
            // Use the full tile display height (not just image_height) for proper coverage
            let full_tile_height = tile.display_size.1;
            let num_stripes = ((full_tile_height / MAX_VIEWPORT_STRIPE_HEIGHT).ceil() as usize).max(1);

            // Use integer pixel boundaries for stripes to avoid floating-point gaps
            // Round stripe height up to ensure full coverage
            let stripe_height_int = (full_tile_height / num_stripes as f32).ceil() as i32;

            if needs_recreate || pipeline.tiles.get(&tile.id).map(|r| r.stripes.len() != num_stripes).unwrap_or(true) {
                // Create stripes for multi-pass rendering
                let mut stripes = Vec::with_capacity(num_stripes);

                for stripe_idx in 0..num_stripes {
                    // Use integer pixel boundaries to avoid gaps
                    let y_offset = (stripe_idx as i32 * stripe_height_int) as f32;
                    // Last stripe extends to exact tile height to avoid gaps
                    let stripe_height = if stripe_idx == num_stripes - 1 {
                        full_tile_height - y_offset
                    } else {
                        stripe_height_int as f32
                    };

                    // Create uniform buffer for this stripe
                    let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
                        label: Some(&format!("Tile {} Stripe {} Uniforms", tile.id, stripe_idx)),
                        size: std::mem::size_of::<TileUniforms>() as u64,
                        usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });

                    // Create bind group entries for all 10 texture slots + sampler + uniforms + label
                    let mut entries = Vec::with_capacity(MAX_TEXTURE_SLICES + 3);

                    for i in 0..MAX_TEXTURE_SLICES {
                        let texture_view = if i < shared_texture.slices.len() {
                            &shared_texture.slices[i].texture_view
                        } else {
                            &pipeline.dummy_texture_view
                        };
                        entries.push(iced::wgpu::BindGroupEntry {
                            binding: i as u32,
                            resource: iced::wgpu::BindingResource::TextureView(texture_view),
                        });
                    }

                    // Sampler at binding 10
                    entries.push(iced::wgpu::BindGroupEntry {
                        binding: MAX_TEXTURE_SLICES as u32,
                        resource: iced::wgpu::BindingResource::Sampler(&pipeline.sampler),
                    });

                    // Uniforms at binding 11
                    entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 1) as u32,
                        resource: uniform_buffer.as_entire_binding(),
                    });

                    // Label texture at binding 12 - use dummy texture since labels are rendered separately
                    entries.push(iced::wgpu::BindGroupEntry {
                        binding: (MAX_TEXTURE_SLICES + 2) as u32,
                        resource: iced::wgpu::BindingResource::TextureView(&pipeline.dummy_texture_view),
                    });

                    let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                        label: Some(&format!("Tile {} Stripe {} BindGroup", tile.id, stripe_idx)),
                        layout: &pipeline.bind_group_layout,
                        entries: &entries,
                    });

                    stripes.push(StripeResources {
                        bind_group,
                        uniform_buffer,
                        y_offset,
                        height: stripe_height,
                    });
                }

                if num_stripes > 1 {
                    log::info!(
                        "[TileShader] Created {} stripes for tile {} (full_tile_height={})",
                        num_stripes,
                        tile.id,
                        full_tile_height
                    );
                }

                pipeline.tiles.insert(tile.id, TileResources { texture_key, stripes });
            }

            // Update uniforms for all stripes of this tile
            if let Some(resources) = pipeline.tiles.get(&tile.id) {
                for stripe in &resources.stripes {
                    let uniforms = TileUniforms::new(tile, &slice_heights, stripe.y_offset, stripe.height);
                    queue.write_buffer(&stripe.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
                }
            }

            // Create or update label resources (per-tile, NOT shared!)
            // Each tile has its own label even if they share the same image
            if let Some(ref label_data) = tile.label_rgba {
                let (lw, lh) = tile.label_raw_size;
                if lw > 0 && lh > 0 && label_data.len() == (lw * lh * 4) as usize {
                    let needs_label_recreate = !pipeline.label_resources.contains_key(&tile.id);

                    if needs_label_recreate {
                        // Create label texture for THIS tile
                        let label_tex = device.create_texture(&iced::wgpu::TextureDescriptor {
                            label: Some(&format!("Tile {} Label Texture", tile.id)),
                            size: iced::wgpu::Extent3d {
                                width: lw,
                                height: lh,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: iced::wgpu::TextureDimension::D2,
                            format: iced::wgpu::TextureFormat::Rgba8Unorm,
                            usage: iced::wgpu::TextureUsages::TEXTURE_BINDING | iced::wgpu::TextureUsages::COPY_DST,
                            view_formats: &[],
                        });

                        queue.write_texture(
                            iced::wgpu::TexelCopyTextureInfo {
                                texture: &label_tex,
                                mip_level: 0,
                                origin: iced::wgpu::Origin3d::ZERO,
                                aspect: iced::wgpu::TextureAspect::All,
                            },
                            label_data,
                            iced::wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(4 * lw),
                                rows_per_image: Some(lh),
                            },
                            iced::wgpu::Extent3d {
                                width: lw,
                                height: lh,
                                depth_or_array_layers: 1,
                            },
                        );

                        let label_view = label_tex.create_view(&iced::wgpu::TextureViewDescriptor::default());

                        // Create uniform buffer for label
                        let label_uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
                            label: Some(&format!("Tile {} Label Uniforms", tile.id)),
                            size: std::mem::size_of::<LabelUniforms>() as u64,
                            usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });

                        let label_bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                            label: Some(&format!("Tile {} Label BindGroup", tile.id)),
                            layout: &pipeline.label_bind_group_layout,
                            entries: &[
                                iced::wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: iced::wgpu::BindingResource::TextureView(&label_view),
                                },
                                iced::wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: iced::wgpu::BindingResource::Sampler(&pipeline.sampler),
                                },
                                iced::wgpu::BindGroupEntry {
                                    binding: 2,
                                    resource: label_uniform_buffer.as_entire_binding(),
                                },
                            ],
                        });

                        // Calculate label display position (in logical pixels)
                        // These values are used in render() for viewport positioning
                        const LABEL_SCALE: f32 = 2.0;
                        let padding = TILE_BORDER_WIDTH;
                        let label_y = padding + tile.image_height;
                        let label_w = tile.display_size.0 - (TILE_BORDER_WIDTH + TILE_INNER_PADDING) * 2.0;
                        let label_h = tile.label_raw_size.1 as f32 * LABEL_SCALE; // 2x scaled height

                        pipeline.label_resources.insert(
                            tile.id,
                            LabelResources {
                                texture: label_tex,
                                texture_view: label_view,
                                bind_group: label_bind_group,
                                uniform_buffer: label_uniform_buffer,
                                display_y: label_y,
                                display_width: label_w,
                                display_height: label_h,
                            },
                        );
                    }

                    // Update label uniforms
                    if let Some(label_res) = pipeline.label_resources.get(&tile.id) {
                        // Get scale factor to convert between logical and physical pixels
                        let scale_factor = icy_engine_gui::get_scale_factor();

                        // viewport_size = the label area size in physical pixels (matches render viewport)
                        // texture_size = raw texture dimensions
                        // display_size = how big to render the texture in physical pixels
                        //
                        // Scale label 2x for better readability
                        const LABEL_SCALE: f32 = 2.0;
                        let content_width = tile.display_size.0 - (TILE_BORDER_WIDTH + TILE_INNER_PADDING) * 2.0;
                        let viewport_w = content_width * scale_factor;
                        let viewport_h = tile.label_raw_size.1 as f32 * LABEL_SCALE * scale_factor;

                        let label_uniforms = LabelUniforms {
                            viewport_size: [viewport_w, viewport_h],
                            _pad0: [0.0, 0.0],
                            texture_size: [tile.label_raw_size.0 as f32, tile.label_raw_size.1 as f32],
                            // Display at 2x scale for better readability
                            display_size: [tile.label_raw_size.0 as f32 * LABEL_SCALE, tile.label_raw_size.1 as f32 * LABEL_SCALE],
                            hover_and_pad: [if tile.is_hovered { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
                        };
                        queue.write_buffer(&label_res.uniform_buffer, 0, bytemuck::bytes_of(&label_uniforms));
                    }
                }
            }
        }

        // Remove tiles that are no longer needed
        let active_ids: std::collections::HashSet<u64> = self.tiles.iter().map(|t| t.id).collect();
        pipeline.tiles.retain(|id, _| active_ids.contains(id));
        pipeline.label_resources.retain(|id, _| active_ids.contains(id));

        // Remove shared textures that are no longer in use
        pipeline.shared_textures.retain(|key, _| used_texture_keys.contains(key));
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        // Get scale factor for HiDPI/Retina displays
        let scale_factor = icy_engine_gui::get_scale_factor();

        // Widget bounds in physical screen coordinates
        let widget_left = clip_bounds.x as f32;
        let widget_top = clip_bounds.y as f32;
        let widget_right = (clip_bounds.x + clip_bounds.width) as f32;
        let widget_bottom = (clip_bounds.y + clip_bounds.height) as f32;

        // Only render visible tiles
        for tile in &self.tiles {
            // Extra space needed for shadow (offset + blur) - in logical pixels
            let shadow_extra_x = SHADOW_OFFSET_X + SHADOW_BLUR_RADIUS;
            let shadow_extra_y = SHADOW_OFFSET_Y + SHADOW_BLUR_RADIUS;

            // Check if tile is visible (accounting for scroll and shadow) - in logical pixels
            let tile_top_logical = tile.position.1 - self.scroll_y;
            let tile_bottom_logical = tile_top_logical + tile.display_size.1 + shadow_extra_y;

            if tile_bottom_logical < 0.0 || tile_top_logical > self.viewport_height {
                continue; // Skip tiles outside viewport
            }

            if let Some(resources) = pipeline.tiles.get(&tile.id) {
                // Calculate tile position within widget bounds (physical screen coordinates)
                // Round to integer pixels to avoid floating-point gaps between stripes
                let tile_x = (widget_left + tile.position.0 * scale_factor).floor();
                let tile_y_base = (widget_top + tile_top_logical * scale_factor).floor();
                let tile_w = ((tile.display_size.0 + shadow_extra_x) * scale_factor).ceil();
                let _total_tile_h = ((tile.display_size.1 + shadow_extra_y) * scale_factor).ceil();

                // Render each stripe separately
                for stripe in &resources.stripes {
                    // Round stripe positions to integer pixels for exact alignment
                    let stripe_y = (tile_y_base + stripe.y_offset * scale_factor).floor();
                    let stripe_h = (stripe.height * scale_factor).ceil();

                    // GPU viewport limit (must cap physical pixels)
                    const MAX_VIEWPORT_DIM: f32 = 8192.0;
                    let safe_stripe_h = stripe_h.min(MAX_VIEWPORT_DIM);

                    // Calculate clipped bounds for this stripe (intersection with widget bounds)
                    let clipped_left = tile_x.max(widget_left);
                    let clipped_top = stripe_y.max(widget_top);
                    let clipped_right = (tile_x + tile_w).min(widget_right);
                    let clipped_bottom = (stripe_y + safe_stripe_h).min(widget_bottom);

                    // Skip if completely outside clip bounds
                    if clipped_left >= clipped_right || clipped_top >= clipped_bottom {
                        continue;
                    }

                    let clipped_w = clipped_right - clipped_left;
                    let clipped_h = clipped_bottom - clipped_top;

                    let mut render_pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
                        label: Some(&format!("Tile {} Stripe Render Pass", tile.id)),
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

                    // Set scissor rect for clipping this stripe
                    render_pass.set_scissor_rect(clipped_left as u32, clipped_top as u32, clipped_w as u32, clipped_h as u32);

                    // Set viewport for this stripe
                    // The viewport covers the full tile width but only this stripe's height
                    if tile_w > 0.0 && safe_stripe_h > 0.0 {
                        render_pass.set_viewport(tile_x, stripe_y, tile_w, safe_stripe_h, 0.0, 1.0);
                        render_pass.set_pipeline(&pipeline.pipeline);
                        render_pass.set_bind_group(0, &stripe.bind_group, &[]);
                        render_pass.draw(0..6, 0..1);
                    }
                }
            }
        }

        // ====================================================================
        // Second Pass: Render Labels Separately
        // ====================================================================
        for tile in &self.tiles {
            // Check if tile is visible
            let tile_top_logical = tile.position.1 - self.scroll_y;
            let tile_bottom_logical = tile_top_logical + tile.display_size.1;

            if tile_bottom_logical < 0.0 || tile_top_logical > self.viewport_height {
                continue;
            }

            // Only render if we have label resources and label data
            if tile.label_raw_size.1 == 0 {
                continue;
            }

            if let Some(label_res) = pipeline.label_resources.get(&tile.id) {
                // Use pre-calculated values from LabelResources (set in prepare())
                let padding = TILE_BORDER_WIDTH + TILE_INNER_PADDING;

                // Convert stored logical pixel values to physical screen coordinates
                let label_x = (widget_left + (tile.position.0 + padding) * scale_factor).floor();
                let label_y = (widget_top + (tile_top_logical + label_res.display_y) * scale_factor).floor();
                let label_w = (label_res.display_width * scale_factor).ceil();
                let label_h = (label_res.display_height * scale_factor).ceil();

                // Clip to widget bounds
                let clipped_left = label_x.max(widget_left);
                let clipped_top = label_y.max(widget_top);
                let clipped_right = (label_x + label_w).min(widget_right);
                let clipped_bottom = (label_y + label_h).min(widget_bottom);

                if clipped_left >= clipped_right || clipped_top >= clipped_bottom {
                    continue;
                }

                let clipped_w = clipped_right - clipped_left;
                let clipped_h = clipped_bottom - clipped_top;

                let mut render_pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
                    label: Some(&format!("Tile {} Label Render Pass", tile.id)),
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

                render_pass.set_scissor_rect(clipped_left as u32, clipped_top as u32, clipped_w as u32, clipped_h as u32);

                if label_w > 0.0 && label_h > 0.0 {
                    render_pass.set_viewport(label_x, label_y, label_w, label_h, 0.0, 1.0);
                    render_pass.set_pipeline(&pipeline.label_pipeline);
                    render_pass.set_bind_group(0, &label_res.bind_group, &[]);
                    render_pass.draw(0..6, 0..1);
                }
            }
        }
    }
}

// ============================================================================
// Shader Program and State (removed - unused)
// ============================================================================

// TileShaderMessage, TileShaderState, and TileShaderProgram were never used
// and have been removed. The tile grid uses TileShaderProgramWrapper directly.
