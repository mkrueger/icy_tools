use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::AtomicU64;

use crate::{Blink, CRTShaderProgram, Message, MonitorSettings, Terminal, UnicodeGlyphCache, Viewport};
use iced::Element;
use iced::Rectangle;
use iced::widget::shader;
use icy_engine::GraphicsType;
use icy_engine::MouseField;
use icy_engine::MouseState;
use icy_engine::Position;
use icy_engine::Screen;
use icy_engine::Size;

pub static TERMINAL_SHADER_INSTANCE_COUNTER: AtomicU64 = AtomicU64::new(1);
pub static PENDING_INSTANCE_REMOVALS: Mutex<Vec<u64>> = Mutex::new(Vec::new());

/// Cached screen info for mouse mapping calculations and cache invalidation
/// Updated during internal_draw to avoid extra locks in internal_update
#[derive(Clone)]
pub struct CachedScreenInfo {
    pub font_w: f32,
    pub font_h: f32,
    pub screen_width: i32,
    pub screen_height: i32,
    pub resolution: Size,
    pub scan_lines: bool,
    /// Cached render size in pixels (from last full render)
    pub render_size: (u32, u32),
    /// Last selection state for cache invalidation (anchor, lead, locked)
    pub last_selection_state: (Option<Position>, Option<Position>, bool),
    /// Last buffer version for cache invalidation
    pub last_buffer_version: u64,
    /// Graphics type of the screen (Text, IGS, Skypix, Rip)
    pub graphics_type: GraphicsType,
    /// Last bounds size for detecting window resize
    pub last_bounds_size: (f32, f32),
}

impl Default for CachedScreenInfo {
    fn default() -> Self {
        Self {
            font_w: 0.0,
            font_h: 0.0,
            screen_width: 0,
            screen_height: 0,
            resolution: Size::default(),
            scan_lines: false,
            render_size: (0, 0),
            last_selection_state: (None, None, false),
            last_buffer_version: u64::MAX,
            graphics_type: GraphicsType::Text,
            last_bounds_size: (0.0, 0.0),
        }
    }
}

pub struct CRTShaderState {
    pub caret_blink: crate::Blink,
    pub character_blink: crate::Blink,

    // Mouse/selection tracking
    pub dragging: bool,
    pub drag_anchor: Option<Position>,
    pub last_drag_position: Option<Position>,
    pub shift_pressed_during_selection: bool,

    // Modifier tracking
    pub alt_pressed: bool,
    pub shift_pressed: bool,
    pub ctrl_pressed: bool,

    // Hover tracking
    pub hovered_cell: Option<Position>,
    pub hovered_link: Option<String>,
    /// Track which RIP field is hovered (by index)
    pub hovered_rip_field: Option<MouseField>,

    /// Cached mouse state from last draw (updated during internal_draw to avoid extra lock in internal_update)
    pub cached_mouse_state: parking_lot::Mutex<Option<MouseState>>,

    /// Cached screen info from last draw (font dimensions, screen size, etc.)
    pub cached_screen_info: parking_lot::Mutex<CachedScreenInfo>,

    pub instance_id: u64,

    pub unicode_glyph_cache: Arc<parking_lot::Mutex<Option<UnicodeGlyphCache>>>,

    pub cached_rgba_blink_on: parking_lot::Mutex<Vec<u8>>,
    pub cached_rgba_blink_off: parking_lot::Mutex<Vec<u8>>,
}

impl CRTShaderState {
    /// Create a new CRTShaderState with blink rates based on the buffer type
    pub fn new(buffer_type: icy_engine::BufferType) -> Self {
        Self {
            caret_blink: Blink::new(buffer_type.get_caret_blink_rate() as u128),
            character_blink: Blink::new(buffer_type.get_blink_rate() as u128),
            dragging: false,
            drag_anchor: None,
            last_drag_position: None,
            shift_pressed_during_selection: false,
            alt_pressed: false,
            shift_pressed: false,
            ctrl_pressed: false,
            hovered_cell: None,
            hovered_link: None,
            hovered_rip_field: None,
            cached_mouse_state: parking_lot::Mutex::new(None),
            cached_screen_info: parking_lot::Mutex::new(CachedScreenInfo::default()),
            instance_id: TERMINAL_SHADER_INSTANCE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            unicode_glyph_cache: Arc::new(parking_lot::Mutex::new(None)),
            cached_rgba_blink_on: parking_lot::Mutex::new(Vec::new()),
            cached_rgba_blink_off: parking_lot::Mutex::new(Vec::new()),
        }
    }

    /// Create a new CRTShaderState from a Screen
    pub fn from_screen(screen: &dyn Screen) -> Self {
        Self::new(screen.buffer_type())
    }

    pub fn reset_caret(&mut self) {
        self.caret_blink.reset();
    }

    /// Update cached screen info from a screen reference
    /// Called during internal_draw while screen is already locked
    pub fn update_cached_screen_info(&self, screen: &dyn Screen) {
        let mut info = self.cached_screen_info.lock();
        if let Some(font) = screen.get_font(0) {
            info.font_w = font.size().width as f32;
            info.font_h = font.size().height as f32;
        }
        info.screen_width = screen.get_width();
        info.screen_height = screen.get_height();
        info.resolution = screen.get_resolution();
        info.scan_lines = screen.scan_lines();
        info.graphics_type = screen.graphics_type();
    }

    /// Map mouse coordinates to cell position using cached screen info
    /// If viewport is provided, returns absolute document coordinates (with scroll offset)
    /// Otherwise returns visible cell coordinates
    pub fn map_mouse_to_cell(&self, monitor: &MonitorSettings, bounds: Rectangle, mx: f32, my: f32, viewport: &Viewport) -> Option<Position> {
        let info = self.cached_screen_info.lock();

        let scale_factor = crate::get_scale_factor();

        // Scale mouse coordinates and bounds
        let scaled_bounds = bounds * scale_factor;

        // Convert mouse to widget-local coordinates (relative to top-left of bounds)
        let local_mx = mx * scale_factor - scaled_bounds.x;
        let local_my = my * scale_factor - scaled_bounds.y;

        if info.font_w <= 0.0 || info.font_h <= 0.0 {
            return None;
        }

        // Use cached render_size if available (this is what the shader actually rendered)
        let (term_px_w, term_px_h) = if info.render_size.0 > 0 && info.render_size.1 > 0 {
            (info.render_size.0 as f32, info.render_size.1 as f32)
        } else {
            // Fallback: calculate from screen dimensions
            let px_w = info.screen_width as f32 * info.font_w;
            let mut px_h = info.screen_height as f32 * info.font_h;
            if info.scan_lines {
                px_h *= 2.0;
            }
            (px_w, px_h)
        };

        // scroll_y is in pixels - convert to lines
        let scroll_offset_lines = (viewport.scroll_y / viewport.zoom / info.font_h).floor() as i32;

        if term_px_w <= 0.0 || term_px_h <= 0.0 {
            return None;
        }

        let avail_w = scaled_bounds.width.max(1.0);
        let avail_h = scaled_bounds.height.max(1.0);
        let uniform_scale = (avail_w / term_px_w).min(avail_h / term_px_h);

        let use_pp = monitor.use_pixel_perfect_scaling;
        let display_scale = if use_pp { uniform_scale.floor().max(1.0) } else { uniform_scale };

        let scaled_w = term_px_w * display_scale;
        let scaled_h = term_px_h * display_scale;

        // Calculate viewport offset within widget (centered)
        let vp_offset_x = (avail_w - scaled_w) / 2.0;
        let vp_offset_y = (avail_h - scaled_h) / 2.0;

        let (vp_x, vp_y) = if use_pp {
            (vp_offset_x.round(), vp_offset_y.round())
        } else {
            (vp_offset_x, vp_offset_y)
        };

        // Convert widget-local mouse coords to terminal-local coords
        let term_px_x = (local_mx - vp_x) / display_scale;
        let mut term_px_y = (local_my - vp_y) / display_scale;

        let actual_font_h = if info.scan_lines {
            term_px_y /= 2.0;
            info.font_h
        } else {
            info.font_h
        };

        let cx = (term_px_x / info.font_w).floor() as i32;
        let visible_cy = (term_px_y / actual_font_h).floor() as i32;

        // Add scroll offset (in lines) to get absolute document row
        let cy = visible_cy + scroll_offset_lines;

        Some(Position::new(cx, cy))
    }

    /// Map mouse coordinates to pixel position using cached screen info
    pub fn map_mouse_to_xy(&self, monitor: &MonitorSettings, bounds: Rectangle, mx: f32, my: f32) -> Option<Position> {
        let info = self.cached_screen_info.lock();

        let scale_factor = crate::get_scale_factor();
        let bounds = bounds * scale_factor;
        let mx = mx * scale_factor;
        let my = my * scale_factor;

        if info.font_w <= 0.0 || info.font_h <= 0.0 {
            return None;
        }

        let resolution_x = info.resolution.width as f32;
        let mut resolution_y = info.resolution.height as f32;

        if info.scan_lines {
            resolution_y *= 2.0;
        }

        let avail_w = bounds.width.max(1.0);
        let avail_h = bounds.height.max(1.0);
        let uniform_scale = (avail_w / resolution_x).min(avail_h / resolution_y);

        let use_pp = monitor.use_pixel_perfect_scaling;
        let display_scale = if use_pp { uniform_scale.floor().max(1.0) } else { uniform_scale };

        let scaled_w = resolution_x * display_scale;
        let scaled_h = resolution_y * display_scale;

        let offset_x = bounds.x + (avail_w - scaled_w) / 2.0;
        let offset_y = bounds.y + (avail_h - scaled_h) / 2.0;

        let (vp_x, vp_y, vp_w, vp_h) = if use_pp {
            (offset_x.round(), offset_y.round(), scaled_w.round(), scaled_h.round())
        } else {
            (offset_x, offset_y, scaled_w, scaled_h)
        };

        if mx < vp_x || my < vp_y || mx >= vp_x + vp_w || my >= vp_y + vp_h {
            return None;
        }

        let local_px_x = (mx - vp_x) / display_scale;
        let local_px_y = (my - vp_y) / display_scale;

        Some(Position::new(local_px_x as i32, local_px_y as i32))
    }
}

impl Drop for CRTShaderState {
    fn drop(&mut self) {
        PENDING_INSTANCE_REMOVALS.lock().push(self.instance_id);
    }
}

impl Default for CRTShaderState {
    fn default() -> Self {
        // Default to CP437 blink rates (most common case)
        let buffer_type = icy_engine::BufferType::CP437;
        Self {
            caret_blink: Blink::new(buffer_type.get_caret_blink_rate() as u128),
            character_blink: Blink::new(buffer_type.get_blink_rate() as u128),
            dragging: false,
            drag_anchor: None,
            last_drag_position: None,
            shift_pressed_during_selection: false,
            alt_pressed: false,
            shift_pressed: false,
            ctrl_pressed: false,
            hovered_cell: None,
            hovered_link: None,
            hovered_rip_field: None,
            cached_mouse_state: parking_lot::Mutex::new(None),
            cached_screen_info: parking_lot::Mutex::new(CachedScreenInfo::default()),
            instance_id: TERMINAL_SHADER_INSTANCE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            unicode_glyph_cache: Arc::new(parking_lot::Mutex::new(None)),

            cached_rgba_blink_on: parking_lot::Mutex::new(Vec::new()),
            cached_rgba_blink_off: parking_lot::Mutex::new(Vec::new()),
        }
    }
}

// Helper function to create shader with terminal and monitor settings
pub fn create_crt_shader<'a>(term: &'a Terminal, monitor_settings: MonitorSettings) -> Element<'a, Message> {
    // Let the parent wrapper decide sizing; shader can just be Fill.
    shader(CRTShaderProgram::new(term, monitor_settings))
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .into()
}

static SCALE_FACTOR_BITS: AtomicU32 = AtomicU32::new(f32::to_bits(1.0));

#[inline]
pub fn set_scale_factor(sf: f32) {
    // You can clamp or sanity-check here if desired
    SCALE_FACTOR_BITS.store(sf.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

#[inline]
pub fn get_scale_factor() -> f32 {
    f32::from_bits(SCALE_FACTOR_BITS.load(std::sync::atomic::Ordering::Relaxed))
}
