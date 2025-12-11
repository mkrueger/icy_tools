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

static MINIMAP_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Generates a unique instance ID for minimap views
fn generate_minimap_id() -> usize {
    MINIMAP_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
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
    /// Hover state changed for scrollbar
    ScrollbarHover(bool),
    /// Request to scroll minimap to keep viewport visible
    /// Contains the viewport info and visible UV range for calculation
    EnsureViewportVisible { viewport_y: f32, viewport_height: f32 },
}

/// Internal cache state for the minimap
struct MinimapCache {
    rgba: Option<Arc<Vec<u8>>>,
    /// Buffer size in characters (for cache validation)
    buffer_size: (usize, usize),
    /// Rendered pixel size (for shader)
    pixel_size: (usize, usize),
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
    /// Last known available height for scroll calculations
    last_available_height: RefCell<f32>,
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
                rgba: None,
                buffer_size: (0, 0),
                pixel_size: (0, 0),
                version: 0,
            }),
            viewport_info: ViewportInfo::default(),
            scroll_position: RefCell::new(0.0),
            scrollbar_visibility: 0.0,
            scrollbar_hover: Arc::new(AtomicBool::new(false)),
            last_available_height: RefCell::new(0.0),
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
        cache.rgba = None;
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
        let (render_width, render_height) = cache.pixel_size;
        drop(cache);

        if render_width == 0 || render_height == 0 {
            return;
        }

        let last_height = *self.last_available_height.borrow();
        if last_height <= 0.0 {
            return;
        }

        // Calculate scale and visible UV height
        // The visible UV height depends on how much of the texture fits on screen
        let visible_uv_height = (last_height / render_height as f32).min(1.0);
        let max_scroll_uv = (1.0 - visible_uv_height).max(0.0);

        let current_scroll = *self.scroll_position.borrow();

        // Current visible UV range
        let visible_uv_start = current_scroll * max_scroll_uv;
        let visible_uv_end = visible_uv_start + visible_uv_height;

        let viewport_end = viewport_y + viewport_height;

        // Check if viewport is above visible area
        if viewport_y < visible_uv_start {
            // Scroll up to show viewport
            let new_scroll_uv = viewport_y;
            let new_scroll = if max_scroll_uv > 0.0 {
                (new_scroll_uv / max_scroll_uv).clamp(0.0, 1.0)
            } else {
                0.0
            };
            *self.scroll_position.borrow_mut() = new_scroll;
        }
        // Check if viewport is below visible area
        else if viewport_end > visible_uv_end {
            // Scroll down to show viewport
            let new_scroll_uv = viewport_end - visible_uv_height;
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
        *self.last_available_height.borrow_mut() = height;
    }

    /// Create the view element for the minimap
    /// Takes a reference to the screen to render and viewport info from the canvas
    /// Uses interior mutability (RefCell) to update cache while taking &self
    pub fn view(&self, screen: &Arc<Mutex<Box<dyn Screen>>>, viewport_info: &ViewportInfo) -> Element<'_, MinimapMessage> {
        // Check if we need to update the cache
        let screen_guard = screen.lock();

        // Get buffer dimensions
        let buf_width = screen_guard.width() as usize;
        let buf_height = screen_guard.height() as usize;

        // Check cache version using screen's version tracking
        let current_version = screen_guard.version() as usize;

        let cached_pixel_size;
        let cached_rgba;

        {
            let cache = self.cache.borrow();
            let cache_valid = cache.rgba.is_some() && cache.buffer_size == (buf_width, buf_height) && cache.version == current_version;

            if cache_valid {
                // Cache is valid - Arc::clone is cheap (just ref count)
                cached_pixel_size = cache.pixel_size;
                cached_rgba = cache.rgba.clone();
            } else {
                drop(cache);
                // Create render options for the full buffer
                let rect = Rectangle::from_coords(0, 0, buf_width as i32, buf_height as i32);
                let render_options = RenderOptions {
                    rect: rect.into(),
                    blink_on: true,
                    selection: None,
                    selection_fg: None,
                    selection_bg: None,
                    override_scan_lines: Some(false), // No scanlines for minimap
                };
                let (size, rgba) = screen_guard.render_to_rgba(&render_options);
                cached_pixel_size = (size.width as usize, size.height as usize);
                let rgba_arc = Arc::new(rgba);
                cached_rgba = Some(Arc::clone(&rgba_arc));

                // Update the cache
                let mut cache = self.cache.borrow_mut();
                cache.rgba = Some(rgba_arc);
                cache.buffer_size = (buf_width, buf_height);
                cache.pixel_size = cached_pixel_size;
                cache.version = current_version;
            }
        }
        drop(screen_guard);

        // Create the shader program with viewport info from canvas
        // Note: available_height will be set from bounds in the shader's draw() method
        let scroll_pos = *self.scroll_position.borrow();

        // Auto-scroll to keep viewport visible
        self.ensure_viewport_visible(viewport_info.y, viewport_info.height);
        let scroll_pos = *self.scroll_position.borrow(); // Re-read after potential update

        let shader_program = if let Some(ref rgba) = cached_rgba {
            MinimapProgram {
                rgba_data: Arc::clone(rgba),
                texture_size: (cached_pixel_size.0 as u32, cached_pixel_size.1 as u32),
                instance_id: self.instance_id,
                viewport_info: viewport_info.clone(),
                scroll_offset: scroll_pos,
                available_height: 0.0, // Will be set from bounds in draw()
            }
        } else {
            // Fallback empty shader
            MinimapProgram {
                rgba_data: Arc::new(vec![0, 0, 0, 255]),
                texture_size: (1, 1),
                instance_id: self.instance_id,
                viewport_info: ViewportInfo::default(),
                scroll_offset: 0.0,
                available_height: 0.0,
            }
        };

        shader(shader_program).width(Length::Fill).height(Length::Fill).into()
    }

    /// Handle mouse press for click-to-navigate functionality
    /// Returns the normalized position (0.0-1.0) if clicked within bounds
    pub fn handle_click(&self, bounds: Size, position: iced::Point) -> Option<(f32, f32)> {
        // Calculate the fill-width rendering area
        let cache = self.cache.borrow();
        let (render_width, render_height) = cache.pixel_size;
        if render_width == 0 || render_height == 0 {
            return None;
        }

        // Fill-width scaling: scale to fill available width
        let scale = bounds.width / render_width as f32;
        let scaled_height = render_height as f32 * scale;

        // Calculate scroll offset
        let max_scroll = (scaled_height - bounds.height).max(0.0);
        let scroll_y = *self.scroll_position.borrow() * max_scroll;

        // Check if click is within the rendered minimap area
        let local_x = position.x;
        let local_y = position.y + scroll_y;

        if local_x >= 0.0 && local_x <= bounds.width && local_y >= 0.0 && local_y <= scaled_height {
            let norm_x = local_x / bounds.width;
            let norm_y = local_y / scaled_height;
            Some((norm_x, norm_y))
        } else {
            None
        }
    }
}
