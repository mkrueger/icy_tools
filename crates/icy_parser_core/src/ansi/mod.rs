//! ANSI escape sequence parser
//!
//! Parses ANSI/VT100 escape sequences into structured commands.
//! Supports CSI (Control Sequence Introducer), ESC, and OSC sequences.

use base64::{Engine as _, engine::general_purpose};
mod sgr;
use crate::{
    AnsiMode, CaretShape, Color, CommandParser, CommandSink, DecPrivateMode, DeviceControlString, DeviceStatusReport, Direction, EraseInDisplayMode,
    EraseInLineMode, OperatingSystemCommand, ParseError, SgrAttribute, TerminalCommand,
};

#[derive(Default)]
pub struct AnsiParser {
    state: ParserState,
    params: Vec<u16>,
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
    // CSI with specific intermediate bytes
    CsiDecPrivate = 10, // CSI ? ... (or CSI ... ?)
    CsiAsterisk = 12,   // CSI ... *
    CsiDollar = 14,     // CSI ... $
    CsiSpace = 16,      // CSI ... SP
    CsiGreater = 18,    // CSI > ...
    CsiExclaim = 20,    // CSI ! ...
    CsiEquals = 22,     // CSI = ...
    // Other states
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
                        b'?' => {
                            self.state = ParserState::CsiDecPrivate;
                            i += 1;
                        }
                        b'>' => {
                            self.state = ParserState::CsiGreater;
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
                            self.reset();
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
                        // Select Communication Speed
                        let ps1 = self.params.first().copied().unwrap_or(0);
                        let ps2 = self.params.get(1).copied().unwrap_or(0);
                        sink.emit(TerminalCommand::CsiSelectCommunicationSpeed(ps1 as u16, ps2 as u16));
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    b'y' => {
                        // Request Checksum of Rectangular Area: ESC[{Pid};{Ppage};{Pt};{Pl};{Pb};{Pr}*y
                        let _pid = self.params.first().copied().unwrap_or(0);
                        let ppage = self.params.get(1).copied().unwrap_or(0) as u8;
                        let pt = self.params.get(2).copied().unwrap_or(0) as u16;
                        let pl = self.params.get(3).copied().unwrap_or(0) as u16;
                        let pb = self.params.get(4).copied().unwrap_or(0) as u16;
                        let pr = self.params.get(5).copied().unwrap_or(0) as u16;
                        sink.emit(TerminalCommand::CsiRequestChecksumRectangularArea(ppage, pt, pl, pb, pr));
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    _ => {
                        sink.report_error(ParseError::MalformedSequence {
                            description: "Unknown or malformed escape sequence",
                        });
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
                        let ps = self.params.first().copied().unwrap_or(0);
                        sink.emit(TerminalCommand::CsiRequestTabStopReport(ps as u16));
                        self.reset();
                        i += 1;
                        printable_start = i;
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
                        sink.emit(TerminalCommand::CsiEraseRectangularArea(pt as u16, pl as u16, pb as u16, pr as u16));
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
                        sink.emit(TerminalCommand::CsiSelectiveEraseRectangularArea(pt as u16, pl as u16, pb as u16, pr as u16));
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    _ => {
                        sink.report_error(ParseError::MalformedSequence {
                            description: "Unknown or malformed escape sequence",
                        });
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
                        sink.emit(TerminalCommand::CsiFontSelection(ps1 as u16, ps2 as u16));
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
                    _ => {
                        sink.report_error(ParseError::MalformedSequence {
                            description: "Unknown or malformed escape sequence",
                        });
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                },

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
                    _ => {
                        // No specific commands implemented for CSI > sequences
                        sink.report_error(ParseError::MalformedSequence {
                            description: "Unsupported CSI > sequence",
                        });
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
                    _ => {
                        // No specific commands implemented for CSI ! sequences
                        sink.report_error(ParseError::MalformedSequence {
                            description: "Unsupported CSI ! sequence",
                        });
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
                    _ => {
                        // No specific commands implemented for CSI = sequences
                        sink.report_error(ParseError::MalformedSequence {
                            description: "Unsupported CSI = sequence",
                        });
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                },

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
    fn handle_csi_final(&mut self, final_byte: u8, sink: &mut dyn CommandSink) {
        match final_byte {
            b'A' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Up, n as u16));
            }
            b'B' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Down, n as u16));
            }
            b'C' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Right, n as u16));
            }
            b'D' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Left, n as u16));
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
            b'j' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Left, n as u16));
            }
            b'k' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Up, n as u16));
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
                        sink.report_error(ParseError::InvalidParameter {
                            command: "CsiEraseInDisplay",
                            value: n,
                        });
                        sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::CursorToEnd));
                    }
                }
            }
            b'K' => {
                let n = self.params.first().copied().unwrap_or(0);
                match EraseInLineMode::from_u16(n) {
                    Some(mode) => sink.emit(TerminalCommand::CsiEraseInLine(mode)),
                    None => {
                        sink.report_error(ParseError::InvalidParameter {
                            command: "CsiEraseInLine",
                            value: n,
                        });
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
            b'r' => {
                let top = self.params.first().copied().unwrap_or(1);
                let bottom = self.params.get(1).copied().unwrap_or(0);
                sink.emit(TerminalCommand::CsiSetScrollingRegion(top as u16, bottom as u16));
            }
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
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiDeleteLine(n as u16));
            }
            b'b' => {
                let n = self.params.first().copied().unwrap_or(1);
                sink.emit(TerminalCommand::CsiRepeatPrecedingCharacter(n as u16));
            }
            b's' => {
                sink.emit(TerminalCommand::CsiSaveCursorPosition);
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
                        sink.report_error(ParseError::MalformedSequence {
                            description: "Unknown or malformed escape sequence",
                        });
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
            },
            b'~' => {
                let n = self.params.first().copied().unwrap_or(0);
                sink.emit(TerminalCommand::CsiSpecialKey(n as u16));
            }
            b'c' => {
                sink.emit(TerminalCommand::CsiDeviceAttributes);
            }
            b'n' => {
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
            b'l' => {
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
            _ => {
                sink.report_error(ParseError::MalformedSequence {
                    description: "Unknown or malformed escape sequence",
                });
            }
        }
    }

    #[inline(always)]
    fn handle_dec_private_csi_final(&mut self, final_byte: u8, sink: &mut dyn CommandSink) {
        match final_byte {
            b'h' => {
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
            }
            b'l' => {
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
            }
            _ => {
                sink.report_error(ParseError::MalformedSequence {
                    description: "Unknown or malformed escape sequence",
                });
            }
        }
    }
}
