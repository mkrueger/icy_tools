use crate::{Size, ansi::BaudEmulation};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TerminalScrolling {
    Smooth,
    Fast,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OriginMode {
    UpperLeftCorner,
    WithinMargins,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AutoWrapMode {
    NoWrap,
    AutoWrap,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontSelectionState {
    NoRequest,
    Success,
    Failure,
}

#[derive(Debug, Clone, Default)]
pub struct MouseState {
    pub mouse_mode: MouseMode,
    pub focus_out_event_enabled: bool,
    pub mouse_tracking_enabled: bool,
    pub alternate_scroll_enabled: bool,
    pub extended_mode: ExtMouseMode,
}

impl MouseState {
    pub fn tracking_enabled(&self) -> bool {
        self.mouse_mode != MouseMode::OFF
    }
}

#[derive(Debug, Clone)]
pub struct TerminalState {
    size: Size,
    pub is_terminal_buffer: bool,

    pub origin_mode: OriginMode,
    pub scroll_state: TerminalScrolling,
    pub auto_wrap_mode: AutoWrapMode,
    margins_top_bottom: Option<(i32, i32)>,
    margins_left_right: Option<(i32, i32)>,
    pub mouse_state: MouseState,
    pub dec_margin_mode_left_right: bool,

    pub font_selection_state: FontSelectionState,

    pub normal_attribute_font_slot: usize,
    pub high_intensity_attribute_font_slot: usize,
    pub blink_attribute_font_slot: usize,
    pub high_intensity_blink_attribute_font_slot: usize,
    pub cleared_screen: bool,
    tab_stops: Vec<i32>,
    baud_rate: BaudEmulation,
    pub fixed_size: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum MouseMode {
    // no mouse reporting
    #[default]
    OFF,

    /// X10 compatibility mode (9)
    X10,
    /// VT200 mode (1000)
    VT200,
    /// VT200 highlight mode (1001)
    #[allow(non_camel_case_types)]
    VT200_Highlight,
    ButtonEvents,
    AnyEvents,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum ExtMouseMode {
    #[default]
    None,
    Extended,
    SGR,
    URXVT,
    PixelPosition,
}

impl TerminalState {
    pub fn from(size: impl Into<Size>) -> Self {
        let mut ret = Self {
            size: size.into(),
            is_terminal_buffer: true,
            scroll_state: TerminalScrolling::Smooth,
            origin_mode: OriginMode::UpperLeftCorner,
            auto_wrap_mode: AutoWrapMode::AutoWrap,
            mouse_state: MouseState::default(),
            margins_top_bottom: None,
            margins_left_right: None,
            dec_margin_mode_left_right: false,
            baud_rate: BaudEmulation::Off,
            tab_stops: vec![],
            font_selection_state: FontSelectionState::NoRequest,
            normal_attribute_font_slot: 0,
            high_intensity_attribute_font_slot: 0,
            blink_attribute_font_slot: 0,
            high_intensity_blink_attribute_font_slot: 0,
            cleared_screen: false,
            fixed_size: false,
        };
        ret.reset_tabs();
        ret
    }

    pub fn mouse_mode(&self) -> MouseMode {
        self.mouse_state.mouse_mode
    }

    pub fn set_mouse_mode(&mut self, mode: MouseMode) {
        self.mouse_state.mouse_mode = mode;
    }

    pub fn reset_mouse_mode(&mut self) {
        self.mouse_state = MouseState::default();
    }

    pub fn get_width(&self) -> i32 {
        self.size.width
    }

    pub fn set_width(&mut self, width: i32) {
        self.size.width = width;
        self.reset_tabs();
    }

    pub fn get_height(&self) -> i32 {
        self.size.height
    }

    pub fn set_height(&mut self, height: i32) {
        self.size.height = height;
    }

    pub fn get_size(&self) -> Size {
        self.size
    }

    pub fn set_size(&mut self, size: impl Into<Size>) {
        self.size = size.into();
    }

    pub fn tab_count(&self) -> usize {
        self.tab_stops.len()
    }

    pub fn get_tabs(&self) -> &[i32] {
        &self.tab_stops
    }

    pub fn clear_tab_stops(&mut self) {
        self.tab_stops.clear();
    }

    pub fn remove_tab_stop(&mut self, x: i32) {
        self.tab_stops.retain(|&t| t != x);
    }

    fn reset_tabs(&mut self) {
        let mut i = 0;
        self.tab_stops.clear();
        while i < self.get_width() {
            self.tab_stops.push(i);
            i += 8;
        }
    }

    pub fn next_tab_stop(&self, x: i32) -> i32 {
        let mut i = 0;
        while i < self.tab_stops.len() && self.tab_stops[i] <= x {
            i += 1;
        }
        if i < self.tab_stops.len() { self.tab_stops[i] } else { self.get_width() }
    }

    pub fn prev_tab_stop(&self, x: i32) -> i32 {
        let mut i = self.tab_stops.len() as i32 - 1;
        while i >= 0 && self.tab_stops[i as usize] >= x {
            i -= 1;
        }
        if i >= 0 { self.tab_stops[i as usize] } else { 0 }
    }

    pub fn set_tab_at(&mut self, x: i32) {
        if !self.tab_stops.contains(&x) {
            self.tab_stops.push(x);
            self.tab_stops.sort_unstable();
        }
    }

    pub fn get_baud_emulation(&self) -> BaudEmulation {
        self.baud_rate
    }

    pub fn set_baud_rate(&mut self, baud_rate: BaudEmulation) {
        self.baud_rate = baud_rate;
    }

    pub fn get_margins_top_bottom(&self) -> Option<(i32, i32)> {
        self.margins_top_bottom
    }

    pub fn get_margins_left_right(&self) -> Option<(i32, i32)> {
        self.margins_left_right
    }

    pub fn needs_scrolling(&self) -> bool {
        self.is_terminal_buffer && self.get_margins_top_bottom().is_some()
    }

    pub fn set_margins_top_bottom(&mut self, top: i32, bottom: i32) {
        self.margins_top_bottom = if top > bottom { None } else { Some((top, bottom)) };
    }

    pub fn set_margins_left_right(&mut self, left: i32, right: i32) {
        self.margins_left_right = if left > right { None } else { Some((left, right)) };
    }

    pub fn clear_margins_top_bottom(&mut self) {
        self.margins_top_bottom = None;
    }

    pub fn clear_margins_left_right(&mut self) {
        self.margins_left_right = None;
    }

    pub fn set_text_window(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.origin_mode = OriginMode::WithinMargins;
        self.set_margins_top_bottom(y0, y1);
        self.set_margins_left_right(x0, x1);
    }

    pub fn clear_text_window(&mut self) {
        self.origin_mode = OriginMode::UpperLeftCorner;

        self.clear_margins_top_bottom();
        self.clear_margins_left_right();
    }

    pub fn reset_terminal(&mut self, size: Size) {
        let size = size.into();

        // Update size first (tab stops depend on width)
        self.size = size;

        // Core modes
        self.origin_mode = OriginMode::UpperLeftCorner;
        self.scroll_state = TerminalScrolling::Smooth;
        self.auto_wrap_mode = AutoWrapMode::AutoWrap;

        // Margins & text window
        self.margins_top_bottom = None;
        self.margins_left_right = None;
        self.dec_margin_mode_left_right = false;

        // Mouse state remains...

        // Font selection result back to "no request"
        self.font_selection_state = FontSelectionState::NoRequest;

        // Screen cleared flag (buffer will usually act on this)
        self.cleared_screen = true;

        // Recompute tab stops
        self.reset_tabs();
    }
}
