// Useful description: https://vt100.net/docs/vt510-rm/chapter4.html
//                     https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Normal-tracking-mode
use std::{
    cmp::{max, min},
    collections::HashMap,
    fmt::Display,
};

use self::sound::{AnsiMusic, MusicState};

use super::{BufferParser, TAB};
use crate::{
    AttributedChar, AutoWrapMode, BEL, BS, Buffer, CR, CallbackAction, Caret, EngineResult, FF, FontSelectionState, HyperLink, IceMode, LF, MouseMode,
    OriginMode, ParserError, Position, TerminalScrolling, update_crc16,
};

mod ansi_commands;
pub mod constants;
mod dcs;
mod osc;
pub mod sound;

#[cfg(test)]
mod sixel_tests;
#[cfg(test)]
mod tests;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
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
    saved_cursor_opt: Option<Caret>,
    pub(crate) parsed_numbers: Vec<i32>,

    pub hyper_links: Vec<HyperLink>,

    current_escape_sequence: String,

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

impl Default for Parser {
    fn default() -> Self {
        Parser {
            state: EngineState::Default,
            saved_pos: Position::default(),
            parsed_numbers: Vec::new(),
            current_escape_sequence: String::with_capacity(32),
            saved_cursor_opt: None,
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
    fn print_char(&mut self, buf: &mut Buffer, current_layer: usize, caret: &mut Caret, ch: char) -> EngineResult<CallbackAction> {
        match &self.state {
            EngineState::ParseAnsiMusic(_) => {
                return self.parse_ansi_music(ch);
            }
            EngineState::ReadEscapeSequence => {
                return {
                    self.state = EngineState::Default;
                    self.current_escape_sequence.push(ch);

                    match ch {
                        '[' => {
                            self.state = EngineState::ReadCSISequence(true);
                            self.parsed_numbers.clear();
                            Ok(CallbackAction::NoUpdate)
                        }
                        ']' => {
                            self.state = EngineState::ReadOSCSequence;
                            self.parsed_numbers.clear();
                            self.parse_string.clear();
                            Ok(CallbackAction::NoUpdate)
                        }
                        '7' => {
                            self.saved_cursor_opt = Some(caret.clone());
                            Ok(CallbackAction::NoUpdate)
                        }
                        '8' => {
                            if let Some(saved_caret) = &self.saved_cursor_opt {
                                *caret = saved_caret.clone();
                            }
                            Ok(CallbackAction::Update)
                        }

                        'c' => {
                            // RIS—Reset to Initial State see https://vt100.net/docs/vt510-rm/RIS.html
                            caret.ff(buf, current_layer);
                            caret.reset();
                            buf.reset_terminal();
                            self.macros.clear();
                            Ok(CallbackAction::Update)
                        }

                        'D' => {
                            // Index
                            caret.index(buf, current_layer);
                            Ok(CallbackAction::Update)
                        }
                        'M' => {
                            // Reverse Index
                            caret.reverse_index(buf, current_layer);
                            Ok(CallbackAction::Update)
                        }

                        'E' => {
                            // Next Line
                            caret.next_line(buf, current_layer);
                            Ok(CallbackAction::Update)
                        }

                        'P' => {
                            // DCS
                            self.state = EngineState::RecordDCS;
                            self.parse_string.clear();
                            self.parsed_numbers.clear();
                            Ok(CallbackAction::NoUpdate)
                        }
                        'H' => {
                            // set tab at current column
                            self.state = EngineState::Default;
                            buf.terminal_state.set_tab_at(caret.get_position().x);
                            Ok(CallbackAction::NoUpdate)
                        }

                        '_' => {
                            // Application Program String
                            self.state = EngineState::ReadAPS;
                            self.parse_string.clear();
                            Ok(CallbackAction::NoUpdate)
                        }

                        '0'..='~' => {
                            // Silently drop unsupported sequences
                            self.state = EngineState::Default;
                            Ok(CallbackAction::NoUpdate)
                        }
                        FF | BEL | BS | '\x09' | '\x7F' | '\x1B' | '\n' | '\r' => {
                            // non standard extension to print esc chars ESC ESC -> ESC
                            self.last_char = ch;
                            let ch = AttributedChar::new(self.last_char, caret.get_attribute());
                            buf.print_char(current_layer, caret, ch);
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
                    return Ok(CallbackAction::NoUpdate);
                }
                self.parse_string.push(ch);
            }
            EngineState::ReadAPSEscape => {
                if ch == '\\' {
                    self.state = EngineState::Default;
                    self.execute_aps_command(buf, caret);
                    return Ok(CallbackAction::NoUpdate);
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
                    return Ok(CallbackAction::NoUpdate);
                }
                if ch == '[' {
                    if *i != 0 {
                        self.state = EngineState::Default;
                        return Err(ParserError::UnsupportedDCSSequence(format!("Error in macro inside dcs, expected '[' got '{ch}'")).into());
                    }
                    self.state = EngineState::ReadPossibleMacroInDCS(1);
                    return Ok(CallbackAction::NoUpdate);
                }
                if ch == '*' {
                    if *i != 1 {
                        self.state = EngineState::Default;
                        return Err(ParserError::UnsupportedDCSSequence(format!("Error in macro inside dcs, expected '*' got '{ch}'")).into());
                    }
                    self.state = EngineState::ReadPossibleMacroInDCS(2);
                    return Ok(CallbackAction::NoUpdate);
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
                    self.invoke_macro_by_id(buf, current_layer, caret, *self.parsed_numbers.first().unwrap());
                    return Ok(CallbackAction::NoUpdate);
                }
                self.parse_string.push('\x1b');
                self.parse_string.push('[');
                self.parse_string.push_str(&self.macro_dcs);
                self.state = EngineState::RecordDCS;
                return Ok(CallbackAction::NoUpdate);
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
                return Ok(CallbackAction::NoUpdate);
            }
            EngineState::RecordDCSEscape => {
                if ch == '\\' {
                    self.state = EngineState::Default;
                    return self.execute_dcs(buf, caret);
                }
                if ch == '[' {
                    self.state = EngineState::ReadPossibleMacroInDCS(1);
                    self.macro_dcs.clear();
                    return Ok(CallbackAction::NoUpdate);
                }
                self.parse_string.push('\x1b');
                self.parse_string.push(ch);
                self.state = EngineState::RecordDCS;
                return Ok(CallbackAction::NoUpdate);
            }

            EngineState::ReadOSCSequence => {
                if ch == '\x1B' {
                    self.state = EngineState::ReadOSCSequenceEscape;
                    return Ok(CallbackAction::NoUpdate);
                }
                if ch == '\x07' {
                    self.state = EngineState::Default;
                    return self.parse_osc(buf, caret);
                }
                self.parse_string.push(ch);
                return Ok(CallbackAction::NoUpdate);
            }
            EngineState::ReadOSCSequenceEscape => {
                if ch == '\\' {
                    self.state = EngineState::Default;
                    return self.parse_osc(buf, caret);
                }
                self.state = EngineState::ReadOSCSequence;
                self.parse_string.push('\x1B');
                self.parse_string.push(ch);
                return Ok(CallbackAction::NoUpdate);
            }

            EngineState::ReadCSICommand => {
                self.current_escape_sequence.push(ch);
                match ch {
                    'l' => {
                        self.state = EngineState::Default;
                        if self.parsed_numbers.len() != 1 {
                            return self.unsupported_escape_error();
                        }
                        match self.parsed_numbers.first() {
                            Some(4) => buf.terminal_state.scroll_state = TerminalScrolling::Fast,
                            Some(6) => {
                                //  buf.terminal_state.origin_mode = OriginMode::WithinMargins;
                            }
                            Some(7) => buf.terminal_state.auto_wrap_mode = AutoWrapMode::NoWrap,
                            Some(25) => caret.set_is_visible(false),
                            Some(33) => caret.set_ice_mode(false),
                            Some(35) => caret.is_blinking = true,

                            Some(69) => {
                                buf.terminal_state.dec_margin_mode_left_right = false;
                                buf.terminal_state.clear_margins_left_right();
                            }

                            Some(9 | 1000..=1007 | 1015 | 1016) => {
                                buf.terminal_state.mouse_mode = MouseMode::Default;
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
                            Some(4) => buf.terminal_state.scroll_state = TerminalScrolling::Smooth,
                            Some(6) => buf.terminal_state.origin_mode = OriginMode::UpperLeftCorner,
                            Some(7) => buf.terminal_state.auto_wrap_mode = AutoWrapMode::AutoWrap,
                            Some(25) => caret.set_is_visible(true),
                            Some(33) => {
                                buf.ice_mode = IceMode::Ice;
                                caret.set_ice_mode(true);
                            }
                            Some(35) => caret.is_blinking = false,

                            Some(69) => buf.terminal_state.dec_margin_mode_left_right = true,

                            // Mouse tracking see https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Normal-tracking-mode
                            Some(9) => buf.terminal_state.mouse_mode = MouseMode::X10,
                            Some(1000) => buf.terminal_state.mouse_mode = MouseMode::VT200,
                            Some(1001) => {
                                buf.terminal_state.mouse_mode = MouseMode::VT200_Highlight;
                            }
                            Some(1002) => buf.terminal_state.mouse_mode = MouseMode::ButtonEvents,
                            Some(1003) => buf.terminal_state.mouse_mode = MouseMode::AnyEvents,

                            Some(1004) => buf.terminal_state.mouse_mode = MouseMode::FocusEvent,
                            Some(1007) => {
                                buf.terminal_state.mouse_mode = MouseMode::AlternateScroll;
                            }
                            Some(1005) => buf.terminal_state.mouse_mode = MouseMode::ExtendedMode,
                            Some(1006) => {
                                buf.terminal_state.mouse_mode = MouseMode::SGRExtendedMode;
                            }
                            Some(1015) => {
                                buf.terminal_state.mouse_mode = MouseMode::URXVTExtendedMode;
                            }
                            Some(1016) => buf.terminal_state.mouse_mode = MouseMode::PixelPosition,

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
                                    return Err(
                                        ParserError::UnsupportedEscapeSequence("Memory Checksum Report (DECCKSR) requires 2 parameters.".to_string()).into(),
                                    );
                                }
                                let mut crc16 = 0;
                                for i in 0..64 {
                                    if let Some(m) = self.macros.get(&i) {
                                        for b in m.as_bytes() {
                                            crc16 = update_crc16(crc16, *b);
                                        }
                                        crc16 = update_crc16(crc16, 0);
                                    } else {
                                        crc16 = update_crc16(crc16, 0);
                                    }
                                }
                                return Ok(CallbackAction::SendString(format!("\x1BP{}!~{crc16:04X}\x1B\\", self.parsed_numbers[1])));
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
                self.current_escape_sequence.push(ch);
                match ch {
                    'n' => {
                        self.state = EngineState::Default;
                        if self.parsed_numbers.len() != 1 {
                            return self.unsupported_escape_error();
                        }
                        match self.parsed_numbers.first() {
                            Some(1) => {
                                // font state report
                                let font_selection_result = match buf.terminal_state.font_selection_state {
                                    FontSelectionState::NoRequest => 99,
                                    FontSelectionState::Success => 0,
                                    FontSelectionState::Failure => 1,
                                };

                                return Ok(CallbackAction::SendString(format!(
                                    "\x1B[=1;{font_selection_result};{};{};{};{}n",
                                    buf.terminal_state.normal_attribute_font_slot,
                                    buf.terminal_state.high_intensity_attribute_font_slot,
                                    buf.terminal_state.blink_attribute_font_slot,
                                    buf.terminal_state.high_intensity_blink_attribute_font_slot
                                )));
                            }
                            Some(2) => {
                                // font mode report
                                let mut params = Vec::new();
                                if buf.terminal_state.origin_mode == OriginMode::WithinMargins {
                                    params.push("6");
                                }
                                if buf.terminal_state.auto_wrap_mode == AutoWrapMode::AutoWrap {
                                    params.push("7");
                                }
                                if caret.is_visible() {
                                    params.push("25");
                                }
                                if caret.ice_mode() {
                                    params.push("33");
                                }
                                if caret.is_blinking {
                                    params.push("35");
                                }

                                match buf.terminal_state.mouse_mode {
                                    MouseMode::Default => {}
                                    MouseMode::X10 => params.push("9"),
                                    MouseMode::VT200 => params.push("1000"),
                                    MouseMode::VT200_Highlight => params.push("1001"),
                                    MouseMode::ButtonEvents => params.push("1002"),
                                    MouseMode::AnyEvents => params.push("1003"),
                                    MouseMode::FocusEvent => params.push("1004"),
                                    MouseMode::AlternateScroll => params.push("1007"),
                                    MouseMode::ExtendedMode => params.push("1005"),
                                    MouseMode::SGRExtendedMode => params.push("1006"),
                                    MouseMode::URXVTExtendedMode => params.push("1015"),
                                    MouseMode::PixelPosition => params.push("1016"),
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
                        return Err(ParserError::UnsupportedEscapeSequence(format!("Error in CSI request: {}", self.current_escape_sequence)).into());
                    }
                }
            }

            EngineState::ReadRIPSupportRequest => {
                self.current_escape_sequence.push(ch);
                if let 'p' = ch {
                    self.soft_terminal_reset(buf, caret);
                } else {
                    // potential rip support request
                    // ignore that for now and continue parsing
                    self.state = EngineState::Default;
                    return self.print_char(buf, current_layer, caret, ch);
                }
            }

            EngineState::ReadDeviceAttrs => {
                self.current_escape_sequence.push(ch);
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
                            return Err(ParserError::UnsupportedEscapeSequence("CSI < Ps c more than 1 number.".to_string()).into());
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

            EngineState::EndCSI(func) => {
                self.current_escape_sequence.push(ch);
                match *func {
                    '*' => match ch {
                        'z' => return self.invoke_macro(buf, current_layer, caret),
                        'r' => return self.select_communication_speed(buf),
                        'y' => return self.request_checksum_of_rectangular_area(buf),
                        _ => {}
                    },

                    '$' => match ch {
                        'w' => {
                            self.state = EngineState::Default;
                            if let Some(2) = self.parsed_numbers.first() {
                                let mut str = "\x1BP2$u".to_string();
                                (0..buf.terminal_state.tab_count()).for_each(|i| {
                                    let tab = buf.terminal_state.get_tabs()[i];
                                    str.push_str(&(tab + 1).to_string());
                                    if i < buf.terminal_state.tab_count().saturating_sub(1) {
                                        str.push('/');
                                    }
                                });
                                str.push_str("\x1B\\");
                                return Ok(CallbackAction::SendString(str));
                            }
                        }
                        'x' => return self.fill_rectangular_area(buf, caret),
                        'z' => return self.erase_rectangular_area(buf),
                        '{' => return self.selective_erase_rectangular_area(buf),

                        _ => {}
                    },

                    ' ' => {
                        self.state = EngineState::Default;

                        match ch {
                            'D' => return self.font_selection(buf, caret),
                            'A' => self.scroll_right(buf, current_layer),
                            '@' => self.scroll_left(buf, current_layer),
                            'd' => return self.tabulation_stop_remove(buf),
                            _ => {
                                self.current_escape_sequence.push(ch);
                                return self.unsupported_escape_error();
                            }
                        }
                    }
                    _ => {
                        self.state = EngineState::Default;
                        return self.unsupported_escape_error();
                    }
                }
            }
            EngineState::ReadCSISequence(is_start) => {
                self.current_escape_sequence.push(ch);
                match ch {
                    'm' => return self.select_graphic_rendition(caret, buf),
                    'H' |    // Cursor Position
                    'f' // CSI Pn1 ; Pn2 f 
                        // HVP - Character and line position
                    => {
                        self.state = EngineState::Default;
                        if self.parsed_numbers.is_empty() {
                            caret.pos = buf.upper_left_position();
                        } else {
                            if self.parsed_numbers[0] >= 0 {
                                // always be in terminal mode for gotoxy
                                caret.pos.y = buf.get_first_visible_line()
                                    + max(0, self.parsed_numbers[0] - 1);
                            }
                            if self.parsed_numbers.len() > 1 {
                                if self.parsed_numbers[1] >= 0 {
                                    caret.pos.x = max(0, self.parsed_numbers[1] - 1);
                                }
                            } else {
                                caret.pos.x = 0;
                            }
                        }
                        buf.terminal_state.limit_caret_pos(buf, caret);
                        return Ok(CallbackAction::Update);
                    }
                    'C' => {
                        // Cursor Forward
                        self.state = EngineState::Default;
                        if self.parsed_numbers.is_empty() {
                            caret.right(buf, 1);
                        } else {
                            caret.right(buf, self.parsed_numbers[0]);
                        }
                        return Ok(CallbackAction::Update);
                    }
                    'j' | // CSI Pn j
                          // HPB - Character position backward
                    'D' => {
                        // Cursor Back
                        self.state = EngineState::Default;
                        if self.parsed_numbers.is_empty() {
                            caret.left(buf, 1);
                        } else {
                            caret.left(buf, self.parsed_numbers[0]);
                        }
                        return Ok(CallbackAction::Update);
                    }
                    'k' | // CSI Pn k
                          // VPB - Line position backward
                    'A' => {
                        // Cursor Up
                        self.state = EngineState::Default;
                        if self.parsed_numbers.is_empty() {
                            caret.up(buf, current_layer, 1);
                        } else {
                            caret.up(buf, current_layer, self.parsed_numbers[0]);
                        }
                        return Ok(CallbackAction::Update);
                    }
                    'B' => {
                        // Cursor Down
                        self.state = EngineState::Default;
                        if self.parsed_numbers.is_empty() {
                            caret.down(buf, current_layer, 1);
                        } else {
                            caret.down(buf, current_layer,self.parsed_numbers[0]);
                        }
                        return Ok(CallbackAction::Update);
                    }
                    's' => {
                        if buf.terminal_state.dec_margin_mode_left_right {
                            return self.set_left_and_right_margins(buf);
                        }
                        self.save_cursor_position(caret);
                        return Ok(CallbackAction::NoUpdate);
                    }
                    'u' => self.restore_cursor_position(caret),
                    'd' => {
                        // CSI Pn d
                        // VPA - Line position absolute
                        self.state = EngineState::Default;
                        let num = match self.parsed_numbers.first() {
                            Some(n) => n - 1,
                            _ => 0,
                        };
                        caret.pos.y = buf.get_first_visible_line() + num;
                        buf.terminal_state.limit_caret_pos(buf, caret);
                        return Ok(CallbackAction::Update);
                    }
                    'e' => {
                        // CSI Pn e
                        // VPR - Line position forward
                        self.state = EngineState::Default;
                        let num = match self.parsed_numbers.first() {
                            Some(n) => *n,
                            _ => 1,
                        };
                        caret.pos.y = buf.get_first_visible_line() + caret.pos.y + num;
                        buf.terminal_state.limit_caret_pos(buf, caret);
                        return Ok(CallbackAction::Update);
                    }
                    '\'' => {
                        // Horizontal Line Position Absolute
                        self.state = EngineState::Default;
                        let num = match self.parsed_numbers.first() {
                            Some(n) => n - 1,
                            _ => 0,
                        };
                        if let Some(layer) = &buf.layers.first() {
                            if let Some(line) = layer.lines.get(caret.pos.y as usize) {
                                caret.pos.x = num.clamp(0, line.get_line_length());
                                buf.terminal_state.limit_caret_pos(buf, caret);
                            }
                        } else {
                            return Err(ParserError::InvalidBuffer.into());
                        }
                        return Ok(CallbackAction::Update);
                    }
                    'a' => {
                        // CSI Pn a
                        // HPR - Character position forward
                        self.state = EngineState::Default;
                        let num = match self.parsed_numbers.first() {
                            Some(n) => *n,
                            _ => 1,
                        };
                        if let Some(layer) = &buf.layers.first() {
                            if let Some(line) = layer.lines.get(caret.pos.y as usize) {
                                caret.pos.x =
                                    min(line.get_line_length(), caret.pos.x + num);
                                buf.terminal_state.limit_caret_pos(buf, caret);
                            }
                        } else {
                            return Err(ParserError::InvalidBuffer.into());
                        }
                        return Ok(CallbackAction::Update);
                    }

                    'G' => {
                        // Cursor Horizontal Absolute
                        self.state = EngineState::Default;
                        let num = match self.parsed_numbers.first() {
                            Some(n) => n - 1,
                            _ => 0,
                        };
                        caret.pos.x = num;
                        buf.terminal_state.limit_caret_pos(buf, caret);
                        return Ok(CallbackAction::Update);
                    }
                    'E' => {
                        // Cursor Next Line
                        self.state = EngineState::Default;
                        let num = match self.parsed_numbers.first() {
                            Some(n) => *n,
                            _ => 1,
                        };
                        caret.pos.y = buf.get_first_visible_line() + caret.pos.y + num;
                        caret.pos.x = 0;
                        buf.terminal_state.limit_caret_pos(buf, caret);
                        return Ok(CallbackAction::Update);
                    }
                    'F' => {
                        // Cursor Previous Line
                        self.state = EngineState::Default;
                        let num = match self.parsed_numbers.first() {
                            Some(n) => *n,
                            _ => 1,
                        };
                        caret.pos.y = buf.get_first_visible_line() + caret.pos.y - num;
                        caret.pos.x = 0;
                        buf.terminal_state.limit_caret_pos(buf, caret);
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
                                    min(buf.terminal_state.get_height(), caret.pos.y + 1),
                                    min(buf.terminal_state.get_width(), caret.pos.x + 1)
                                );
                                return Ok(CallbackAction::SendString(s));
                            }
                            Some(255) => {
                                // Current screen size
                                let s = format!(
                                    "\x1b[{};{}R",
                                    buf.terminal_state.get_height(), buf.terminal_state.get_width()
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
                    'X' => return self.erase_character(caret, buf, current_layer),
                    '@' => {
                        // Insert character
                        self.state = EngineState::Default;

                        if let Some(number) = self.parsed_numbers.first() {
                            for _ in 0..*number {
                                caret.ins(buf, current_layer);
                            }
                        } else {
                            caret.ins(buf, current_layer);
                            if self.parsed_numbers.len() != 1 {
                                return self.unsupported_escape_error();
                            }
                        }
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
                            if let Some(layer) = buf.layers.first() {
                                if caret.pos.y < layer.lines.len() as i32 {
                                    buf.remove_terminal_line(current_layer, caret.pos.y);
                                }
                            } else {
                                return Err(ParserError::InvalidBuffer.into());
                            }
                        } else {
                            if self.parsed_numbers.len() != 1 {
                                return self.unsupported_escape_error();
                            }
                            if let Some(number) = self.parsed_numbers.first() {
                                let mut number = *number;
                                if let Some(layer) = buf.layers.first() {
                                    number = min(number, layer.lines.len() as i32 - caret.pos.y);
                                } else {
                                    return Err(ParserError::InvalidBuffer.into());
                                }
                                for _ in 0..number {
                                    buf.remove_terminal_line(current_layer, caret.pos.y);
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
                        return Ok(CallbackAction::NoUpdate);
                    }

                    '|' => {
                        if !matches!(self.ansi_music, MusicOption::Off) {
                            self.cur_music = Some(AnsiMusic::default());
                            self.dotted_note = false;
                            self.state = EngineState::ParseAnsiMusic(MusicState::ParseMusicStyle);
                        }
                        return Ok(CallbackAction::NoUpdate);
                    }

                    'P' => {
                        // Delete character
                        self.state = EngineState::Default;
                        if self.parsed_numbers.is_empty() {
                            caret.del(buf, current_layer);
                        } else {
                            if self.parsed_numbers.len() != 1 {
                                return self.unsupported_escape_error();
                            }
                            if let Some(number) = self.parsed_numbers.first() {
                                for _ in 0..*number {
                                    caret.del(buf,current_layer);
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
                            buf.insert_terminal_line( current_layer, caret.pos.y);
                        } else {
                            if self.parsed_numbers.len() != 1 {
                                return self.unsupported_escape_error();
                            }
                            if let Some(number) = self.parsed_numbers.first() {
                                for _ in 0..*number {
                                    buf.insert_terminal_line(current_layer,caret.pos.y);
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
                            buf.clear_buffer_down(current_layer,caret);
                        } else if let Some(number) = self.parsed_numbers.first() {
                            match *number {
                                0 => {
                                    buf.clear_buffer_down(current_layer,caret);
                                }
                                1 => {
                                    buf.clear_buffer_up(current_layer,caret);
                                }
                                2 |  // clear entire screen
                                3 => {
                                    buf.clear_screen(current_layer,caret);
                                }
                                _ => {
                                    buf.clear_buffer_down(current_layer,caret);
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
                        return Ok(CallbackAction::NoUpdate);
                    }
                    '=' => {
                        if !is_start {
                            self.state = EngineState::Default;
                            return self.unsupported_escape_error();
                        }
                        // read custom command
                        self.state = EngineState::ReadCSIRequest;
                        return Ok(CallbackAction::NoUpdate);
                    }
                    '!' => {
                        if !is_start {
                            self.state = EngineState::Default;
                            return Ok(CallbackAction::RunSkypixSequence(self.parsed_numbers.clone()));
                        }
                        // read custom command
                        self.state = EngineState::ReadRIPSupportRequest;
                        return Ok(CallbackAction::NoUpdate);
                    }
                    '<' => {
                        if !is_start {
                            self.state = EngineState::Default;
                            return self.unsupported_escape_error();
                        }
                        // read custom command
                        self.state = EngineState::ReadDeviceAttrs;
                        return Ok(CallbackAction::NoUpdate);
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
                            buf.clear_line_end(current_layer,caret);
                        } else {
                            match self.parsed_numbers.first() {
                                Some(0) => {
                                    buf.clear_line_end(current_layer,caret);
                                }
                                Some(1) => {
                                    buf.clear_line_start(current_layer,caret);
                                }
                                Some(2) => {
                                    buf.clear_line(current_layer,caret);
                                }
                                _ => {
                                    return self.unsupported_escape_error();
                                }
                            }
                        }
                        return Ok(CallbackAction::Update);
                    }
                    'c' => return self.device_attributes(),
                    'r' => return if self.parsed_numbers.len() > 2 {
                        self.change_scrolling_region(buf, caret)
                    } else {
                        self.set_top_and_bottom_margins(buf, caret)
                    },
                    'h' => {
                        self.state = EngineState::Default;
                        if self.parsed_numbers.len() != 1 {
                            return self.unsupported_escape_error();
                        }
                        match self.parsed_numbers.first() {
                            Some(4) => {
                                caret.insert_mode = true;
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
                                caret.insert_mode = false;
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
                                caret.pos.x = 0;
                            } // home
                            Some(2) => {
                                caret.ins(buf, current_layer);
                            } // home
                            Some(3) => {
                                caret.del(buf, current_layer);
                            }
                            Some(4) => {
                                caret.eol(buf);
                            }
                            Some(5 | 6) => {} // pg up/downf
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
                            4 => self.select_24bit_color(buf, caret),
                            _ => self.unsupported_escape_error()
                        };
                    }
                    'S' => {
                        // Scroll Up
                        self.state = EngineState::Default;
                        let num = if let Some(number) = self.parsed_numbers.first() {
                            *number
                        } else {
                            1
                        };
                        (0..num).for_each(|_| buf.scroll_up(current_layer));
                        return Ok(CallbackAction::Update);
                    }
                    'T' => {
                        // Scroll Down
                        self.state = EngineState::Default;
                        let num = if let Some(number) = self.parsed_numbers.first() {
                            *number
                        } else {
                            1
                        };
                        (0..num).for_each(|_| buf.scroll_down(current_layer));
                        return Ok(CallbackAction::Update);
                    }
                    'b' => {
                        // CSI Pn b
                        // REP - Repeat the preceding graphic character Pn times (REP).
                        self.state = EngineState::Default;
                        let num: i32 = if let Some(number) = self.parsed_numbers.first() {
                            *number
                        } else {
                            1
                        };
                        let ch = AttributedChar::new(self.last_char, caret.get_attribute());
                        (0..num).for_each(|_| buf.print_char(current_layer, caret, ch));
                        return Ok(CallbackAction::Update);
                    }
                    'g' => {
                        // CSI Ps g
                        // TBC - Tabulation clear
                        self.state = EngineState::Default;
                        if self.parsed_numbers.len() > 1 {
                            return Err(ParserError::UnsupportedEscapeSequence(
                                format!("Invalid parameter number in clear tab stops: {}", self.parsed_numbers.len()),
                            ).into());
                        }

                        let num: i32 = if let Some(number) = self.parsed_numbers.first() {
                            *number
                        } else {
                            0
                        };

                        match num {
                            0 => { buf.terminal_state.remove_tab_stop(caret.get_position().x) }
                            3 | 5 => {
                                buf.terminal_state.clear_tab_stops();
                            }
                            _ => {
                                return Err(ParserError::UnsupportedEscapeSequence(
                                    format!("Unsupported option in clear tab stops sequence: {num}"),
                                ).into());
                            }
                        }
                        return Ok(CallbackAction::NoUpdate);
                    }
                    'Y' => {
                        // CVT - Cursor line tabulation
                        self.state = EngineState::Default;
                        if self.parsed_numbers.len() > 1 {
                            return Err(ParserError::UnsupportedEscapeSequence(
                                format!("Invalid parameter number in goto next tab stop: {}", self.parsed_numbers.len()),
                            ).into());
                        }

                        let num: i32 = if let Some(number) = self.parsed_numbers.first() {
                            *number
                        } else {
                            1
                        };
                        (0..num).for_each(|_| caret.set_x_position(buf.terminal_state.next_tab_stop(caret.get_position().x)));
                        return Ok(CallbackAction::Update);
                    }
                    'Z' => {
                        // CBT - Cursor backward tabulation
                        self.state = EngineState::Default;
                        if self.parsed_numbers.len() > 1 {
                            return Err(ParserError::UnsupportedEscapeSequence(
                                format!("Invalid parameter number in goto next tab stop: {}", self.parsed_numbers.len()),
                            ).into());
                        }

                        let num: i32 = if let Some(number) = self.parsed_numbers.first() {
                            *number
                        } else {
                            1
                        };
                        (0..num).for_each(|_| caret.set_x_position(buf.terminal_state.prev_tab_stop(caret.get_position().x)));
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
                        return Ok(CallbackAction::NoUpdate);
                    }
                }
            }

            EngineState::Default => match ch {
                '\x1B' => {
                    self.reset_escape_sequence();
                    self.state = EngineState::Default;
                    self.state = EngineState::ReadEscapeSequence;
                    return Ok(CallbackAction::NoUpdate);
                }
                LF => {
                    return Ok(caret.lf(buf, current_layer));
                }
                FF => {
                    caret.ff(buf, current_layer);
                    return Ok(CallbackAction::Update);
                }
                CR => {
                    caret.cr(buf);
                    return Ok(CallbackAction::Update);
                }
                BEL => return Ok(CallbackAction::Beep),
                TAB => caret.tab_forward(buf),
                '\x7F' => {
                    caret.del(buf, current_layer);
                    return Ok(CallbackAction::Update);
                }
                _ => {
                    if ch == crate::BS && self.bs_is_ctrl_char {
                        caret.bs(buf, current_layer);
                    } else if (ch == '\x00' || ch == '\u{00FF}') && self.bs_is_ctrl_char {
                        caret.reset_color_attribute();
                    } else {
                        self.last_char = ch;
                        let ch = AttributedChar::new(self.last_char, caret.get_attribute());
                        buf.print_char(current_layer, caret, ch);
                    }
                    return Ok(CallbackAction::Update);
                }
            },
        }

        Ok(CallbackAction::NoUpdate)
    }
}

impl Parser {
    fn invoke_macro_by_id(&mut self, buf: &mut Buffer, current_layer: usize, caret: &mut Caret, id: i32) {
        let m = if let Some(m) = self.macros.get(&(id as usize)) {
            m.clone()
        } else {
            return;
        };
        for ch in m.chars() {
            if let Err(err) = self.print_char(buf, current_layer, caret, ch) {
                self.state = EngineState::Default;
                log::error!("Error during macro invocation: {}", err);
            }
        }
    }

    fn execute_aps_command(&self, _buf: &mut Buffer, _caret: &mut Caret) {
        log::warn!("TODO execute APS command: {}", fmt_error_string(&self.parse_string));
    }

    fn reset_escape_sequence(&mut self) {
        self.current_escape_sequence.clear();
        self.current_escape_sequence.push_str("<ESC>");
    }

    fn unsupported_escape_error(&self) -> EngineResult<CallbackAction> {
        Err(ParserError::UnsupportedEscapeSequence(self.current_escape_sequence.clone()).into())
    }
}

fn set_font_selection_success(buf: &mut Buffer, caret: &mut Caret, slot: usize) {
    buf.terminal_state.font_selection_state = FontSelectionState::Success;
    caret.set_font_page(slot);

    if caret.attribute.is_blinking() && caret.attribute.is_bold() {
        buf.terminal_state.high_intensity_blink_attribute_font_slot = slot;
    } else if caret.attribute.is_blinking() {
        buf.terminal_state.blink_attribute_font_slot = slot;
    } else if caret.attribute.is_bold() {
        buf.terminal_state.high_intensity_attribute_font_slot = slot;
    } else {
        buf.terminal_state.normal_attribute_font_slot = slot;
    }
}

pub fn parse_next_number(x: i32, ch: u8) -> i32 {
    x.saturating_mul(10).saturating_add(ch as i32).saturating_sub(b'0' as i32)
}

pub fn fmt_error_string(input: &str) -> String {
    input.chars().take(40).collect::<String>()
}
