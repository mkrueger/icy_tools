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
    pub fn ensure_viewport_visible(&self, viewport_y: f32, viewport_height: f32) {
        let cache = self.cache.borrow();
        let render_height = cache.total_rendered_height;
        let (full_w, full_h) = cache.full_content_size;
        drop(cache);

        if render_height == 0 || full_h == 0 || full_w == 0 {
            return;
        }

        let shared = self.shared_state.lock();
        let avail_width = shared.available_width;
        let avail_height = shared.available_height;
        drop(shared);

        if avail_width <= 0.0 || avail_height <= 0.0 {
            return;
        }

        // Calculate scale (same as in view())
        let scale = avail_width / full_w as f32;

        // How much of the rendered content is visible on screen (in content pixels)
        let visible_content_px = avail_height / scale;

        // visible_uv_height = how much of rendered content is visible (0-1)
        let visible_uv_height = (visible_content_px / render_height as f32).min(1.0);
        let max_scroll_uv = (1.0 - visible_uv_height).max(0.0);

        let current_scroll = *self.scroll_position.borrow();

        // Current visible UV range (in terms of rendered content, not full content)
        let visible_uv_start = current_scroll * max_scroll_uv;
        let visible_uv_end = visible_uv_start + visible_uv_height;

        // Convert viewport_y from full content space to rendered content space
        let rendered_ratio = render_height as f32 / full_h as f32;
        let viewport_y_rendered = viewport_y * rendered_ratio;
        let viewport_height_rendered = viewport_height * rendered_ratio;
        let viewport_end_rendered = viewport_y_rendered + viewport_height_rendered;

        // Check if viewport is above visible area
        if viewport_y_rendered < visible_uv_start {
            let new_scroll_uv = viewport_y_rendered;
            let new_scroll = if max_scroll_uv > 0.0 {
                (new_scroll_uv / max_scroll_uv).clamp(0.0, 1.0)
            } else {
                0.0
            };
            *self.scroll_position.borrow_mut() = new_scroll;
        }
        // Check if viewport is below visible area
        else if viewport_end_rendered > visible_uv_end {
            let new_scroll_uv = viewport_end_rendered - visible_uv_height;
            let new_scroll = if max_scroll_uv > 0.0 {
                (new_scroll_uv / max_scroll_uv).clamp(0.0, 1.0)
            } else {
                0.0
            };
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

        shader(shader_program).width(Length::Fill).height(Length::Fill).into()
    }

    /// Handle mouse press for click-to-navigate functionality
    /// Returns the normalized position (0.0-1.0) if clicked within bounds
    pub fn handle_click(&self, bounds: Size, position: iced::Point) -> Option<(f32, f32)> {
        let cache = self.cache.borrow();
        let render_width = if let Some(first) = cache.slices.first() { first.width } else { return None };
        let render_height = cache.total_rendered_height;
        let (_full_w, full_h) = cache.full_content_size;
        drop(cache);

        if render_width == 0 || render_height == 0 || full_h == 0 {
            return None;
        }

        // Fill-width scaling
        let scale = bounds.width / render_width as f32;
        let scaled_height = render_height as f32 * scale;

        // Calculate scroll offset
        let max_scroll_content = (full_h as f32 - render_height as f32).max(0.0);
        let scroll_y_content = *self.scroll_position.borrow() * max_scroll_content;

        // Check if click is within the rendered minimap area
        let local_x = position.x;
        let local_y = position.y;

        if local_x >= 0.0 && local_x <= bounds.width && local_y >= 0.0 && local_y <= scaled_height {
            let norm_x = local_x / bounds.width;
            // Map Y from rendered region to full content
            let content_y = (local_y / scale) + scroll_y_content;
            let norm_y = (content_y / full_h as f32).clamp(0.0, 1.0);
            Some((norm_x, norm_y))
        } else {
            None
        }
    }
}
