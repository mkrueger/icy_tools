//! ANSI escape sequence parser
//!
//! Parses ANSI/VT100 escape sequences into structured commands.
//! Supports CSI (Control Sequence Introducer), ESC, and OSC sequences.

use std::u16;

use base64::{Engine as _, engine::general_purpose};
pub mod music;
pub mod sgr;
use crate::{
    AnsiMode, BACKSPACE, BELL, BaudEmulation, CARRIAGE_RETURN, CaretShape, Color, CommandParser, CommandSink, CommunicationLine, DELETE, DecMode,
    DeviceControlString, Direction, ESC, EraseInDisplayMode, EraseInLineMode, FORM_FEED, LINE_FEED, MusicState, OperatingSystemCommand, ParseError,
    SgrAttribute, TAB, TerminalCommand, TerminalRequest, Wrapping,
};

pub struct AnsiParser {
    state: ParserState,
    params: Vec<u16>,
    parse_buffer: Vec<u8>,
    last_char: u8,
    macros: std::collections::HashMap<usize, Vec<u8>>,

    // ANSI Music support
    pub music_option: music::MusicOption,
    music_state: music::MusicState,
    cur_music: Option<music::AnsiMusic>,
    cur_octave: usize,
    cur_length: i32,
    cur_tempo: i32,
    dotted_note: bool,
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self {
            state: ParserState::Default,
            params: Vec::new(),
            parse_buffer: Vec::new(),
            last_char: 0,
            macros: std::collections::HashMap::new(),
            music_option: music::MusicOption::Off,
            music_state: music::MusicState::Default,
            cur_music: None,
            cur_octave: 5,
            cur_length: 4,
            cur_tempo: 120,
            dotted_note: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum ParserState {
    Default = 0,
    Escape = 1,
    CsiEntry = 2,
    CsiParam = 3,
    // CSI with specific intermediate bytes
    CsiDecPrivate = 10, // CSI ? ... (or CSI ... ?)
    CsiAsterisk = 12,   // CSI ... *
    CsiDollar = 14,     // CSI ... $
    CsiSpace = 16,      // CSI ... SP
    CsiGreater = 18,    // CSI > ...
    CsiExclaim = 20,    // CSI ! ...
    CsiEquals = 22,     // CSI = ...
    CsiLess = 24,       // CSI < ...
    // Other states
    OscString = 5,
    DcsString = 6,
    DcsEscape = 7,
    ApsString = 8,
    ApsEscape = 9,
    AnsiMusic = 11, // ANSI Music parsing
}

impl Default for ParserState {
    fn default() -> Self {
        ParserState::Default
    }
}

impl AnsiParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the ANSI music option
    pub fn set_music_option(&mut self, option: music::MusicOption) {
        self.music_option = option;
    }

    /// Get the current ANSI music option
    pub fn music_option(&self) -> music::MusicOption {
        self.music_option
    }

    fn reset(&mut self) {
        self.params.clear();
        self.state = ParserState::Default;
    }

    fn parse_dcs(&mut self, sink: &mut dyn CommandSink) {
        // Check for CTerm custom font: "CTerm:Font:{slot}:{base64_data}"
        if self.parse_buffer.starts_with(b"CTerm:Font:") {
            let start_index = b"CTerm:Font:".len();
            if let Some(colon_pos) = self.parse_buffer[start_index..].iter().position(|&b| b == b':') {
                let slot_end: usize = start_index + colon_pos;
                // Parse slot number
                if let Ok(slot_str) = std::str::from_utf8(&self.parse_buffer[start_index..slot_end]) {
                    if let Ok(slot) = slot_str.parse::<usize>() {
                        // Decode base64 font data (after second colon)
                        let data_start = slot_end + 1;
                        match general_purpose::STANDARD.decode(&self.parse_buffer[data_start..]) {
                            Ok(decoded_data) => {
                                sink.device_control(DeviceControlString::LoadFont(slot, decoded_data));
                                return;
                            }
                            Err(_) => {
                                sink.report_error(
                                    ParseError::MalformedSequence {
                                        description: "Invalid base64 in DCS font data",
                                        sequence: Some(format!("ESC P CTerm:Font:{}:...", slot)),
                                        context: Some(format!("Font slot {}", slot)),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                return;
                            }
                        }
                    }
                }
            }
            // If parsing failed, report error
            let seq_preview = String::from_utf8_lossy(&self.parse_buffer[..self.parse_buffer.len().min(20)]).to_string();
            sink.report_error(
                ParseError::MalformedSequence {
                    description: "Unknown or malformed DCS sequence",
                    sequence: Some(format!("ESC P {}...", seq_preview)),
                    context: Some("CTerm:Font parsing failed".to_string()),
                },
                crate::ErrorLevel::Error,
            );
            return;
        }

        // Parse parameters from DCS buffer
        let mut i = 0;
        self.params.clear();
        self.params.push(0);

        while i < self.parse_buffer.len() {
            let byte = self.parse_buffer[i];
            match byte {
                b'0'..=b'9' => {
                    let last = self.params.pop().unwrap_or(0);
                    self.params.push(last.wrapping_mul(10).wrapping_add((byte - b'0') as u16));
                }
                b';' => {
                    self.params.push(0);
                }
                _ => {
                    break;
                }
            }
            i += 1;
        }

        // Check for macro definition: ESC P {params} ! z {data} ESC \
        if i + 2 < self.parse_buffer.len() && self.parse_buffer[i] == b'!' && self.parse_buffer[i + 1] == b'z' {
            self.parse_macro_definition(i + 2);
            return;
        }

        // Check for Sixel graphics: ESC P {params} q {data} ESC \
        if i < self.parse_buffer.len() && self.parse_buffer[i] == b'q' {
            let aspect_ratio = self.params.get(0).copied();
            let zero_color = self.params.get(1).copied();
            let grid_size = self.params.get(2).copied();

            sink.device_control(DeviceControlString::Sixel {
                aspect_ratio,
                zero_color,
                grid_size,
                sixel_data: self.parse_buffer[i + 1..].to_vec(),
            });
            return;
        }

        // Unknown DCS - emit as Unknown
        let seq_preview = String::from_utf8_lossy(&self.parse_buffer[..self.parse_buffer.len().min(30)]).to_string();
        sink.report_error(
            ParseError::MalformedSequence {
                description: "Unknown or malformed escape sequence",
                sequence: Some(format!("ESC P {}...", seq_preview)),
                context: Some("DCS command not recognized".to_string()),
            },
            crate::ErrorLevel::Error,
        );
    }

    fn parse_macro_definition(&mut self, start_index: usize) {
        let pid = self.params.first().copied().unwrap_or(0) as usize;
        let pdt = self.params.get(1).copied().unwrap_or(0);
        let encoding = self.params.get(2).copied().unwrap_or(0);

        // pdt = 1 means clear all macros first
        if pdt == 1 {
            self.macros.clear();
        }

        match encoding {
            0 => {
                // Text encoding - store as-is
                self.macros.insert(pid, self.parse_buffer[start_index..].to_vec());
            }
            1 => {
                // Hex encoding - decode it
                if let Ok(decoded) = self.parse_hex_macro(&self.parse_buffer[start_index..]) {
                    self.macros.insert(pid, decoded);
                }
            }
            _ => {}
        }
    }

    fn parse_hex_macro(&self, data: &[u8]) -> Result<Vec<u8>, ()> {
        let mut result = Vec::new();
        let mut i = 0;
        let mut repeat_count: usize = 0;
        let mut in_repeat = false;
        let mut repeat_start = 0;

        while i < data.len() {
            if data[i] == b'!' {
                // Repeat sequence: !{count};{hex_data};
                i += 1;
                repeat_count = 0;
                while i < data.len() && data[i].is_ascii_digit() {
                    repeat_count = repeat_count.wrapping_mul(10).wrapping_add((data[i] - b'0') as usize);
                    i += 1;
                }
                if i < data.len() && data[i] == b';' {
                    i += 1;
                    in_repeat = true;
                    repeat_start = result.len();
                }
            } else if in_repeat && data[i] == b';' {
                // End of repeat section
                let repeat_data = result[repeat_start..].to_vec();
                for _ in 1..repeat_count {
                    result.extend_from_slice(&repeat_data);
                }
                in_repeat = false;
                i += 1;
            } else if i + 1 < data.len() {
                // Parse hex pair
                let high = Self::hex_digit(data[i])?;
                let low = Self::hex_digit(data[i + 1])?;
                result.push((high << 4) | low);
                i += 2;
            } else {
                i += 1;
            }
        }

        if in_repeat {
            let repeat_data = result[repeat_start..].to_vec();
            for _ in 1..repeat_count {
                result.extend_from_slice(&repeat_data);
            }
        }

        Ok(result)
    }

    fn hex_digit(byte: u8) -> Result<u8, ()> {
        match byte {
            b'0'..=b'9' => Ok(byte - b'0'),
            b'A'..=b'F' => Ok(byte - b'A' + 10),
            b'a'..=b'f' => Ok(byte - b'a' + 10),
            _ => Err(()),
        }
    }

    fn invoke_macro(&mut self, macro_id: usize, sink: &mut dyn CommandSink) {
        if let Some(macro_data) = self.macros.get(&macro_id).cloned() {
            self.parse(&macro_data, sink);
        }
    }

    #[inline(always)]
    fn flush_printable(&mut self, sink: &mut dyn CommandSink, input: &[u8], printable_start: usize, i: usize) {
        if i > printable_start {
            self.last_char = input[i - 1];
            sink.print(&input[printable_start..i]);
        }
    }
}

impl CommandParser for AnsiParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        let mut i = 0;
        let mut printable_start = 0;
        while i < input.len() {
            let byte = input[i];
            match self.state {
                ParserState::Default => {
                    match byte {
                        ESC => {
                            self.flush_printable(sink, input, printable_start, i);
                            self.state = ParserState::Escape;
                            i += 1;
                            printable_start = i;
                        }
                        BELL => {
                            self.flush_printable(sink, input, printable_start, i);
                            sink.emit(TerminalCommand::Bell);
                            i += 1;
                            printable_start = i;
                        }
                        BACKSPACE => {
                            self.flush_printable(sink, input, printable_start, i);
                            sink.emit(TerminalCommand::Backspace);
                            i += 1;
                            printable_start = i;
                        }
                        TAB => {
                            self.flush_printable(sink, input, printable_start, i);
                            sink.emit(TerminalCommand::Tab);
                            i += 1;
                            printable_start = i;
                        }
                        LINE_FEED => {
                            self.flush_printable(sink, input, printable_start, i);
                            sink.emit(TerminalCommand::LineFeed);
                            i += 1;
                            printable_start = i;
                        }
                        FORM_FEED => {
                            self.flush_printable(sink, input, printable_start, i);
                            sink.emit(TerminalCommand::FormFeed);
                            i += 1;
                            printable_start = i;
                        }
                        CARRIAGE_RETURN => {
                            self.flush_printable(sink, input, printable_start, i);
                            sink.emit(TerminalCommand::CarriageReturn);
                            i += 1;
                            printable_start = i;
                        }
                        DELETE => {
                            self.flush_printable(sink, input, printable_start, i);
                            sink.emit(TerminalCommand::Delete);
                            i += 1;
                            printable_start = i;
                        }
                        _ => {
                            // Printable or other character
                            i += 1;
                        }
                    }
                }

                ParserState::Escape => {
                    match byte {
                        b'[' => {
                            // CSI - Control Sequence Introducer
                            self.params.clear();
                            self.state = ParserState::CsiEntry;
                            i += 1;
                            printable_start = i;
                        }
                        b']' => {
                            // OSC - Operating System Command
                            self.parse_buffer.clear();
                            self.state = ParserState::OscString;
                            i += 1;
                            printable_start = i;
                        }
                        b'P' => {
                            // DCS - Device Control String
                            self.parse_buffer.clear();
                            self.state = ParserState::DcsString;
                            i += 1;
                            printable_start = i;
                        }
                        b'_' => {
                            // APS - Application Program String
                            self.parse_buffer.clear();
                            self.state = ParserState::ApsString;
                            i += 1;
                            printable_start = i;
                        }
                        b'D' => {
                            // IND - Index
                            sink.emit(TerminalCommand::EscIndex);
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        b'E' => {
                            // NEL - Next Line
                            sink.emit(TerminalCommand::EscNextLine);
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        b'H' => {
                            // HTS - Horizontal Tab Set
                            sink.emit(TerminalCommand::EscSetTab);
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        b'M' => {
                            // RI - Reverse Index
                            sink.emit(TerminalCommand::EscReverseIndex);
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        b'7' => {
                            // DECSC - Save Cursor
                            sink.emit(TerminalCommand::EscSaveCursor);
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        b'8' => {
                            // DECRC - Restore Cursor
                            sink.emit(TerminalCommand::EscRestoreCursor);
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        b'c' => {
                            // RIS - Reset to Initial State
                            sink.emit(TerminalCommand::EscReset);
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        FORM_FEED | BELL | BACKSPACE | TAB | DELETE | ESC | LINE_FEED | CARRIAGE_RETURN => {
                            // non standard extension to print esc chars ESC ESC -> ESC
                            self.last_char = byte;
                            sink.print(&[byte]);
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        _ => {
                            // Unknown escape sequence
                            sink.report_error(
                                ParseError::MalformedSequence {
                                    description: "Unknown or malformed escape sequence",
                                    sequence: Some(format!(
                                        "ESC {}",
                                        if byte.is_ascii_graphic() {
                                            format!("'{}'", byte as char)
                                        } else {
                                            format!("0x{:02X}", byte)
                                        }
                                    )),
                                    context: Some("ESC followed by unrecognized character".to_string()),
                                },
                                crate::ErrorLevel::Error,
                            );
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                    }
                }

                ParserState::CsiEntry => {
                    match byte {
                        b'0'..=b'9' => {
                            // Start of parameter
                            let digit = (byte - b'0') as u16;
                            self.params.push(digit);
                            self.state = ParserState::CsiParam;
                            i += 1;
                        }
                        b';' => {
                            // Empty parameter (default to 0)
                            self.params.push(0);
                            i += 1;
                        }
                        b'?' => {
                            self.state = ParserState::CsiDecPrivate;
                            i += 1;
                        }
                        b'>' => {
                            self.state = ParserState::CsiGreater;
                            i += 1;
                        }
                        b'<' => {
                            self.state = ParserState::CsiLess;
                            i += 1;
                        }
                        b'!' => {
                            self.state = ParserState::CsiExclaim;
                            i += 1;
                        }
                        b'=' => {
                            self.state = ParserState::CsiEquals;
                            i += 1;
                        }
                        b'*' => {
                            self.state = ParserState::CsiAsterisk;
                            i += 1;
                        }
                        b'$' => {
                            self.state = ParserState::CsiDollar;
                            i += 1;
                        }
                        b' ' => {
                            self.state = ParserState::CsiSpace;
                            i += 1;
                        }
                        b'@'..=b'~' => {
                            // Final byte without parameters - handle CSI command
                            self.handle_csi_final(byte, sink);
                            if self.state != ParserState::AnsiMusic {
                                self.reset();
                            }
                            i += 1;
                            printable_start = i;
                        }
                        ESC => {
                            // Malformed sequence - report error and fall back to ESC state
                            sink.report_error(
                                ParseError::MalformedSequence {
                                    description: "Incomplete CSI sequence interrupted by ESC",
                                    sequence: Some("CSI <incomplete>".to_string()),
                                    context: Some("Falling back to ESC state to parse next sequence".to_string()),
                                },
                                crate::ErrorLevel::Warning,
                            );
                            self.reset();
                            self.state = ParserState::Escape;
                            i += 1;
                            printable_start = i;
                        }
                        _ => {
                            // Invalid CSI sequence
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                    }
                }

                ParserState::CsiParam => {
                    match byte {
                        b'0'..=b'9' => {
                            // Continue building current parameter
                            let digit = (byte - b'0') as u16;
                            if let Some(last) = self.params.last_mut() {
                                *last = last.wrapping_mul(10).wrapping_add(digit);
                            } else {
                                self.params.push(digit);
                            }
                            i += 1;
                        }
                        b';' => {
                            // Next parameter
                            self.params.push(0);
                            i += 1;
                        }
                        b'\'' => {
                            // Single quote - special case final byte (non-standard but used)
                            self.handle_csi_final(byte, sink);
                            if self.state != ParserState::AnsiMusic {
                                self.reset();
                            }
                            i += 1;
                            printable_start = i;
                        }
                        b' ' => {
                            self.state = ParserState::CsiSpace;
                            i += 1;
                        }
                        b'*' => {
                            self.state = ParserState::CsiAsterisk;
                            i += 1;
                        }
                        b'$' => {
                            self.state = ParserState::CsiDollar;
                            i += 1;
                        }
                        b'@'..=b'~' => {
                            // Final byte
                            self.handle_csi_final(byte, sink);
                            if self.state != ParserState::AnsiMusic {
                                self.reset();
                            }
                            i += 1;
                            printable_start = i;
                        }
                        ESC => {
                            // Malformed sequence - report error and fall back to ESC state
                            sink.report_error(
                                ParseError::MalformedSequence {
                                    description: "Incomplete CSI sequence interrupted by ESC",
                                    sequence: Some(format!("CSI {:?} <incomplete>", self.params)),
                                    context: Some("Falling back to ESC state to parse next sequence".to_string()),
                                },
                                crate::ErrorLevel::Warning,
                            );
                            self.reset();
                            self.state = ParserState::Escape;
                            i += 1;
                            printable_start = i;
                        }
                        _ => {
                            // Invalid character
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                    }
                }

                // CSI ? (DEC Private Mode)
                ParserState::CsiDecPrivate => match byte {
                    b'0'..=b'9' => {
                        let digit = (byte - b'0') as u16;
                        if let Some(last) = self.params.last_mut() {
                            *last = last.wrapping_mul(10).wrapping_add(digit);
                        } else {
                            self.params.push(digit);
                        }
                        i += 1;
                    }
                    b';' => {
                        self.params.push(0);
                        i += 1;
                    }
                    b'@'..=b'~' => {
                        // TODO: inline DEC private mode CSI handling here
                        self.handle_dec_private_csi_final(byte, sink);
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    ESC => {
                        // Malformed sequence - report error and fall back to ESC state
                        sink.report_error(
                            ParseError::MalformedSequence {
                                description: "Incomplete CSI ? sequence interrupted by ESC",
                                sequence: Some(format!("CSI ? {:?} <incomplete>", self.params)),
                                context: Some("Falling back to ESC state to parse next sequence".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        self.reset();
                        self.state = ParserState::Escape;
                        i += 1;
                        printable_start = i;
                    }
                    _ => {
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                },

                // CSI * (Asterisk sequences)
                ParserState::CsiAsterisk => match byte {
                    b'0'..=b'9' => {
                        let digit = (byte - b'0') as u16;
                        if let Some(last) = self.params.last_mut() {
                            *last = last.wrapping_mul(10).wrapping_add(digit);
                        } else {
                            self.params.push(digit);
                        }
                        i += 1;
                    }
                    b';' => {
                        self.params.push(0);
                        i += 1;
                    }
                    b'z' => {
                        // Invoke Macro - execute it internally
                        let n = self.params.first().copied().unwrap_or(0) as usize;
                        self.invoke_macro(n, sink);
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    b'r' => {
                        // Select Communication Speed: ESC[{Ps1};{Ps2}*r
                        // Ps1 = communication line (0/1/none = Host Transmit, 2 = Host Receive,
                        //       3 = Printer, 4 = Modem Hi, 5 = Modem Lo)
                        // Ps2 = baud rate
                        let ps1 = self.params.first().copied().unwrap_or(0);
                        let ps2 = self.params.get(1).copied().unwrap_or(0);
                        let comm_line = CommunicationLine::from_u16(ps1 as u16);
                        let baud_option = *BaudEmulation::OPTIONS.get(ps2 as usize).unwrap_or(&BaudEmulation::Off);

                        sink.emit(TerminalCommand::CsiSelectCommunicationSpeed(comm_line, baud_option));
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    b'y' => {
                        // Request Checksum of Rectangular Area: ESC[{Pid};{Ppage};{Pt};{Pl};{Pb};{Pr}*y
                        let id = self.params.first().copied().unwrap_or(0) as u8;
                        let page = self.params.get(1).copied().unwrap_or(0) as u8;
                        let top = self.params.get(2).copied().unwrap_or(0) as u16;
                        let left = self.params.get(3).copied().unwrap_or(0) as u16;
                        let bottom = self.params.get(4).copied().unwrap_or(0) as u16;
                        let right = self.params.get(5).copied().unwrap_or(0) as u16;
                        sink.request(TerminalRequest::RequestChecksumRectangularArea {
                            id,
                            page,
                            top,
                            left,
                            bottom,
                            right,
                        });
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    ESC => {
                        // Malformed sequence - report error and fall back to ESC state
                        sink.report_error(
                            ParseError::MalformedSequence {
                                description: "Incomplete CSI * sequence interrupted by ESC",
                                sequence: Some(format!("CSI {:?} * <incomplete>", self.params)),
                                context: Some("Falling back to ESC state to parse next sequence".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        self.reset();
                        self.state = ParserState::Escape;
                        i += 1;
                        printable_start = i;
                    }
                    _ => {
                        sink.report_error(
                            ParseError::MalformedSequence {
                                description: "Unknown or malformed escape sequence",
                                sequence: Some(format!(
                                    "CSI {:?} * {}",
                                    self.params,
                                    if byte.is_ascii_graphic() {
                                        format!("'{}'", byte as char)
                                    } else {
                                        format!("0x{:02X}", byte)
                                    }
                                )),
                                context: Some("CSI * followed by unrecognized character".to_string()),
                            },
                            crate::ErrorLevel::Error,
                        );
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                },

                // CSI $ (Dollar sequences)
                ParserState::CsiDollar => match byte {
                    b'0'..=b'9' => {
                        let digit: u16 = (byte - b'0') as u16;
                        if let Some(last) = self.params.last_mut() {
                            *last = last.wrapping_mul(10).wrapping_add(digit);
                        } else {
                            self.params.push(digit);
                        }
                        i += 1;
                    }
                    b';' => {
                        self.params.push(0);
                        i += 1;
                    }
                    b'w' => {
                        // DECRQTSR - Request Tab Stop Report
                        sink.request(TerminalRequest::RequestTabStopReport);
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    b'x' => {
                        // DECFRA - Fill Rectangular Area
                        let pchar = self.params.first().copied().unwrap_or(0) as u8;
                        let pt = self.params.get(1).copied().unwrap_or(1);
                        let pl = self.params.get(2).copied().unwrap_or(1);
                        let pb = self.params.get(3).copied().unwrap_or(1);
                        let pr = self.params.get(4).copied().unwrap_or(1);
                        sink.emit(TerminalCommand::CsiFillRectangularArea {
                            char: pchar,
                            top: pt as u16,
                            left: pl as u16,
                            bottom: pb as u16,
                            right: pr as u16,
                        });
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    b'z' => {
                        // DECERA - Erase Rectangular Area
                        let pt = self.params.first().copied().unwrap_or(1);
                        let pl = self.params.get(1).copied().unwrap_or(1);
                        let pb = self.params.get(2).copied().unwrap_or(1);
                        let pr = self.params.get(3).copied().unwrap_or(1);
                        sink.emit(TerminalCommand::CsiEraseRectangularArea {
                            top: pt as u16,
                            left: pl as u16,
                            bottom: pb as u16,
                            right: pr as u16,
                        });
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    b'{' => {
                        // DECSERA - Selective Erase Rectangular Area
                        let pt = self.params.first().copied().unwrap_or(1);
                        let pl = self.params.get(1).copied().unwrap_or(1);
                        let pb = self.params.get(2).copied().unwrap_or(1);
                        let pr = self.params.get(3).copied().unwrap_or(1);
                        sink.emit(TerminalCommand::CsiSelectiveEraseRectangularArea {
                            top: pt as u16,
                            left: pl as u16,
                            bottom: pb as u16,
                            right: pr as u16,
                        });
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    ESC => {
                        // Malformed sequence - report error and fall back to ESC state
                        sink.report_error(
                            ParseError::MalformedSequence {
                                description: "Incomplete CSI $ sequence interrupted by ESC",
                                sequence: Some(format!("CSI {:?} $ <incomplete>", self.params)),
                                context: Some("Falling back to ESC state to parse next sequence".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        self.reset();
                        self.state = ParserState::Escape;
                        i += 1;
                        printable_start = i;
                    }
                    _ => {
                        sink.report_error(
                            ParseError::MalformedSequence {
                                description: "Unknown or malformed escape sequence",
                                sequence: Some(format!(
                                    "CSI {:?} $ {}",
                                    self.params,
                                    if byte.is_ascii_graphic() {
                                        format!("'{}'", byte as char)
                                    } else {
                                        format!("0x{:02X}", byte)
                                    }
                                )),
                                context: Some("CSI $ followed by unrecognized character (after DECSERA)".to_string()),
                            },
                            crate::ErrorLevel::Error,
                        );
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                },

                // CSI SP (Space sequences)
                ParserState::CsiSpace => match byte {
                    b'0'..=b'9' => {
                        let digit = (byte - b'0') as u16;
                        if let Some(last) = self.params.last_mut() {
                            *last = last.wrapping_mul(10).wrapping_add(digit);
                        } else {
                            self.params.push(digit);
                        }
                        i += 1;
                    }
                    b';' => {
                        self.params.push(0);
                        i += 1;
                    }
                    b'q' => {
                        // DECSCUSR - Set Caret Style
                        let style = self.params.first().copied().unwrap_or(1);
                        let (blinking, shape) = match style {
                            0 | 1 => (true, CaretShape::Block),
                            2 => (false, CaretShape::Block),
                            3 => (true, CaretShape::Underline),
                            4 => (false, CaretShape::Underline),
                            5 => (true, CaretShape::Bar),
                            6 => (false, CaretShape::Bar),
                            _ => (true, CaretShape::Block),
                        };
                        sink.emit(TerminalCommand::CsiSetCaretStyle(blinking, shape));
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    b'D' => {
                        // Font Selection
                        let ps1 = self.params.first().copied().unwrap_or(0);
                        let ps2 = self.params.get(1).copied().unwrap_or(0);
                        sink.emit(TerminalCommand::CsiFontSelection {
                            slot: ps1 as u16,
                            font_number: ps2 as u16,
                        });
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    b'A' => {
                        // Scroll Right
                        let n = self.params.first().copied().unwrap_or(1);
                        sink.emit(TerminalCommand::CsiScroll(Direction::Right, n as u16));
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    b'@' => {
                        // Scroll Left
                        let n = self.params.first().copied().unwrap_or(1);
                        sink.emit(TerminalCommand::CsiScroll(Direction::Left, n as u16));
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    b'd' => {
                        // Tabulation Clear
                        let ps = self.params.first().copied().unwrap_or(0);
                        if ps == 0 {
                            sink.emit(TerminalCommand::CsiClearTabulation);
                        } else {
                            sink.emit(TerminalCommand::CsiClearAllTabs);
                        }
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    ESC => {
                        // Malformed sequence - report error and fall back to ESC state
                        sink.report_error(
                            ParseError::MalformedSequence {
                                description: "Incomplete CSI SP sequence interrupted by ESC",
                                sequence: Some(format!("CSI {:?} SP <incomplete>", self.params)),
                                context: Some("Falling back to ESC state to parse next sequence".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        self.reset();
                        self.state = ParserState::Escape;
                        i += 1;
                        printable_start = i;
                    }
                    _ => {
                        sink.report_error(
                            ParseError::MalformedSequence {
                                description: "Unknown or malformed escape sequence",
                                sequence: Some(format!(
                                    "CSI $ {}",
                                    if byte.is_ascii_graphic() {
                                        format!("'{}'", byte as char)
                                    } else {
                                        format!("0x{:02X}", byte)
                                    }
                                )),
                                context: Some("CSI $ followed by unrecognized character".to_string()),
                            },
                            crate::ErrorLevel::Error,
                        );
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                },

                // CSI > (Greater sequences)
                // CSI > (Greater sequences) - not commonly used
                ParserState::CsiGreater => match byte {
                    b'0'..=b'9' => {
                        let digit = (byte - b'0') as u16;
                        if let Some(last) = self.params.last_mut() {
                            *last = last.wrapping_mul(10).wrapping_add(digit);
                        } else {
                            self.params.push(digit);
                        }
                        i += 1;
                    }
                    b';' => {
                        self.params.push(0);
                        i += 1;
                    }
                    ESC => {
                        // Malformed sequence - report error and fall back to ESC state
                        sink.report_error(
                            ParseError::MalformedSequence {
                                description: "Incomplete CSI > sequence interrupted by ESC",
                                sequence: Some(format!("CSI > {:?} <incomplete>", self.params)),
                                context: Some("Falling back to ESC state to parse next sequence".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        self.reset();
                        self.state = ParserState::Escape;
                        i += 1;
                        printable_start = i;
                    }
                    _ => {
                        // CSI > sequences
                        match byte {
                            b'c' => {
                                // Secondary Device Attributes
                                sink.request(TerminalRequest::SecondaryDeviceAttributes);
                            }
                            _ => {
                                sink.report_error(
                                    ParseError::MalformedSequence {
                                        description: "Unsupported CSI > sequence",
                                        sequence: Some(format!(
                                            "CSI > {}",
                                            if byte.is_ascii_graphic() {
                                                format!("'{}'", byte as char)
                                            } else {
                                                format!("0x{:02X}", byte)
                                            }
                                        )),
                                        context: Some(format!("CSI > with params {:?}", self.params)),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                            }
                        }
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                },

                // CSI < (Less Than sequences) - Extended Device Attributes
                ParserState::CsiLess => match byte {
                    b'0'..=b'9' => {
                        let digit = (byte - b'0') as u16;
                        if let Some(last) = self.params.last_mut() {
                            *last = last.wrapping_mul(10).wrapping_add(digit);
                        } else {
                            self.params.push(digit);
                        }
                        i += 1;
                    }
                    b';' => {
                        self.params.push(0);
                        i += 1;
                    }
                    b'c' => {
                        // Extended Device Attributes: ESC[<...c
                        // Reports terminal capabilities
                        sink.request(TerminalRequest::ExtendedDeviceAttributes);
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    ESC => {
                        // Malformed sequence - report error and fall back to ESC state
                        sink.report_error(
                            ParseError::MalformedSequence {
                                description: "Incomplete CSI < sequence interrupted by ESC",
                                sequence: Some(format!("CSI < {:?} <incomplete>", self.params)),
                                context: Some("Falling back to ESC state to parse next sequence".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        self.reset();
                        self.state = ParserState::Escape;
                        i += 1;
                        printable_start = i;
                    }
                    _ => {
                        sink.report_error(
                            ParseError::MalformedSequence {
                                description: "Unsupported CSI < sequence",
                                sequence: Some(format!(
                                    "CSI < {}",
                                    if byte.is_ascii_graphic() {
                                        format!("'{}'", byte as char)
                                    } else {
                                        format!("0x{:02X}", byte)
                                    }
                                )),
                                context: Some(format!("CSI < with params {:?}", self.params)),
                            },
                            crate::ErrorLevel::Error,
                        );
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                },

                // CSI ! (Exclaim sequences) - not commonly used
                ParserState::CsiExclaim => match byte {
                    b'0'..=b'9' => {
                        let digit = (byte - b'0') as u16;
                        if let Some(last) = self.params.last_mut() {
                            *last = last.wrapping_mul(10).wrapping_add(digit);
                        } else {
                            self.params.push(digit);
                        }
                        i += 1;
                    }
                    b';' => {
                        self.params.push(0);
                        i += 1;
                    }
                    ESC => {
                        // CSI ! is a valid RIP (Remote Imaging Protocol) sequence
                        // Silently fall back to ESC state without error reporting
                        self.reset();
                        self.state = ParserState::Escape;
                        i += 1;
                        printable_start = i;
                    }
                    _ => {
                        // No specific commands implemented for CSI ! sequences - it's a RIP request
                        // Silently ignore it.
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                },

                // CSI = (Equals sequences) - not commonly used
                ParserState::CsiEquals => match byte {
                    b'0'..=b'9' => {
                        let digit = (byte - b'0') as u16;
                        if let Some(last) = self.params.last_mut() {
                            *last = last.wrapping_mul(10).wrapping_add(digit);
                        } else {
                            self.params.push(digit);
                        }
                        i += 1;
                    }
                    b';' => {
                        self.params.push(0);
                        i += 1;
                    }
                    b'n' => {
                        // Font/mode reports: ESC[={n}n
                        if self.params.len() == 1 {
                            match self.params.first() {
                                Some(1) => sink.request(TerminalRequest::FontStateReport),
                                Some(2) => sink.request(TerminalRequest::FontModeReport),
                                Some(3) => sink.request(TerminalRequest::FontDimensionReport),
                                _ => {
                                    sink.report_error(
                                        ParseError::MalformedSequence {
                                            description: "Unsupported CSI = n sequence",
                                            sequence: Some(format!("CSI = {} n", self.params.first().unwrap_or(&0))),
                                            context: Some("Font/mode report with unsupported parameter".to_string()),
                                        },
                                        crate::ErrorLevel::Error,
                                    );
                                }
                            }
                        } else {
                            sink.report_error(
                                ParseError::MalformedSequence {
                                    description: "Invalid parameter count for CSI = n",
                                    sequence: Some(format!("CSI = {:?} n", self.params)),
                                    context: Some(format!("Expected 1 parameter, got {}", self.params.len())),
                                },
                                crate::ErrorLevel::Error,
                            );
                        }
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    b'r' => {
                        sink.emit(TerminalCommand::ResetMargins);
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    b'm' => {
                        // Set specific margins: ESC[={margin_type};{value}m
                        if self.params.len() == 2 {
                            let margin_type_val = self.params[0];
                            let value = self.params[1];

                            if let Some(margin_type) = crate::MarginType::from_u16(margin_type_val) {
                                sink.emit(TerminalCommand::CsiEqualsSetSpecificMargins(margin_type, value));
                            } else {
                                sink.report_error(
                                    ParseError::MalformedSequence {
                                        description: "Invalid margin type for CSI = m",
                                        sequence: Some(format!("CSI = {} ; {} m", margin_type_val, value)),
                                        context: Some(format!("Invalid margin type: {}", margin_type_val)),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                            }
                        } else {
                            sink.report_error(
                                ParseError::MalformedSequence {
                                    description: "Invalid parameter count for CSI = m",
                                    sequence: Some(format!("CSI = {:?} m", self.params)),
                                    context: Some(format!("Expected 2 parameters, got {}", self.params.len())),
                                },
                                crate::ErrorLevel::Error,
                            );
                        }
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    ESC => {
                        // Malformed sequence - report error and fall back to ESC state
                        sink.report_error(
                            ParseError::MalformedSequence {
                                description: "Incomplete CSI = sequence interrupted by ESC",
                                sequence: Some(format!("CSI = {:?} <incomplete>", self.params)),
                                context: Some("Falling back to ESC state to parse next sequence".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        self.reset();
                        self.state = ParserState::Escape;
                        i += 1;
                        printable_start = i;
                    }
                    _ => {
                        // No specific commands implemented for CSI = sequences
                        sink.report_error(
                            ParseError::MalformedSequence {
                                description: "Unsupported CSI = sequence",
                                sequence: Some(format!(
                                    "CSI = {:?} {}",
                                    self.params,
                                    if byte.is_ascii_graphic() {
                                        format!("'{}'", byte as char)
                                    } else {
                                        format!("0x{:02X}", byte)
                                    }
                                )),
                                context: Some("CSI = followed by unrecognized command".to_string()),
                            },
                            crate::ErrorLevel::Error,
                        );
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                },

                ParserState::OscString => {
                    // Use memchr to quickly find the next BEL or ESC byte
                    if let Some(term_pos) = memchr::memchr2(BELL, ESC, &input[i..]) {
                        let term_byte = input[i + term_pos];
                        // Copy everything up to terminator into parse_buffer
                        self.parse_buffer.extend_from_slice(&input[i..i + term_pos]);

                        if term_byte == BELL {
                            // BEL - End of OSC
                            self.emit_osc_sequence(sink);
                            self.reset();
                            i += term_pos + 1;
                            printable_start = i;
                        } else {
                            // ESC - might be ST (String Terminator: ESC \)
                            i += term_pos;
                            if i + 1 < input.len() && input[i + 1] == b'\\' {
                                // ST - String Terminator
                                self.emit_osc_sequence(sink);
                                self.reset();
                                i += 2; // Skip both ESC and \
                                printable_start = i;
                            } else {
                                // Collect ESC as part of OSC string
                                self.parse_buffer.push(ESC);
                                i += 1;
                            }
                        }
                    } else {
                        // No terminator found - consume rest of input
                        self.parse_buffer.extend_from_slice(&input[i..]);
                        i = input.len();
                    }
                }

                ParserState::DcsString => {
                    // Use memchr to quickly find the next ESC byte
                    if let Some(esc_pos) = memchr::memchr(ESC, &input[i..]) {
                        // Copy everything up to ESC into parse_buffer
                        self.parse_buffer.extend_from_slice(&input[i..i + esc_pos]);
                        i += esc_pos;
                        // Now we're at ESC - transition to DcsEscape state
                        self.state = ParserState::DcsEscape;
                        i += 1;
                    } else {
                        // No ESC found - consume rest of input
                        self.parse_buffer.extend_from_slice(&input[i..]);
                        i = input.len();
                    }
                }

                ParserState::DcsEscape => {
                    if byte == b'\\' {
                        // ST - String Terminator (ESC \)
                        self.parse_dcs(sink);
                        self.parse_buffer.clear();
                        self.reset();
                        i += 1;
                        printable_start = i;
                    } else if byte == b'[' {
                        // Start of macro invocation inside DCS: ESC [ <num> * z
                        // Save position and parse the macro sequence
                        i += 1;

                        // Parse number
                        let mut macro_id = 0u16;
                        while i < input.len() && input[i].is_ascii_digit() {
                            macro_id = macro_id.wrapping_mul(10).wrapping_add((input[i] - b'0') as u16);
                            i += 1;
                        }

                        // Expect '*'
                        if i < input.len() && input[i] == b'*' {
                            i += 1;
                            // Expect 'z'
                            if i < input.len() && input[i] == b'z' {
                                // Invoke the macro
                                self.state = ParserState::DcsString;
                                self.invoke_macro(macro_id as usize, sink);
                                i += 1;
                                continue;
                            }
                        }

                        // If we get here, it wasn't a valid macro sequence
                        // Treat the ESC [ and following chars as regular DCS data
                        self.parse_buffer.push(ESC);
                        self.parse_buffer.push(b'[');
                        // The next iteration will handle the current byte
                        self.state = ParserState::DcsString;
                    } else {
                        // False alarm - ESC was part of DCS data
                        self.parse_buffer.push(ESC);
                        self.parse_buffer.push(byte);
                        self.state = ParserState::DcsString;
                        i += 1;
                    }
                }

                ParserState::ApsString => {
                    // Use memchr to quickly find the next ESC byte
                    if let Some(esc_pos) = memchr::memchr(ESC, &input[i..]) {
                        // Copy everything up to ESC into parse_buffer
                        self.parse_buffer.extend_from_slice(&input[i..i + esc_pos]);
                        i += esc_pos;
                        // Now we're at ESC - transition to ApsEscape state
                        self.state = ParserState::ApsEscape;
                        i += 1;
                    } else {
                        // No ESC found - consume rest of input
                        self.parse_buffer.extend_from_slice(&input[i..]);
                        i = input.len();
                    }
                }

                ParserState::ApsEscape => {
                    if byte == b'\\' {
                        // ST - String Terminator (ESC \)
                        sink.aps(&self.parse_buffer);
                        self.reset();
                        i += 1;
                        printable_start = i;
                    } else {
                        // False alarm - ESC was part of APS data
                        self.parse_buffer.push(ESC);
                        self.parse_buffer.push(byte);
                        self.state = ParserState::ApsString;
                        i += 1;
                    }
                }

                ParserState::AnsiMusic => {
                    // Handle ANSI music parsing
                    self.parse_ansi_music(byte, sink);
                    i += 1;
                    printable_start = i;
                }
            }
        }

        // Emit any remaining printable bytes
        if i > printable_start && self.state == ParserState::Default {
            self.last_char = input[i - 1];
            sink.print(&input[printable_start..i]);
        }
    }
}

impl AnsiParser {
    #[inline(always)]
    fn emit_osc_sequence(&mut self, sink: &mut dyn CommandSink) {
        // OSC format: ESC ] Ps ; Pt BEL
        // Ps is the command number, Pt is the text

        if self.parse_buffer.is_empty() {
            return;
        }

        // Find semicolon separator
        if let Some(semicolon_pos) = self.parse_buffer.iter().position(|&b| b == b';') {
            let ps_bytes = &self.parse_buffer[..semicolon_pos];
            let pt_bytes = &self.parse_buffer[semicolon_pos + 1..];

            // Parse command number
            if let Ok(ps_str) = std::str::from_utf8(ps_bytes) {
                if let Ok(ps) = ps_str.parse::<u32>() {
                    match ps {
                        0 => {
                            // Set icon name and window title
                            sink.operating_system_command(OperatingSystemCommand::SetTitle(pt_bytes.to_vec()));
                        }
                        1 => {
                            // Set icon name
                            sink.operating_system_command(OperatingSystemCommand::SetIconName(pt_bytes.to_vec()));
                        }
                        2 => {
                            // Set window title
                            sink.operating_system_command(OperatingSystemCommand::SetWindowTitle(pt_bytes.to_vec()));
                        }
                        4 => {
                            // Set Palette Color: OSC 4 ; index ; rgb:rr/gg/bb BEL
                            // Format: "4;0;rgb:00/00/00" or just "0;rgb:00/00/00" after the first semicolon
                            self.parse_osc_palette(pt_bytes, sink);
                        }
                        8 => {
                            // Hyperlink: OSC 8 ; params ; URI BEL
                            if let Some(uri_pos) = pt_bytes.iter().position(|&b| b == b';') {
                                let params = pt_bytes[..uri_pos].to_vec();
                                let uri = pt_bytes[uri_pos + 1..].to_vec();
                                sink.operating_system_command(OperatingSystemCommand::Hyperlink { params, uri });
                            }
                        }
                        _ => {
                            // Unknown OSC command
                            sink.report_error(
                                ParseError::MalformedSequence {
                                    description: "Unknown or malformed escape sequence",
                                    sequence: Some(format!("OSC {}", ps)),
                                    context: Some("Unknown OSC command number".to_string()),
                                },
                                crate::ErrorLevel::Error,
                            );
                        }
                    }
                    return;
                }
            }
        }

        // Malformed OSC
        let seq_preview = String::from_utf8_lossy(&self.parse_buffer[..self.parse_buffer.len().min(30)]).to_string();
        sink.report_error(
            ParseError::MalformedSequence {
                description: "Unknown or malformed escape sequence",
                sequence: Some(format!("OSC {}...", seq_preview)),
                context: Some("Could not parse OSC command number".to_string()),
            },
            crate::ErrorLevel::Error,
        );
    }

    #[inline(always)]
    fn parse_osc_palette(&self, data: &[u8], sink: &mut dyn CommandSink) {
        // Parse OSC 4 palette entries: "index;rgb:rr/gg/bb"
        // Can have multiple entries separated by more semicolons

        let data_str = match std::str::from_utf8(data) {
            Ok(s) => s,
            Err(_) => {
                sink.report_error(
                    ParseError::MalformedSequence {
                        description: "Invalid UTF-8 in OSC 4 palette sequence",
                        sequence: Some(format!("OSC 4 ; {:?}", &data[..data.len().min(20)])),
                        context: Some("Palette data is not valid UTF-8".to_string()),
                    },
                    crate::ErrorLevel::Error,
                );
                return;
            }
        };

        // Split by semicolons to handle multiple palette entries
        let parts: Vec<&str> = data_str.split(';').collect();

        let mut i = 0;
        while i + 1 < parts.len() {
            // Parse color index
            let index = match parts[i].parse::<u32>() {
                Ok(idx) if idx <= 255 => idx as u8,
                _ => {
                    sink.report_error(
                        ParseError::MalformedSequence {
                            description: "Invalid color index in OSC 4",
                            sequence: Some(format!("OSC 4 ; {} ; ...", parts[i])),
                            context: Some("Color index must be 0-255".to_string()),
                        },
                        crate::ErrorLevel::Error,
                    );
                    i += 2;
                    continue;
                }
            };

            // Parse rgb:rr/gg/bb format
            let color_spec = parts[i + 1];
            if let Some(rgb_part) = color_spec.strip_prefix("rgb:").or_else(|| color_spec.strip_prefix("RGB:")) {
                let rgb_parts: Vec<&str> = rgb_part.split('/').collect();
                if rgb_parts.len() == 3 {
                    // Parse hex values (can be 1-4 hex digits each, we take first 2)
                    let r = Self::parse_hex_color_component(rgb_parts[0]);
                    let g = Self::parse_hex_color_component(rgb_parts[1]);
                    let b = Self::parse_hex_color_component(rgb_parts[2]);

                    if let (Some(r), Some(g), Some(b)) = (r, g, b) {
                        sink.operating_system_command(OperatingSystemCommand::SetPaletteColor(index, r, g, b));
                    } else {
                        sink.report_error(
                            ParseError::MalformedSequence {
                                description: "Invalid RGB values in OSC 4",
                                sequence: Some(format!("OSC 4 ; {} ; {}", index, color_spec)),
                                context: Some(format!("Failed to parse hex values: {}", rgb_part)),
                            },
                            crate::ErrorLevel::Error,
                        );
                    }
                } else {
                    sink.report_error(
                        ParseError::MalformedSequence {
                            description: "Invalid RGB format in OSC 4",
                            sequence: Some(format!("OSC 4 ; {} ; {}", index, color_spec)),
                            context: Some(format!("Expected 3 components separated by '/', got {}", rgb_parts.len())),
                        },
                        crate::ErrorLevel::Error,
                    );
                }
            } else {
                sink.report_error(
                    ParseError::MalformedSequence {
                        description: "Missing 'rgb:' prefix in OSC 4",
                        sequence: Some(format!("OSC 4 ; {} ; {}", index, color_spec)),
                        context: Some("Expected format: 'rgb:rr/gg/bb'".to_string()),
                    },
                    crate::ErrorLevel::Error,
                );
            }

            i += 2;
        }
    }

    #[inline(always)]
    fn parse_hex_color_component(hex_str: &str) -> Option<u8> {
        // X11 color spec can be 1-4 hex digits: h, hh, hhh, hhhh
        // We take the most significant byte (first 2 hex digits)
        if hex_str.is_empty() || hex_str.len() > 4 {
            return None;
        }

        // Pad or truncate to 2 hex digits
        let normalized = if hex_str.len() == 1 {
            // Single digit: repeat it (e.g., "f" -> "ff")
            format!("{}{}", hex_str, hex_str)
        } else {
            // Take first 2 characters
            hex_str[..2.min(hex_str.len())].to_string()
        };

        u8::from_str_radix(&normalized, 16).ok()
    }

    #[inline(always)]
    fn handle_csi_final(&mut self, final_byte: u8, sink: &mut dyn CommandSink) {
        match final_byte {
            b'A' | b'k' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Up, n as u16, Wrapping::Never));
            }

            b'B' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Down, n as u16, Wrapping::Never));
            }
            b'C' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Right, n as u16, Wrapping::Never));
            }
            b'D' | b'j' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Left, n as u16, Wrapping::Never));
            }
            b'E' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiCursorNextLine(n as u16));
            }
            b'F' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiCursorPreviousLine(n as u16));
            }
            b'G' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiCursorHorizontalAbsolute(n as u16));
            }
            b'H' | b'f' => {
                let row = self.params.first().copied().unwrap_or(1);
                let col = self.params.get(1).copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiCursorPosition(row as u16, col as u16));
            }
            b'd' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiLinePositionAbsolute(n as u16));
            }
            b'e' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiLinePositionForward(n as u16));
            }
            b'a' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiCharacterPositionForward(n as u16));
            }
            b'\'' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiHorizontalPositionAbsolute(n as u16));
            }
            b'J' => {
                let n = self.params.first().copied().unwrap_or(0);
                match EraseInDisplayMode::from_u16(n) {
                    Some(mode) => sink.emit(TerminalCommand::CsiEraseInDisplay(mode)),
                    None => {
                        sink.report_error(
                            ParseError::InvalidParameter {
                                command: "CsiEraseInDisplay",
                                value: format!("{}", n).to_string(),
                                expected: None,
                            },
                            crate::ErrorLevel::Error,
                        );
                        sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::CursorToEnd));
                    }
                }
            }
            b'K' => {
                let n = self.params.first().copied().unwrap_or(0);
                match EraseInLineMode::from_u16(n) {
                    Some(mode) => sink.emit(TerminalCommand::CsiEraseInLine(mode)),
                    None => {
                        sink.report_error(
                            ParseError::InvalidParameter {
                                command: "CsiEraseInLine",
                                value: format!("{}", n).to_string(),
                                expected: None,
                            },
                            crate::ErrorLevel::Error,
                        );
                        sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::CursorToEnd));
                    }
                }
            }
            b'S' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiScroll(Direction::Up, n as u16));
            }
            b'T' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiScroll(Direction::Down, n as u16));
            }
            b'm' => {
                let params: &[u16] = if self.params.is_empty() { &[0u16] } else { &self.params };
                sgr::parse_sgr(params, sink);
            }
            b'r' => match self.params.len() {
                0 => sink.emit(TerminalCommand::ResetMargins),
                1 => sink.emit(TerminalCommand::SetTopBottomMargin {
                    top: 0,
                    bottom: self.params[0],
                }),
                2 => {
                    if self.params[0] > self.params[1] {
                        sink.emit(TerminalCommand::ResetMargins)
                    } else {
                        sink.emit(TerminalCommand::SetTopBottomMargin {
                            top: self.params[0],
                            bottom: self.params[1],
                        })
                    }
                }
                3 => sink.emit(TerminalCommand::CsiSetScrollingRegion {
                    top: self.params[0],
                    bottom: self.params[1],
                    left: self.params[2],
                    right: u16::MAX,
                }),
                4 => sink.emit(TerminalCommand::CsiSetScrollingRegion {
                    top: self.params[0],
                    bottom: self.params[1],
                    left: self.params[2],
                    right: self.params[3],
                }),
                _ => {
                    sink.report_error(
                        ParseError::MalformedSequence {
                            description: "Invalid parameter count for CSI = r",
                            sequence: Some(format!("CSI = {:?} r", self.params)),
                            context: Some(format!("Expected 2-4 parameters for scroll region, got {}", self.params.len())),
                        },
                        crate::ErrorLevel::Error,
                    );
                }
            },
            b'@' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiInsertCharacter(n as u16));
            }
            b'P' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiDeleteCharacter(n as u16));
            }
            b'X' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiEraseCharacter(n as u16));
            }
            b'L' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiInsertLine(n as u16));
            }
            b'M' => {
                // CSI M can be either Delete Line or ANSI Music (conflicting)
                if self.music_option == music::MusicOption::Conflicting || self.music_option == music::MusicOption::Both {
                    if self.params.is_empty() {
                        // Start ANSI music parsing
                        self.state = ParserState::AnsiMusic;
                        self.cur_music = Some(music::AnsiMusic::default());
                        self.music_state = MusicState::Default;
                        return; // Don't reset - stay in AnsiMusic state
                    }
                }
                // Normal Delete Line
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiDeleteLine(n as u16));
            }
            b'N' => {
                // CSI N for ANSI Music (Banana style)
                if self.music_option == music::MusicOption::Banana || self.music_option == music::MusicOption::Both {
                    // Start ANSI music parsing
                    self.state = ParserState::AnsiMusic;
                    self.cur_music = Some(music::AnsiMusic::default());
                    self.music_state = MusicState::Default;
                    return; // Don't reset - stay in AnsiMusic state
                }
                // Otherwise, unknown command - ignore
                sink.report_error(
                    ParseError::MalformedSequence {
                        description: "Unknown CSI N command",
                        sequence: Some("CSI N".to_string()),
                        context: Some(format!("ANSI Music is disabled (option: {:?})", self.music_option)),
                    },
                    crate::ErrorLevel::Error,
                );
            }
            b'b' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.print(&vec![self.last_char; n as usize]);
            }
            b's' => {
                if self.params.len() == 2 {
                    sink.emit(TerminalCommand::ResetLeftAndRightMargin {
                        left: self.params[0],
                        right: self.params[1],
                    });
                } else {
                    sink.emit(TerminalCommand::CsiSaveCursorPosition);
                }
            }
            b'u' => {
                sink.emit(TerminalCommand::CsiRestoreCursorPosition);
            }
            b'g' => {
                let ps = self.params.first().copied().unwrap_or(0);
                if ps == 0 {
                    sink.emit(TerminalCommand::CsiClearTabulation);
                } else {
                    sink.emit(TerminalCommand::CsiClearAllTabs);
                }
            }
            b'Y' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiCursorLineTabulationForward(n as u16));
            }
            b'Z' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiCursorBackwardTabulation(n as u16));
            }
            b't' => match self.params.len() {
                3 => {
                    let cmd = self.params.first().copied().unwrap_or(0);
                    if cmd == 8 {
                        let height = self.params.get(1).copied().unwrap_or(1).max(1).min(60) as u16;
                        let width = self.params.get(2).copied().unwrap_or(1).max(1).min(132) as u16;
                        sink.emit(TerminalCommand::CsiResizeTerminal(height, width));
                    } else {
                        sink.report_error(
                            ParseError::MalformedSequence {
                                description: "Unknown or malformed escape sequence",
                                sequence: Some(format!(
                                    "CSI {} ; {} ; {} t",
                                    cmd,
                                    self.params.get(1).unwrap_or(&0),
                                    self.params.get(2).unwrap_or(&0)
                                )),
                                context: Some(format!("Unknown CSI t command: {}", cmd)),
                            },
                            crate::ErrorLevel::Error,
                        );
                    }
                }
                4 => {
                    let fg_or_bg = self.params.first().copied().unwrap_or(0);
                    let r = self.params.get(1).copied().unwrap_or(0) as u8;
                    let g = self.params.get(2).copied().unwrap_or(0) as u8;
                    let b = self.params.get(3).copied().unwrap_or(0) as u8;
                    let color = Color::Rgb(r, g, b);
                    match fg_or_bg {
                        0 => sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(color))),
                        1 => sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(color))),
                        _ => sink.report_error(
                            ParseError::MalformedSequence {
                                description: "Unknown or malformed escape sequence",
                                sequence: Some(format!("CSI {} ; {} ; {} ; {} t", fg_or_bg, r, g, b)),
                                context: Some(format!("Invalid fg_or_bg parameter: {} (expected 0 or 1)", fg_or_bg)),
                            },
                            crate::ErrorLevel::Error,
                        ),
                    }
                }
                _ => {
                    sink.report_error(
                        ParseError::MalformedSequence {
                            description: "Unknown or malformed escape sequence",
                            sequence: Some(format!("CSI {:?} t", self.params)),
                            context: Some(format!("Invalid parameter count for CSI t: {}", self.params.len())),
                        },
                        crate::ErrorLevel::Error,
                    );
                }
            },
            b'~' => {
                let n = self.params.first().copied().unwrap_or(0);
                if let Ok(key) = crate::SpecialKey::try_from(n) {
                    sink.emit(TerminalCommand::CsiSpecialKey(key));
                } else {
                    sink.report_error(
                        ParseError::InvalidParameter {
                            command: "CSI Special Key",
                            value: format!("{}", n).to_string(),
                            expected: Some("A valid sepecial key, report if one is missing".to_string()),
                        },
                        crate::ErrorLevel::Error,
                    );
                }
            }
            b'c' => {
                sink.request(TerminalRequest::DeviceAttributes);
            }
            b'n' => {
                let n = self.params.first().copied().unwrap_or(0);
                match n {
                    5 => sink.request(TerminalRequest::DeviceStatusReport),
                    6 => sink.request(TerminalRequest::CursorPositionReport),
                    255 => sink.request(TerminalRequest::ScreenSizeReport),
                    _ => {
                        sink.report_error(
                            ParseError::InvalidParameter {
                                command: "CsiDeviceStatusReport",
                                value: format!("{}", n).to_string(),
                                expected: None,
                            },
                            crate::ErrorLevel::Error,
                        );
                    }
                }
            }
            b'h' => {
                for &param in &self.params {
                    match AnsiMode::from_u16(param) {
                        Some(mode) => {
                            sink.emit(TerminalCommand::CsiSetMode(mode, true));
                        }
                        None => {
                            sink.report_error(
                                ParseError::InvalidParameter {
                                    command: "CsiSetMode",
                                    value: format!("{}", param).to_string(),
                                    expected: None,
                                },
                                crate::ErrorLevel::Error,
                            );
                        }
                    }
                }
            }
            b'l' => {
                for &param in &self.params {
                    match AnsiMode::from_u16(param) {
                        Some(mode) => {
                            sink.emit(TerminalCommand::CsiSetMode(mode, false));
                        }
                        None => {
                            sink.report_error(
                                ParseError::InvalidParameter {
                                    command: "CsiSetMode",
                                    value: format!("{}", param).to_string(),
                                    expected: None,
                                },
                                crate::ErrorLevel::Error,
                            );
                        }
                    }
                }
            }

            b'|' => {
                if self.music_option != music::MusicOption::Off {
                    if self.params.is_empty() {
                        self.state = ParserState::AnsiMusic;
                        self.dotted_note = false;
                        self.cur_music = Some(music::AnsiMusic::default());
                        self.music_state = MusicState::ParseMusicStyle;
                        return;
                    }
                }
            }
            _ => {
                sink.report_error(
                    ParseError::MalformedSequence {
                        description: "Unknown or malformed escape sequence",
                        sequence: Some(format!(
                            "CSI {:?} {}",
                            self.params,
                            if final_byte.is_ascii_graphic() {
                                format!("'{}'", final_byte as char)
                            } else {
                                format!("0x{:02X}", final_byte)
                            }
                        )),
                        context: Some("Unrecognized CSI command (not DEC private mode)".to_string()),
                    },
                    crate::ErrorLevel::Error,
                );
            }
        }
    }

    #[inline(always)]
    fn handle_dec_private_csi_final(&mut self, final_byte: u8, sink: &mut dyn CommandSink) {
        match final_byte {
            b'h' | b'l' => {
                let enabled = final_byte == b'h';
                for &param in &self.params {
                    match DecMode::from_u16(param) {
                        Some(mode) => {
                            sink.emit(TerminalCommand::CsiDecSetMode(mode, enabled));
                        }
                        None => {
                            sink.report_error(
                                ParseError::InvalidParameter {
                                    command: "CsiDecSetMode",
                                    value: format!("{}", param).to_string(),
                                    expected: None,
                                },
                                crate::ErrorLevel::Error,
                            );
                        }
                    }
                }
            }
            b'n' => {
                // DEC Private Device Status Report
                if self.params.len() == 1 {
                    match self.params.first() {
                        Some(62) => {
                            // DSR—Macro Space Report
                            sink.request(TerminalRequest::MacroSpaceReport);
                        }
                        Some(63) => {
                            // Memory Checksum Report (DECCKSR) - needs 2 params
                            sink.report_error(
                                ParseError::InvalidParameter {
                                    command: "DECCKSR",
                                    value: format!("{}", 63).to_string(),
                                    expected: None,
                                },
                                crate::ErrorLevel::Error,
                            );
                        }
                        _ => {
                            sink.report_error(
                                ParseError::InvalidParameter {
                                    command: "DEC DSR",
                                    value: self.params[0].to_string(),
                                    expected: None,
                                },
                                crate::ErrorLevel::Error,
                            );
                        }
                    }
                } else if self.params.len() == 2 && self.params[0] == 63 {
                    // Memory Checksum Report (DECCKSR) with 2 params
                    // Calculate checksum from all macros (0-63)
                    let pid = self.params[1];
                    let mut sum: u32 = 0;
                    for i in 0..64 {
                        if let Some(m) = self.macros.get(&i) {
                            for b in m {
                                sum = sum.wrapping_add(*b as u32);
                            }
                        }
                    }
                    let checksum: u16 = (sum & 0xFFFF) as u16;
                    sink.request(TerminalRequest::MemoryChecksumReport(pid, checksum));
                } else {
                    sink.report_error(
                        ParseError::MalformedSequence {
                            description: "Invalid parameter count for DEC DSR",
                            sequence: Some(format!("CSI ? {:?} n", self.params)),
                            context: Some(format!("Expected 2 parameters for memory checksum, got {}", self.params.len())),
                        },
                        crate::ErrorLevel::Error,
                    );
                }
            }
            _ => {
                sink.report_error(
                    ParseError::MalformedSequence {
                        description: "Unknown or malformed escape sequence",
                        sequence: Some(format!(
                            "CSI ? {:?} {}",
                            self.params,
                            if final_byte.is_ascii_graphic() {
                                format!("'{}'", final_byte as char)
                            } else {
                                format!("0x{:02X}", final_byte)
                            }
                        )),
                        context: Some("Unrecognized DEC private mode sequence".to_string()),
                    },
                    crate::ErrorLevel::Error,
                );
            }
        }
    }
}
