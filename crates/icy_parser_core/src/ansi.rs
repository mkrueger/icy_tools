//! ANSI escape sequence parser
//!
//! Parses ANSI/VT100 escape sequences into structured commands.
//! Supports CSI (Control Sequence Introducer), ESC, and OSC sequences.

use base64::{Engine as _, engine::general_purpose};

use crate::{
    AnsiMode, Blink, CaretShape, Color, CommandParser, CommandSink, DecPrivateMode, DeviceControlString, DeviceStatusReport, Direction, EraseInDisplayMode,
    EraseInLineMode, Frame, Intensity, OperatingSystemCommand, ParseError, SgrAttribute, TerminalCommand, Underline,
};

/// SGR lookup table entry - describes what a particular SGR parameter code means
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SgrLutEntry {
    /// Regular SGR attribute that can be directly used
    SetAttribute(SgrAttribute),
    /// Extended foreground color (38) - needs sub-parameters (38;5;n or 38;2;r;g;b)
    ExtendedForeground,
    /// Extended background color (48) - needs sub-parameters (48;5;n or 48;2;r;g;b)
    ExtendedBackground,
    /// Undefined/unsupported SGR code
    Undefined,
}

// SGR lookup table: maps SGR parameter values (0-107) to their meaning
static SGR_LUT: [SgrLutEntry; 108] = [
    SgrLutEntry::SetAttribute(SgrAttribute::Reset),                        // 0
    SgrLutEntry::SetAttribute(SgrAttribute::Intensity(Intensity::Bold)),   // 1
    SgrLutEntry::SetAttribute(SgrAttribute::Intensity(Intensity::Faint)),  // 2
    SgrLutEntry::SetAttribute(SgrAttribute::Italic(true)),                 // 3
    SgrLutEntry::SetAttribute(SgrAttribute::Underline(Underline::Single)), // 4
    SgrLutEntry::SetAttribute(SgrAttribute::Blink(Blink::Slow)),           // 5
    SgrLutEntry::SetAttribute(SgrAttribute::Blink(Blink::Rapid)),          // 6
    SgrLutEntry::SetAttribute(SgrAttribute::Inverse(true)),                // 7
    SgrLutEntry::SetAttribute(SgrAttribute::Concealed(true)),              // 8
    SgrLutEntry::SetAttribute(SgrAttribute::CrossedOut(true)),             // 9
    SgrLutEntry::SetAttribute(SgrAttribute::Font(0)),                      // 10
    SgrLutEntry::SetAttribute(SgrAttribute::Font(1)),                      // 11
    SgrLutEntry::SetAttribute(SgrAttribute::Font(2)),                      // 12
    SgrLutEntry::SetAttribute(SgrAttribute::Font(3)),                      // 13
    SgrLutEntry::SetAttribute(SgrAttribute::Font(4)),                      // 14
    SgrLutEntry::SetAttribute(SgrAttribute::Font(5)),                      // 15
    SgrLutEntry::SetAttribute(SgrAttribute::Font(6)),                      // 16
    SgrLutEntry::SetAttribute(SgrAttribute::Font(7)),                      // 17
    SgrLutEntry::SetAttribute(SgrAttribute::Font(8)),                      // 18
    SgrLutEntry::SetAttribute(SgrAttribute::Font(9)),                      // 19
    SgrLutEntry::SetAttribute(SgrAttribute::Fraktur),                      // 20
    SgrLutEntry::SetAttribute(SgrAttribute::Underline(Underline::Double)), // 21
    SgrLutEntry::SetAttribute(SgrAttribute::Intensity(Intensity::Normal)), // 22
    SgrLutEntry::SetAttribute(SgrAttribute::Italic(false)),                // 23
    SgrLutEntry::SetAttribute(SgrAttribute::Underline(Underline::Off)),    // 24
    SgrLutEntry::SetAttribute(SgrAttribute::Blink(Blink::Off)),            // 25
    SgrLutEntry::Undefined,                                                // 26 - proportional spacing (rarely supported)
    SgrLutEntry::SetAttribute(SgrAttribute::Inverse(false)),               // 27
    SgrLutEntry::SetAttribute(SgrAttribute::Concealed(false)),             // 28
    SgrLutEntry::SetAttribute(SgrAttribute::CrossedOut(false)),            // 29
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(0))),   // 30 - Black
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(1))),   // 31 - Red
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(2))),   // 32 - Green
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(3))),   // 33 - Yellow
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(4))),   // 34 - Blue
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(5))),   // 35 - Magenta
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(6))),   // 36 - Cyan
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(7))),   // 37 - White
    SgrLutEntry::ExtendedForeground,                                       // 38 - extended foreground (needs sub-params)
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Default)),   // 39
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(0))),   // 40 - Black
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(1))),   // 41 - Red
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(2))),   // 42 - Green
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(3))),   // 43 - Yellow
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(4))),   // 44 - Blue
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(5))),   // 45 - Magenta
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(6))),   // 46 - Cyan
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(7))),   // 47 - White
    SgrLutEntry::ExtendedBackground,                                       // 48 - extended background (needs sub-params)
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Default)),   // 49
    SgrLutEntry::Undefined,                                                // 50 - disable proportional spacing
    SgrLutEntry::SetAttribute(SgrAttribute::Frame(Frame::Framed)),         // 51
    SgrLutEntry::SetAttribute(SgrAttribute::Frame(Frame::Encircled)),      // 52
    SgrLutEntry::SetAttribute(SgrAttribute::Overlined(true)),              // 53
    SgrLutEntry::SetAttribute(SgrAttribute::Frame(Frame::Off)),            // 54
    SgrLutEntry::SetAttribute(SgrAttribute::Overlined(false)),             // 55
    SgrLutEntry::Undefined,                                                // 56 - reserved
    SgrLutEntry::Undefined,                                                // 57 - reserved
    SgrLutEntry::Undefined,                                                // 58 - underline color (rarely supported)
    SgrLutEntry::Undefined,                                                // 59 - default underline color
    SgrLutEntry::SetAttribute(SgrAttribute::IdeogramUnderline),            // 60
    SgrLutEntry::SetAttribute(SgrAttribute::IdeogramDoubleUnderline),      // 61
    SgrLutEntry::SetAttribute(SgrAttribute::IdeogramOverline),             // 62
    SgrLutEntry::SetAttribute(SgrAttribute::IdeogramDoubleOverline),       // 63
    SgrLutEntry::SetAttribute(SgrAttribute::IdeogramStress),               // 64
    SgrLutEntry::SetAttribute(SgrAttribute::IdeogramAttributesOff),        // 65
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined, // 66-70
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined, // 71-75
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined, // 76-80
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined, // 81-85
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,                                               // 86-89
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(8))),  // 90 - Bright Black
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(9))),  // 91 - Bright Red
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(10))), // 92 - Bright Green
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(11))), // 93 - Bright Yellow
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(12))), // 94 - Bright Blue
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(13))), // 95 - Bright Magenta
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(14))), // 96 - Bright Cyan
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(15))), // 97 - Bright White
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,                                               // 98-99
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(8))),  // 100 - Bright Black
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(9))),  // 101 - Bright Red
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(10))), // 102 - Bright Green
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(11))), // 103 - Bright Yellow
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(12))), // 104 - Bright Blue
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(13))), // 105 - Bright Magenta
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(14))), // 106 - Bright Cyan
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(15))), // 107 - Bright White
];

#[derive(Default)]
pub struct AnsiParser {
    state: ParserState,
    params: Vec<u16>,
    intermediate_bytes: Vec<u8>,
    parse_buffer: Vec<u8>,
    macros: std::collections::HashMap<usize, Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum ParserState {
    Ground = 0,
    Escape = 1,
    CsiEntry = 2,
    CsiParam = 3,
    CsiIntermediate = 4,
    OscString = 5,
    DcsString = 6,
    DcsEscape = 7,
    ApsString = 8,
    ApsEscape = 9,
}

impl Default for ParserState {
    fn default() -> Self {
        ParserState::Ground
    }
}

impl AnsiParser {
    pub fn new() -> Self {
        Self::default()
    }

    fn reset(&mut self) {
        self.params.clear();
        self.intermediate_bytes.clear();
        self.state = ParserState::Ground;
    }

    fn parse_dcs(&mut self, sink: &mut dyn CommandSink) {
        // Check for CTerm custom font: "CTerm:Font:{slot}:{base64_data}"
        if self.parse_buffer.starts_with(b"CTerm:Font:") {
            let start_index = b"CTerm:Font:".len();
            if let Some(colon_pos) = self.parse_buffer[start_index..].iter().position(|&b| b == b':') {
                let slot_end = start_index + colon_pos;
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
                                sink.report_error(ParseError::MalformedSequence {
                                    description: "Invalid base64 in DCS font data",
                                });
                                return;
                            }
                        }
                    }
                }
            }
            // If parsing failed, report error
            sink.report_error(ParseError::MalformedSequence {
                description: "Unknown or malformed DCS sequence",
            });
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
                    self.params.push(last * 10 + (byte - b'0') as u16);
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
            let vertical_scale = match self.params.first() {
                Some(0 | 1 | 5 | 6) | None => 2,
                Some(2) => 5,
                Some(3 | 4) => 3,
                _ => 1,
            };

            // Get background color (param 1: 1 = transparent, otherwise opaque black)
            let bg_color = if self.params.get(1) == Some(&1) {
                (0, 0, 0) // Transparent
            } else {
                (0, 0, 0) // Opaque black
            };

            sink.device_control(DeviceControlString::Sixel(vertical_scale, bg_color, &self.parse_buffer[i + 1..]));
            return;
        }

        // Unknown DCS - emit as Unknown
        sink.report_error(ParseError::MalformedSequence {
            description: "Unknown or malformed escape sequence",
        });
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
        let mut repeat_count = 0;
        let mut in_repeat = false;
        let mut repeat_start = 0;

        while i < data.len() {
            if data[i] == b'!' {
                // Repeat sequence: !{count};{hex_data};
                i += 1;
                repeat_count = 0;
                while i < data.len() && data[i].is_ascii_digit() {
                    repeat_count = repeat_count * 10 + (data[i] - b'0') as usize;
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
            // Recursively parse the macro content
            self.parse(&macro_data, sink);
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
                ParserState::Ground => {
                    match byte {
                        0x1B => {
                            // ESC - start escape sequence
                            if i > printable_start {
                                sink.print(&input[printable_start..i]);
                            }
                            self.state = ParserState::Escape;
                            i += 1;
                            printable_start = i;
                        }
                        0x07 => {
                            // BEL
                            if i > printable_start {
                                sink.print(&input[printable_start..i]);
                            }
                            sink.emit(TerminalCommand::Bell);
                            i += 1;
                            printable_start = i;
                        }
                        0x08 => {
                            // BS
                            if i > printable_start {
                                sink.print(&input[printable_start..i]);
                            }
                            sink.emit(TerminalCommand::Backspace);
                            i += 1;
                            printable_start = i;
                        }
                        0x09 => {
                            // HT
                            if i > printable_start {
                                sink.print(&input[printable_start..i]);
                            }
                            sink.emit(TerminalCommand::Tab);
                            i += 1;
                            printable_start = i;
                        }
                        0x0A => {
                            // LF
                            if i > printable_start {
                                sink.print(&input[printable_start..i]);
                            }
                            sink.emit(TerminalCommand::LineFeed);
                            i += 1;
                            printable_start = i;
                        }
                        0x0C => {
                            // FF
                            if i > printable_start {
                                sink.print(&input[printable_start..i]);
                            }
                            sink.emit(TerminalCommand::FormFeed);
                            i += 1;
                            printable_start = i;
                        }
                        0x0D => {
                            // CR
                            if i > printable_start {
                                sink.print(&input[printable_start..i]);
                            }
                            sink.emit(TerminalCommand::CarriageReturn);
                            i += 1;
                            printable_start = i;
                        }
                        0x7F => {
                            // DEL
                            if i > printable_start {
                                sink.print(&input[printable_start..i]);
                            }
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
                            self.intermediate_bytes.clear();
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
                        _ => {
                            // Unknown escape sequence
                            sink.report_error(ParseError::MalformedSequence {
                                description: "Unknown or malformed escape sequence",
                            });
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
                        b'?' | b'>' | b'!' | b'=' => {
                            // Private marker
                            self.intermediate_bytes.push(byte);
                            i += 1;
                        }
                        b'@'..=b'~' => {
                            // Final byte without parameters
                            self.emit_csi_sequence(byte, sink);
                            self.reset();
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
                                *last = last.saturating_mul(10).saturating_add(digit);
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
                            self.emit_csi_sequence(byte, sink);
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        b' '..=b'/' => {
                            // Intermediate byte
                            self.intermediate_bytes.push(byte);
                            self.state = ParserState::CsiIntermediate;
                            i += 1;
                        }
                        b'@'..=b'~' => {
                            // Final byte
                            self.emit_csi_sequence(byte, sink);
                            self.reset();
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

                ParserState::CsiIntermediate => {
                    match byte {
                        b'@'..=b'~' => {
                            // Final byte
                            self.emit_csi_sequence(byte, sink);
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        b' '..=b'/' => {
                            // Another intermediate byte
                            self.intermediate_bytes.push(byte);
                            i += 1;
                        }
                        _ => {
                            // Invalid character
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                    }
                }

                ParserState::OscString => {
                    match byte {
                        0x07 => {
                            // BEL - End of OSC
                            self.emit_osc_sequence(sink);
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        0x1B => {
                            // ESC - might be ST (String Terminator: ESC \)
                            if i + 1 < input.len() && input[i + 1] == b'\\' {
                                // ST - String Terminator
                                self.emit_osc_sequence(sink);
                                self.reset();
                                i += 2; // Skip both ESC and \
                                printable_start = i;
                            } else {
                                // Collect ESC as part of OSC string
                                self.parse_buffer.push(byte);
                                i += 1;
                            }
                        }
                        _ => {
                            // Collect byte
                            self.parse_buffer.push(byte);
                            i += 1;
                        }
                    }
                }

                ParserState::DcsString => {
                    match byte {
                        0x1B => {
                            // ESC - might be ST (String Terminator: ESC \)
                            self.state = ParserState::DcsEscape;
                            i += 1;
                        }
                        _ => {
                            // Collect byte
                            self.parse_buffer.push(byte);
                            i += 1;
                        }
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
                    } else {
                        // False alarm - ESC was part of DCS data
                        self.parse_buffer.push(0x1B);
                        self.parse_buffer.push(byte);
                        self.state = ParserState::DcsString;
                        i += 1;
                    }
                }

                ParserState::ApsString => {
                    match byte {
                        0x1B => {
                            // ESC - might be ST (String Terminator: ESC \)
                            self.state = ParserState::ApsEscape;
                            i += 1;
                        }
                        _ => {
                            // Collect byte
                            self.parse_buffer.push(byte);
                            i += 1;
                        }
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
                        self.parse_buffer.push(0x1B);
                        self.parse_buffer.push(byte);
                        self.state = ParserState::ApsString;
                        i += 1;
                    }
                }
            }
        }

        // Emit any remaining printable bytes
        if i > printable_start && self.state == ParserState::Ground {
            sink.print(&input[printable_start..i]);
        }
    }

    fn flush(&mut self, _sink: &mut dyn CommandSink) {
        // Reset parser state on flush
        self.reset();
    }
}

impl AnsiParser {
    #[inline(always)]
    fn emit_csi_sequence(&mut self, final_byte: u8, sink: &mut dyn CommandSink) {
        // Check for intermediate byte prefixes
        let is_dec_private = self.intermediate_bytes.first() == Some(&b'?');
        let is_asterisk = self.intermediate_bytes.first() == Some(&b'*');
        let is_dollar = self.intermediate_bytes.first() == Some(&b'$');
        let is_space = self.intermediate_bytes.first() == Some(&b' ');

        // Handle sequences with intermediate bytes first
        if is_asterisk {
            // CSI * sequences
            match final_byte {
                b'z' => {
                    // Invoke Macro - execute it internally
                    let n = self.params.first().copied().unwrap_or(0) as usize;
                    self.invoke_macro(n, sink);
                    return;
                }
                b'r' => {
                    // Select Communication Speed
                    let ps1 = self.params.first().copied().unwrap_or(0);
                    let ps2 = self.params.get(1).copied().unwrap_or(0);
                    sink.emit(TerminalCommand::CsiSelectCommunicationSpeed(ps1 as u16, ps2 as u16));
                    return;
                }
                b'y' => {
                    // Request Checksum of Rectangular Area: ESC[{Pid};{Ppage};{Pt};{Pl};{Pb};{Pr}*y
                    // Pid is ignored, extract ppage, pt, pl, pb, pr
                    let _pid = self.params.first().copied().unwrap_or(0);
                    let ppage = self.params.get(1).copied().unwrap_or(0) as u8;
                    let pt = self.params.get(2).copied().unwrap_or(0) as u16;
                    let pl = self.params.get(3).copied().unwrap_or(0) as u16;
                    let pb = self.params.get(4).copied().unwrap_or(0) as u16;
                    let pr = self.params.get(5).copied().unwrap_or(0) as u16;
                    sink.emit(TerminalCommand::CsiRequestChecksumRectangularArea(ppage, pt, pl, pb, pr));
                    return;
                }
                _ => {
                    sink.report_error(ParseError::MalformedSequence {
                        description: "Unknown or malformed escape sequence",
                    });
                    return;
                }
            }
        }

        if is_dollar {
            // CSI $ sequences
            match final_byte {
                b'w' => {
                    // DECRQTSR - Request Tab Stop Report
                    let ps = self.params.first().copied().unwrap_or(0);
                    sink.emit(TerminalCommand::CsiRequestTabStopReport(ps as u16));
                    return;
                }
                b'x' => {
                    // DECFRA - Fill Rectangular Area
                    let pchar = self.params.first().copied().unwrap_or(0);
                    let pt = self.params.get(1).copied().unwrap_or(1);
                    let pl = self.params.get(2).copied().unwrap_or(1);
                    let pb = self.params.get(3).copied().unwrap_or(1);
                    let pr = self.params.get(4).copied().unwrap_or(1);
                    sink.emit(TerminalCommand::CsiFillRectangularArea(
                        pchar as u16,
                        pt as u16,
                        pl as u16,
                        pb as u16,
                        pr as u16,
                    ));
                    return;
                }
                b'z' => {
                    // DECERA - Erase Rectangular Area
                    let pt = self.params.first().copied().unwrap_or(1);
                    let pl = self.params.get(1).copied().unwrap_or(1);
                    let pb = self.params.get(2).copied().unwrap_or(1);
                    let pr = self.params.get(3).copied().unwrap_or(1);
                    sink.emit(TerminalCommand::CsiEraseRectangularArea(pt as u16, pl as u16, pb as u16, pr as u16));
                    return;
                }
                b'{' => {
                    // DECSERA - Selective Erase Rectangular Area
                    let pt = self.params.first().copied().unwrap_or(1);
                    let pl = self.params.get(1).copied().unwrap_or(1);
                    let pb = self.params.get(2).copied().unwrap_or(1);
                    let pr = self.params.get(3).copied().unwrap_or(1);
                    sink.emit(TerminalCommand::CsiSelectiveEraseRectangularArea(pt as u16, pl as u16, pb as u16, pr as u16));
                    return;
                }
                _ => {
                    sink.report_error(ParseError::MalformedSequence {
                        description: "Unknown or malformed escape sequence",
                    });
                    return;
                }
            }
        }

        if is_space {
            // CSI SP sequences
            match final_byte {
                b'q' => {
                    // DECSCUSR - Set Caret Style
                    // Ps = 0 or 1 -> blinking block, 2 -> steady block
                    // Ps = 3 -> blinking underline, 4 -> steady underline
                    // Ps = 5 -> blinking bar, 6 -> steady bar
                    let style = self.params.first().copied().unwrap_or(1); // Default is blinking block
                    let (blinking, shape) = match style {
                        0 | 1 => (true, CaretShape::Block),
                        2 => (false, CaretShape::Block),
                        3 => (true, CaretShape::Underline),
                        4 => (false, CaretShape::Underline),
                        5 => (true, CaretShape::Bar),
                        6 => (false, CaretShape::Bar),
                        _ => (true, CaretShape::Block), // Invalid: default to blinking block
                    };
                    sink.emit(TerminalCommand::CsiSetCaretStyle(blinking, shape));
                    return;
                }
                b'D' => {
                    // Font Selection
                    let ps1 = self.params.first().copied().unwrap_or(0);
                    let ps2 = self.params.get(1).copied().unwrap_or(0);
                    sink.emit(TerminalCommand::CsiFontSelection(ps1 as u16, ps2 as u16));
                    return;
                }
                b'A' => {
                    // Scroll Right
                    let n = self.params.first().copied().unwrap_or(1);
                    sink.emit(TerminalCommand::CsiScroll(Direction::Right, n as u16));
                    return;
                }
                b'@' => {
                    // Scroll Left
                    let n = self.params.first().copied().unwrap_or(1);
                    sink.emit(TerminalCommand::CsiScroll(Direction::Left, n as u16));
                    return;
                }
                b'd' => {
                    // Tabulation Clear
                    let ps = self.params.first().copied().unwrap_or(0);
                    if ps == 0 {
                        sink.emit(TerminalCommand::CsiClearTabulation);
                    } else {
                        sink.emit(TerminalCommand::CsiClearAllTabs);
                    }
                    return;
                }
                _ => {
                    sink.report_error(ParseError::MalformedSequence {
                        description: "Unknown or malformed escape sequence",
                    });
                    return;
                }
            }
        }

        match final_byte {
            b'A' => {
                // CUU - Cursor Up
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Up, n as u16));
            }
            b'B' => {
                // CUD - Cursor Down
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Down, n as u16));
            }
            b'C' => {
                // CUF - Cursor Forward
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Right, n as u16));
            }
            b'D' => {
                // CUB - Cursor Back
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Left, n as u16));
            }
            b'E' => {
                // CNL - Cursor Next Line
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiCursorNextLine(n as u16));
            }
            b'F' => {
                // CPL - Cursor Previous Line
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiCursorPreviousLine(n as u16));
            }
            b'G' => {
                // CHA - Cursor Horizontal Absolute
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiCursorHorizontalAbsolute(n as u16));
            }
            b'H' | b'f' => {
                // CUP - Cursor Position
                let row = self.params.first().copied().unwrap_or(1);
                let col = self.params.get(1).copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiCursorPosition(row as u16, col as u16));
            }
            b'j' => {
                // HPB - Character Position Backward (alias for CUB)
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Left, n as u16));
            }
            b'k' => {
                // VPB - Line Position Backward (alias for CUU)
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Up, n as u16));
            }
            b'd' => {
                // VPA - Line Position Absolute
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiLinePositionAbsolute(n as u16));
            }
            b'e' => {
                // VPR - Line Position Forward
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiLinePositionForward(n as u16));
            }
            b'a' => {
                // HPR - Character Position Forward
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiCharacterPositionForward(n as u16));
            }
            b'\'' => {
                // HPA - Horizontal Position Absolute
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiHorizontalPositionAbsolute(n as u16));
            }
            b'J' => {
                // ED - Erase in Display
                let n = self.params.first().copied().unwrap_or(0);
                match EraseInDisplayMode::from_u16(n) {
                    Some(mode) => sink.emit(TerminalCommand::CsiEraseInDisplay(mode)),
                    None => {
                        sink.report_error(ParseError::InvalidParameter {
                            command: "CsiEraseInDisplay",
                            value: n,
                        });
                        // Use default mode on error
                        sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::CursorToEnd));
                    }
                }
            }
            b'K' => {
                // EL - Erase in Line
                let n = self.params.first().copied().unwrap_or(0);
                match EraseInLineMode::from_u16(n) {
                    Some(mode) => sink.emit(TerminalCommand::CsiEraseInLine(mode)),
                    None => {
                        sink.report_error(ParseError::InvalidParameter {
                            command: "CsiEraseInLine",
                            value: n,
                        });
                        // Use default mode on error
                        sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::CursorToEnd));
                    }
                }
            }
            b'S' => {
                // SU - Scroll Up
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiScroll(Direction::Up, n as u16));
            }
            b'T' => {
                // SD - Scroll Down
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiScroll(Direction::Down, n as u16));
            }
            b'm' => {
                // SGR - Select Graphic Rendition
                self.parse_sgr(sink);
            }
            b'r' => {
                // DECSTBM - Set Scrolling Region
                let top = self.params.first().copied().unwrap_or(1);
                let bottom = self.params.get(1).copied().unwrap_or(0);
                sink.emit(TerminalCommand::CsiSetScrollingRegion(top as u16, bottom as u16));
            }
            b'@' => {
                // ICH - Insert Character
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiInsertCharacter(n as u16));
            }
            b'P' => {
                // DCH - Delete Character
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiDeleteCharacter(n as u16));
            }
            b'X' => {
                // ECH - Erase Character
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiEraseCharacter(n as u16));
            }
            b'L' => {
                // IL - Insert Line
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiInsertLine(n as u16));
            }
            b'M' => {
                // DL - Delete Line
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiDeleteLine(n as u16));
            }
            b'b' => {
                // REP - Repeat preceding character
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiRepeatPrecedingCharacter(n as u16));
            }
            b's' => {
                // SCOSC - Save Cursor Position
                sink.emit(TerminalCommand::CsiSaveCursorPosition);
            }
            b'u' => {
                // SCORC - Restore Cursor Position
                sink.emit(TerminalCommand::CsiRestoreCursorPosition);
            }
            b'g' => {
                // TBC - Tabulation Clear
                let ps = self.params.first().copied().unwrap_or(0);
                if ps == 0 {
                    sink.emit(TerminalCommand::CsiClearTabulation);
                } else {
                    sink.emit(TerminalCommand::CsiClearAllTabs);
                }
            }
            b'Y' => {
                // CVT - Cursor Line Tabulation (forward to next tab)
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiCursorLineTabulationForward(n as u16));
            }
            b'Z' => {
                // CBT - Cursor Backward Tabulation
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiCursorBackwardTabulation(n as u16));
            }
            b't' => {
                // Window Manipulation / 24-bit color selection
                match self.params.len() {
                    3 => {
                        // Window manipulation: ESC[8;{height};{width}t
                        let cmd = self.params.first().copied().unwrap_or(0);
                        if cmd == 8 {
                            let height = self.params.get(1).copied().unwrap_or(1).max(1).min(60) as u16;
                            let width = self.params.get(2).copied().unwrap_or(1).max(1).min(132) as u16;
                            sink.emit(TerminalCommand::CsiResizeTerminal(height, width));
                        } else {
                            sink.report_error(ParseError::MalformedSequence {
                                description: "Unknown or malformed escape sequence",
                            });
                        }
                    }
                    4 => {
                        // 24-bit color selection: ESC[{fg/bg};{r};{g};{b}t
                        let fg_or_bg = self.params.first().copied().unwrap_or(0);
                        let r = self.params.get(1).copied().unwrap_or(0) as u8;
                        let g = self.params.get(2).copied().unwrap_or(0) as u8;
                        let b = self.params.get(3).copied().unwrap_or(0) as u8;
                        let color = Color::Rgb(r, g, b);
                        match fg_or_bg {
                            0 => sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(color))),
                            1 => sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(color))),
                            _ => sink.report_error(ParseError::MalformedSequence {
                                description: "Unknown or malformed escape sequence",
                            }),
                        }
                    }
                    _ => {
                        sink.report_error(ParseError::MalformedSequence {
                            description: "Unknown or malformed escape sequence",
                        });
                    }
                }
            }
            b'~' => {
                // Special keys (Home, Insert, Delete, End, PageUp, PageDown)
                let n = self.params.first().copied().unwrap_or(0);
                sink.emit(TerminalCommand::CsiSpecialKey(n as u16));
            }
            b'c' => {
                // DA - Device Attributes
                sink.emit(TerminalCommand::CsiDeviceAttributes);
            }
            b'n' => {
                // DSR - Device Status Report
                let n = self.params.first().copied().unwrap_or(0);
                match DeviceStatusReport::from_u16(n) {
                    Some(report) => sink.emit(TerminalCommand::CsiDeviceStatusReport(report)),
                    None => {
                        sink.report_error(ParseError::InvalidParameter {
                            command: "CsiDeviceStatusReport",
                            value: n,
                        });
                    }
                }
            }
            b'h' => {
                if is_dec_private {
                    // DECSET - DEC Private Mode Set - emit each mode individually
                    for &param in &self.params {
                        match DecPrivateMode::from_u16(param) {
                            Some(mode) => {
                                sink.emit(TerminalCommand::CsiDecPrivateModeSet(mode));
                            }
                            None => {
                                sink.report_error(ParseError::InvalidParameter {
                                    command: "CsiDecPrivateModeSet",
                                    value: param,
                                });
                            }
                        }
                    }
                } else {
                    // SM - Set Mode - emit each mode individually
                    for &param in &self.params {
                        match AnsiMode::from_u16(param) {
                            Some(mode) => {
                                sink.emit(TerminalCommand::CsiSetMode(mode));
                            }
                            None => {
                                sink.report_error(ParseError::InvalidParameter {
                                    command: "CsiSetMode",
                                    value: param,
                                });
                            }
                        }
                    }
                }
            }
            b'l' => {
                if is_dec_private {
                    // DECRST - DEC Private Mode Reset - emit each mode individually
                    for &param in &self.params {
                        match DecPrivateMode::from_u16(param) {
                            Some(mode) => {
                                sink.emit(TerminalCommand::CsiDecPrivateModeReset(mode));
                            }
                            None => {
                                sink.report_error(ParseError::InvalidParameter {
                                    command: "CsiDecPrivateModeReset",
                                    value: param,
                                });
                            }
                        }
                    }
                } else {
                    // RM - Reset Mode - emit each mode individually
                    for &param in &self.params {
                        match AnsiMode::from_u16(param) {
                            Some(mode) => {
                                sink.emit(TerminalCommand::CsiResetMode(mode));
                            }
                            None => {
                                sink.report_error(ParseError::InvalidParameter {
                                    command: "CsiResetMode",
                                    value: param,
                                });
                            }
                        }
                    }
                }
            }
            _ => {
                // Unknown CSI sequence
                sink.report_error(ParseError::MalformedSequence {
                    description: "Unknown or malformed escape sequence",
                });
            }
        }
    }

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
                            sink.operating_system_command(OperatingSystemCommand::SetTitle(pt_bytes));
                        }
                        1 => {
                            // Set icon name
                            sink.operating_system_command(OperatingSystemCommand::SetIconName(pt_bytes));
                        }
                        2 => {
                            // Set window title
                            sink.operating_system_command(OperatingSystemCommand::SetWindowTitle(pt_bytes));
                        }
                        8 => {
                            // Hyperlink: OSC 8 ; params ; URI BEL
                            if let Some(uri_pos) = pt_bytes.iter().position(|&b| b == b';') {
                                let params = &pt_bytes[..uri_pos];
                                let uri = &pt_bytes[uri_pos + 1..];
                                sink.operating_system_command(OperatingSystemCommand::Hyperlink { params, uri });
                            }
                        }
                        _ => {
                            // Unknown OSC command
                            sink.report_error(ParseError::MalformedSequence {
                                description: "Unknown or malformed escape sequence",
                            });
                        }
                    }
                    return;
                }
            }
        }

        // Malformed OSC
        sink.report_error(ParseError::MalformedSequence {
            description: "Unknown or malformed escape sequence",
        });
    }

    #[inline(always)]
    fn parse_sgr(&mut self, sink: &mut dyn CommandSink) {
        let params: &[u16] = if self.params.is_empty() { &[0u16] } else { &self.params };

        let mut i = 0;
        while i < params.len() {
            let param = params[i];

            if param < 108 {
                // Use lookup table for standard SGR codes
                match SGR_LUT[param as usize] {
                    SgrLutEntry::SetAttribute(attr) => {
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(attr));
                    }
                    SgrLutEntry::ExtendedForeground => {
                        // Extended foreground color: ESC[38;5;nm or ESC[38;2;r;g;bm
                        if i + 2 < params.len() {
                            match params[i + 1] {
                                5 => {
                                    // 256-color palette: ESC[38;5;nm
                                    let color = params[i + 2] as u8;
                                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Extended(color))));
                                    i += 2; // Skip the 5 and color parameters
                                }
                                2 => {
                                    // RGB color: ESC[38;2;r;g;bm
                                    if i + 4 < params.len() {
                                        let r = params[i + 2] as u8;
                                        let g = params[i + 3] as u8;
                                        let b = params[i + 4] as u8;
                                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Rgb(r, g, b))));
                                        i += 4; // Skip the 2, r, g, b parameters
                                    } else {
                                        sink.report_error(ParseError::IncompleteSequence);
                                    }
                                }
                                _ => {
                                    sink.report_error(ParseError::InvalidParameter {
                                        command: "CsiSelectGraphicRendition",
                                        value: params[i + 1],
                                    });
                                }
                            }
                        } else {
                            sink.report_error(ParseError::IncompleteSequence);
                        }
                    }
                    SgrLutEntry::ExtendedBackground => {
                        // Extended background color: ESC[48;5;nm or ESC[48;2;r;g;bm
                        if i + 2 < params.len() {
                            match params[i + 1] {
                                5 => {
                                    // 256-color palette: ESC[48;5;nm
                                    let color = params[i + 2] as u8;
                                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Extended(color))));
                                    i += 2; // Skip the 5 and color parameters
                                }
                                2 => {
                                    // RGB color: ESC[48;2;r;g;bm
                                    if i + 4 < params.len() {
                                        let r = params[i + 2] as u8;
                                        let g = params[i + 3] as u8;
                                        let b = params[i + 4] as u8;
                                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Rgb(r, g, b))));
                                        i += 4; // Skip the 2, r, g, b parameters
                                    } else {
                                        sink.report_error(ParseError::IncompleteSequence);
                                    }
                                }
                                _ => {
                                    sink.report_error(ParseError::InvalidParameter {
                                        command: "CsiSelectGraphicRendition",
                                        value: params[i + 1],
                                    });
                                }
                            }
                        } else {
                            sink.report_error(ParseError::IncompleteSequence);
                        }
                    }
                    SgrLutEntry::Undefined => {
                        sink.report_error(ParseError::InvalidParameter {
                            command: "CsiSelectGraphicRendition",
                            value: param,
                        });
                    }
                }
            } else {
                // Out of range
                sink.report_error(ParseError::InvalidParameter {
                    command: "CsiSelectGraphicRendition",
                    value: param,
                });
            }

            i += 1;
        }
    }
}
