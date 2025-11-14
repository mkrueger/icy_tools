// Useful description: https://vt100.net/docs/vt510-rm/chapter4.html
//                     https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Normal-tracking-mode
use std::{
    cmp::{max, min},
    collections::HashMap,
    fmt::Display,
};

use serde::{Deserialize, Serialize};

use self::sound::{AnsiMusic, MusicState};

use super::{BufferParser, TAB};
use crate::{
    AttributedChar, AutoWrapMode, BEL, BS, CR, CallbackAction, Caret, EditableScreen, EngineResult, ExtMouseMode, FF, FontSelectionState, HyperLink, IceMode,
    LF, MouseMode, OriginMode, ParserError, Position, TerminalScrolling,
};

mod ansi_commands;
pub mod constants;
mod dcs;
pub mod mouse_event;
mod osc;
pub mod sound;
#[derive(Debug, Clone, PartialEq)]
pub enum EngineState {
    Default,
    ReadEscapeSequence,

    ReadCSISequence(bool),
    ReadCSICommand,        // CSI ?
    ReadCSIRequest,        // CSI =
    ReadRIPSupportRequest, // CSI !
    ReadDeviceAttrs,       // CSI <
    EndCSI(char),

    RecordDCS,
    RecordDCSEscape,
    ReadPossibleMacroInDCS(u8),

    ParseAnsiMusic(sound::MusicState),

    ReadAPS,
    ReadAPSEscape,

    ReadOSCSequence,
    ReadOSCSequenceEscape,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MusicOption {
    #[default]
    Off,
    Conflicting,
    Banana,
    Both,
}

impl Display for MusicOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<String> for MusicOption {
    fn from(s: String) -> Self {
        match s.as_str() {
            "Conflicting" => MusicOption::Conflicting,
            "Banana" => MusicOption::Banana,
            "Both" => MusicOption::Both,
            _ => MusicOption::Off,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaudEmulation {
    #[default]
    Off,
    Rate(u32),
}

impl Display for BaudEmulation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Off => write!(f, "Off"),
            Self::Rate(v) => write!(f, "{v}"),
        }
    }
}

impl BaudEmulation {
    pub const OPTIONS: [BaudEmulation; 12] = [
        BaudEmulation::Off,
        BaudEmulation::Rate(300),
        BaudEmulation::Rate(600),
        BaudEmulation::Rate(1200),
        BaudEmulation::Rate(2400),
        BaudEmulation::Rate(4800),
        BaudEmulation::Rate(9600),
        BaudEmulation::Rate(19200),
        BaudEmulation::Rate(38400),
        BaudEmulation::Rate(57600),
        BaudEmulation::Rate(76800),
        BaudEmulation::Rate(115_200),
    ];

    pub fn get_baud_rate(&self) -> u32 {
        match self {
            BaudEmulation::Off => 0,
            BaudEmulation::Rate(baud) => *baud,
        }
    }
}

pub struct Parser {
    pub(crate) state: EngineState,
    saved_pos: Position,
    saved_cursor_state: Option<SavedCursorState>,
    pub(crate) parsed_numbers: Vec<i32>,

    pub hyper_links: Vec<HyperLink>,

    /*     current_sixel_color: i32,
        sixel_cursor: Position,
        current_sixel_palette: Palette,
    */
    pub ansi_music: MusicOption,
    cur_music: Option<AnsiMusic>,
    cur_octave: usize,
    cur_length: i32,
    cur_tempo: i32,
    dotted_note: bool,

    last_char: char,
    pub(crate) macros: HashMap<usize, String>,
    pub parse_string: String,
    pub macro_dcs: String,
    pub bs_is_ctrl_char: bool,
    pub got_skypix_sequence: bool,
}

#[derive(Clone)]
struct SavedCursorState {
    caret: Caret,
    origin_mode: OriginMode,
    auto_wrap_mode: AutoWrapMode,
}

impl Default for Parser {
    fn default() -> Self {
        Parser {
            state: EngineState::Default,
            saved_pos: Position::default(),
            parsed_numbers: Vec::with_capacity(8),
            saved_cursor_state: None,
            ansi_music: MusicOption::Off,
            cur_music: None,
            cur_octave: 3,
            cur_length: 4,
            cur_tempo: 120,
            dotted_note: false,
            parse_string: String::with_capacity(64),
            macro_dcs: String::with_capacity(256),
            macros: HashMap::new(),
            last_char: '\0',
            hyper_links: Vec::new(),
            bs_is_ctrl_char: false,
            got_skypix_sequence: false,
        }
    }
}

impl BufferParser for Parser {
    #[allow(clippy::single_match)]
    fn print_char(&mut self, buf: &mut dyn EditableScreen, ch: char) -> EngineResult<CallbackAction> {
        match &self.state {
            EngineState::ParseAnsiMusic(_) => {
                return self.parse_ansi_music(ch);
            }
            EngineState::ReadEscapeSequence => {
                return {
                    self.state = EngineState::Default;

                    match ch {
                        '[' => {
                            self.state = EngineState::ReadCSISequence(true);
                            self.parsed_numbers.clear();
                            Ok(CallbackAction::None)
                        }
                        ']' => {
                            self.state = EngineState::ReadOSCSequence;
                            self.parsed_numbers.clear();
                            self.parse_string.clear();
                            Ok(CallbackAction::None)
                        }
                        '7' => {
                            // DECSC - Save Cursor
                            self.saved_cursor_state = Some(SavedCursorState {
                                caret: buf.caret().clone(),
                                origin_mode: buf.terminal_state().origin_mode,
                                auto_wrap_mode: buf.terminal_state().auto_wrap_mode,
                            });
                            Ok(CallbackAction::None)
                        }
                        '8' => {
                            // DECRC - Restore Cursor
                            if let Some(saved_state) = &self.saved_cursor_state {
                                *buf.caret_mut() = saved_state.caret.clone();
                                buf.terminal_state_mut().origin_mode = saved_state.origin_mode;
                                buf.terminal_state_mut().auto_wrap_mode = saved_state.auto_wrap_mode;
                            } else {
                                // If no saved state, reset to defaults per VT100 spec
                                buf.caret_mut().reset();
                                buf.terminal_state_mut().origin_mode = OriginMode::UpperLeftCorner;
                                buf.terminal_state_mut().auto_wrap_mode = AutoWrapMode::AutoWrap;
                            }
                            Ok(CallbackAction::Update)
                        }

                        'c' => {
                            // RIS—Reset to Initial State see https://vt100.net/docs/vt510-rm/RIS.html
                            buf.clear_screen();
                            buf.caret_mut().reset();
                            buf.reset_terminal();
                            self.macros.clear();
                            self.saved_cursor_state = None;
                            self.saved_pos = Position::default();
                            Ok(CallbackAction::Update)
                        }

                        'D' => {
                            // Index
                            buf.index();
                            Ok(CallbackAction::Update)
                        }
                        'M' => {
                            // Reverse Index
                            buf.reverse_index();
                            Ok(CallbackAction::Update)
                        }

                        'E' => {
                            // Next Line
                            buf.next_line();
                            Ok(CallbackAction::Update)
                        }

                        'P' => {
                            // DCS
                            self.state = EngineState::RecordDCS;
                            self.parse_string.clear();
                            self.parsed_numbers.clear();
                            Ok(CallbackAction::None)
                        }
                        'H' => {
                            // set tab at current column
                            self.state = EngineState::Default;
                            let x = buf.caret().x;
                            buf.terminal_state_mut().set_tab_at(x);
                            Ok(CallbackAction::None)
                        }

                        '_' => {
                            // Application Program String
                            self.state = EngineState::ReadAPS;
                            self.parse_string.clear();
                            Ok(CallbackAction::None)
                        }

                        '0'..='~' => {
                            // Silently drop unsupported sequences
                            self.state = EngineState::Default;
                            Ok(CallbackAction::None)
                        }
                        FF | BEL | BS | '\x09' | '\x7F' | '\x1B' | '\n' | '\r' => {
                            // non standard extension to print esc chars ESC ESC -> ESC
                            self.last_char = ch;
                            let ch = AttributedChar::new(self.last_char, buf.caret().attribute);
                            buf.print_char(ch);
                            Ok(CallbackAction::Update)
                        }
                        _ => {
                            self.state = EngineState::Default;
                            self.unsupported_escape_error()
                        }
                    }
                };
            }
            EngineState::ReadAPS => {
                if ch == '\x1B' {
                    self.state = EngineState::ReadAPSEscape;
                    return Ok(CallbackAction::None);
                }
                self.parse_string.push(ch);
            }
            EngineState::ReadAPSEscape => {
                if ch == '\\' {
                    self.state = EngineState::Default;
                    self.execute_aps_command(buf);
                    return Ok(CallbackAction::None);
                }
                self.state = EngineState::ReadAPS;
                self.parse_string.push('\x1B');
                self.parse_string.push(ch);
            }

            EngineState::ReadPossibleMacroInDCS(i) => {
                // \x1B[<num>*z
                // read macro inside dcs sequence, 3 states:´
                // 0: [
                // 1: <num>
                // 2: *
                // z
                self.macro_dcs.push(ch);
                if ch.is_ascii_digit() {
                    if *i != 1 {
                        self.state = EngineState::Default;
                        return Err(ParserError::UnsupportedDCSSequence(format!("Error in macro inside dcs, expected number got '{ch}'")).into());
                    }
                    let d = match self.parsed_numbers.pop() {
                        Some(number) => number,
                        _ => 0,
                    };
                    self.parsed_numbers.push(parse_next_number(d, ch as u8));
                    return Ok(CallbackAction::None);
                }
                if ch == '[' {
                    if *i != 0 {
                        self.state = EngineState::Default;
                        return Err(ParserError::UnsupportedDCSSequence(format!("Error in macro inside dcs, expected '[' got '{ch}'")).into());
                    }
                    self.state = EngineState::ReadPossibleMacroInDCS(1);
                    return Ok(CallbackAction::None);
                }
                if ch == '*' {
                    if *i != 1 {
                        self.state = EngineState::Default;
                        return Err(ParserError::UnsupportedDCSSequence(format!("Error in macro inside dcs, expected '*' got '{ch}'")).into());
                    }
                    self.state = EngineState::ReadPossibleMacroInDCS(2);
                    return Ok(CallbackAction::None);
                }
                if ch == 'z' {
                    if *i != 2 {
                        self.state = EngineState::Default;
                        return Err(ParserError::UnsupportedDCSSequence(format!("Error in macro inside dcs, expected 'z' got '{ch}'")).into());
                    }
                    if self.parsed_numbers.len() != 1 {
                        self.state = EngineState::Default;
                        return Err(ParserError::UnsupportedDCSSequence(format!("Macro hasn't one number defined got '{}'", self.parsed_numbers.len())).into());
                    }
                    self.state = EngineState::RecordDCS;
                    self.invoke_macro_by_id(buf, *self.parsed_numbers.first().unwrap());
                    return Ok(CallbackAction::None);
                }
                self.parse_string.push('\x1b');
                self.parse_string.push('[');
                self.parse_string.push_str(&self.macro_dcs);
                self.state = EngineState::RecordDCS;
                return Ok(CallbackAction::None);
            }
            EngineState::RecordDCS => {
                match ch {
                    '\x1B' => {
                        self.state = EngineState::RecordDCSEscape;
                    }
                    _ => {
                        self.parse_string.push(ch);
                    }
                }
                return Ok(CallbackAction::None);
            }
            EngineState::RecordDCSEscape => {
                if ch == '\\' {
                    self.state = EngineState::Default;
                    return self.execute_dcs(buf);
                }
                if ch == '[' {
                    self.state = EngineState::ReadPossibleMacroInDCS(1);
                    self.macro_dcs.clear();
                    return Ok(CallbackAction::None);
                }
                self.parse_string.push('\x1b');
                self.parse_string.push(ch);
                self.state = EngineState::RecordDCS;
                return Ok(CallbackAction::None);
            }

            EngineState::ReadOSCSequence => {
                if ch == '\x1B' {
                    self.state = EngineState::ReadOSCSequenceEscape;
                    return Ok(CallbackAction::None);
                }
                if ch == '\x07' {
                    self.state = EngineState::Default;
                    return self.parse_osc(buf);
                }
                self.parse_string.push(ch);
                return Ok(CallbackAction::None);
            }
            EngineState::ReadOSCSequenceEscape => {
                if ch == '\\' {
                    self.state = EngineState::Default;
                    return self.parse_osc(buf);
                }
                self.state = EngineState::ReadOSCSequence;
                self.parse_string.push('\x1B');
                self.parse_string.push(ch);
                return Ok(CallbackAction::None);
            }

            EngineState::ReadCSICommand => {
                match ch {
                    'l' => {
                        self.state = EngineState::Default;
                        if self.parsed_numbers.len() != 1 {
                            return self.unsupported_escape_error();
                        }
                        match self.parsed_numbers.first() {
                            Some(4) => buf.terminal_state_mut().scroll_state = TerminalScrolling::Fast,
                            Some(6) => {
                                //  buf.terminal_state().origin_mode = OriginMode::WithinMargins;
                            }
                            Some(7) => buf.terminal_state_mut().auto_wrap_mode = AutoWrapMode::NoWrap,
                            Some(25) => buf.caret_mut().visible = false,
                            Some(33) => {
                                // only turn off ice mode for terminals. While loading it stays on.
                                if buf.terminal_state().is_terminal_buffer {
                                    *buf.ice_mode_mut() = IceMode::Blink
                                }
                            }
                            Some(35) => buf.caret_mut().blinking = true,

                            Some(69) => {
                                buf.terminal_state_mut().dec_margin_mode_left_right = false;
                                buf.terminal_state_mut().clear_margins_left_right();
                            }

                            // Mouse tracking modes - turn off
                            Some(9 | 1000 | 1001 | 1002 | 1003) => {
                                buf.terminal_state_mut().reset_mouse_mode();
                            }
                            Some(1004) => {
                                // Turn off focus event reporting
                                buf.terminal_state_mut().mouse_state.focus_out_event_enabled = false;
                            }
                            Some(1007) => {
                                // Turn off alternate scroll mode
                                buf.terminal_state_mut().mouse_state.alternate_scroll_enabled = false;
                            }
                            Some(1005) => {
                                // Turn off UTF-8 extended mouse mode
                                if matches!(buf.terminal_state_mut().mouse_state.extended_mode, ExtMouseMode::Extended) {
                                    buf.terminal_state_mut().mouse_state.extended_mode = ExtMouseMode::None;
                                }
                            }
                            Some(1006) => {
                                // Turn off SGR extended mouse mode
                                if matches!(buf.terminal_state_mut().mouse_state.extended_mode, ExtMouseMode::SGR) {
                                    buf.terminal_state_mut().mouse_state.extended_mode = ExtMouseMode::None;
                                }
                            }
                            Some(1015) => {
                                // Turn off URXVT extended mouse mode
                                if matches!(buf.terminal_state_mut().mouse_state.extended_mode, ExtMouseMode::URXVT) {
                                    buf.terminal_state_mut().mouse_state.extended_mode = ExtMouseMode::None;
                                }
                            }
                            Some(1016) => {
                                // Turn off pixel position mode
                                if matches!(buf.terminal_state_mut().mouse_state.extended_mode, ExtMouseMode::PixelPosition) {
                                    buf.terminal_state_mut().mouse_state.extended_mode = ExtMouseMode::None;
                                }
                            }
                            _ => {
                                return self.unsupported_escape_error();
                            }
                        }
                    }
                    'h' => {
                        self.state = EngineState::Default;
                        if self.parsed_numbers.len() != 1 {
                            return self.unsupported_escape_error();
                        }
                        match self.parsed_numbers.first() {
                            Some(4) => buf.terminal_state_mut().scroll_state = TerminalScrolling::Smooth,
                            Some(6) => buf.terminal_state_mut().origin_mode = OriginMode::UpperLeftCorner,
                            Some(7) => buf.terminal_state_mut().auto_wrap_mode = AutoWrapMode::AutoWrap,
                            Some(25) => buf.caret_mut().visible = true,
                            Some(33) => *buf.ice_mode_mut() = IceMode::Ice,
                            Some(35) => buf.caret_mut().blinking = false,

                            Some(69) => buf.terminal_state_mut().dec_margin_mode_left_right = true,

                            // Mouse tracking see https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Normal-tracking-mode
                            Some(9) => buf.terminal_state_mut().set_mouse_mode(MouseMode::X10),
                            Some(1000) => buf.terminal_state_mut().set_mouse_mode(MouseMode::VT200),
                            Some(1001) => {
                                buf.terminal_state_mut().set_mouse_mode(MouseMode::VT200_Highlight);
                            }
                            Some(1002) => buf.terminal_state_mut().set_mouse_mode(MouseMode::ButtonEvents),
                            Some(1003) => buf.terminal_state_mut().set_mouse_mode(MouseMode::AnyEvents),

                            Some(1004) => buf.terminal_state_mut().mouse_state.focus_out_event_enabled = true,
                            Some(1007) => {
                                buf.terminal_state_mut().mouse_state.alternate_scroll_enabled = true;
                            }
                            Some(1005) => buf.terminal_state_mut().mouse_state.extended_mode = ExtMouseMode::Extended,
                            Some(1006) => {
                                buf.terminal_state_mut().mouse_state.extended_mode = ExtMouseMode::SGR;
                            }
                            Some(1015) => {
                                buf.terminal_state_mut().mouse_state.extended_mode = ExtMouseMode::URXVT;
                            }
                            Some(1016) => {
                                buf.terminal_state_mut().mouse_state.extended_mode = ExtMouseMode::PixelPosition;
                            }

                            Some(cmd) => {
                                return Err(ParserError::UnsupportedCustomCommand(*cmd).into());
                            }
                            None => return self.unsupported_escape_error(),
                        }
                    }
                    '0'..='9' => {
                        let d = match self.parsed_numbers.pop() {
                            Some(number) => number,
                            _ => 0,
                        };
                        self.parsed_numbers.push(parse_next_number(d, ch as u8));
                    }
                    ';' => {
                        self.parsed_numbers.push(0);
                    }
                    'n' => {
                        self.state = EngineState::Default;
                        match self.parsed_numbers.first() {
                            Some(62) => {
                                // DSR—Macro Space Report
                                return Ok(CallbackAction::SendString("\x1B[32767*{".to_string()));
                            }
                            Some(63) => {
                                // Memory Checksum Report (DECCKSR)
                                if self.parsed_numbers.len() != 2 {
                                    return Err(ParserError::UnsupportedEscapeSequence.into());
                                }
                                let mut sum: u32 = 0;
                                for i in 0..64 {
                                    if let Some(m) = self.macros.get(&i) {
                                        for b in m.as_bytes() {
                                            sum = sum.wrapping_add(*b as u32);
                                        }
                                    }
                                }
                                let checksum: u16 = (sum & 0xFFFF) as u16;
                                return Ok(CallbackAction::SendString(format!("\x1BP{}!~{checksum:04X}\x1B\\", self.parsed_numbers[1])));
                            }
                            _ => {
                                return self.unsupported_escape_error();
                            }
                        }
                    }
                    _ => {
                        self.state = EngineState::Default;
                        // error in control sequence, terminate reading
                        return self.unsupported_escape_error();
                    }
                }
            }

            EngineState::ReadCSIRequest => {
                match ch {
                    'n' => {
                        self.state = EngineState::Default;
                        if self.parsed_numbers.len() != 1 {
                            return self.unsupported_escape_error();
                        }
                        match self.parsed_numbers.first() {
                            Some(1) => {
                                // font state report
                                let font_selection_result = match buf.terminal_state().font_selection_state {
                                    FontSelectionState::NoRequest => 99,
                                    FontSelectionState::Success => 0,
                                    FontSelectionState::Failure => 1,
                                };

                                return Ok(CallbackAction::SendString(format!(
                                    "\x1B[=1;{font_selection_result};{};{};{};{}n",
                                    buf.terminal_state().normal_attribute_font_slot,
                                    buf.terminal_state().high_intensity_attribute_font_slot,
                                    buf.terminal_state().blink_attribute_font_slot,
                                    buf.terminal_state().high_intensity_blink_attribute_font_slot
                                )));
                            }
                            Some(2) => {
                                // font mode report
                                let mut params = Vec::new();
                                if buf.terminal_state().origin_mode == OriginMode::WithinMargins {
                                    params.push("6");
                                }
                                if buf.terminal_state().auto_wrap_mode == AutoWrapMode::AutoWrap {
                                    params.push("7");
                                }
                                if buf.caret().visible {
                                    params.push("25");
                                }
                                if buf.ice_mode() == IceMode::Ice {
                                    params.push("33");
                                }
                                if buf.caret().blinking {
                                    params.push("35");
                                }

                                match buf.terminal_state().mouse_mode() {
                                    MouseMode::OFF => {}
                                    MouseMode::X10 => params.push("9"),
                                    MouseMode::VT200 => params.push("1000"),
                                    MouseMode::VT200_Highlight => params.push("1001"),
                                    MouseMode::ButtonEvents => params.push("1002"),
                                    MouseMode::AnyEvents => params.push("1003"),
                                }

                                if buf.terminal_state().mouse_state.focus_out_event_enabled {
                                    params.push("1004");
                                }

                                if buf.terminal_state().mouse_state.alternate_scroll_enabled {
                                    params.push("1007");
                                }

                                // Report the extended encoding mode
                                match buf.terminal_state().mouse_state.extended_mode {
                                    ExtMouseMode::None => {}
                                    ExtMouseMode::Extended => params.push("1005"),
                                    ExtMouseMode::SGR => params.push("1006"),
                                    ExtMouseMode::URXVT => params.push("1015"),
                                    ExtMouseMode::PixelPosition => params.push("1016"),
                                }
                                let mode_report = if params.is_empty() {
                                    "\x1B[=2;n".to_string()
                                } else {
                                    format!("\x1B[=2;{}n", params.join(";"))
                                };

                                return Ok(CallbackAction::SendString(mode_report));
                            }
                            Some(3) => {
                                // font dimension request
                                let dim = buf.get_font_dimensions();
                                return Ok(CallbackAction::SendString(format!("\x1B[=3;{};{}n", dim.height, dim.width)));
                            }
                            _ => {
                                return self.unsupported_escape_error();
                            }
                        }
                    }
                    '0'..='9' => {
                        let d = match self.parsed_numbers.pop() {
                            Some(number) => number,
                            _ => 0,
                        };
                        self.parsed_numbers.push(parse_next_number(d, ch as u8));
                    }
                    ';' => {
                        self.parsed_numbers.push(0);
                    }
                    'r' => return self.reset_margins(buf),
                    'm' => {
                        self.state = EngineState::Default;
                        if self.parsed_numbers.len() != 2 {
                            return self.unsupported_escape_error();
                        }
                        return self.set_specific_margin(buf);
                    }
                    _ => {
                        self.state = EngineState::Default;
                        // error in control sequence, terminate reading
                        return Err(ParserError::UnsupportedEscapeSequence.into());
                    }
                }
            }

            EngineState::ReadRIPSupportRequest => {
                if let 'p' = ch {
                    self.soft_terminal_reset(buf);
                } else {
                    // potential rip support request
                    // ignore that for now and continue parsing
                    self.state = EngineState::Default;
                    return self.print_char(buf, ch);
                }
            }

            EngineState::ReadDeviceAttrs => {
                match ch {
                    '0'..='9' => {
                        let d = match self.parsed_numbers.pop() {
                            Some(number) => number,
                            _ => 0,
                        };
                        self.parsed_numbers.push(parse_next_number(d, ch as u8));
                    }
                    ';' => {
                        self.parsed_numbers.push(0);
                    }
                    'c' => {
                        self.state = EngineState::Default;

                        if self.parsed_numbers.len() > 1 {
                            return Err(ParserError::UnsupportedEscapeSequence.into());
                        }
                        /*
                           1 - Loadable fonts are availabe via Device Control Strings
                           2 - Bright Background (ie: DECSET 32) is supported
                           3 - Palette entries may be modified via an Operating System Command
                               string
                           4 - Pixel operations are supported (currently, sixel and PPM
                               graphics)
                           5 - The current font may be selected via CSI Ps1 ; Ps2 sp D
                           6 - Extended palette is available
                           7 - Mouse is available
                        */
                        return Ok(CallbackAction::SendString("\x1B[<1;2;3;4;5;6;7c".to_string()));
                    }
                    _ => {
                        self.state = EngineState::Default;
                        // error in control sequence, terminate reading
                        return self.unsupported_escape_error();
                    }
                }
            }

            EngineState::EndCSI(func) => match *func {
                '>' => {
                    // CSI > sequences
                    self.state = EngineState::Default;
                    match ch {
                        'c' => {
                            // CSI > c - Secondary Device Attributes
                            return self.secondary_device_attributes();
                        }
                        _ => {
                            return self.unsupported_escape_error();
                        }
                    }
                }

                '*' => match ch {
                    'z' => return self.invoke_macro(buf),
                    'r' => return self.select_communication_speed(buf),
                    'y' => return self.request_checksum_of_rectangular_area(buf),
                    _ => {}
                },

                '$' => match ch {
                    'w' => {
                        self.state = EngineState::Default;
                        if let Some(2) = self.parsed_numbers.first() {
                            let mut str = "\x1BP2$u".to_string();
                            (0..buf.terminal_state().tab_count()).for_each(|i| {
                                let tab = buf.terminal_state().get_tabs()[i];
                                str.push_str(&(tab + 1).to_string());
                                if i < buf.terminal_state().tab_count().saturating_sub(1) {
                                    str.push('/');
                                }
                            });
                            str.push_str("\x1B\\");
                            return Ok(CallbackAction::SendString(str));
                        }
                    }
                    'x' => return self.fill_rectangular_area(buf),
                    'z' => return self.erase_rectangular_area(buf),
                    '{' => return self.selective_erase_rectangular_area(buf),

                    _ => {}
                },

                ' ' => {
                    self.state = EngineState::Default;

                    match ch {
                        'D' => return self.font_selection(buf),
                        'A' => self.scroll_right(buf),
                        '@' => self.scroll_left(buf),
                        'd' => return self.tabulation_stop_remove(buf),
                        'q' => {
                            // DECSCUSR—Set Cursor Style
                            // CSI Ps SP q
                            // Ps = 0 -> blinking block (default)
                            // Ps = 1 -> blinking block
                            // Ps = 2 -> steady block
                            // Ps = 3 -> blinking underline
                            // Ps = 4 -> steady underline
                            // Ps = 5 -> blinking bar
                            // Ps = 6 -> steady bar

                            let style = if self.parsed_numbers.is_empty() {
                                0 // default to blinking block
                            } else {
                                self.parsed_numbers[0]
                            };

                            match style {
                                0 | 1 => {
                                    // Blinking block (default)
                                    buf.caret_mut().blinking = true;
                                    buf.caret_mut().shape = crate::CaretShape::Block;
                                }
                                2 => {
                                    // Steady block
                                    buf.caret_mut().blinking = false;
                                    buf.caret_mut().shape = crate::CaretShape::Block;
                                }
                                3 => {
                                    // Blinking underline
                                    buf.caret_mut().blinking = true;
                                    buf.caret_mut().shape = crate::CaretShape::Underline;
                                }
                                4 => {
                                    // Steady underline
                                    buf.caret_mut().blinking = false;
                                    buf.caret_mut().shape = crate::CaretShape::Underline;
                                }
                                5 => {
                                    // Blinking bar
                                    buf.caret_mut().blinking = true;
                                    buf.caret_mut().shape = crate::CaretShape::Bar;
                                }
                                6 => {
                                    // Steady bar
                                    buf.caret_mut().blinking = false;
                                    buf.caret_mut().shape = crate::CaretShape::Bar;
                                }
                                _ => {
                                    // Invalid cursor style, ignore
                                    return Ok(CallbackAction::None);
                                }
                            }
                            return Ok(CallbackAction::Update);
                        }
                        _ => {
                            return self.unsupported_escape_error();
                        }
                    }
                }
                _ => {
                    self.state = EngineState::Default;
                    return self.unsupported_escape_error();
                }
            },
            EngineState::ReadCSISequence(is_start) => {
                match ch {
                    'm' => return self.select_graphic_rendition(buf),
                    'H' |    // Cursor Position
                    'f' // CSI Pn1 ; Pn2 f 
                        // HVP - Character and line position
                    => {
                        self.state = EngineState::Default;
                        if self.parsed_numbers.is_empty() {
                            let pos = buf.upper_left_position();
                            buf.caret_mut().set_position(pos);
                        } else {
                            let row = self.parsed_numbers.first().copied().unwrap_or(1).saturating_sub(1);
                            let l = buf.get_first_visible_line();
                            buf.caret_mut().y = l + row;
                            if self.parsed_numbers.len() > 1 {
                                if self.parsed_numbers[1] >= 0 {
                                    buf.caret_mut().x = max(0, self.parsed_numbers[1] - 1);
                                }
                            } else {
                                buf.caret_mut().x = 0;
                            }
                        }
                        buf.limit_caret_pos();
                        return Ok(CallbackAction::Update);
                    }
                    'C' => {
                        // Cursor Forward
                        self.state = EngineState::Default;
                        let amount = self.parsed_numbers.first().copied().unwrap_or(1);
                        buf.right(amount);
                        return Ok(CallbackAction::Update);
                    }
                    'j' | // CSI Pn j
                          // HPB - Character position backward
                    'D' => {
                        // Cursor Back
                        self.state = EngineState::Default;
                        let amount = self.parsed_numbers.first().copied().unwrap_or(1);
                        buf.left(amount);
                        return Ok(CallbackAction::Update);
                    }
                    'k' | // CSI Pn k
                          // VPB - Line position backward
                    'A' => {
                        // Cursor Up
                        self.state = EngineState::Default;
                        let amount = self.parsed_numbers.first().copied().unwrap_or(1);
                        buf.up(amount);
                        return Ok(CallbackAction::Update);
                    }
                    'B' => {
                        // Cursor Down
                        self.state = EngineState::Default;
                        let amount = self.parsed_numbers.first().copied().unwrap_or(1);
                        buf.down(amount);
                        return Ok(CallbackAction::Update);
                    }
                    's' => {
                        if buf.terminal_state().dec_margin_mode_left_right {
                            return self.set_left_and_right_margins(buf);
                        }
                        self.save_cursor_position(buf.caret());
                        return Ok(CallbackAction::None);
                    }
                    'u' => self.restore_cursor_position(buf.caret_mut()),
                    'd' => {
                        // CSI Pn d
                        // VPA - Line position absolute
                        self.state = EngineState::Default;
                        let num = self.parsed_numbers.first().copied().unwrap_or(1).saturating_sub(1);
                        let first = buf.get_first_visible_line();
                        buf.caret_mut().y = first + num;
                        buf.limit_caret_pos();
                        return Ok(CallbackAction::Update);
                    }
                    'e' => {
                        // CSI Pn e
                        // VPR - Line position forward
                        self.state = EngineState::Default;
                        let num = self.parsed_numbers.first().copied().unwrap_or(1);
                        let first = buf.get_first_visible_line();
                        let y = buf.caret().y;
                        buf.caret_mut().y = first + y + num;
                        buf.limit_caret_pos();
                        return Ok(CallbackAction::Update);
                    }
                    '\'' => {
                        // Horizontal Line Position Absolute
                        self.state = EngineState::Default;
                        let num = self.parsed_numbers.first().copied().unwrap_or(1).saturating_sub(1);

                        buf.caret_mut().x = num;
                        buf.limit_caret_pos();
                        return Ok(CallbackAction::Update);
                    }
                    'a' => {
                        // CSI Pn a
                        // HPR - Character position forward
                        self.state = EngineState::Default;
                        let num = self.parsed_numbers.first().copied().unwrap_or(1);
                        let x = buf.caret().x;
                        buf.caret_mut().x = x + num;
                        buf.limit_caret_pos();
                        return Ok(CallbackAction::Update);
                    }

                    'G' => {
                        // Cursor Horizontal Absolute
                        self.state = EngineState::Default;
                        let num = self.parsed_numbers.first().copied().unwrap_or(1).saturating_sub(1);

                        buf.caret_mut().x = num;
                        buf.limit_caret_pos();
                        return Ok(CallbackAction::Update);
                    }
                    'E' => {
                        // Cursor Next Line
                        self.state = EngineState::Default;
                        let num = self.parsed_numbers.first().copied().unwrap_or(1);

                        let first = buf.get_first_visible_line();
                        let y = buf.caret().y + num;
                        buf.caret_mut().y = first + y;
                        buf.caret_mut().x = 0;
                        buf.limit_caret_pos();
                        return Ok(CallbackAction::Update);
                    }
                    'F' => {
                        // Cursor Previous Line
                        self.state = EngineState::Default;
                        let num = self.parsed_numbers.first().copied().unwrap_or(1);

                        let first = buf.get_first_visible_line();
                        let y = buf.caret().y - num;
                        buf.caret_mut().y = first + y;
                        buf.caret_mut().x = 0;
                        buf.limit_caret_pos();
                        return Ok(CallbackAction::Update);
                    }

                    'n' => {
                        // CSI Ps n
                        // DSR - Device Status Report
                        self.state = EngineState::Default;
                        if self.parsed_numbers.is_empty() {
                            return self.unsupported_escape_error();
                        }
                        if self.parsed_numbers.len() != 1 {
                            return self.unsupported_escape_error();
                        }
                        match self.parsed_numbers.first() {
                            Some(5) => {
                                // Device status report
                                return Ok(CallbackAction::SendString("\x1b[0n".to_string()));
                            }
                            Some(6) => {
                                // Get cursor position
                                let s = format!(
                                    "\x1b[{};{}R",
                                    min(buf.terminal_state().get_height(), buf.caret().y + 1),
                                    min(buf.terminal_state().get_width(), buf.caret().x + 1)
                                );
                                return Ok(CallbackAction::SendString(s));
                            }
                            Some(255) => {
                                // Current screen size
                                let s = format!(
                                    "\x1b[{};{}R",
                                    buf.terminal_state().get_height(), buf.terminal_state().get_width()
                                );
                                return Ok(CallbackAction::SendString(s));
                            }
                            _ => {
                                return self.unsupported_escape_error();
                            }
                        }
                    }

                    /*  TODO:
                        Insert Column 	  CSI Pn ' }
                        Delete Column 	  CSI Pn ' ~
                    */
                    'X' => return self.erase_character(buf),
                    '@' => {
                        // Insert character (ICH)
                        // Inserts blank characters at cursor position without moving cursor
                        self.state = EngineState::Default;

                        let count = self.parsed_numbers.first().copied().unwrap_or(1);
                        let original_pos = buf.caret().position();
                        for _ in 0..count {
                            buf.ins();
                        }
                        // Restore cursor position (ins() advances it, but ICH should not)
                        buf.caret_mut().set_position(original_pos);
                        return Ok(CallbackAction::Update);
                    }
                    'M' => {
                        // Delete line
                        self.state = EngineState::Default;
                        if matches!(self.ansi_music, MusicOption::Conflicting)
                            || matches!(self.ansi_music, MusicOption::Both)
                        {
                            self.cur_music = Some(AnsiMusic::default());
                            self.dotted_note = false;
                            self.state = EngineState::ParseAnsiMusic(MusicState::ParseMusicStyle);
                        } else if self.parsed_numbers.is_empty() {
                            if buf.caret().y < buf.line_count() as i32 {
                                buf.remove_terminal_line(buf.caret().y);
                            }
                        } else {
                            if self.parsed_numbers.len() != 1 {
                                return self.unsupported_escape_error();
                            }
                            if let Some(number) = self.parsed_numbers.first() {
                                let mut number = *number;
                                number = min(number, buf.line_count() as i32 - buf.caret().y);
                                for _ in 0..number {
                                    buf.remove_terminal_line(buf.caret().y);
                                }
                            } else {
                                return self.unsupported_escape_error();
                            }
                        }
                        return Ok(CallbackAction::Update);
                    }
                    'N' => {
                        if matches!(self.ansi_music, MusicOption::Banana)
                            || matches!(self.ansi_music, MusicOption::Both)
                        {
                            self.cur_music = Some(AnsiMusic::default());
                            self.dotted_note = false;
                            self.state = EngineState::ParseAnsiMusic(MusicState::ParseMusicStyle);
                        }
                        return Ok(CallbackAction::None);
                    }

                    '|' => {
                        if !matches!(self.ansi_music, MusicOption::Off) {
                            self.cur_music = Some(AnsiMusic::default());
                            self.dotted_note = false;
                            self.state = EngineState::ParseAnsiMusic(MusicState::ParseMusicStyle);
                        }
                        return Ok(CallbackAction::None);
                    }

                    'P' => {
                        // Delete character
                        self.state = EngineState::Default;
                        if self.parsed_numbers.is_empty() {
                            buf.del();
                        } else {
                            if self.parsed_numbers.len() != 1 {
                                return self.unsupported_escape_error();
                            }
                            if let Some(number) = self.parsed_numbers.first() {
                                for _ in 0..*number {
                                    buf.del();
                                }
                            } else {
                                return self.unsupported_escape_error();
                            }
                        }
                        return Ok(CallbackAction::Update);
                    }

                    'L' => {
                        // Insert line
                        self.state = EngineState::Default;
                        if self.parsed_numbers.is_empty() {
                            buf.insert_terminal_line(buf.caret().y);
                        } else {
                            if self.parsed_numbers.len() != 1 {
                                return self.unsupported_escape_error();
                            }
                            if let Some(number) = self.parsed_numbers.first() {
                                for _ in 0..*number {
                                    buf.insert_terminal_line(buf.caret().y);
                                }
                            } else {
                                return self.unsupported_escape_error();
                            }
                        }
                        return Ok(CallbackAction::Update);
                    }

                    'J' => {
                        // Erase in Display
                        self.state = EngineState::Default;
                        if self.parsed_numbers.is_empty() {
                            buf.clear_buffer_down();
                        } else if let Some(number) = self.parsed_numbers.first() {
                            match *number {
                                0 => {
                                    buf.clear_buffer_down();
                                }
                                1 => {
                                    buf.clear_buffer_up();
                                }
                                2 |  // clear entire screen
                                3 => {
                                    buf.clear_screen();
                                    buf.clear_scrollback();
                                }
                                _ => {
                                    buf.clear_buffer_down();
                                    return self.unsupported_escape_error();
                                }
                            }
                        } else {
                            return self.unsupported_escape_error();
                        }
                        return Ok(CallbackAction::Update);
                    }

                    '?' => {
                        if !is_start {
                            self.state = EngineState::Default;
                            return self.unsupported_escape_error();
                        }
                        // read custom command
                        self.state = EngineState::ReadCSICommand;
                        return Ok(CallbackAction::None);
                    }
                    '=' => {
                        if !is_start {
                            self.state = EngineState::Default;
                            return self.unsupported_escape_error();
                        }
                        // read custom command
                        self.state = EngineState::ReadCSIRequest;
                        return Ok(CallbackAction::None);
                    }
                    '!' => {
                        if !is_start {
                            self.state = EngineState::Default;
                            return Ok(CallbackAction::RunSkypixSequence(self.parsed_numbers.clone()));
                        }
                        // read custom command
                        self.state = EngineState::ReadRIPSupportRequest;
                        return Ok(CallbackAction::None);
                    }
                    '<' => {
                        if !is_start {
                            self.state = EngineState::Default;
                            return self.unsupported_escape_error();
                        }
                        // read custom command
                        self.state = EngineState::ReadDeviceAttrs;
                        return Ok(CallbackAction::None);
                    }

                    '*' => {
                        self.state = EngineState::EndCSI('*');
                    }
                    '$' => {
                        self.state = EngineState::EndCSI('$');
                    }
                    ' ' => {
                        self.state = EngineState::EndCSI(' ');
                    }

                    'K' => {
                        // Erase in line
                        self.state = EngineState::Default;
                        if self.parsed_numbers.is_empty() {
                            buf.clear_line_end();
                        } else {
                            match self.parsed_numbers.first() {
                                Some(0) => {
                                    buf.clear_line_end();
                                }
                                Some(1) => {
                                    buf.clear_line_start();
                                }
                                Some(2) => {
                                    buf.clear_line();
                                }
                                _ => {
                                    return self.unsupported_escape_error();
                                }
                            }
                        }
                        return Ok(CallbackAction::Update);
                    }
                    'c' => {
                        // CSI c or CSI 0 c - Primary Device Attributes
                        return self.device_attributes();
                    }
                    'r' => return if self.parsed_numbers.len() > 2 {
                        self.change_scrolling_region(buf)
                    } else {
                        self.set_top_and_bottom_margins(buf)
                    },
                    'h' => {
                        self.state = EngineState::Default;
                        if self.parsed_numbers.len() != 1 {
                            return self.unsupported_escape_error();
                        }
                        match self.parsed_numbers.first() {
                            Some(4) => {
                                buf.caret_mut().insert_mode = true;
                            }
                            _ => {
                                return self.unsupported_escape_error();
                            }
                        }
                        return Ok(CallbackAction::Update);
                    }

                    'l' => {
                        self.state = EngineState::Default;
                        if self.parsed_numbers.len() != 1 {
                            return self.unsupported_escape_error();
                        }
                        match self.parsed_numbers.first() {
                            Some(4) => {
                                buf.caret_mut().insert_mode = false;
                            }
                            _ => {
                                return self.unsupported_escape_error();
                            }
                        }
                        return Ok(CallbackAction::Update);
                    }
                    '~' => {
                        self.state = EngineState::Default;
                        if self.parsed_numbers.len() != 1 {
                            return self.unsupported_escape_error();
                        }
                        match self.parsed_numbers.first() {
                            Some(1) => {
                                buf.caret_mut().x = 0;
                            } // home
                            Some(2) => {
                                buf.ins();
                            } // home
                            Some(3) => {
                                buf.del();
                            }
                            Some(4) => {
                                buf.eol();
                            }
                            Some(5) => {  // Page Up
                                let lines = buf.terminal_state().get_height() - 1;
                                (0..lines).for_each(|_| buf.scroll_down());
                            }
                            Some(6) => {  // Page Down
                                let lines = buf.terminal_state().get_height() - 1;
                                (0..lines).for_each(|_| buf.scroll_up());
                            }
                            _ => {
                                return self.unsupported_escape_error();
                            }
                        }
                        return Ok(CallbackAction::Update);
                    }

                    't' => {
                        self.state = EngineState::Default;
                        return match self.parsed_numbers.len() {
                            3 => self.window_manipulation(buf),
                            4 => self.select_24bit_color(buf),
                            _ => self.unsupported_escape_error()
                        };
                    }
                    'S' => {
                        // Scroll Up
                        self.state = EngineState::Default;
                        let num = self.parsed_numbers.first().copied().unwrap_or(1);
                        (0..num).for_each(|_| buf.scroll_up());
                        return Ok(CallbackAction::Update);
                    }
                    'T' => {
                        // Scroll Down
                        self.state = EngineState::Default;
                        let num = self.parsed_numbers.first().copied().unwrap_or(1);
                        (0..num).for_each(|_| buf.scroll_down());
                        return Ok(CallbackAction::Update);
                    }
                    'b' => {
                        // CSI Pn b
                        // REP - Repeat the preceding graphic character Pn times (REP).
                        self.state = EngineState::Default;
                        let num = self.parsed_numbers.first().copied().unwrap_or(1);
                        let ch = AttributedChar::new(self.last_char, buf.caret().attribute);
                        (0..num).for_each(|_| buf.print_char(ch));
                        return Ok(CallbackAction::Update);
                    }
                    'g' => {
                        // CSI Ps g
                        // TBC - Tabulation clear
                        self.state = EngineState::Default;
                        if self.parsed_numbers.len() > 1 {
                            return Err(ParserError::UnsupportedEscapeSequence.into());
                        }

                        let num = self.parsed_numbers.first().copied().unwrap_or(0);
                        match num {
                            0 => {
                                let x = buf.caret().position().x;
                                buf.terminal_state_mut().remove_tab_stop(x);
                            }
                            3 | 5 => {
                                buf.terminal_state_mut().clear_tab_stops();
                            }
                            _ => {
                                return Err(ParserError::UnsupportedEscapeSequence.into());
                            }
                        }
                        return Ok(CallbackAction::None);
                    }
                    'Y' => {
                        // CVT - Cursor line tabulation
                        self.state = EngineState::Default;
                        if self.parsed_numbers.len() > 1 {
                            return Err(ParserError::UnsupportedEscapeSequence.into());
                        }

                        let num = self.parsed_numbers.first().copied().unwrap_or(1);
                        (0..num).for_each(|_| {
                            let x = buf.terminal_state().next_tab_stop(buf.caret().position().x);
                            buf.caret_mut().x = x;
                        });
                        return Ok(CallbackAction::Update);
                    }
                    'Z' => {
                        // CBT - Cursor backward tabulation
                        self.state = EngineState::Default;
                        if self.parsed_numbers.len() > 1 {
                            return Err(ParserError::UnsupportedEscapeSequence.into());
                        }

                        let num = self.parsed_numbers.first().copied().unwrap_or(1);
                        (0..num).for_each(|_| {
                            let x = buf.terminal_state().prev_tab_stop(buf.caret().position().x);
                            buf.caret_mut().x = x;
                        });
                        return Ok(CallbackAction::Update);
                    }
                    _ => {
                        self.state = EngineState::ReadCSISequence(false);
                        if ('\x40'..='\x7E').contains(&ch) {
                            // unknown control sequence, terminate reading
                            self.state = EngineState::Default;
                            return self.unsupported_escape_error();
                        }

                        if ch.is_ascii_digit() {
                            let d = match self.parsed_numbers.pop() {
                                Some(number) => number,
                                _ => 0,
                            };
                            self.parsed_numbers.push(parse_next_number(d, ch as u8));
                        } else if ch == ';' {
                            self.parsed_numbers.push(0);
                        } else {
                            self.state = EngineState::Default;
                            // error in control sequence, terminate reading
                            return self.unsupported_escape_error();
                        }
                        return Ok(CallbackAction::None);
                    }
                }
            }

            EngineState::Default => match ch {
                '\x1B' => {
                    self.state = EngineState::Default;
                    self.state = EngineState::ReadEscapeSequence;
                    return Ok(CallbackAction::None);
                }
                LF => {
                    return Ok(buf.lf());
                }
                FF => {
                    buf.ff();
                    return Ok(CallbackAction::Update);
                }
                CR => {
                    buf.cr();
                    return Ok(CallbackAction::Update);
                }
                BEL => return Ok(CallbackAction::Beep),
                TAB => buf.tab_forward(),
                '\x7F' => {
                    buf.del();
                    return Ok(CallbackAction::Update);
                }
                _ => {
                    if ch == crate::BS && self.bs_is_ctrl_char {
                        buf.bs();
                    } else if (ch == '\x00' || ch == '\u{00FF}') && self.bs_is_ctrl_char {
                        buf.caret_default_colors();
                    } else {
                        self.last_char = ch;
                        let ch = AttributedChar::new(self.last_char, buf.caret().attribute);
                        buf.print_char(ch);
                    }
                    return Ok(CallbackAction::Update);
                }
            },
        }

        Ok(CallbackAction::None)
    }
}

impl Parser {
    fn invoke_macro_by_id(&mut self, buf: &mut dyn EditableScreen, id: i32) {
        let m = if let Some(m) = self.macros.get(&(id as usize)) {
            m.clone()
        } else {
            return;
        };
        for ch in m.chars() {
            if let Err(err) = self.print_char(buf, ch) {
                self.state = EngineState::Default;
                log::error!("Error during macro invocation: {}", err);
            }
        }
    }

    fn execute_aps_command(&self, _buf: &mut dyn EditableScreen) {
        log::warn!("TODO execute APS command: {}", fmt_error_string(&self.parse_string));
    }

    fn unsupported_escape_error(&self) -> EngineResult<CallbackAction> {
        Err(ParserError::UnsupportedEscapeSequence.into())
    }
}

fn set_font_selection_success(buf: &mut dyn EditableScreen, slot: usize) {
    buf.terminal_state_mut().font_selection_state = FontSelectionState::Success;
    buf.caret_mut().set_font_page(slot);

    if buf.caret().attribute.is_blinking() && buf.caret().attribute.is_bold() {
        buf.terminal_state_mut().high_intensity_blink_attribute_font_slot = slot;
    } else if buf.caret().attribute.is_blinking() {
        buf.terminal_state_mut().blink_attribute_font_slot = slot;
    } else if buf.caret().attribute.is_bold() {
        buf.terminal_state_mut().high_intensity_attribute_font_slot = slot;
    } else {
        buf.terminal_state_mut().normal_attribute_font_slot = slot;
    }
}

#[inline(always)]
pub fn parse_next_number(x: i32, ch: u8) -> i32 {
    x.saturating_mul(10).saturating_add(ch as i32).saturating_sub(b'0' as i32)
}

pub fn fmt_error_string(input: &str) -> String {
    input.chars().take(40).collect::<String>()
}
