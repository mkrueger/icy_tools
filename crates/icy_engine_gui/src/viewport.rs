use icy_engine::{Position, Rectangle, Size};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Instant;

use crate::ScrollbarState;

/// Default scroll animation speed (units per second for interpolation)
pub const DEFAULT_SCROLL_ANIMATION_SPEED: f32 = 15.0;

/// Default animation tick interval in milliseconds (~60fps)
pub const ANIMATION_TICK_MS: u64 = 16;

/// Viewport for managing screen view transformations
/// Handles scrolling, zooming, and coordinate transformations
///
/// All scroll values are in CONTENT coordinates (not screen pixels).
/// This makes the math simpler: scroll_x is the content pixel offset.
#[derive(Debug)]
pub struct Viewport {
    /// Scroll offset in CONTENT pixels from top-left
    pub scroll_x: f32,
    pub scroll_y: f32,

    /// Zoom level (1.0 = 100%, 2.0 = 200%, etc.)
    pub zoom: f32,

    /// Size of the visible viewport in screen pixels (widget size)
    pub visible_width: f32,
    pub visible_height: f32,

    /// Size of the content being viewed in content pixels (at zoom 1.0)
    pub content_width: f32,
    pub content_height: f32,

    /// Smooth scrolling animation state (in content pixels)
    pub target_scroll_x: f32,
    pub target_scroll_y: f32,
    pub scroll_animation_speed: f32,

    /// Last update time for animation
    pub last_update: Option<Instant>,

    pub changed: AtomicBool,

    /// Widget bounds height in logical pixels
    /// Updated by the shader based on available widget bounds
    pub bounds_height: Arc<AtomicU32>,
    /// Widget bounds width in logical pixels
    /// Updated by the shader based on available widget bounds
    pub bounds_width: Arc<AtomicU32>,
    /// Visible content height in content pixels (computed by shader)
    /// This is how much of the content is visible at the current zoom level
    /// Stored as f32 bits for atomic access
    pub computed_visible_height: Arc<AtomicU32>,
    /// Visible content width in content pixels (computed by shader)
    /// Stored as f32 bits for atomic access
    pub computed_visible_width: Arc<AtomicU32>,

    /// Scrollbar state (animations, hover, drag) - integrated for convenience
    /// Scrollbar and viewport are always used together
    pub scrollbar: ScrollbarState,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            scroll_x: 0.0,
            scroll_y: 0.0,
            zoom: 1.0,
            visible_width: 800.0,
            visible_height: 600.0,
            content_width: 800.0,
            content_height: 600.0,
            target_scroll_x: 0.0,
            target_scroll_y: 0.0,
            scroll_animation_speed: DEFAULT_SCROLL_ANIMATION_SPEED,
            last_update: None,
            changed: AtomicBool::new(false),
            bounds_height: Arc::new(AtomicU32::new(0)),
            bounds_width: Arc::new(AtomicU32::new(0)),
            computed_visible_height: Arc::new(AtomicU32::new(0)),
            computed_visible_width: Arc::new(AtomicU32::new(0)),
            scrollbar: ScrollbarState::new(),
        }
    }
}

impl Viewport {
    pub fn new(visible_size: Size, content_size: Size) -> Self {
        Self {
            visible_width: visible_size.width as f32,
            visible_height: visible_size.height as f32,
            content_width: content_size.width as f32,
            content_height: content_size.height as f32,
            ..Default::default()
        }
    }

    /// Get the visible region in content coordinates
    pub fn visible_region(&self) -> Rectangle {
        // scroll_x/y are already in content coordinates
        let x = self.scroll_x as i32;
        let y = self.scroll_y as i32;
        let width = self.visible_content_width().ceil() as i32;
        let height = self.visible_content_height().ceil() as i32;

        Rectangle::from(x, y, width, height)
    }

    /// Get the visible region with explicit visible size (screen pixels)
    pub fn visible_region_with_size(&self, visible_width: f32, visible_height: f32) -> Rectangle {
        // scroll_x/y are already in content coordinates
        let x = self.scroll_x as i32;
        let y = self.scroll_y as i32;
        let width = (visible_width / self.zoom).ceil() as i32;
        let height = (visible_height / self.zoom).ceil() as i32;

        Rectangle::from(x, y, width, height)
    }

    /// Convert screen coordinates to content coordinates
    pub fn screen_to_content(&self, screen_x: f32, screen_y: f32) -> Position {
        // screen position / zoom + scroll offset (in content coords)
        Position::new((screen_x / self.zoom + self.scroll_x) as i32, (screen_y / self.zoom + self.scroll_y) as i32)
    }

    /// Convert content coordinates to screen coordinates
    pub fn content_to_screen(&self, content_pos: Position) -> (f32, f32) {
        // (content position - scroll offset) * zoom
        (
            (content_pos.x as f32 - self.scroll_x) * self.zoom,
            (content_pos.y as f32 - self.scroll_y) * self.zoom,
        )
    }

    /// How many content pixels are visible at current zoom
    /// Uses shader-computed values if available, otherwise falls back to visible_size / zoom
    pub fn visible_content_width(&self) -> f32 {
        let computed = f32::from_bits(self.computed_visible_width.load(Ordering::Relaxed));
        if computed > 0.0 { computed } else { self.visible_width / self.zoom }
    }

    /// How many content pixels are visible at current zoom
    /// Uses shader-computed values if available, otherwise falls back to visible_size / zoom
    pub fn visible_content_height(&self) -> f32 {
        let computed = f32::from_bits(self.computed_visible_height.load(Ordering::Relaxed));
        if computed > 0.0 { computed } else { self.visible_height / self.zoom }
    }

    /// Get maximum scroll values (in content pixels)
    pub fn max_scroll_x(&self) -> f32 {
        (self.content_width - self.visible_content_width()).max(0.0)
    }

    pub fn max_scroll_y(&self) -> f32 {
        (self.content_height - self.visible_content_height()).max(0.0)
    }

    /// Check if content is scrollable vertically (has more content than visible area)
    pub fn is_scrollable_y(&self) -> bool {
        self.max_scroll_y() > 0.0
    }

    /// Check if content is scrollable horizontally (has more content than visible area)
    pub fn is_scrollable_x(&self) -> bool {
        self.max_scroll_x() > 0.0
    }

    /// Get bounds height in logical pixels
    pub fn bounds_height(&self) -> u32 {
        self.bounds_height.load(Ordering::Relaxed)
    }

    /// Get bounds width in logical pixels  
    pub fn bounds_width(&self) -> u32 {
        self.bounds_width.load(Ordering::Relaxed)
    }

    /// Clamp scroll values to valid range
    pub fn clamp_scroll(&mut self) {
        self.scroll_x = self.scroll_x.clamp(0.0, self.max_scroll_x());
        self.scroll_y = self.scroll_y.clamp(0.0, self.max_scroll_y());
        self.target_scroll_x = self.target_scroll_x.clamp(0.0, self.max_scroll_x());
        self.target_scroll_y = self.target_scroll_y.clamp(0.0, self.max_scroll_y());
        println!("max_y: {} height:{}", self.max_scroll_y(), self.content_height);
    }

    /// Clamp scroll values with explicit visible size (in screen pixels)
    pub fn clamp_scroll_with_size(&mut self, visible_width: f32, visible_height: f32) {
        let visible_content_w = visible_width / self.zoom;
        let visible_content_h = visible_height / self.zoom;
        let max_scroll_x = (self.content_width - visible_content_w).max(0.0);
        let max_scroll_y = (self.content_height - visible_content_h).max(0.0);

        self.scroll_x = self.scroll_x.clamp(0.0, max_scroll_x);
        self.scroll_y = self.scroll_y.clamp(0.0, max_scroll_y);
        self.target_scroll_x = self.target_scroll_x.clamp(0.0, max_scroll_x);
        self.target_scroll_y = self.target_scroll_y.clamp(0.0, max_scroll_y);
    }

    /// Set zoom level and adjust scroll to keep center point stable
    pub fn set_zoom(&mut self, new_zoom: f32, _center_x: f32, _center_y: f32) {
        self.zoom = new_zoom.clamp(0.1, 10.0);
        // scroll_x/y are in content coordinates, so they don't need adjustment
        // Just clamp to new valid range
        self.clamp_scroll();
        self.target_scroll_x = self.scroll_x;
        self.target_scroll_y = self.scroll_y;
    }

    /// Scroll X by delta (for mouse wheel, trackpad) - delta is in content pixels
    pub fn scroll_x_by(&mut self, delta: f32) {
        self.scroll_x += delta;
        self.target_scroll_x = self.scroll_x;
        self.clamp_scroll();
        self.changed.store(true, Ordering::Relaxed);
    }

    /// Scroll Y by delta (for mouse wheel, trackpad) - delta is in content pixels
    pub fn scroll_y_by(&mut self, delta: f32) {
        self.scroll_y += delta;
        self.target_scroll_y = self.scroll_y;
        self.clamp_scroll();
        self.changed.store(true, Ordering::Relaxed);
    }

    /// Scroll X by delta with smooth animation (for PageUp/PageDown)
    pub fn scroll_x_by_smooth(&mut self, delta: f32) {
        self.target_scroll_x += delta;
        self.clamp_scroll();
        self.last_update = Some(Instant::now());
    }

    /// Scroll Y by delta with smooth animation (for PageUp/PageDown)
    pub fn scroll_y_by_smooth(&mut self, delta: f32) {
        self.target_scroll_y += delta;
        self.clamp_scroll();
        self.last_update = Some(Instant::now());
    }

    /// Set scroll X position directly (no animation)
    pub fn scroll_x_to(&mut self, x: f32) {
        self.scroll_x = x;
        self.target_scroll_x = x;
        self.clamp_scroll();
        self.changed.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Set scroll Y position directly (no animation)
    pub fn scroll_y_to(&mut self, y: f32) {
        self.scroll_y = y;
        self.target_scroll_y = y;
        self.clamp_scroll();
        self.changed.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Set scroll X position with smooth animation (for Home/End/PageUp/PageDown)
    pub fn scroll_x_to_smooth(&mut self, x: f32) {
        self.target_scroll_x = x;
        self.clamp_scroll();
        self.last_update = Some(Instant::now());
    }

    /// Set scroll Y position with smooth animation (for Home/End/PageUp/PageDown)
    pub fn scroll_y_to_smooth(&mut self, y: f32) {
        self.target_scroll_y = y;
        self.clamp_scroll();
        self.last_update = Some(Instant::now());
    }

    /// Update smooth scrolling animation AND scrollbar animation
    /// Returns true if the viewport changed and a redraw is needed
    pub fn update_animation(&mut self) {
        // Update scrollbar animation
        self.scrollbar.update_animation();

        // Early return if not animating scroll
        if !self.is_animating() {
            return;
        }

        let now = Instant::now();
        let delta_time = if let Some(last) = self.last_update {
            now.duration_since(last).as_secs_f32()
        } else {
            0.016 // ~60fps fallback
        };
        self.last_update = Some(now);
        let mut changed = false;

        // Ease-out cubic interpolation for smoother, more natural feel
        // t approaches 1 over time, ease_factor provides deceleration
        let t = (self.scroll_animation_speed * delta_time).min(1.0);
        let ease_factor = 1.0 - (1.0 - t).powi(3); // Cubic ease-out

        if (self.scroll_x - self.target_scroll_x).abs() > 0.5 {
            self.scroll_x += (self.target_scroll_x - self.scroll_x) * ease_factor;
            changed = true;
        } else if self.scroll_x != self.target_scroll_x {
            self.scroll_x = self.target_scroll_x;
            changed = true;
        }

        if (self.scroll_y - self.target_scroll_y).abs() > 0.5 {
            self.scroll_y += (self.target_scroll_y - self.scroll_y) * ease_factor;
            changed = true;
        } else if self.scroll_y != self.target_scroll_y {
            self.scroll_y = self.target_scroll_y;
            changed = true;
        }

        if changed {
            self.changed.store(true, std::sync::atomic::Ordering::Relaxed);
            // Sync scrollbar position when scroll changes
            self.sync_scrollbar_position();
        }
    }

    /// Sync scrollbar position with current scroll position
    pub fn sync_scrollbar_position(&mut self) {
        let max_y = self.max_scroll_y();
        if max_y > 0.0 {
            self.scrollbar.set_scroll_position(self.scroll_y / max_y);
        }
        let max_x = self.max_scroll_x();
        if max_x > 0.0 {
            self.scrollbar.set_scroll_position_x(self.scroll_x / max_x);
        }
    }

    /// Check if viewport is currently animating scroll position
    pub fn is_animating(&self) -> bool {
        (self.scroll_x - self.target_scroll_x).abs() > 0.5 || (self.scroll_y - self.target_scroll_y).abs() > 0.5
    }

    /// Check if any animation is needed (scroll or scrollbar)
    pub fn needs_animation(&self) -> bool {
        self.is_animating() || self.scrollbar.needs_animation()
    }

    /// Update viewport size
    pub fn set_visible_size(&mut self, width: f32, height: f32) {
        self.visible_width = width;
        self.visible_height = height;
        self.clamp_scroll();
    }

    /// Update content size
    pub fn set_content_size(&mut self, width: f32, height: f32) {
        self.content_width = width;
        self.content_height = height;
        self.clamp_scroll();
    }
}
