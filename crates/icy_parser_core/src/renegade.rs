//! Renegade parser
//!
//! Uses pipe codes for colors: |XX where XX is a two-digit number:
//! - 00-15: Foreground colors (0=black, 1=blue, ..., 15=white)
//! - 16-23: Background colors (16=black bg, 17=blue bg, ..., 23=white bg)

use crate::{CommandParser, CommandSink, TerminalCommand};

/// Renegade BBS parser
pub struct RenegadeParser {
    state: State,
    first_digit: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Normal,
    Pipe,
    FirstDigit(u8),
}

impl Default for RenegadeParser {
    fn default() -> Self {
        Self::new()
    }
}

impl RenegadeParser {
    pub fn new() -> Self {
        Self {
            state: State::Normal,
            first_digit: 0,
        }
    }
}

impl CommandParser for RenegadeParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        let mut start = 0;

        for (i, &byte) in input.iter().enumerate() {
            match self.state {
                State::Normal => {
                    if byte == b'|' {
                        // Emit any accumulated text
                        if start < i {
                            sink.print(&input[start..i]);
                        }
                        self.state = State::Pipe;
                        start = i + 1;
                    }
                }
                State::Pipe => {
                    if byte >= b'0' && byte <= b'3' {
                        // Valid first digit (0-3)
                        self.first_digit = byte - b'0';
                        self.state = State::FirstDigit(self.first_digit);
                    } else {
                        // Invalid sequence, emit literal pipe and continue
                        sink.print(b"|");
                        self.state = State::Normal;
                        start = i; // Re-process this byte
                    }
                }
                State::FirstDigit(tens) => {
                    if byte.is_ascii_digit() {
                        let ones = byte - b'0';
                        let color_code = tens.wrapping_mul(10).wrapping_add(ones);

                        if color_code < 16 {
                            // Foreground color (0-15)
                            sink.emit(TerminalCommand::CsiSelectGraphicRendition(crate::SgrAttribute::Foreground(crate::Color::Base(
                                color_code,
                            ))));
                        } else if color_code < 24 {
                            // Background color (16-23 maps to 0-7)
                            sink.emit(TerminalCommand::CsiSelectGraphicRendition(crate::SgrAttribute::Background(crate::Color::Base(
                                color_code - 16,
                            ))));
                        } else {
                            // Invalid color code, emit as literal
                            let literal = format!("|{}{}", tens, ones);
                            sink.print(literal.as_bytes());
                        }

                        self.state = State::Normal;
                        start = i + 1;
                    } else {
                        // Invalid second digit
                        let literal = format!("|{}", tens);
                        sink.print(literal.as_bytes());
                        self.state = State::Normal;
                        start = i; // Re-process this byte
                    }
                }
            }
        }

        // Emit any remaining text
        if start < input.len() && self.state == State::Normal {
            sink.print(&input[start..]);
        }
    }
}
