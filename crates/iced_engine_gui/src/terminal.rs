use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use iced::{Color, widget};
use icy_engine::Screen;

use crate::{ScrollbarState, Viewport};

pub struct Terminal {
    pub screen: Arc<Mutex<Box<dyn Screen>>>,
    pub original_screen: Option<Arc<Mutex<Box<dyn Screen>>>>,
    pub viewport: Viewport,
    pub scrollbar: ScrollbarState,
    pub scrollbar_hover_state: Arc<AtomicBool>, // Shared atomic hover state for scrollbar
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
            Viewport::new(resolution, virtual_size)
        };

        Self {
            screen,
            original_screen: None,
            viewport,
            scrollbar: ScrollbarState::new(),
            scrollbar_hover_state: Arc::new(AtomicBool::new(false)),
            font_size: 16.0,
            char_width: 9.6, // Approximate for monospace
            char_height: 20.0,
            id: widget::Id::unique(),
            has_focus: false,
        }
    }

    /// Update viewport when screen size changes
    pub fn update_viewport_size(&mut self) {
        {
            let scr = self.screen.lock();
            let virtual_size = scr.virtual_size();
            // Only update content size, not visible size (which is the widget size, not screen size)
            self.viewport.set_content_size(virtual_size.width as f32, virtual_size.height as f32);

            let resolution = scr.get_resolution();
            self.viewport.set_visible_size(resolution.width as f32, resolution.height as f32);
        }
        // Sync scrollbar position with viewport (after the lock is dropped)
        self.sync_scrollbar_with_viewport();
    }

    /// Sync scrollbar state with viewport scroll position
    pub fn sync_scrollbar_with_viewport(&mut self) {
        let max_scroll = self.viewport.max_scroll_y();
        if max_scroll > 0.0 {
            let scroll_ratio = self.viewport.scroll_y / max_scroll;
            self.scrollbar.set_scroll_position(scroll_ratio);
        } else {
            self.scrollbar.set_scroll_position(0.0);
        }
    }

    /// Update animations for both viewport and scrollbar
    /// Should be called from ViewportTick
    pub fn update_animations(&mut self) {
        // Update viewport animation
        self.viewport.update_animation();

        // Sync scrollbar position after viewport animation
        self.sync_scrollbar_with_viewport();

        // Update scrollbar fade animation (uses same delta_time logic as viewport)
        self.scrollbar.update_animation();
    }

    /// Check if any animations are active
    pub fn needs_animation(&self) -> bool {
        self.viewport.is_animating() || self.scrollbar.needs_animation()
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
            {
                let scr = self.screen.lock();
                let resolution = scr.get_resolution();
                // Use resolution as visible size and scroll to bottom immediately (no animation)
                let max_scroll_y = (self.viewport.content_height * self.viewport.zoom - resolution.height as f32).max(0.0);
                self.viewport.scroll_to_immediate(0.0, max_scroll_y);
                // Clamp with the correct visible size
                self.viewport.clamp_scroll_with_size(resolution.width as f32, resolution.height as f32);
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
