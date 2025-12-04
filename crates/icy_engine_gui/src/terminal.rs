use parking_lot::{Mutex, RwLock};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32};

use iced::{Color, widget};
use icy_engine::Screen;

use crate::{ScrollbarState, Viewport};

pub struct Terminal {
    pub screen: Arc<Mutex<Box<dyn Screen>>>,
    pub original_screen: Option<Arc<Mutex<Box<dyn Screen>>>>,
    pub viewport: Arc<RwLock<Viewport>>,
    pub scrollbar: ScrollbarState,
    pub scrollbar_hover_state: Arc<AtomicBool>, // Shared atomic hover state for scrollbar
    /// Computed visible height from shader (in content units, e.g., lines)
    /// Updated by the shader based on available widget bounds
    pub computed_visible_height: Arc<AtomicU32>,
    pub font_size: f32,
    pub char_width: f32,
    pub char_height: f32,
    pub id: widget::Id,
    pub has_focus: bool,
}

impl Terminal {
    pub fn new(screen: Arc<Mutex<Box<dyn Screen>>>) -> Self {
        // Initialize viewport with screen size
        let viewport = {
            let scr = screen.lock();
            let virtual_size = scr.virtual_size();
            let resolution = scr.get_resolution();
            Arc::new(RwLock::new(Viewport::new(resolution, virtual_size)))
        };

        Self {
            screen,
            original_screen: None,
            viewport,
            scrollbar: ScrollbarState::new(),
            scrollbar_hover_state: Arc::new(AtomicBool::new(false)),
            computed_visible_height: Arc::new(AtomicU32::new(0)),
            font_size: 16.0,
            char_width: 9.6, // Approximate for monospace
            char_height: 20.0,
            id: widget::Id::unique(),
            has_focus: false,
        }
    }

    /// Update viewport when screen size changes
    pub fn update_viewport_size(&mut self) {
        let scr = self.screen.lock();
        let virtual_size = scr.virtual_size();
        let resolution = scr.get_resolution();
        drop(scr);

        {
            let mut vp = self.viewport.write();
            // Only update content size, not visible size (which is the widget size, not screen size)
            vp.set_content_size(virtual_size.width as f32, virtual_size.height as f32);
            vp.set_visible_size(resolution.width as f32, resolution.height as f32);
        }
        // Sync scrollbar position with viewport (after the lock is dropped)
        self.sync_scrollbar_with_viewport();
    }

    /// Update viewport visible size based on available widget dimensions
    /// Call this when the widget size changes to properly calculate scrollbar
    pub fn set_viewport_visible_size(&mut self, width: f32, height: f32) {
        self.viewport.write().set_visible_size(width, height);
        self.sync_scrollbar_with_viewport();
    }

    /// Sync scrollbar state with viewport scroll position
    /// Uses computed_visible_height if available for accurate scrollbar positioning
    pub fn sync_scrollbar_with_viewport(&mut self) {
        let mut vp = self.viewport.write();
        // Use computed visible height from shader if available, otherwise use viewport's visible_height
        let computed = self.computed_visible_height.load(std::sync::atomic::Ordering::Relaxed) as f32;
        let visible_height = if computed > 0.0 { computed } else { vp.visible_height };
        let visible_width = vp.visible_width;

        // Clamp scroll values with the correct visible height
        vp.clamp_scroll_with_size(visible_width, visible_height);

        // Vertical scrollbar
        let max_scroll_y = (vp.content_height * vp.zoom - visible_height).max(0.0);
        if max_scroll_y > 0.0 {
            let scroll_ratio = vp.scroll_y / max_scroll_y;
            self.scrollbar.set_scroll_position(scroll_ratio.clamp(0.0, 1.0));
        } else {
            self.scrollbar.set_scroll_position(0.0);
        }

        // Horizontal scrollbar
        let max_scroll_x = (vp.content_width * vp.zoom - visible_width).max(0.0);
        if max_scroll_x > 0.0 {
            let scroll_ratio_x = vp.scroll_x / max_scroll_x;
            self.scrollbar.set_scroll_position_x(scroll_ratio_x.clamp(0.0, 1.0));
        } else {
            self.scrollbar.set_scroll_position_x(0.0);
        }
    }

    /// Get the effective visible height (uses computed value from shader if available)
    fn get_effective_visible_height(&self) -> f32 {
        let computed = self.computed_visible_height.load(std::sync::atomic::Ordering::Relaxed) as f32;
        if computed > 0.0 { computed } else { self.viewport.read().visible_height }
    }

    /// Get maximum scroll Y value using the correct visible height
    pub fn max_scroll_y(&self) -> f32 {
        let vp = self.viewport.read();
        let visible_height = self.get_effective_visible_height();
        (vp.content_height * vp.zoom - visible_height).max(0.0)
    }

    /// Scroll by delta with proper clamping
    pub fn scroll_by(&mut self, dx: f32, dy: f32) {
        let mut vp = self.viewport.write();
        vp.scroll_by(dx, dy);
        // Re-clamp with the correct visible height
        let computed = self.computed_visible_height.load(std::sync::atomic::Ordering::Relaxed) as f32;
        let visible_height = if computed > 0.0 { computed } else { vp.visible_height };
        let visible_width = vp.visible_width;
        vp.clamp_scroll_with_size(visible_width, visible_height);
    }

    /// Scroll to position with proper clamping
    pub fn scroll_to(&mut self, x: f32, y: f32) {
        let mut vp = self.viewport.write();
        vp.scroll_to(x, y);
        // Re-clamp with the correct visible height
        let computed = self.computed_visible_height.load(std::sync::atomic::Ordering::Relaxed) as f32;
        let visible_height = if computed > 0.0 { computed } else { vp.visible_height };
        let visible_width = vp.visible_width;
        vp.clamp_scroll_with_size(visible_width, visible_height);
    }

    /// Scroll to position immediately with proper clamping
    pub fn scroll_to_immediate(&mut self, x: f32, y: f32) {
        let mut vp = self.viewport.write();
        vp.scroll_to_immediate(x, y);
        // Re-clamp with the correct visible height
        let computed = self.computed_visible_height.load(std::sync::atomic::Ordering::Relaxed) as f32;
        let visible_height = if computed > 0.0 { computed } else { vp.visible_height };
        let visible_width = vp.visible_width;
        vp.clamp_scroll_with_size(visible_width, visible_height);
    }

    /// Update animations for both viewport and scrollbar
    /// Should be called from ViewportTick
    pub fn update_animations(&mut self) {
        // Update viewport animation
        self.viewport.write().update_animation();

        // Sync scrollbar position after viewport animation
        self.sync_scrollbar_with_viewport();

        // Update scrollbar fade animation (uses same delta_time logic as viewport)
        self.scrollbar.update_animation();
    }

    /// Check if any animations are active
    pub fn needs_animation(&self) -> bool {
        self.viewport.read().is_animating() || self.scrollbar.needs_animation()
    }

    pub fn is_in_scrollback_mode(&self) -> bool {
        self.original_screen.is_some()
    }

    pub fn enter_scrollback_mode(&mut self, scrollback: Arc<Mutex<Box<dyn Screen>>>) {
        if self.original_screen.is_none() {
            // Save the original screen
            self.original_screen = Some(self.screen.clone());
            // Switch to scrollback
            self.screen = scrollback;
            // Update viewport for scrollback content
            self.update_viewport_size();

            // Get the resolution to use as visible size for scrolling calculations
            let scr = self.screen.lock();
            let resolution = scr.get_resolution();
            drop(scr);

            {
                let mut vp = self.viewport.write();
                // Use resolution as visible size and scroll to bottom immediately (no animation)
                let max_scroll_y = (vp.content_height * vp.zoom - resolution.height as f32).max(0.0);
                vp.scroll_to_immediate(0.0, max_scroll_y);
                // Clamp with the correct visible size
                vp.clamp_scroll_with_size(resolution.width as f32, resolution.height as f32);
            }

            // Sync scrollbar position with the new viewport position
            self.sync_scrollbar_with_viewport();
        }
    }

    pub fn exit_scrollback_mode(&mut self) {
        if let Some(original) = self.original_screen.take() {
            self.screen = original;
            // Update viewport back to normal content
            self.update_viewport_size();
            // Sync scrollbar position when exiting scrollback
            self.sync_scrollbar_with_viewport();
        }
    }

    pub fn reset_caret_blink(&mut self) {}

    // Helper function to convert buffer color to iced Color
    pub fn buffer_color_to_iced(color: icy_engine::Color) -> Color {
        let (r, g, b) = color.get_rgb_f32();
        Color::from_rgb(r, g, b)
    }
}
