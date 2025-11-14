//! PCBoard parser
//!
//! PCBoard uses `@X` codes for colors:
//! - `@X` followed by two hex digits (foreground/background in DOS attribute format)
//! - Example: `@X0F` = white on black (0x0F = bright white foreground, black background)
//! - `@@` escapes to literal `@`

use crate::{Color, CommandParser, CommandSink, SgrAttribute, TerminalCommand};

/// Convert DOS color attribute to SGR commands
/// DOS attribute byte format:
/// - Bits 0-2: Foreground color (0-7)
/// - Bit 3: Foreground intensity/bright
/// - Bits 4-6: Background color (0-7)
/// - Bit 7: Blink or background intensity (iCE colors)
fn emit_dos_color_as_sgr(sink: &mut dyn CommandSink, attr: u8) {
    let fg = attr & 0x0F; // Foreground: bits 0-3
    let bg = (attr >> 4) & 0x0F; // Background: bits 4-7

    // Emit foreground color
    let fg_color = Color::Base(fg);
    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(fg_color)));

    // Emit background color
    let bg_color = Color::Base(bg);
    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(bg_color)));
}

/// PCBoard parser state
pub struct PcBoardParser {
    state: State,
    color_pos: u8,
    color_value: u8,
    accumulated: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Normal,
    AtSign,        // Just saw @, waiting for next char
    ColorX,        // Saw @X, waiting for first hex
    ColorFirstHex, // Saw first hex digit, waiting for second
    Macro,         // Accumulating macro name until closing @
}

impl Default for PcBoardParser {
    fn default() -> Self {
        Self::new()
    }
}

impl PcBoardParser {
    pub fn new() -> Self {
        Self {
            state: State::Normal,
            color_pos: 0,
            color_value: 0,
            accumulated: Vec::new(),
        }
    }

    fn reset_state(&mut self) {
        self.state = State::Normal;
        self.accumulated.clear();
    }

    fn hex_to_value(ch: u8) -> Option<u8> {
        match ch {
            b'0'..=b'9' => Some(ch - b'0'),
            b'a'..=b'f' => Some(10 + ch - b'a'),
            b'A'..=b'F' => Some(10 + ch - b'A'),
            _ => None,
        }
    }
}

impl CommandParser for PcBoardParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        let mut start = 0;

        for (i, &byte) in input.iter().enumerate() {
            match self.state {
                State::Normal => {
                    if byte == b'@' {
                        // Emit any accumulated printable text
                        if start < i {
                            sink.print(&input[start..i]);
                        }
                        self.state = State::AtSign;
                        self.accumulated.clear();
                        start = i + 1;
                    }
                }
                State::AtSign => {
                    match byte {
                        b'@' => {
                            // Escaped @ - emit literal @
                            sink.print(b"@");
                            self.reset_state();
                            start = i + 1;
                        }
                        b'X' | b'x' => {
                            // Color code sequence
                            self.state = State::ColorX;
                            self.color_pos = 0;
                            self.color_value = 0;
                        }
                        _ => {
                            // Start accumulating macro name
                            self.accumulated.push(byte);
                            self.state = State::Macro;
                        }
                    }
                }
                State::Macro => {
                    if byte == b'@' {
                        // End of macro - ignore it and continue
                        self.reset_state();
                        start = i + 1;
                    } else {
                        // Continue accumulating macro name
                        self.accumulated.push(byte);
                    }
                }
                State::ColorX => {
                    if let Some(val) = Self::hex_to_value(byte) {
                        self.color_value = val;
                        self.state = State::ColorFirstHex;
                    } else {
                        // Invalid hex char after @X, treat as literal
                        sink.print(b"@X");
                        self.reset_state();
                        start = i; // Re-process this byte
                    }
                }
                State::ColorFirstHex => {
                    if let Some(val) = Self::hex_to_value(byte) {
                        self.color_value = (self.color_value << 4) | val;

                        // Emit SGR commands for DOS color attribute
                        emit_dos_color_as_sgr(sink, self.color_value);

                        self.reset_state();
                        start = i + 1;
                    } else {
                        // Invalid second hex digit
                        sink.print(b"@X");
                        self.reset_state();
                        start = i; // Re-process this byte
                    }
                }
            }
        }

        // Emit any remaining printable text
        if start < input.len() && self.state == State::Normal {
            sink.print(&input[start..]);
        }
    }
}
