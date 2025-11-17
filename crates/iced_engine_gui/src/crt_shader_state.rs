use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::sync::{Mutex, atomic::AtomicU64};

use crate::{Blink, CRTShaderProgram, Message, MonitorSettings, Terminal, UnicodeGlyphCache};
use iced::Element;
use iced::widget::shader;
use icy_engine::MouseField;
use icy_engine::Position;

pub static TERMINAL_SHADER_INSTANCE_COUNTER: AtomicU64 = AtomicU64::new(1);
pub static PENDING_INSTANCE_REMOVALS: Mutex<Vec<u64>> = Mutex::new(Vec::new());

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

    pub last_rendered_size: Option<(u32, u32)>,
    pub instance_id: u64,

    pub unicode_glyph_cache: Arc<parking_lot::Mutex<Option<UnicodeGlyphCache>>>,

    pub cached_rgba_blink_on: parking_lot::Mutex<Vec<u8>>,
    pub cached_rgba_blink_off: parking_lot::Mutex<Vec<u8>>,
    pub cached_size: parking_lot::Mutex<(u32, u32)>,
    pub cached_font_wh: parking_lot::Mutex<(usize, usize)>,
    pub content_dirty: parking_lot::Mutex<bool>,
    pub last_selection_state: parking_lot::Mutex<(Option<Position>, Option<Position>, bool)>, // (anchor, lead, locked)
    pub last_buffer_version: parking_lot::Mutex<u64>,                                         // Track buffer version for cache invalidation
}

impl CRTShaderState {
    pub fn reset_caret(&mut self) {
        self.caret_blink.reset();
    }
}

impl Drop for CRTShaderState {
    fn drop(&mut self) {
        if let Ok(mut v) = PENDING_INSTANCE_REMOVALS.lock() {
            v.push(self.instance_id);
        }
    }
}

impl Default for CRTShaderState {
    fn default() -> Self {
        Self {
            caret_blink: Blink::new((1000.0 / 1.875) as u128 / 2),
            character_blink: Blink::new((1000.0 / 1.8) as u128),
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
            last_rendered_size: None,
            instance_id: TERMINAL_SHADER_INSTANCE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            unicode_glyph_cache: Arc::new(parking_lot::Mutex::new(None)),

            cached_rgba_blink_on: parking_lot::Mutex::new(Vec::new()),
            cached_rgba_blink_off: parking_lot::Mutex::new(Vec::new()),
            cached_size: parking_lot::Mutex::new((0, 0)),
            cached_font_wh: parking_lot::Mutex::new((0, 0)),
            content_dirty: parking_lot::Mutex::new(true),
            last_selection_state: parking_lot::Mutex::new((None, None, false)),
            last_buffer_version: parking_lot::Mutex::new(u64::MAX),
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
