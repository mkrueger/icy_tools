use icy_parser_core::BaudEmulation;

use crate::{Position, Size};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TerminalScrolling {
    #[default]
    Smooth,
    Fast,
    Disabled,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum OriginMode {
    #[default]
    UpperLeftCorner,
    WithinMargins,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum AutoWrapMode {
    #[default]
    AutoWrap,
    NoWrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FontSelectionState {
    #[default]
    NoRequest,
    Success,
    Failure,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MouseState {
    pub mouse_mode: MouseMode,
    pub focus_out_event_enabled: bool,

    /// Is set by icy_term based on connection settings
    /// Not part of the ANSI standard - let users enable/disable mouse tracking
    pub mouse_tracking_enabled: bool,
    pub alternate_scroll_enabled: bool,
    pub extended_mode: ExtMouseMode,
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            mouse_mode: MouseMode::default(),
            focus_out_event_enabled: false,
            mouse_tracking_enabled: true, // Default to enabled
            alternate_scroll_enabled: false,
            extended_mode: ExtMouseMode::default(),
        }
    }
}

impl MouseState {
    pub fn tracking_enabled(&self) -> bool {
        self.mouse_tracking_enabled && self.mouse_mode != MouseMode::OFF
    }
}

#[derive(Debug, Clone, Default)]
pub struct TerminalState {
    size: Size,
    pub is_terminal_buffer: bool,

    pub origin_mode: OriginMode,
    pub scroll_state: TerminalScrolling,
    pub auto_wrap_mode: AutoWrapMode,
    margins_top_bottom: Option<(i32, i32)>,
    margins_left_right: Option<(i32, i32)>,
    pub mouse_state: MouseState,

    pub font_selection_state: FontSelectionState,

    pub normal_attribute_font_slot: usize,
    pub high_intensity_attribute_font_slot: usize,
    pub blink_attribute_font_slot: usize,
    pub high_intensity_blink_attribute_font_slot: usize,
    pub cleared_screen: bool,
    pub cr_is_if: bool, // that basically skips /r
    tab_stops: Vec<i32>,
    baud_rate: BaudEmulation,

    /// DECLRMM - Left Right Margin Mode (DEC private mode 69)
    ///
    /// Defines whether or not the set left and right margins (DECSLRM) control
    /// function can set margins.
    ///
    /// - When set (`true`): DECSLRM can set the left and right margins. All line
    ///   attributes currently in page memory are set to single width, single height.
    ///   The terminal ignores any sequences to change line attributes to double
    ///   width or double height (DECDWL or DECDHL).
    ///
    /// - When reset (`false`, default): DECSLRM cannot set the left and right margins.
    ///   The margins are set to the page borders. The terminal can process sequences
    ///   to change line attributes to double width or double height.
    ///
    /// Available in: VT Level 4 mode only
    /// Format: CSI ? 69 h (set) / CSI ? 69 l (reset)
    dec_left_right_margins: bool,

    // Attributes used for determining the real current device attribute:
    pub inverse_video: bool,
    pub ice_colors: bool,

    // Special for Viewdata terminals - they reset colors on row change.
    pub(crate) vd_last_row: i32,

    /// UTF-8 parser for handling multi-byte sequences across print calls
    pub(crate) utf8_parser: utf8parse::Parser,
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
    ExtendedUTF8,
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
            baud_rate: BaudEmulation::Off,
            tab_stops: vec![],
            font_selection_state: FontSelectionState::NoRequest,
            normal_attribute_font_slot: 0,
            high_intensity_attribute_font_slot: 0,
            blink_attribute_font_slot: 0,
            high_intensity_blink_attribute_font_slot: 0,
            cleared_screen: false,
            cr_is_if: false,
            inverse_video: false,
            ice_colors: false,
            dec_left_right_margins: false,
            vd_last_row: 0,
            utf8_parser: utf8parse::Parser::new(),
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
        // need to preserve whether mouse tracking is enabled since it's a icy_term addition
        let enabled = self.mouse_state.mouse_tracking_enabled;
        self.mouse_state = MouseState::default();
        self.mouse_state.mouse_tracking_enabled = enabled;
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

    /// Returns whether DECLRMM (Left Right Margin Mode) is enabled.
    ///
    /// When `true`, the DECSLRM control function can set left and right margins.
    /// When `false` (default), DECSLRM cannot set margins and they default to page borders.
    ///
    /// See DECLRMM (DEC private mode 69) in VT420 specification.
    pub fn dec_left_right_margins(&self) -> bool {
        self.dec_left_right_margins
    }

    /// Sets the DECLRMM (Left Right Margin Mode) state.
    ///
    /// # Arguments
    /// * `enabled` - Whether to enable (`true`) or disable (`false`) left/right margin setting
    ///
    /// # Effects
    /// - When enabled: DECSLRM can set margins, line attributes become single width/height
    /// - When disabled: Clears any existing left/right margins, allows double width/height attributes
    ///
    /// See DECLRMM (DEC private mode 69) in VT420 specification.
    pub fn set_dec_left_right_margins(&mut self, enabled: bool) {
        self.dec_left_right_margins = enabled;
        if !enabled {
            self.margins_left_right = None;
        }
    }

    pub fn set_margins_left_right(&mut self, left: i32, right: i32) {
        if self.dec_left_right_margins {
            self.margins_left_right = if left > right { None } else { Some((left, right)) };
        }
    }

    pub fn clear_margins_top_bottom(&mut self) {
        self.margins_top_bottom = None;
    }

    pub fn clear_margins_left_right(&mut self) {
        self.margins_left_right = None;
    }

    /// Returns true if the given position is within the scroll region (top/bottom margins).
    /// This is used for scrolling operations which should respect margins regardless of origin mode.
    pub fn in_scroll_region(&self, pos: Position) -> bool {
        if let Some((top, bottom)) = self.margins_top_bottom {
            pos.y >= top && pos.y <= bottom
        } else {
            false
        }
    }

    /// Returns true if the given position is within the current text margins.
    /// Retruns false if origin mode is UpperLeftCorner or position is outside margins.
    pub fn in_margin(&self, pos: Position) -> bool {
        if self.origin_mode == OriginMode::UpperLeftCorner || self.margins_top_bottom.is_none() && self.margins_left_right.is_none() {
            return false;
        }

        if let Some((top, bottom)) = self.margins_top_bottom {
            if pos.y < top || pos.y > bottom {
                return false;
            }
        }

        if let Some((left, right)) = self.margins_left_right {
            if pos.x < left || pos.x > right {
                return false;
            }
        }
        true
    }

    pub fn set_text_window(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.origin_mode = OriginMode::WithinMargins;
        self.set_dec_left_right_margins(true);
        self.set_margins_top_bottom(y0, y1);
        self.set_margins_left_right(x0, x1);
    }

    pub fn reset_text_window(&mut self) {
        self.origin_mode = OriginMode::UpperLeftCorner;

        self.set_dec_left_right_margins(false);
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
        self.dec_left_right_margins = false;

        // Mouse state remains...

        // Font selection result back to "no request"
        self.font_selection_state = FontSelectionState::NoRequest;

        // Screen cleared flag (buffer will usually act on this)
        self.cleared_screen = true;
        self.inverse_video = false;

        // Recompute tab stops
        self.reset_tabs();
    }
}
