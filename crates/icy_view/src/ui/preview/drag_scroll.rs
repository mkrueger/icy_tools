//! Drag scrolling with inertia support
//!
//! Provides shared drag-to-scroll functionality with inertia for both
//! terminal and image viewers.

use std::time::Instant;

/// Drag scroll state for tracking velocity and inertia
#[derive(Debug, Clone)]
pub struct DragScrollState {
    /// Whether we're currently dragging to scroll
    pub is_dragging: bool,
    /// Start drag position in screen pixels
    start_drag_pos: (f32, f32),
    /// Viewport position at drag start (content coordinates)
    start_viewport_pos: (f32, f32),
    /// Last drag position for velocity calculation
    last_drag_pos: (f32, f32),
    /// Time of last drag event for velocity calculation
    last_drag_time: Option<Instant>,
    /// Current scroll velocity for inertia (content pixels per second)
    scroll_velocity: (f32, f32),            
    /// Whether inertia scrolling is active
    pub inertia_active: bool,
}

impl Default for DragScrollState {
    fn default() -> Self {
        Self::new()
    }
}

impl DragScrollState {
    /// Create a new drag scroll state
    pub fn new() -> Self {
        Self {
            is_dragging: false,
            start_drag_pos: (0.0, 0.0),
            start_viewport_pos: (0.0, 0.0),
            last_drag_pos: (0.0, 0.0),
            last_drag_time: None,
            scroll_velocity: (0.0, 0.0),
            inertia_active: false,
        }
    }

    /// Start dragging at the given position, recording current viewport position
    pub fn start_drag(&mut self, screen_pos: (f32, f32), viewport_pos: (f32, f32)) {
        self.is_dragging = true;
        self.start_drag_pos = screen_pos;
        self.start_viewport_pos = viewport_pos;
        self.last_drag_pos = screen_pos;
        self.last_drag_time = Some(Instant::now());
        self.scroll_velocity = (0.0, 0.0);
        self.inertia_active = false;
    }

    /// End dragging and start inertia if we have velocity
    pub fn end_drag(&mut self) {
        if self.is_dragging {
            self.is_dragging = false;

            // Check if we have significant velocity for inertia
            let (vx, vy) = self.scroll_velocity;
            if vx.abs() > 10.0 || vy.abs() > 10.0 {
                self.inertia_active = true;
            }
        }
    }

    /// Process a drag movement, returns the absolute scroll position to set
    /// The zoom parameter is used to convert screen pixels to content pixels
    pub fn process_drag(&mut self, screen_pos: (f32, f32), zoom: f32) -> Option<(f32, f32)> {
        if !self.is_dragging {
            return None;
        }

        let now = Instant::now();

        // Calculate total offset from drag start in content coordinates
        let screen_dx = screen_pos.0 - self.start_drag_pos.0;
        let screen_dy = screen_pos.1 - self.start_drag_pos.1;
        let content_dx = screen_dx / zoom;
        let content_dy = screen_dy / zoom;

        // Calculate new absolute viewport position (drag moves opposite to content)
        let new_x = self.start_viewport_pos.0 - content_dx;
        let new_y = self.start_viewport_pos.1 - content_dy;

        // Calculate velocity from last position for inertia
        if let Some(last_time) = self.last_drag_time {
            let dt = now.duration_since(last_time).as_secs_f32();

            // Only calculate velocity if reasonable time has passed
            const MIN_DT: f32 = 0.001; // 1ms minimum - don't miss events
            const MAX_DT: f32 = 0.1; // Ignore if too long (paused drag)
            const MAX_VELOCITY: f32 = 8000.0; // Allow higher velocity for snappy feel

            if dt >= MIN_DT && dt < MAX_DT {
                let last_screen_dx = screen_pos.0 - self.last_drag_pos.0;
                let last_screen_dy = screen_pos.1 - self.last_drag_pos.1;
                let last_content_dx = last_screen_dx / zoom;
                let last_content_dy = last_screen_dy / zoom;

                // Velocity is inverted (content moves opposite to drag)
                let vel_x = (-last_content_dx / dt).clamp(-MAX_VELOCITY, MAX_VELOCITY);
                let vel_y = (-last_content_dy / dt).clamp(-MAX_VELOCITY, MAX_VELOCITY);

                // Light smoothing - keep it responsive but filter noise
                // egui uses very light smoothing, we use 0.5 to give recent movement more weight
                let smoothing = 0.5;
                self.scroll_velocity.0 = self.scroll_velocity.0 * (1.0 - smoothing) + vel_x * smoothing;
                self.scroll_velocity.1 = self.scroll_velocity.1 * (1.0 - smoothing) + vel_y * smoothing;
            }
        }

        self.last_drag_pos = screen_pos;
        self.last_drag_time = Some(now);

        Some((new_x, new_y))
    }

    /// Update inertia scrolling, returns the scroll delta (dx, dy) to apply in content coordinates
    /// Returns None if inertia is not active
    pub fn update_inertia(&mut self, delta_seconds: f32) -> Option<(f32, f32)> {
        if !self.inertia_active || self.is_dragging {
            return None;
        }

        let (vx, vy) = self.scroll_velocity;

        // Apply velocity (convert per-second to per-frame)
        let dx = vx * delta_seconds;
        let dy = vy * delta_seconds;

        // Apply friction: velocity *= (1 - friction * dt)
        // Lower friction = longer, smoother slide (egui uses ~1.0-3.0)
        let friction_coeff = 3.0;
        let friction = 1.0 - friction_coeff * delta_seconds;
        self.scroll_velocity.0 *= friction.max(0.0);
        self.scroll_velocity.1 *= friction.max(0.0);

        // Stop inertia when velocity is very low
        if vx.abs() < 10.0 && vy.abs() < 10.0 {
            self.inertia_active = false;
            self.scroll_velocity = (0.0, 0.0);
            return None;
        }

        if dx.abs() > 0.01 || dy.abs() > 0.01 { Some((dx, dy)) } else { None }
    }

    /// Check if animation updates are needed
    pub fn needs_animation(&self) -> bool {
        self.is_dragging || self.inertia_active
    }

    /// Stop all scrolling (called when user scrolls manually)
    pub fn stop(&mut self) {
        self.inertia_active = false;
        self.scroll_velocity = (0.0, 0.0);
    }
}
