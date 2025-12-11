use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::{Blink, CRTShaderProgram, Message, MonitorSettings, Terminal, UnicodeGlyphCache, Viewport};
use iced::Element;
use iced::widget::shader;
use icy_engine::GraphicsType;
use icy_engine::MouseState;
use icy_engine::Position;
use icy_engine::Screen;
use icy_engine::Size;

pub static TERMINAL_SHADER_INSTANCE_COUNTER: AtomicU64 = AtomicU64::new(1);
pub static PENDING_INSTANCE_REMOVALS: Mutex<Vec<u64>> = Mutex::new(Vec::new());

// Global modifier state - survives widget state resets
static GLOBAL_CTRL_PRESSED: AtomicBool = AtomicBool::new(false);
static GLOBAL_ALT_PRESSED: AtomicBool = AtomicBool::new(false);
static GLOBAL_SHIFT_PRESSED: AtomicBool = AtomicBool::new(false);
static GLOBAL_COMMAND_PRESSED: AtomicBool = AtomicBool::new(false);

/// Set global modifier state (called from keyboard events)
/// `command` should be true when the platform "command" key is pressed:
/// - macOS: Command (⌘) key
/// - Windows/Linux: Ctrl key
pub fn set_global_modifiers(ctrl: bool, alt: bool, shift: bool, command: bool) {
    GLOBAL_CTRL_PRESSED.store(ctrl, Ordering::Relaxed);
    GLOBAL_ALT_PRESSED.store(alt, Ordering::Relaxed);
    GLOBAL_SHIFT_PRESSED.store(shift, Ordering::Relaxed);
    GLOBAL_COMMAND_PRESSED.store(command, Ordering::Relaxed);
}

/// Get global Ctrl state
pub fn is_ctrl_pressed() -> bool {
    GLOBAL_CTRL_PRESSED.load(Ordering::Relaxed)
}

/// Get global Alt state
pub fn is_alt_pressed() -> bool {
    GLOBAL_ALT_PRESSED.load(Ordering::Relaxed)
}

/// Get global Shift state
pub fn is_shift_pressed() -> bool {
    GLOBAL_SHIFT_PRESSED.load(Ordering::Relaxed)
}

/// Get global Command state (Cmd on macOS, Ctrl on Windows/Linux)
/// Use this for cross-platform shortcuts like Cmd/Ctrl+C, Cmd/Ctrl+scroll for zoom
pub fn is_command_pressed() -> bool {
    GLOBAL_COMMAND_PRESSED.load(Ordering::Relaxed)
}

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

    // Hover tracking
    pub hovered_cell: Option<Position>,

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
            caret_blink: Blink::new(buffer_type.caret_blink_rate() as u128),
            character_blink: Blink::new(buffer_type.blink_rate() as u128),
            dragging: false,
            drag_anchor: None,
            last_drag_position: None,
            shift_pressed_during_selection: false,
            hovered_cell: None,
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
        if let Some(font) = screen.font(0) {
            info.font_w = font.size().width as f32;
            info.font_h = font.size().height as f32;
        }
        info.screen_width = screen.width();
        info.screen_height = screen.height();
        info.resolution = screen.resolution();
        info.scan_lines = screen.scan_lines();
        info.graphics_type = screen.graphics_type();
    }

    /// Map mouse coordinates to cell position using shared RenderInfo from shader.
    /// Returns absolute document coordinates (with scroll offset applied).
    pub fn map_mouse_to_cell(&self, render_info: &crate::RenderInfo, mx: f32, my: f32, viewport: &Viewport) -> Option<Position> {
        let scale_factor = crate::get_scale_factor();

        // Scale mouse coordinates
        let scaled_mx = mx * scale_factor;
        let scaled_my = my * scale_factor;

        // Use RenderInfo from shader (exact same values used for rendering)
        if render_info.font_width <= 0.0 || render_info.font_height <= 0.0 {
            return None;
        }

        // Convert screen coords to terminal pixel coords using RenderInfo
        let (term_x, mut term_y) = render_info.screen_to_terminal_pixels(scaled_mx, scaled_my)?;

        // Handle scanlines (doubled vertical resolution in render)
        let effective_font_height = if render_info.scan_lines {
            term_y /= 2.0;
            render_info.font_height
        } else {
            render_info.font_height
        };

        let cx = (term_x / render_info.font_width).floor() as i32;
        let visible_cy = (term_y / effective_font_height).floor() as i32;

        // scroll_y is in content coordinates - convert to lines
        let scroll_offset_lines = (viewport.scroll_y / render_info.font_height).floor() as i32;

        // Add scroll offset to get absolute document row
        let cy = visible_cy + scroll_offset_lines;

        Some(Position::new(cx, cy))
    }

    /// Map mouse coordinates to pixel position using shared RenderInfo from shader.
    pub fn map_mouse_to_xy(&self, render_info: &crate::RenderInfo, mx: f32, my: f32) -> Option<Position> {
        let scale_factor = crate::get_scale_factor();

        // Scale mouse coordinates
        let scaled_mx = mx * scale_factor;
        let scaled_my = my * scale_factor;

        // Convert screen coords to terminal pixel coords using RenderInfo
        let (term_x, term_y) = render_info.screen_to_terminal_pixels(scaled_mx, scaled_my)?;

        Some(Position::new(term_x as i32, term_y as i32))
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
            caret_blink: Blink::new(buffer_type.caret_blink_rate() as u128),
            character_blink: Blink::new(buffer_type.blink_rate() as u128),
            dragging: false,
            drag_anchor: None,
            last_drag_position: None,
            shift_pressed_during_selection: false,
            hovered_cell: None,
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
