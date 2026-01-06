use icy_engine::{Position, Rectangle, Size};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Default scroll animation speed (units per second for interpolation)
pub const DEFAULT_SCROLL_ANIMATION_SPEED: f32 = 15.0;

/// Default animation tick interval in milliseconds (~60fps)
pub const ANIMATION_TICK_MS: u64 = 16;

/// Viewport-like state for smooth scrolling + coordinate transforms.
///
/// This is a generic helper used by list/grid views. Terminal rendering uses its own
/// internal viewport handling.
#[derive(Debug)]
pub struct ScrollViewport {
    scroll_x: f32,
    scroll_y: f32,

    zoom: f32,

    visible_width: f32,
    visible_height: f32,

    content_width: f32,
    content_height: f32,

    target_scroll_x: f32,
    target_scroll_y: f32,
    scroll_animation_speed: f32,

    last_update: Option<Instant>,

    changed: AtomicBool,
}

impl Default for ScrollViewport {
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
        }
    }
}

impl ScrollViewport {
    pub fn new(visible_size: Size, content_size: Size) -> Self {
        Self {
            visible_width: visible_size.width as f32,
            visible_height: visible_size.height as f32,
            content_width: content_size.width as f32,
            content_height: content_size.height as f32,
            ..Default::default()
        }
    }

    pub fn scroll_x(&self) -> f32 {
        self.scroll_x
    }

    pub fn scroll_y(&self) -> f32 {
        self.scroll_y
    }

    pub fn target_scroll_x(&self) -> f32 {
        self.target_scroll_x
    }

    pub fn target_scroll_y(&self) -> f32 {
        self.target_scroll_y
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    pub fn visible_width_px(&self) -> f32 {
        self.visible_width
    }

    pub fn visible_height_px(&self) -> f32 {
        self.visible_height
    }

    pub fn content_width(&self) -> f32 {
        self.content_width
    }

    pub fn content_height(&self) -> f32 {
        self.content_height
    }

    pub fn take_changed(&self) -> bool {
        self.changed.swap(false, Ordering::Relaxed)
    }

    pub fn mark_changed(&self) {
        self.changed.store(true, Ordering::Relaxed);
    }

    pub fn visible_region(&self) -> Rectangle {
        let x = self.scroll_x as i32;
        let y = self.scroll_y as i32;
        let width = self.visible_content_width().ceil() as i32;
        let height = self.visible_content_height().ceil() as i32;
        Rectangle::from(x, y, width, height)
    }

    pub fn visible_region_with_size(&self, visible_width: f32, visible_height: f32) -> Rectangle {
        let x = self.scroll_x as i32;
        let y = self.scroll_y as i32;
        let width = (visible_width / self.zoom).ceil() as i32;
        let height = (visible_height / self.zoom).ceil() as i32;
        Rectangle::from(x, y, width, height)
    }

    pub fn screen_to_content(&self, screen_x: f32, screen_y: f32) -> Position {
        Position::new((screen_x / self.zoom + self.scroll_x) as i32, (screen_y / self.zoom + self.scroll_y) as i32)
    }

    pub fn content_to_screen(&self, content_pos: Position) -> (f32, f32) {
        (
            (content_pos.x as f32 - self.scroll_x) * self.zoom,
            (content_pos.y as f32 - self.scroll_y) * self.zoom,
        )
    }

    pub fn visible_content_width(&self) -> f32 {
        self.visible_width / self.zoom.max(0.001)
    }

    pub fn visible_content_height(&self) -> f32 {
        self.visible_height / self.zoom.max(0.001)
    }

    pub fn max_scroll_x(&self) -> f32 {
        (self.content_width - self.visible_content_width()).max(0.0)
    }

    pub fn max_scroll_y(&self) -> f32 {
        (self.content_height - self.visible_content_height()).max(0.0)
    }

    pub fn is_scrollable_x(&self) -> bool {
        self.max_scroll_x() > 0.0
    }

    pub fn is_scrollable_y(&self) -> bool {
        self.max_scroll_y() > 0.0
    }

    /// Set zoom while keeping the content under the given screen-space center stable.
    ///
    /// `center_x`/`center_y` are in screen pixels relative to the viewport.
    pub fn set_zoom(&mut self, zoom: f32, center_x: f32, center_y: f32) {
        let zoom = zoom.max(0.001);

        // Content coordinate under the center before zoom change.
        let content_before_x = center_x / self.zoom.max(0.001) + self.scroll_x;
        let content_before_y = center_y / self.zoom.max(0.001) + self.scroll_y;

        self.zoom = zoom;

        // Adjust scroll so the same content point stays under the center.
        self.scroll_x = content_before_x - center_x / self.zoom;
        self.scroll_y = content_before_y - center_y / self.zoom;
        self.target_scroll_x = self.scroll_x;
        self.target_scroll_y = self.scroll_y;

        self.clamp_scroll();
        self.changed.store(true, Ordering::Relaxed);
    }

    pub fn clamp_scroll(&mut self) {
        let max_x = self.max_scroll_x();
        let max_y = self.max_scroll_y();
        self.scroll_x = self.scroll_x.clamp(0.0, max_x);
        self.scroll_y = self.scroll_y.clamp(0.0, max_y);
        self.target_scroll_x = self.target_scroll_x.clamp(0.0, max_x);
        self.target_scroll_y = self.target_scroll_y.clamp(0.0, max_y);
    }

    pub fn set_visible_size(&mut self, width: f32, height: f32) {
        self.visible_width = width;
        self.visible_height = height;
        self.clamp_scroll();
    }

    pub fn set_content_size(&mut self, width: f32, height: f32) {
        self.content_width = width;
        self.content_height = height;
        self.clamp_scroll();
    }

    pub fn scroll_x_by(&mut self, delta: f32) {
        self.scroll_x += delta;
        self.target_scroll_x = self.scroll_x;
        self.clamp_scroll();
        self.changed.store(true, Ordering::Relaxed);
    }

    pub fn scroll_y_by(&mut self, delta: f32) {
        self.scroll_y += delta;
        self.target_scroll_y = self.scroll_y;
        self.clamp_scroll();
        self.changed.store(true, Ordering::Relaxed);
    }

    pub fn scroll_x_by_smooth(&mut self, delta: f32) {
        self.target_scroll_x += delta;
        self.clamp_scroll();
        self.last_update = Some(Instant::now());
    }

    pub fn scroll_y_by_smooth(&mut self, delta: f32) {
        self.target_scroll_y += delta;
        self.clamp_scroll();
        self.last_update = Some(Instant::now());
    }

    pub fn scroll_x_to(&mut self, x: f32) {
        self.scroll_x = x;
        self.target_scroll_x = x;
        self.clamp_scroll();
        self.changed.store(true, Ordering::Relaxed);
    }

    pub fn scroll_y_to(&mut self, y: f32) {
        self.scroll_y = y;
        self.target_scroll_y = y;
        self.clamp_scroll();
        self.changed.store(true, Ordering::Relaxed);
    }

    pub fn scroll_x_to_smooth(&mut self, x: f32) {
        self.target_scroll_x = x;
        self.clamp_scroll();
        self.last_update = Some(Instant::now());
    }

    pub fn scroll_y_to_smooth(&mut self, y: f32) {
        self.target_scroll_y = y;
        self.clamp_scroll();
        self.last_update = Some(Instant::now());
    }

    pub fn is_animating(&self) -> bool {
        (self.scroll_x - self.target_scroll_x).abs() > 0.5 || (self.scroll_y - self.target_scroll_y).abs() > 0.5
    }

    pub fn needs_animation(&self) -> bool {
        self.is_animating()
    }

    pub fn update_animation(&mut self) {
        if !self.needs_animation() {
            self.last_update = None;
            return;
        }

        let now = Instant::now();
        let dt = if let Some(last) = self.last_update {
            now.duration_since(last).as_secs_f32()
        } else {
            0.0
        };
        self.last_update = Some(now);

        let speed = self.scroll_animation_speed.max(0.0);
        let t = (dt * speed).clamp(0.0, 1.0);

        self.scroll_x = self.scroll_x + (self.target_scroll_x - self.scroll_x) * t;
        self.scroll_y = self.scroll_y + (self.target_scroll_y - self.scroll_y) * t;

        self.clamp_scroll();
        self.changed.store(true, Ordering::Relaxed);
    }
}
