//! Shader-based tile grid rendering
//!
//! Uses wgpu textures to render thumbnails efficiently in a grid layout.
//! Supports very tall images (up to 80,000px) by splitting into multiple texture slices.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use iced::Rectangle;
use iced::mouse;
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

/// Maximum display height for a tile image
/// Must stay under GPU viewport limits (~8192px)
pub const MAX_TILE_IMAGE_HEIGHT: f32 = 4000.0;

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
    /// Total content height (for scrollbar)
    pub content_height: f32,
    /// Background color (RGBA)
    pub background_color: [f32; 4],
    /// Selection color (RGBA)
    pub selection_color: [f32; 4],
    /// Hover color (RGBA)
    pub hover_color: [f32; 4],
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
    /// Total image height in pixels
    total_image_height: f32,
    /// Heights of each slice in pixels (packed as 3 vec4s = 12 floats for 10 slices + 2 padding)
    slice_heights: [[f32; 4]; 3],
}

impl TileUniforms {
    /// Create uniforms for a tile with pre-computed rectangles and slice info
    fn new(tile: &TileTexture, tag_height: f32, slice_heights: &[u32]) -> Self {
        let shadow_extra_x = SHADOW_OFFSET_X + SHADOW_BLUR_RADIUS;
        let shadow_extra_y = SHADOW_OFFSET_Y + SHADOW_BLUR_RADIUS;

        // Total size including shadow
        let total_width = tile.display_size.0 + shadow_extra_x;
        let total_height = tile.display_size.1 + shadow_extra_y;

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
        
        // Image height is the content area minus padding (top and bottom)
        // The texture will be stretched/scaled to fit this area
        let image_h = content_h - padding * 2.0;

        // Label rect - below image with inner padding
        let label_x = padding;
        let label_y = image_y + image_h + TILE_INNER_PADDING;
        let label_w = image_w;
        let label_h = if tag_height > 0.0 { tag_height + TILE_INNER_PADDING } else { 0.0 };

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
            slice_heights: packed_heights,
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
#[allow(dead_code)]
struct SharedTextureResources {
    /// Texture slices (1-10 slices depending on image height)
    slices: Vec<TextureSlice>,
    /// Original image dimensions
    texture_size: (u32, u32),
}

/// Per-tile GPU resources (unique to each tile for position/hover/selection state)
#[allow(dead_code)]
struct TileResources {
    /// Key to look up shared texture resources
    texture_key: usize,
    bind_group: iced::wgpu::BindGroup,
    uniform_buffer: iced::wgpu::Buffer,
}

/// Renderer for the tile grid shader
pub struct TileGridShaderRenderer {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group_layout: iced::wgpu::BindGroupLayout,
    sampler: iced::wgpu::Sampler,
    /// 1x1 transparent texture for unused texture slots
    dummy_texture_view: iced::wgpu::TextureView,
    /// Per-tile resources (unique to each tile)
    tiles: HashMap<u64, TileResources>,
    /// Shared texture resources keyed by Arc pointer address
    shared_textures: HashMap<usize, SharedTextureResources>,
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

        // Create bind group layout with 10 texture slots + sampler + uniforms
        let mut entries = Vec::with_capacity(MAX_TEXTURE_SLICES + 2);
        
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

        TileGridShaderRenderer {
            pipeline,
            bind_group_layout,
            sampler,
            dummy_texture_view,
            tiles: HashMap::new(),
            shared_textures: HashMap::new(),
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
                let num_slices = ((total_height as usize + MAX_SLICE_HEIGHT as usize - 1) / MAX_SLICE_HEIGHT as usize)
                    .min(MAX_TEXTURE_SLICES);
                
                if num_slices > 1 {
                    log::info!("[TileShader] Creating {} slices for tile {}x{}", num_slices, tile.width, tile.height);
                }
                
                let mut slices = Vec::with_capacity(num_slices);
                let mut y_offset = 0u32;
                    
                log::debug!(
                    "[TileShader] Creating texture slices for tile {} ({}x{}) with key {:x} num: {num_slices}",
                    tile.id, tile.width, tile.height, texture_key
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

                log::debug!("[TIMING] Texture creation for tile {} ({}x{}): {:?}", 
                           tile.id, tile.width, tile.height, texture_start.elapsed());

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

            if needs_recreate {
                // Get the shared texture slices
                let shared_texture = pipeline.shared_textures.get(&texture_key).unwrap();

                // Create per-tile uniform buffer
                let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
                    label: Some(&format!("Tile Uniforms {}", tile.id)),
                    size: std::mem::size_of::<TileUniforms>() as u64,
                    usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });

                // Create bind group entries for all 10 texture slots
                let mut entries = Vec::with_capacity(MAX_TEXTURE_SLICES + 2);
                
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

                let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                    label: Some(&format!("Tile BindGroup {}", tile.id)),
                    layout: &pipeline.bind_group_layout,
                    entries: &entries,
                });

                pipeline.tiles.insert(
                    tile.id,
                    TileResources {
                        texture_key,
                        bind_group,
                        uniform_buffer,
                    },
                );
            }

            // Update uniforms for this tile (always, as hover/selection state may change)
            if let Some(resources) = pipeline.tiles.get(&tile.id) {
                let shared_texture = pipeline.shared_textures.get(&texture_key).unwrap();
                
                // Collect slice heights
                let slice_heights: Vec<u32> = shared_texture.slices.iter().map(|s| s.height).collect();
                
                let uniforms = TileUniforms::new(tile, 0.0, &slice_heights);
                queue.write_buffer(&resources.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
            }
        }

        // Remove tiles that are no longer needed
        let active_ids: std::collections::HashSet<u64> = self.tiles.iter().map(|t| t.id).collect();
        pipeline.tiles.retain(|id, _| active_ids.contains(id));

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
                // Multiply logical coordinates by scale factor to get physical pixels
                let tile_x = widget_left + tile.position.0 * scale_factor;
                let tile_y = widget_top + tile_top_logical * scale_factor;
                let tile_w = (tile.display_size.0 + shadow_extra_x) * scale_factor;
                let tile_h = (tile.display_size.1 + shadow_extra_y) * scale_factor;

                // Calculate clipped bounds (intersection with widget bounds)
                let clipped_left = tile_x.max(widget_left);
                let clipped_top = tile_y.max(widget_top);
                let clipped_right = (tile_x + tile_w).min(widget_right);
                let clipped_bottom = (tile_y + tile_h).min(widget_bottom);

                // Skip if completely outside clip bounds
                if clipped_left >= clipped_right || clipped_top >= clipped_bottom {
                    continue;
                }

                let clipped_w = clipped_right - clipped_left;
                let clipped_h = clipped_bottom - clipped_top;

                let mut render_pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
                    label: Some(&format!("Tile Render Pass {}", tile.id)),
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

                // Set scissor rect for clipping
                render_pass.set_scissor_rect(clipped_left as u32, clipped_top as u32, clipped_w as u32, clipped_h as u32);

                // GPU viewport limits - keep dimensions reasonable
                const MAX_VIEWPORT_DIM: f32 = 8192.0;
                const MIN_VIEWPORT_COORD: f32 = -16384.0;
                
                let safe_tile_x = tile_x.max(MIN_VIEWPORT_COORD);
                let safe_tile_y = tile_y.max(MIN_VIEWPORT_COORD);
                let safe_tile_w = tile_w.min(MAX_VIEWPORT_DIM);
                let safe_tile_h = tile_h.min(MAX_VIEWPORT_DIM);

                if safe_tile_w > 0.0 && safe_tile_h > 0.0 {
                    render_pass.set_viewport(safe_tile_x, safe_tile_y, safe_tile_w, safe_tile_h, 0.0, 1.0);
                    render_pass.set_pipeline(&pipeline.pipeline);
                    render_pass.set_bind_group(0, &resources.bind_group, &[]);
                    render_pass.draw(0..6, 0..1);
                }
            }
        }
    }
}

// ============================================================================
// Shader Program and State
// ============================================================================

/// Messages generated by the tile grid shader
#[derive(Debug, Clone)]
pub enum TileShaderMessage {
    /// A tile was clicked
    TileClicked(u64),
    /// A tile was double-clicked
    TileDoubleClicked(u64),
    /// Scroll by delta
    Scroll(f32),
}

/// State for the tile grid shader program
#[derive(Debug, Default)]
#[allow(dead_code)] // Fields reserved for future double-click detection
pub struct TileShaderState {
    /// Currently hovered tile
    pub hovered_tile: Option<u64>,
    /// Last click time for double-click detection
    last_click_time: Option<std::time::Instant>,
    /// Last clicked tile
    last_clicked_tile: Option<u64>,
}

/// Shader program for the tile grid
pub struct TileShaderProgram {
    pub tiles: Vec<TileTexture>,
    pub scroll_y: f32,
    pub content_height: f32,
    pub selected_tile: Option<u64>,
}

impl TileShaderProgram {
    pub fn new() -> Self {
        Self {
            tiles: Vec::new(),
            scroll_y: 0.0,
            content_height: 0.0,
            selected_tile: None,
        }
    }
}

impl<Message> shader::Program<Message> for TileShaderProgram
where
    Message: Clone + 'static,
{
    type State = TileShaderState;
    type Primitive = TileGridShader;

    fn draw(&self, state: &Self::State, _cursor: mouse::Cursor, bounds: Rectangle) -> Self::Primitive {
        // Update hover state in tiles
        let tiles: Vec<TileTexture> = self
            .tiles
            .iter()
            .map(|t| {
                let mut tile = t.clone();
                tile.is_hovered = state.hovered_tile == Some(t.id);
                tile.is_selected = self.selected_tile == Some(t.id);
                tile
            })
            .collect();

        TileGridShader {
            tiles,
            scroll_y: self.scroll_y,
            viewport_height: bounds.height,
            content_height: self.content_height,
            background_color: [0.1, 0.1, 0.1, 1.0],
            selection_color: [0.3, 0.5, 0.8, 0.5],
            hover_color: [0.5, 0.5, 0.5, 0.3],
        }
    }

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<iced::widget::Action<Message>> {
        // no mouse over control -> clear hover and return
        if !cursor.is_over(bounds) {
            state.hovered_tile = None;
            return None;
        }

        // Handle mouse events
        if let Some(cursor_pos) = cursor.position_in(bounds) {
            // Check which tile is hovered
            state.hovered_tile = None;
            for tile in &self.tiles {
                let tile_top = tile.position.1 - self.scroll_y;
                let tile_bottom = tile_top + tile.display_size.1;
                let tile_left = tile.position.0;
                let tile_right = tile_left + tile.display_size.0;

                if cursor_pos.x >= tile_left && cursor_pos.x <= tile_right && cursor_pos.y >= tile_top && cursor_pos.y <= tile_bottom {
                    state.hovered_tile = Some(tile.id);
                    break;
                }
            }
        } else {
            state.hovered_tile = None;
        }

        // Handle scroll events
        if let iced::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) = event {
            if cursor.is_over(bounds) {
                let _scroll_delta = match delta {
                    iced::mouse::ScrollDelta::Lines { y, .. } => *y * 50.0,
                    iced::mouse::ScrollDelta::Pixels { y, .. } => *y,
                };
                // Scrolling would be handled by returning a message
                // but we need the outer component to handle it
            }
        }

        None
    }

    fn mouse_interaction(&self, state: &Self::State, _bounds: Rectangle, _cursor: mouse::Cursor) -> mouse::Interaction {
        if state.hovered_tile.is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}
