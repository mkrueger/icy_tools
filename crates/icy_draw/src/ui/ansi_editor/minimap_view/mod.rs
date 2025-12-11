mod minimap_shader;

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use iced::widget::shader;
use iced::{Element, Length, Size, Task};
use icy_engine::{Rectangle, RenderOptions, Screen};
use parking_lot::Mutex;

use minimap_shader::MinimapProgram;
pub use minimap_shader::ViewportInfo;

// ============================================================================
// Texture Slicing Constants
// ============================================================================

/// Maximum height per texture slice (GPU limit is typically 8192, we use 8000 for safety)
pub const MAX_SLICE_HEIGHT: u32 = 8000;

/// Maximum number of texture slices (10 slices * 8000px = 80,000px max height)
pub const MAX_TEXTURE_SLICES: usize = 10;

static MINIMAP_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Generates a unique instance ID for minimap views
fn generate_minimap_id() -> usize {
    MINIMAP_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Shared state between MinimapView and shader for communicating bounds
#[derive(Debug, Default)]
pub struct SharedMinimapState {
    pub available_width: f32,
    pub available_height: f32,
}

/// A single texture slice for tall images
#[derive(Clone, Debug)]
pub struct TextureSliceData {
    /// RGBA pixel data for this slice
    pub rgba_data: Arc<Vec<u8>>,
    /// Width of this slice
    pub width: u32,
    /// Height of this slice
    pub height: u32,
}

/// Messages for the minimap view
#[derive(Clone, Debug)]
pub enum MinimapMessage {
    /// Click on minimap to scroll to position (normalized 0.0-1.0 in texture space)
    Click(f32, f32),
    /// Drag on minimap to scroll (normalized 0.0-1.0 in texture space)
    Drag(f32, f32),
    /// Scroll the minimap vertically (mouse wheel)
    Scroll(f32, f32),
    /// Scrollbar hover state changed
    ScrollbarHover(bool),
    /// Request to scroll minimap to keep viewport visible
    EnsureViewportVisible { viewport_y: f32, viewport_height: f32 },
}

/// Internal cache state for the minimap with texture slicing support
struct MinimapCache {
    /// Texture slices (up to MAX_TEXTURE_SLICES)
    slices: Vec<TextureSliceData>,
    /// Heights of each slice in pixels
    slice_heights: Vec<u32>,
    /// Full content size in pixels (original image size)
    full_content_size: (u32, u32),
    /// Total rendered height across all slices
    total_rendered_height: u32,
    /// Render region height (for cache validation)
    render_height: i32,
    /// Version for cache invalidation
    version: usize,
}

/// A minimap view that displays a scaled-down version of the buffer
/// with a viewport rectangle overlay showing the current visible area.
/// The minimap fills the available width and is scrollable vertically.
pub struct MinimapView {
    instance_id: usize,
    /// Cache uses interior mutability so view() can update it while taking &self
    cache: RefCell<MinimapCache>,
    viewport_info: ViewportInfo,
    /// Current scroll position (0.0 to 1.0) - RefCell for interior mutability in view()
    scroll_position: RefCell<f32>,
    /// Scrollbar visibility (for fade animation)
    scrollbar_visibility: f32,
    /// Shared state for scrollbar hover tracking
    scrollbar_hover: Arc<AtomicBool>,
    /// Shared state for communicating bounds from shader
    shared_state: Arc<Mutex<SharedMinimapState>>,
}

impl Default for MinimapView {
    fn default() -> Self {
        Self::new()
    }
}

impl MinimapView {
    pub fn new() -> Self {
        Self {
            instance_id: generate_minimap_id(),
            cache: RefCell::new(MinimapCache {
                slices: Vec::new(),
                slice_heights: Vec::new(),
                full_content_size: (0, 0),
                total_rendered_height: 0,
                render_height: 0,
                version: 0,
            }),
            viewport_info: ViewportInfo::default(),
            scroll_position: RefCell::new(0.0),
            scrollbar_visibility: 0.0,
            scrollbar_hover: Arc::new(AtomicBool::new(false)),
            shared_state: Arc::new(Mutex::new(SharedMinimapState::default())),
        }
    }

    /// Update the viewport information (the visible area rectangle)
    /// Parameters are normalized values (0.0 to 1.0)
    pub fn set_viewport(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.viewport_info = ViewportInfo { x, y, width, height };
    }

    /// Invalidate the cache, forcing a re-render on next view()
    pub fn invalidate_cache(&mut self) {
        let mut cache = self.cache.borrow_mut();
        cache.version = cache.version.wrapping_add(1);
        cache.slices.clear();
    }

    /// Update scrollbar visibility for fade animation
    pub fn update_scrollbar_visibility(&mut self, delta: f32) {
        let is_hovered = self.scrollbar_hover.load(Ordering::Relaxed);
        if is_hovered {
            self.scrollbar_visibility = (self.scrollbar_visibility + delta * 8.0).min(1.0);
        } else {
            self.scrollbar_visibility = (self.scrollbar_visibility - delta * 2.0).max(0.0);
        }
    }

    /// Check if scrollbar needs animation update
    pub fn needs_scrollbar_animation(&self) -> bool {
        let is_hovered = self.scrollbar_hover.load(Ordering::Relaxed);
        (is_hovered && self.scrollbar_visibility < 1.0) || (!is_hovered && self.scrollbar_visibility > 0.0)
    }

    /// Update the minimap view state
    pub fn update(&mut self, message: MinimapMessage) -> Task<MinimapMessage> {
        match message {
            MinimapMessage::Click(_x, _y) => {
                // Parent handles the actual scrolling
                Task::none()
            }
            MinimapMessage::Drag(_x, _y) => {
                // Parent handles the actual scrolling
                Task::none()
            }
            MinimapMessage::Scroll(_x, y) => {
                // Scroll the minimap vertically
                let mut sp = self.scroll_position.borrow_mut();
                *sp = (*sp - y * 0.01).clamp(0.0, 1.0);
                Task::none()
            }
            MinimapMessage::ScrollbarHover(hovered) => {
                self.scrollbar_hover.store(hovered, Ordering::Relaxed);
                Task::none()
            }
            MinimapMessage::EnsureViewportVisible { viewport_y, viewport_height } => {
                // Auto-scroll the minimap to keep the viewport visible
                // viewport_y and viewport_height are in texture UV space (0-1)
                self.ensure_viewport_visible(viewport_y, viewport_height);
                Task::none()
            }
        }
    }

    /// Ensure the viewport rectangle is visible in the minimap
    /// Adjusts scroll_position so the viewport is always on screen
    /// Uses interior mutability so it can be called from view()
    ///
    /// viewport_y and viewport_height are normalized (0-1) relative to full buffer
    pub fn ensure_viewport_visible(&self, viewport_y: f32, viewport_height: f32) {
        let cache = self.cache.borrow();
        let total_rendered_height = cache.total_rendered_height as f32;
        let (full_w, full_h) = cache.full_content_size;
        drop(cache);

        if total_rendered_height == 0.0 || full_h == 0 || full_w == 0 {
            return;
        }

        let shared = self.shared_state.lock();
        let avail_width = shared.available_width;
        let avail_height = shared.available_height;
        drop(shared);

        if avail_width <= 0.0 || avail_height <= 0.0 {
            return;
        }

        // Scale factor: minimap fills available width
        let scale = avail_width / full_w as f32;

        // Scaled height of the entire rendered content
        let scaled_content_height = total_rendered_height * scale;

        // If content fits in available space, no scrolling needed
        if scaled_content_height <= avail_height {
            *self.scroll_position.borrow_mut() = 0.0;
            return;
        }

        // Convert viewport from full buffer space to texture space
        // viewport_y is in full buffer coordinates (0-1 over full_content_height)
        // We need texture coordinates (0-1 over total_rendered_height)
        //
        // Example: full_h = 87232px, total_rendered_height = 80000px
        // render_ratio = 80000/87232 = 0.917
        // If viewport_y = 0.5 (50% of buffer = 43616px)
        // In texture space: 43616px / 80000px = 0.545
        // So: viewport_y_tex = viewport_y / render_ratio
        let render_ratio = total_rendered_height / full_h as f32;

        // Convert to texture space (divide by render_ratio)
        let viewport_y_tex = viewport_y / render_ratio.max(0.001);
        let viewport_h_tex = viewport_height / render_ratio.max(0.001);

        // Viewport position and size in scaled pixels (minimap screen space)
        let viewport_top_scaled = viewport_y_tex * total_rendered_height * scale;
        let viewport_height_scaled = viewport_h_tex * total_rendered_height * scale;
        let viewport_bottom_scaled = viewport_top_scaled + viewport_height_scaled;

        // Maximum scroll offset in pixels
        let max_scroll_px = scaled_content_height - avail_height;

        // Current scroll offset in pixels
        let current_scroll = *self.scroll_position.borrow();
        let current_scroll_px = current_scroll * max_scroll_px;

        // Visible range in scaled pixels
        let visible_top = current_scroll_px;
        let visible_bottom = current_scroll_px + avail_height;

        // Check if viewport is above visible area
        if viewport_top_scaled < visible_top {
            // Scroll up to show viewport at top
            let new_scroll_px = viewport_top_scaled;
            let new_scroll = (new_scroll_px / max_scroll_px).clamp(0.0, 1.0);
            *self.scroll_position.borrow_mut() = new_scroll;
        }
        // Check if viewport is below visible area
        else if viewport_bottom_scaled > visible_bottom {
            // Scroll down to show viewport at bottom
            let new_scroll_px = viewport_bottom_scaled - avail_height;
            let new_scroll = (new_scroll_px / max_scroll_px).clamp(0.0, 1.0);
            *self.scroll_position.borrow_mut() = new_scroll;
        }
    }

    /// Store available height for auto-scroll calculations
    pub fn set_available_height(&self, height: f32) {
        self.shared_state.lock().available_height = height;
    }

    /// Create the view element for the minimap
    ///
    /// Uses texture slicing to support very tall content (up to 80,000px):
    /// 1. Calculate scale to fill available width
    /// 2. Render the full content (or limited portion for very tall files)
    /// 3. Split into texture slices of max MAX_SLICE_HEIGHT pixels each
    /// 4. The shader samples from the appropriate slice based on Y coordinate
    pub fn view(&self, screen: &Arc<Mutex<Box<dyn Screen>>>, viewport_info: &ViewportInfo) -> Element<'_, MinimapMessage> {
        let screen_guard = screen.lock();
        let current_version = screen_guard.version() as usize;

        // Get full BUFFER dimensions in PIXELS (not just terminal/visible size)
        let resolution = screen_guard.virtual_size();
        let content_width = resolution.width as f32;
        let content_height = resolution.height as f32;

        // Get available space from shared state (updated by shader in previous frame)
        let (avail_width, avail_height) = {
            let shared = self.shared_state.lock();
            let w = if shared.available_width > 0.0 { shared.available_width } else { 200.0 };
            let h = if shared.available_height > 0.0 { shared.available_height } else { 600.0 };
            (w, h)
        };

        // Maximum total height we can render (limited by MAX_TEXTURE_SLICES * MAX_SLICE_HEIGHT)
        let max_total_height = (MAX_TEXTURE_SLICES as u32 * MAX_SLICE_HEIGHT) as f32;

        // Render the full content (up to max texture limit) - NOT limited by available screen space
        let total_content_to_render = content_height.min(max_total_height);

        // Auto-scroll to keep viewport visible
        self.ensure_viewport_visible(viewport_info.y, viewport_info.height);

        // Get current scroll position for shader
        let scroll_pos = *self.scroll_position.borrow();

        // Render the full content (not just visible portion)
        let render_height = total_content_to_render as i32;

        // Check if we need to re-render
        let need_render = {
            let cache = self.cache.borrow();
            cache.slices.is_empty()
                || cache.version != current_version
                || cache.render_height != render_height
                || cache.full_content_size != (resolution.width as u32, resolution.height as u32)
        };

        let (slices, slice_heights, total_rendered_height, full_content_size) = if need_render {
            // Render the full content from the beginning (scrolling is handled in shader)
            let actual_render_height = render_height.min(resolution.height);
            let viewport_region = Rectangle::from(0, 0, resolution.width, actual_render_height);

            let render_options = RenderOptions::default();
            let (size, rgba) = screen_guard.render_region_to_rgba(viewport_region, &render_options);

            let pixel_width = size.width as u32;
            let pixel_height = size.height as u32;
            let full_size = (resolution.width as u32, resolution.height as u32);

            // Split RGBA data into slices
            let mut slices = Vec::new();
            let mut heights = Vec::new();
            let bytes_per_row = (pixel_width * 4) as usize;
            let mut y_offset: u32 = 0;

            while y_offset < pixel_height && slices.len() < MAX_TEXTURE_SLICES {
                let slice_height = (pixel_height - y_offset).min(MAX_SLICE_HEIGHT);
                let start_byte = (y_offset as usize) * bytes_per_row;
                let end_byte = ((y_offset + slice_height) as usize) * bytes_per_row;

                if end_byte <= rgba.len() {
                    let slice_data = rgba[start_byte..end_byte].to_vec();
                    slices.push(TextureSliceData {
                        rgba_data: Arc::new(slice_data),
                        width: pixel_width,
                        height: slice_height,
                    });
                    heights.push(slice_height);
                }
                y_offset += slice_height;
            }

            // Update cache
            {
                let mut cache = self.cache.borrow_mut();
                cache.slices = slices.clone();
                cache.slice_heights = heights.clone();
                cache.full_content_size = full_size;
                cache.total_rendered_height = pixel_height;
                cache.render_height = render_height;
                cache.version = current_version;
            }
            (slices, heights, pixel_height, full_size)
        } else {
            let cache = self.cache.borrow();
            (
                cache.slices.clone(),
                cache.slice_heights.clone(),
                cache.total_rendered_height,
                cache.full_content_size,
            )
        };

        drop(screen_guard);

        // Create the shader program with texture slices
        let shader_program = if !slices.is_empty() {
            MinimapProgram {
                slices,
                slice_heights,
                texture_width: full_content_size.0,
                total_rendered_height,
                instance_id: self.instance_id,
                viewport_info: viewport_info.clone(),
                scroll_offset: scroll_pos,
                available_height: avail_height,
                full_content_height: full_content_size.1 as f32,
                shared_state: Arc::clone(&self.shared_state),
            }
        } else {
            // Fallback: empty 1x1 texture
            MinimapProgram {
                slices: vec![TextureSliceData {
                    rgba_data: Arc::new(vec![0, 0, 0, 255]),
                    width: 1,
                    height: 1,
                }],
                slice_heights: vec![1],
                texture_width: 1,
                total_rendered_height: 1,
                instance_id: self.instance_id,
                viewport_info: ViewportInfo::default(),
                scroll_offset: 0.0,
                available_height: avail_height,
                full_content_height: 1.0,
                shared_state: Arc::clone(&self.shared_state),
            }
        };

        let r = shader(shader_program).width(Length::Fill).height(Length::Fill).into();

        r
    }

    /// Handle mouse press for click-to-navigate functionality
    /// Returns the normalized position (0.0-1.0) in full buffer space
    /// This position represents where the CENTER of the viewport should be
    pub fn handle_click(&self, _bounds: Size, position: iced::Point) -> Option<(f32, f32)> {
        let cache = self.cache.borrow();
        let render_width = if let Some(first) = cache.slices.first() { first.width } else { return None };
        let total_rendered_height = cache.total_rendered_height as f32;
        let (full_w, full_h) = cache.full_content_size;
        drop(cache);

        if render_width == 0 || total_rendered_height == 0.0 || full_h == 0 || full_w == 0 {
            return None;
        }

        let shared = self.shared_state.lock();
        let avail_width = shared.available_width;
        let avail_height = shared.available_height;
        drop(shared);

        if avail_width <= 0.0 || avail_height <= 0.0 {
            return None;
        }

        // Scale factor (minimap fills available width)
        let scale = avail_width / full_w as f32;
        let scaled_content_height = total_rendered_height * scale;

        // Calculate visible UV range (same logic as in shader prepare())
        let visible_uv_height = (avail_height / scaled_content_height).min(1.0);
        let max_scroll_uv = (1.0 - visible_uv_height).max(0.0);
        let scroll_uv = *self.scroll_position.borrow() * max_scroll_uv;

        // Check bounds
        let local_x = position.x;
        let local_y = position.y;

        println!("=== MINIMAP CLICK DEBUG ===");
        println!("Click position: ({}, {})", local_x, local_y);
        println!("Avail size: {}x{}", avail_width, avail_height);
        println!("Full buffer size: {}x{}", full_w, full_h);
        println!("Total rendered height: {}", total_rendered_height);
        println!("Scale: {}", scale);
        println!("Scaled content height: {}", scaled_content_height);
        println!("Visible UV height: {}", visible_uv_height);
        println!("Max scroll UV: {}", max_scroll_uv);
        println!("Current scroll UV: {}", scroll_uv);

        if local_x < 0.0 || local_x > avail_width || local_y < 0.0 || local_y > avail_height {
            println!("Click out of bounds!");
            return None;
        }

        // Convert screen position to texture UV
        // screen_uv_y is 0-1 in the visible area
        let screen_uv_y = local_y / avail_height;
        // texture_uv_y is the absolute position in the rendered texture (0-1)
        let texture_uv_y = scroll_uv + screen_uv_y * visible_uv_height;

        // Convert texture UV to full buffer coordinates
        // (in case we couldn't render the full buffer due to texture limits)
        let render_ratio = total_rendered_height / full_h as f32;
        let buffer_y = texture_uv_y / render_ratio;

        let norm_x = local_x / avail_width;
        let norm_y = buffer_y.clamp(0.0, 1.0);

        println!("Screen UV Y: {}", screen_uv_y);
        println!("Texture UV Y: {}", texture_uv_y);
        println!("Render ratio: {}", render_ratio);
        println!("Buffer Y (before clamp): {}", buffer_y);
        println!("Result: norm_x={}, norm_y={}", norm_x, norm_y);
        println!("===========================");

        Some((norm_x, norm_y))
    }
}
