//! Minimap shader implementation with sliding window texture support
//!
//! Uses a sliding window of texture slices (matching Terminal).

use std::collections::HashMap;
use std::sync::Arc;

use iced::mouse;
use iced::widget::shader;
use iced::Rectangle;
use parking_lot::Mutex;

use icy_engine_gui::tile_cache::MAX_TEXTURE_SLICES;
use icy_engine_gui::CheckerboardColors;

use super::{MinimapMessage, SharedMinimapState, TextureSliceData};

/// Uniform data for the minimap shader (multi-texture version)
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct MinimapUniforms {
    /// Viewport rectangle (normalized 0-1 in texture space): x, y, width, height
    viewport_rect: [f32; 4],
    /// Viewport border color RGBA
    viewport_color: [f32; 4],
    /// Visible UV range (what part of texture is currently shown): min_y, max_y, unused, unused
    visible_uv_range: [f32; 4],
    /// Render dimensions: texture_width, texture_height, available_width, available_height
    render_dimensions: [f32; 4],
    /// Viewport border thickness in pixels
    border_thickness: f32,
    /// Whether to show viewport overlay
    show_viewport: f32,
    /// Number of texture slices (1..=MAX_TEXTURE_SLICES)
    num_slices: f32,
    /// Total image height across all slices
    total_image_height: f32,
    /// Heights of each slice in pixels (packed as 3 vec4s = 12 floats for 10 slices + 2 padding)
    slice_heights: [[f32; 4]; 3],
    /// First checkerboard color (RGBA)
    checker_color1: [f32; 4],
    /// Second checkerboard color (RGBA)
    checker_color2: [f32; 4],
    /// Checkerboard params: x=cell_size, y=enabled, z=max_layer_height, w=unused
    checker_params: [f32; 4],

    /// Solid background color for the minimap canvas (RGBA)
    canvas_bg: [f32; 4],
}

/// Viewport information for the minimap overlay
#[derive(Clone, Debug, Default)]
pub struct ViewportInfo {
    /// Normalized X position of viewport (0.0-1.0)
    pub x: f32,
    /// Normalized Y position of viewport (0.0-1.0)
    pub y: f32,
    /// Normalized width of viewport (0.0-1.0)
    pub width: f32,
    /// Normalized height of viewport (0.0-1.0)
    pub height: f32,
}

pub(crate) fn viewport_info_from_effective_view(
    content_width: f32,
    content_height: f32,
    visible_width: f32,
    visible_height: f32,
    scroll_offset_x: f32,
    scroll_offset_y: f32,
) -> ViewportInfo {
    let cw = content_width.max(1.0);
    let ch = content_height.max(1.0);

    let width = (visible_width / cw).clamp(0.0, 1.0);
    let height = (visible_height / ch).clamp(0.0, 1.0);

    let max_x = (1.0 - width).max(0.0);
    let max_y = (1.0 - height).max(0.0);
    let x = (scroll_offset_x / cw).clamp(0.0, max_x);
    let y = (scroll_offset_y / ch).clamp(0.0, max_y);

    ViewportInfo { x, y, width, height }
}

#[cfg(test)]
mod tests {
    use super::{normalized_position_from_minimap, viewport_info_from_effective_view};

    fn assert_approx(a: f32, b: f32) {
        let eps = 1e-6;
        assert!((a - b).abs() <= eps, "{a} != {b}");
    }

    #[test]
    fn viewport_maps_to_normalized() {
        let v = viewport_info_from_effective_view(200.0, 1000.0, 50.0, 200.0, 0.0, 300.0);
        assert_approx(v.x, 0.0);
        assert_approx(v.y, 0.3);
        assert_approx(v.width, 0.25);
        assert_approx(v.height, 0.2);
    }

    #[test]
    fn clamps_when_visible_exceeds_content() {
        let v = viewport_info_from_effective_view(100.0, 100.0, 500.0, 1000.0, 0.0, 0.0);
        assert_approx(v.width, 1.0);
        assert_approx(v.height, 1.0);
    }

    #[test]
    fn clamps_scroll_offsets() {
        let v = viewport_info_from_effective_view(100.0, 100.0, 10.0, 10.0, -50.0, 250.0);
        assert_approx(v.x, 0.0);
        // y is clamped to (1 - height) to keep the overlay fully inside.
        assert_approx(v.y, 0.9);
    }

    #[test]
    fn clamps_x_to_right_edge() {
        // width = 0.25 -> max_x = 0.75
        let v = viewport_info_from_effective_view(200.0, 100.0, 50.0, 10.0, 9999.0, 0.0);
        assert_approx(v.width, 0.25);
        assert_approx(v.x, 0.75);
    }

    #[test]
    fn avoids_div_by_zero() {
        let v = viewport_info_from_effective_view(0.0, 0.0, 10.0, 10.0, 5.0, 5.0);
        // With max(1.0) the values stay finite and clamped.
        assert!(v.x.is_finite() && v.y.is_finite() && v.width.is_finite() && v.height.is_finite());
        assert!(v.x >= 0.0 && v.x <= 1.0);
        assert!(v.y >= 0.0 && v.y <= 1.0);
        assert!(v.width >= 0.0 && v.width <= 1.0);
        assert!(v.height >= 0.0 && v.height <= 1.0);
    }

    #[test]
    fn click_mapping_clamps_out_of_bounds() {
        // Full content is 1000px tall; rendered window is 200px starting at Y=400.
        // bounds are 100x100; texture_w matches bounds_w => scale=1.
        let bounds_w = 100.0;
        let bounds_h = 100.0;
        let texture_w = 100.0;
        let window_h = 200.0;
        let first_slice_start_y = 400.0;
        let full_h = 1000.0;

        // Local scroll: visible_uv_height=0.5, max_scroll_uv=0.5, scroll_uv_y=0.25 => local=0.5.
        let local_scroll_offset = 0.5;

        // Above bounds clamps to top of visible range: y = (400 + 0.25*200)/1000 = 0.45
        let (_x, y_top) = normalized_position_from_minimap(
            50.0,
            -999.0,
            bounds_w,
            bounds_h,
            texture_w,
            window_h,
            local_scroll_offset,
            first_slice_start_y,
            full_h,
        )
        .unwrap();
        assert_approx(y_top, 0.45);

        // Below bounds clamps to bottom of visible range: y = (400 + 0.75*200)/1000 = 0.55
        let (_x, y_bottom) = normalized_position_from_minimap(
            50.0,
            999.0,
            bounds_w,
            bounds_h,
            texture_w,
            window_h,
            local_scroll_offset,
            first_slice_start_y,
            full_h,
        )
        .unwrap();
        assert_approx(y_bottom, 0.55);
    }
}

/// The minimap shader program (high-level interface)
/// This implements shader::Program and creates MinimapPrimitive for rendering
#[derive(Debug, Clone)]
pub struct MinimapProgram {
    /// Texture slices (up to MAX_TEXTURE_SLICES)
    pub slices: Vec<TextureSliceData>,
    /// Heights of each slice in pixels
    pub slice_heights: Vec<u32>,
    /// Width of the texture (same for all slices)
    pub texture_width: u32,
    /// Total rendered height across all slices
    pub total_rendered_height: u32,
    /// Unique instance ID
    pub instance_id: usize,
    /// Viewport overlay information
    pub viewport_info: ViewportInfo,
    /// Scroll offset (0.0 = top, 1.0 = bottom)
    pub scroll_offset: f32,
    /// Full content height in pixels (for proper viewport scaling)
    pub full_content_height: f32,
    /// Where the first slice starts in document Y coordinates
    pub first_slice_start_y: f32,
    /// Shared state for communicating bounds back to MinimapView
    pub shared_state: Arc<Mutex<SharedMinimapState>>,
    /// Checkerboard colors for transparency (from MonitorSettings)
    pub checkerboard_colors: CheckerboardColors,

    /// Viewport overlay color (RGBA)
    pub viewport_color: [f32; 4],

    /// Solid minimap canvas background color (RGBA)
    pub canvas_bg: [f32; 4],
}

/// State for tracking mouse dragging in the minimap
#[derive(Debug, Clone, Default)]
pub struct MinimapState {
    /// Whether the left mouse button is currently pressed
    is_dragging: bool,
    /// Last redraw timestamp for tracking animation frames
    last_redraw: Option<std::time::Instant>,
    /// Last pointer position (absolute) for continuous scroll during drag
    last_pointer_position: Option<iced::Point>,
}

impl MinimapProgram {
    /// Helper to calculate normalized position from cursor position
    /// Takes absolute position and bounds, calculates relative position internally
    fn calculate_normalized_position(&self, absolute_pos: iced::Point, bounds: Rectangle) -> Option<(f32, f32)> {
        let tex_w = self.texture_width;
        let tex_h = self.total_rendered_height;
        if tex_w == 0 || tex_h == 0 {
            return None;
        }

        // Calculate position relative to bounds (for mouse capture outside bounds)
        let relative_x = absolute_pos.x - bounds.x;
        let relative_y = absolute_pos.y - bounds.y;

        normalized_position_from_minimap(
            relative_x,
            relative_y,
            bounds.width,
            bounds.height,
            tex_w as f32,
            tex_h as f32,
            self.scroll_offset,
            self.first_slice_start_y,
            self.full_content_height,
        )
    }
}

fn normalized_position_from_minimap(
    relative_x: f32,
    relative_y: f32,
    bounds_w: f32,
    bounds_h: f32,
    texture_w: f32,
    window_h: f32,
    local_scroll_offset: f32,
    first_slice_start_y: f32,
    full_content_height: f32,
) -> Option<(f32, f32)> {
    if bounds_w <= 0.0 || bounds_h <= 0.0 || texture_w <= 0.0 || window_h <= 0.0 {
        return None;
    }

    let scale = bounds_w / texture_w;
    let scaled_h = window_h * scale;

    // Calculate visible UV range (same logic as in prepare())
    let visible_uv_height = (bounds_h / scaled_h).min(1.0);
    let max_scroll_uv = (1.0 - visible_uv_height).max(0.0);
    let scroll_uv_y = local_scroll_offset.clamp(0.0, 1.0) * max_scroll_uv;

    // Pointer can be outside when drag-out autoscroll is active; clamp to edge.
    let local_x = relative_x.clamp(0.0, bounds_w);

    // IMPORTANT: When the scaled content height is smaller than the available height,
    // the minimap shader letterboxes (unused area below content). Click mapping must
    // normalize Y over the *content* height, not the full bounds height.
    // This matches the WGSL logic:
    //   used_height_ratio = min(scaled_h / bounds_h, 1)
    //   content_uv_y = screen_uv.y / used_height_ratio
    let content_h = scaled_h.min(bounds_h).max(1.0);
    let local_y = relative_y.clamp(0.0, bounds_h).clamp(0.0, content_h);

    // Position in screen space (0-1 of visible content area)
    let screen_y = local_y / content_h;

    // Convert to texture UV space by mapping through visible range
    // texture_uv is 0-1 over the rendered window texture
    let texture_uv_y = (scroll_uv_y + screen_y * visible_uv_height).clamp(0.0, 1.0);

    // Convert from texture space (sliding window) to full buffer space.
    let buffer_y_px = first_slice_start_y + texture_uv_y * window_h;
    let norm_y = (buffer_y_px / full_content_height.max(1.0)).clamp(0.0, 1.0);
    let norm_x = (local_x / bounds_w).clamp(0.0, 1.0);

    Some((norm_x, norm_y))
}

#[cfg(test)]
mod click_viewport_mapping_tests {
    use super::normalized_position_from_minimap;

    fn assert_approx(a: f32, b: f32) {
        let eps = 1e-6;
        assert!((a - b).abs() <= eps, "{a} != {b}");
    }

    #[test]
    fn click_y_maps_over_content_height_when_letterboxed() {
        // scaled_h < bounds_h (content is letterboxed vertically)
        // bounds: 100px high, content renders to 50px high -> bottom of content is at y=50.
        let bounds_w = 100.0;
        let bounds_h = 100.0;
        let texture_w = 200.0; // scale = 0.5
        let window_h = 100.0;
        let first_slice_start_y = 0.0;
        let full_h = 100.0;
        let local_scroll_offset = 0.0;

        // Mid content: y=25 => 0.5
        let (_x, y_mid) = normalized_position_from_minimap(
            50.0,
            25.0,
            bounds_w,
            bounds_h,
            texture_w,
            window_h,
            local_scroll_offset,
            first_slice_start_y,
            full_h,
        )
        .unwrap();
        assert_approx(y_mid, 0.5);

        // Bottom of content: y=50 => 1.0 (not 0.5)
        let (_x, y_bottom_of_content) = normalized_position_from_minimap(
            50.0,
            50.0,
            bounds_w,
            bounds_h,
            texture_w,
            window_h,
            local_scroll_offset,
            first_slice_start_y,
            full_h,
        )
        .unwrap();
        assert_approx(y_bottom_of_content, 1.0);

        // Click in letterbox area clamps to bottom of content
        let (_x, y_letterbox) = normalized_position_from_minimap(
            50.0,
            90.0,
            bounds_w,
            bounds_h,
            texture_w,
            window_h,
            local_scroll_offset,
            first_slice_start_y,
            full_h,
        )
        .unwrap();
        assert_approx(y_letterbox, 1.0);
    }
}

impl shader::Program<MinimapMessage> for MinimapProgram {
    type State = MinimapState;
    type Primitive = MinimapPrimitive;

    fn draw(&self, _state: &Self::State, _cursor: mouse::Cursor, bounds: Rectangle) -> Self::Primitive {
        // Update shared state with current bounds (for next frame's rendering)
        {
            let mut shared = self.shared_state.lock();
            shared.available_width = bounds.width;
            shared.available_height = bounds.height;
        }

        MinimapPrimitive {
            slices: self.slices.clone(),
            slice_heights: self.slice_heights.clone(),
            texture_width: self.texture_width,
            total_rendered_height: self.total_rendered_height,
            instance_id: self.instance_id,
            viewport_info: self.viewport_info.clone(),
            scroll_offset: self.scroll_offset,
            available_height: bounds.height,
            full_content_height: self.full_content_height,
            first_slice_start_y: self.first_slice_start_y,
            checkerboard_colors: self.checkerboard_colors.clone(),
            viewport_color: self.viewport_color,
            canvas_bg: self.canvas_bg,
        }
    }

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<iced::widget::Action<MinimapMessage>> {
        match event {
            // Handle redraw requests for continuous drag scrolling
            iced::Event::Window(iced::window::Event::RedrawRequested(now)) => {
                if state.is_dragging {
                    state.last_redraw = Some(*now);
                    // Re-send scroll position based on last known pointer position
                    if let Some(last_pos) = state.last_pointer_position {
                        if let Some((norm_x, norm_y)) = self.calculate_normalized_position(last_pos, bounds) {
                            return Some(iced::widget::Action::publish(MinimapMessage::ScrollTo { norm_x, norm_y }));
                        }
                    }
                } else {
                    state.last_redraw = None;
                }
            }

            // Handle mouse button press - start dragging
            iced::Event::Mouse(mouse::Event::ButtonPressed {
                button: mouse::Button::Left, ..
            }) => {
                // Use position_in for initial click - must be inside bounds
                if let Some(pos) = cursor.position_in(bounds) {
                    state.is_dragging = true;
                    state.last_redraw = None;
                    let absolute_pos = iced::Point::new(pos.x + bounds.x, pos.y + bounds.y);
                    state.last_pointer_position = Some(absolute_pos);
                    if let Some((norm_x, norm_y)) = self.calculate_normalized_position(absolute_pos, bounds) {
                        return Some(iced::widget::Action::publish(MinimapMessage::ScrollTo { norm_x, norm_y }));
                    }
                }
            }

            // Handle mouse button release - stop dragging
            iced::Event::Mouse(mouse::Event::ButtonReleased {
                button: mouse::Button::Left, ..
            }) => {
                if state.is_dragging {
                    state.is_dragging = false;
                    state.last_redraw = None;
                    state.last_pointer_position = None;
                }
            }

            // Handle cursor movement while dragging
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.is_dragging {
                    // Use cursor.position() for mouse capture effect - works even outside bounds
                    if let Some(pos) = cursor.position() {
                        state.last_pointer_position = Some(pos);
                        if let Some((norm_x, norm_y)) = self.calculate_normalized_position(pos, bounds) {
                            return Some(iced::widget::Action::publish(MinimapMessage::ScrollTo { norm_x, norm_y }));
                        }
                    }
                }
            }

            _ => {}
        }

        None
    }
}

/// The minimap shader primitive (low-level GPU rendering)
#[derive(Debug, Clone)]
pub struct MinimapPrimitive {
    /// Texture slices (up to MAX_TEXTURE_SLICES)
    pub slices: Vec<TextureSliceData>,
    /// Heights of each slice in pixels
    pub slice_heights: Vec<u32>,
    /// Width of the texture (same for all slices)
    pub texture_width: u32,
    /// Total rendered height across all slices
    pub total_rendered_height: u32,
    /// Unique instance ID
    pub instance_id: usize,
    /// Viewport overlay information
    pub viewport_info: ViewportInfo,
    /// Scroll offset (0.0 = top, 1.0 = bottom)
    pub scroll_offset: f32,
    /// Available height for rendering
    pub available_height: f32,
    /// Full content height in pixels
    pub full_content_height: f32,
    /// Where the first slice starts in document Y coordinates
    pub first_slice_start_y: f32,
    /// Checkerboard colors for transparency (from MonitorSettings)
    pub checkerboard_colors: CheckerboardColors,

    /// Viewport overlay color (RGBA)
    pub viewport_color: [f32; 4],

    /// Solid minimap canvas background color (RGBA)
    pub canvas_bg: [f32; 4],
}

impl MinimapPrimitive {
    /// Compute a hash of the texture data pointers to detect changes
    fn compute_data_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        for slice in &self.slices {
            // Use the Arc pointer address as a unique identifier
            let ptr = Arc::as_ptr(&slice.rgba_data) as usize;
            ptr.hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// Texture array for GPU
#[allow(dead_code)]
struct TextureArray {
    texture: iced::wgpu::Texture,
    texture_view: iced::wgpu::TextureView,
}

/// Per-instance GPU resources with texture slicing
struct InstanceResources {
    /// Texture array containing all slices
    texture_array: TextureArray,
    /// Bind group for rendering
    bind_group: iced::wgpu::BindGroup,
    /// Uniform buffer
    uniform_buffer: iced::wgpu::Buffer,
    /// Total texture dimensions for cache validation
    texture_size: (u32, u32),
    /// Number of slices for cache validation
    num_slices: usize,
    /// Individual slice sizes (width, height) for cache validation
    slice_sizes: Vec<(u32, u32)>,
    /// Hash of texture data pointers to detect when data changed
    texture_data_hash: u64,
}

/// The minimap shader renderer (GPU pipeline) with multi-texture support
pub struct MinimapShaderRenderer {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group_layout: iced::wgpu::BindGroupLayout,
    sampler: iced::wgpu::Sampler,
    instances: HashMap<usize, InstanceResources>,
}

impl shader::Pipeline for MinimapShaderRenderer {
    fn new(device: &iced::wgpu::Device, _queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("Minimap Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(include_str!("minimap.wgsl").into()),
        });

        // Create bind group layout with texture array + sampler + uniforms (3 entries total)
        let entries = vec![
            // Binding 0: Texture array
            iced::wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: iced::wgpu::ShaderStages::FRAGMENT,
                ty: iced::wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: iced::wgpu::TextureViewDimension::D2Array,
                    sample_type: iced::wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            // Binding 1: Sampler
            iced::wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: iced::wgpu::ShaderStages::FRAGMENT,
                ty: iced::wgpu::BindingType::Sampler(iced::wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            // Binding 2: Uniforms
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
        ];

        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("Minimap Bind Group Layout"),
            entries: &entries,
        });

        let pipeline_layout = device.create_pipeline_layout(&iced::wgpu::PipelineLayoutDescriptor {
            label: Some("Minimap Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some("Minimap Pipeline"),
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

        // Use linear filtering for smooth minimap scaling
        let sampler = device.create_sampler(&iced::wgpu::SamplerDescriptor {
            label: Some("Minimap Sampler"),
            address_mode_u: iced::wgpu::AddressMode::ClampToEdge,
            address_mode_v: iced::wgpu::AddressMode::ClampToEdge,
            address_mode_w: iced::wgpu::AddressMode::ClampToEdge,
            mag_filter: iced::wgpu::FilterMode::Linear,
            min_filter: iced::wgpu::FilterMode::Linear,
            mipmap_filter: iced::wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        MinimapShaderRenderer {
            pipeline,
            bind_group_layout,
            sampler,
            instances: HashMap::new(),
        }
    }
}

impl shader::Primitive for MinimapPrimitive {
    type Pipeline = MinimapShaderRenderer;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        bounds: &iced::Rectangle,
        _viewport: &iced::advanced::graphics::Viewport,
    ) {
        let id = self.instance_id;
        let num_slices = self.slices.len().min(MAX_TEXTURE_SLICES);
        let tex_w = self.texture_width.max(1);
        let tex_h = self.total_rendered_height.max(1);

        // Check if we need to recreate resources
        let current_slice_sizes: Vec<(u32, u32)> = self.slices.iter().take(MAX_TEXTURE_SLICES).map(|s| (s.width, s.height)).collect();

        let needs_recreate = match pipeline.instances.get(&id) {
            Some(resources) => resources.texture_size != (tex_w, tex_h) || resources.num_slices != num_slices || resources.slice_sizes != current_slice_sizes,
            None => true,
        };

        if needs_recreate {
            // Find max dimensions across all slices for the array texture
            let max_slice_w = self.slices.iter().take(num_slices).map(|s| s.width.max(1)).max().unwrap_or(1);
            let max_slice_h = self.slices.iter().take(num_slices).map(|s| s.height.max(1)).max().unwrap_or(1);
            let layer_count = (num_slices as u32).max(1);

            // Create texture array with uniform layer dimensions
            let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                label: Some(&format!("Minimap Texture Array {}", id)),
                size: iced::wgpu::Extent3d {
                    width: max_slice_w,
                    height: max_slice_h,
                    depth_or_array_layers: layer_count,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: iced::wgpu::TextureDimension::D2,
                format: iced::wgpu::TextureFormat::Rgba8Unorm,
                usage: iced::wgpu::TextureUsages::TEXTURE_BINDING | iced::wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            let texture_view = texture.create_view(&iced::wgpu::TextureViewDescriptor {
                dimension: Some(iced::wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });

            let texture_array = TextureArray { texture, texture_view };

            // Create uniform buffer
            let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
                label: Some(&format!("Minimap Uniforms {}", id)),
                size: std::mem::size_of::<MinimapUniforms>() as u64,
                usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            // Create bind group with 3 entries: texture array, sampler, uniforms
            let entries = vec![
                // Binding 0: Texture array
                iced::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: iced::wgpu::BindingResource::TextureView(&texture_array.texture_view),
                },
                // Binding 1: Sampler
                iced::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: iced::wgpu::BindingResource::Sampler(&pipeline.sampler),
                },
                // Binding 2: Uniforms
                iced::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ];

            let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                label: Some(&format!("Minimap BindGroup {}", id)),
                layout: &pipeline.bind_group_layout,
                entries: &entries,
            });

            pipeline.instances.insert(
                id,
                InstanceResources {
                    texture_array,
                    bind_group,
                    uniform_buffer,
                    texture_size: (tex_w, tex_h),
                    num_slices,
                    slice_sizes: current_slice_sizes.clone(),
                    texture_data_hash: 0, // Will be updated below
                },
            );
        }

        let Some(resources) = pipeline.instances.get_mut(&id) else {
            return;
        };

        // Check if texture data has changed using pointer-based hash
        let current_hash = self.compute_data_hash();
        let needs_texture_upload = resources.texture_data_hash != current_hash;

        // Upload texture data only if changed
        if needs_texture_upload {
            for (i, slice_data) in self.slices.iter().enumerate().take(num_slices) {
                if !slice_data.rgba_data.is_empty() {
                    let bytes_per_row = 4 * slice_data.width;

                    queue.write_texture(
                        iced::wgpu::TexelCopyTextureInfo {
                            texture: &resources.texture_array.texture,
                            mip_level: 0,
                            origin: iced::wgpu::Origin3d { x: 0, y: 0, z: i as u32 },
                            aspect: iced::wgpu::TextureAspect::All,
                        },
                        &slice_data.rgba_data,
                        iced::wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(bytes_per_row),
                            rows_per_image: Some(slice_data.height),
                        },
                        iced::wgpu::Extent3d {
                            width: slice_data.width,
                            height: slice_data.height,
                            depth_or_array_layers: 1,
                        },
                    );
                }
            }
            resources.texture_data_hash = current_hash;
        }

        // Update uniforms
        let show_viewport = if self.viewport_info.width > 0.0 && self.viewport_info.height > 0.0 {
            1.0
        } else {
            0.0
        };

        // Calculate visible UV range based on texture size, available height, and scroll
        let avail_h = self.available_height.max(1.0);
        let scale = bounds.width / tex_w as f32;
        let scaled_h = tex_h as f32 * scale;

        // How much of the texture is visible (in normalized UV coordinates)
        let visible_uv_height = (avail_h / scaled_h).min(1.0);
        let max_scroll_uv = (1.0 - visible_uv_height).max(0.0);
        let scroll_uv_y = self.scroll_offset * max_scroll_uv;

        let visible_uv_min_y = scroll_uv_y;
        let visible_uv_max_y = scroll_uv_y + visible_uv_height;

        // Convert viewport from full-buffer-space to texture-space (sliding window).
        // viewport_info is normalized over the full document height.
        // The rendered texture represents a window starting at `first_slice_start_y`.
        let full_h = self.full_content_height.max(1.0);
        let buffer_y_px = self.viewport_info.y * full_h;
        let buffer_h_px = self.viewport_info.height * full_h;

        let tex_h_f = tex_h as f32;
        let viewport_y_tex = ((buffer_y_px - self.first_slice_start_y) / tex_h_f.max(1.0)).clamp(0.0, 1.0);
        let viewport_h_tex = (buffer_h_px / tex_h_f.max(1.0)).clamp(0.0, 1.0);

        // Pack slice heights into 3 vec4s (matches WGSL packing)
        // slice_heights[0] = [h0, h1, h2, first_slice_start_y]
        // slice_heights[1] = [h3, h4, h5, h6]
        // slice_heights[2] = [h7, h8, h9, 0]
        let mut packed_heights = [[0.0f32; 4]; 3];
        for (i, &h) in self.slice_heights.iter().enumerate().take(MAX_TEXTURE_SLICES) {
            match i {
                0 => packed_heights[0][0] = h as f32,
                1 => packed_heights[0][1] = h as f32,
                2 => packed_heights[0][2] = h as f32,
                3 => packed_heights[1][0] = h as f32,
                4 => packed_heights[1][1] = h as f32,
                5 => packed_heights[1][2] = h as f32,
                6 => packed_heights[1][3] = h as f32,
                7 => packed_heights[2][0] = h as f32,
                8 => packed_heights[2][1] = h as f32,
                9 => packed_heights[2][2] = h as f32,
                _ => {}
            }
        }
        packed_heights[0][3] = self.first_slice_start_y;

        // Calculate max layer height (all texture array layers have this size)
        let max_layer_height = self.slices.iter().take(num_slices).map(|s| s.height).max().unwrap_or(1) as f32;

        let uniforms = MinimapUniforms {
            viewport_rect: [self.viewport_info.x, viewport_y_tex, self.viewport_info.width, viewport_h_tex],
            viewport_color: self.viewport_color,
            visible_uv_range: [visible_uv_min_y, visible_uv_max_y, 0.0, 0.0],
            // Pass texture and available dimensions for aspect-ratio-correct rendering
            render_dimensions: [tex_w as f32, tex_h as f32, bounds.width, bounds.height],
            border_thickness: 2.5,
            show_viewport,
            num_slices: num_slices as f32,
            total_image_height: tex_h as f32,
            slice_heights: packed_heights,
            checker_color1: self.checkerboard_colors.color1_rgba(),
            checker_color2: self.checkerboard_colors.color2_rgba(),
            checker_params: [self.checkerboard_colors.cell_size, 1.0, max_layer_height, 0.0],
            canvas_bg: self.canvas_bg,
        };

        let uniform_bytes = unsafe { std::slice::from_raw_parts(&uniforms as *const MinimapUniforms as *const u8, std::mem::size_of::<MinimapUniforms>()) };
        queue.write_buffer(&resources.uniform_buffer, 0, uniform_bytes);
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        let Some(resources) = pipeline.instances.get(&self.instance_id) else {
            return;
        };

        // The viewport is just the visible area (clip_bounds)
        // Scrolling is handled in the shader via visible_uv_range uniforms
        let vp_x = clip_bounds.x as f32;
        let vp_y = clip_bounds.y as f32;
        let vp_w = clip_bounds.width as f32;
        let vp_h = clip_bounds.height as f32;

        let mut render_pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
            label: Some("Minimap Render Pass"),
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

        if vp_w > 0.0 && vp_h > 0.0 {
            render_pass.set_scissor_rect(clip_bounds.x, clip_bounds.y, clip_bounds.width, clip_bounds.height);
            render_pass.set_viewport(vp_x, vp_y, vp_w, vp_h, 0.0, 1.0);
            render_pass.set_pipeline(&pipeline.pipeline);
            render_pass.set_bind_group(0, &resources.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }
    }
}
