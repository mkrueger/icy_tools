use crate::ScrollbarState;
use icy_engine::{Position, Rectangle, Size};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Default scroll animation speed (units per second for interpolation)
pub const DEFAULT_SCROLL_ANIMATION_SPEED: f32 = 15.0;

/// Default animation tick interval in milliseconds (~60fps)
pub const ANIMATION_TICK_MS: u64 = 16;

/// Scrollbar layout constants (same as in scrollbar_overlay.rs)
const SCROLLBAR_MIN_WIDTH: f32 = 3.0;
const SCROLLBAR_MAX_WIDTH: f32 = 9.0;
const SCROLLBAR_TOP_PADDING: f32 = 0.0;
const SCROLLBAR_BOTTOM_PADDING: f32 = 0.0;
const SCROLLBAR_LEFT_PADDING: f32 = 0.0;
const SCROLLBAR_RIGHT_PADDING: f32 = 0.0;
const SCROLLBAR_MIN_THUMB_SIZE: f32 = 30.0;

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
    /// Derived from widget visible size and zoom
    pub fn visible_content_width(&self) -> f32 {
        self.visible_width / self.zoom.max(0.001)
    }

    /// How many content pixels are visible at current zoom
    /// Derived from widget visible size and zoom
    pub fn visible_content_height(&self) -> f32 {
        self.visible_height / self.zoom.max(0.001)
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

    /// Clamp scroll values to valid range
    pub fn clamp_scroll(&mut self) {
        self.scroll_x = self.scroll_x.clamp(0.0, self.max_scroll_x());
        self.scroll_y = self.scroll_y.clamp(0.0, self.max_scroll_y());
        self.target_scroll_x = self.target_scroll_x.clamp(0.0, self.max_scroll_x());
        self.target_scroll_y = self.target_scroll_y.clamp(0.0, self.max_scroll_y());
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

    /// Set zoom level and adjust scroll to keep the given center point stable.
    ///
    /// `center_x/center_y` are in SCREEN coordinates (widget pixels).
    pub fn set_zoom(&mut self, new_zoom: f32, center_x: f32, center_y: f32) {
        let old_zoom = self.zoom.max(0.001);
        let new_zoom = new_zoom.clamp(0.1, 10.0);

        // Content position currently under the given screen point.
        let content_x = center_x / old_zoom + self.scroll_x;
        let content_y = center_y / old_zoom + self.scroll_y;

        self.zoom = new_zoom;
        let effective_new_zoom = self.zoom.max(0.001);

        // Adjust scroll so that the same content point stays under the same screen point.
        self.scroll_x = content_x - center_x / effective_new_zoom;
        self.scroll_y = content_y - center_y / effective_new_zoom;

        self.clamp_scroll();
        self.target_scroll_x = self.scroll_x;
        self.target_scroll_y = self.scroll_y;
        self.sync_scrollbar_position();
        self.changed.store(true, Ordering::Relaxed);
    }

    /// Scroll X by delta (for mouse wheel, trackpad) - delta is in content pixels
    pub fn scroll_x_by(&mut self, delta: f32) {
        self.scroll_x += delta;
        self.target_scroll_x = self.scroll_x;
        self.clamp_scroll();
        self.sync_scrollbar_position();
        self.changed.store(true, Ordering::Relaxed);
    }

    /// Scroll Y by delta (for mouse wheel, trackpad) - delta is in content pixels
    pub fn scroll_y_by(&mut self, delta: f32) {
        self.scroll_y += delta;
        self.target_scroll_y = self.scroll_y;
        self.clamp_scroll();
        self.sync_scrollbar_position();
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
        self.sync_scrollbar_position();
        self.changed.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Set scroll Y position directly (no animation)
    pub fn scroll_y_to(&mut self, y: f32) {
        self.scroll_y = y;
        self.target_scroll_y = y;
        self.clamp_scroll();
        self.sync_scrollbar_position();
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

    /// Update smooth scrolling animation
    /// Note: Called automatically by ScrollbarOverlay (ViewportAccess version).
    /// Only call manually when using ScrollbarOverlayCallback.
    pub fn update_animation(&mut self) {
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
    /// Note: Called automatically by ScrollbarOverlay (ViewportAccess version).
    /// Only call manually when using ScrollbarOverlayCallback.
    pub fn is_animating(&self) -> bool {
        (self.scroll_x - self.target_scroll_x).abs() > 0.5 || (self.scroll_y - self.target_scroll_y).abs() > 0.5
    }

    /// Check if any animation is needed (scroll or scrollbar)
    /// Note: Called automatically by ScrollbarOverlay (ViewportAccess version).
    /// Only call manually when using ScrollbarOverlayCallback.
    pub fn needs_animation(&self) -> bool {
        self.is_animating() || self.scrollbar.needs_animation()
    }

    /// Update viewport size
    pub fn set_visible_size(&mut self, width: f32, height: f32) {
        let old_scroll_x = self.scroll_x;
        let old_scroll_y = self.scroll_y;
        let old_target_x = self.target_scroll_x;
        let old_target_y = self.target_scroll_y;
        self.visible_width = width;
        self.visible_height = height;
        self.clamp_scroll();

        if self.scroll_x != old_scroll_x || self.scroll_y != old_scroll_y || self.target_scroll_x != old_target_x || self.target_scroll_y != old_target_y {
            self.sync_scrollbar_position();
            self.changed.store(true, Ordering::Relaxed);
        }
    }

    /// Update content size
    pub fn set_content_size(&mut self, width: f32, height: f32) {
        let old_scroll_x = self.scroll_x;
        let old_scroll_y = self.scroll_y;
        let old_target_x = self.target_scroll_x;
        let old_target_y = self.target_scroll_y;
        self.content_width = width;
        self.content_height = height;
        self.clamp_scroll();

        if self.scroll_x != old_scroll_x || self.scroll_y != old_scroll_y || self.target_scroll_x != old_target_x || self.target_scroll_y != old_target_y {
            self.sync_scrollbar_position();
            self.changed.store(true, Ordering::Relaxed);
        }
    }

    // =========================================================================
    // Scrollbar event handling - self-contained like iced's built-in widgets
    // =========================================================================

    /// Get vertical scrollbar height ratio (visible / content)
    pub fn height_ratio(&self) -> f32 {
        let visible = self.visible_content_height();
        visible / self.content_height.max(1.0)
    }

    /// Get horizontal scrollbar width ratio (visible / content)
    pub fn width_ratio(&self) -> f32 {
        let visible = self.visible_content_width();
        visible / self.content_width.max(1.0)
    }

    /// Check if vertical scrollbar is needed
    pub fn needs_vscrollbar(&self) -> bool {
        self.height_ratio() < 1.0
    }

    /// Check if horizontal scrollbar is needed
    pub fn needs_hscrollbar(&self) -> bool {
        self.width_ratio() < 1.0
    }

    /// Calculate thumb position and size for vertical scrollbar
    /// Returns (thumb_y, thumb_height) in bounds coordinates
    fn calc_vthumb(&self, bounds_height: f32) -> (f32, f32) {
        let available = bounds_height - SCROLLBAR_TOP_PADDING - SCROLLBAR_BOTTOM_PADDING;
        let thumb_height = (available * self.height_ratio()).max(SCROLLBAR_MIN_THUMB_SIZE);
        let max_thumb_offset = available - thumb_height;
        let thumb_y = SCROLLBAR_TOP_PADDING + (max_thumb_offset * self.scrollbar.scroll_position);
        (thumb_y, thumb_height)
    }

    /// Calculate thumb position and size for horizontal scrollbar
    /// Returns (thumb_x, thumb_width) in bounds coordinates
    fn calc_hthumb(&self, bounds_width: f32) -> (f32, f32) {
        let available = bounds_width - SCROLLBAR_LEFT_PADDING - SCROLLBAR_RIGHT_PADDING;
        let thumb_width = (available * self.width_ratio()).max(SCROLLBAR_MIN_THUMB_SIZE);
        let max_thumb_offset = available - thumb_width;
        let thumb_x = SCROLLBAR_LEFT_PADDING + (max_thumb_offset * self.scrollbar.scroll_position_x);
        (thumb_x, thumb_width)
    }

    /// Calculate scroll ratio (0.0-1.0) from mouse Y position in vertical scrollbar
    fn calc_vscroll_ratio_from_y(&self, mouse_y: f32, bounds_height: f32) -> f32 {
        let available = bounds_height - SCROLLBAR_TOP_PADDING - SCROLLBAR_BOTTOM_PADDING;
        let (_, thumb_height) = self.calc_vthumb(bounds_height);
        let click_offset = mouse_y - SCROLLBAR_TOP_PADDING - (thumb_height / 2.0);
        let max_thumb_offset = available - thumb_height;
        if max_thumb_offset > 0.0 {
            (click_offset / max_thumb_offset).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Calculate scroll ratio (0.0-1.0) from mouse X position in horizontal scrollbar
    fn calc_hscroll_ratio_from_x(&self, mouse_x: f32, bounds_width: f32) -> f32 {
        let available = bounds_width - SCROLLBAR_LEFT_PADDING - SCROLLBAR_RIGHT_PADDING;
        let (_, thumb_width) = self.calc_hthumb(bounds_width);
        let click_offset = mouse_x - SCROLLBAR_LEFT_PADDING - (thumb_width / 2.0);
        let max_thumb_offset = available - thumb_width;
        if max_thumb_offset > 0.0 {
            (click_offset / max_thumb_offset).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Handle vertical scrollbar mouse press
    /// pos_y: mouse Y position relative to scrollbar bounds
    /// bounds_height: height of the scrollbar widget
    /// Returns true if event was consumed
    pub fn handle_vscrollbar_press(&mut self, pos_y: f32, bounds_height: f32) -> bool {
        self.scrollbar.set_dragging(true);
        let ratio = self.calc_vscroll_ratio_from_y(pos_y, bounds_height);
        let absolute_y = ratio * self.max_scroll_y();
        self.scroll_y_to(absolute_y);
        true
    }

    /// Handle horizontal scrollbar mouse press
    /// pos_x: mouse X position relative to scrollbar bounds
    /// bounds_width: width of the scrollbar widget
    /// Returns true if event was consumed
    pub fn handle_hscrollbar_press(&mut self, pos_x: f32, bounds_width: f32) -> bool {
        self.scrollbar.set_dragging_x(true);
        let ratio = self.calc_hscroll_ratio_from_x(pos_x, bounds_width);
        let absolute_x = ratio * self.max_scroll_x();
        self.scroll_x_to(absolute_x);
        true
    }

    /// Handle vertical scrollbar mouse release
    /// Returns true if was dragging
    pub fn handle_vscrollbar_release(&mut self) -> bool {
        if self.scrollbar.is_dragging {
            self.scrollbar.set_dragging(false);
            true
        } else {
            false
        }
    }

    /// Handle horizontal scrollbar mouse release
    /// Returns true if was dragging
    pub fn handle_hscrollbar_release(&mut self) -> bool {
        if self.scrollbar.is_dragging_x {
            self.scrollbar.set_dragging_x(false);
            true
        } else {
            false
        }
    }

    /// Handle vertical scrollbar mouse move (while dragging)
    /// pos_y: mouse Y position (can be outside bounds during drag)
    /// bounds_height: height of the scrollbar widget
    /// Returns true if dragging and scroll position changed
    pub fn handle_vscrollbar_drag(&mut self, pos_y: f32, bounds_height: f32) -> bool {
        if self.scrollbar.is_dragging {
            let ratio = self.calc_vscroll_ratio_from_y(pos_y, bounds_height);
            let absolute_y = ratio * self.max_scroll_y();
            self.scroll_y_to(absolute_y);
            true
        } else {
            false
        }
    }

    /// Handle horizontal scrollbar mouse move (while dragging)
    /// pos_x: mouse X position (can be outside bounds during drag)
    /// bounds_width: width of the scrollbar widget
    /// Returns true if dragging and scroll position changed
    pub fn handle_hscrollbar_drag(&mut self, pos_x: f32, bounds_width: f32) -> bool {
        if self.scrollbar.is_dragging_x {
            let ratio = self.calc_hscroll_ratio_from_x(pos_x, bounds_width);
            let absolute_x = ratio * self.max_scroll_x();
            self.scroll_x_to(absolute_x);
            true
        } else {
            false
        }
    }

    /// Handle vertical scrollbar hover state change
    /// Returns true if hover state changed
    pub fn handle_vscrollbar_hover(&mut self, is_hovered: bool) -> bool {
        if self.scrollbar.is_hovered != is_hovered {
            self.scrollbar.set_hovered(is_hovered);
            true
        } else {
            false
        }
    }

    /// Handle horizontal scrollbar hover state change
    /// Returns true if hover state changed
    pub fn handle_hscrollbar_hover(&mut self, is_hovered: bool) -> bool {
        if self.scrollbar.is_hovered_x != is_hovered {
            self.scrollbar.set_hovered_x(is_hovered);
            true
        } else {
            false
        }
    }

    /// Get scrollbar layout constants for rendering
    pub fn scrollbar_width(&self) -> f32 {
        SCROLLBAR_MIN_WIDTH + (SCROLLBAR_MAX_WIDTH - SCROLLBAR_MIN_WIDTH) * self.scrollbar.visibility
    }

    /// Get horizontal scrollbar layout constants for rendering
    pub fn hscrollbar_height(&self) -> f32 {
        SCROLLBAR_MIN_WIDTH + (SCROLLBAR_MAX_WIDTH - SCROLLBAR_MIN_WIDTH) * self.scrollbar.visibility_x
    }
}
