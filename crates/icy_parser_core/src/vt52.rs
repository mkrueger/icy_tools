//! VT52 (Video Terminal 52) parser
//!
//! VT52 is a simple terminal protocol used by DEC terminals and later adopted
//! by the Atari ST. It uses ESC sequences for cursor control, screen clearing,
//! and basic text formatting.

use crate::{Color, CommandParser, CommandSink, DecPrivateMode, Direction, EraseInDisplayMode, EraseInLineMode, SgrAttribute, TerminalCommand};

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Default,
    Escape,
    ReadFgColor,
    ReadBgColor,
    ReadCursorLine,
    ReadCursorRow(u8),
    ReadInsertLineCount,
}

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VT52Mode {
    Mixed,
    Atari,
    Standard,
}

pub struct Vt52Parser {
    vt52: VT52Mode,
    state: State,
    reverse_video: bool,
}

impl Vt52Parser {
    pub fn new(vt52: VT52Mode) -> Self {
        Self {
            vt52,
            state: State::Default,
            reverse_video: false,
        }
    }
    pub fn is_default_state(&self) -> bool {
        self.state == State::Default
    }

    /// Parse VT52 hex color code from ASCII byte
    #[inline]
    fn parse_vt52_color_mixed(byte: u8) -> Option<Color> {
        if byte <= 0x0F {
            // ATARI ST extension
            Some(Color::Base(byte as u8))
        } else if byte >= b'0' && byte <= b'0' + 15 {
            // Support for backwards compatibility with VT52
            let index = byte.wrapping_sub(b'0');
            Some(Color::Base(index as u8))
        } else if byte >= b'a' && byte <= b'f' {
            // Support for backwards compatibility with VT52
            let index = byte.wrapping_sub(b'a');
            Some(Color::Base(index as u8))
        } else if byte >= b'A' && byte <= b'F' {
            // Support for backwards compatibility with VT52
            let index = byte.wrapping_sub(b'A');
            Some(Color::Base(index as u8))
        } else {
            None
        }
    }

    #[inline]
    fn parse_vt52_color_standard(byte: u8) -> Option<Color> {
        if byte >= b'0' && byte <= b'0' + 15 {
            // Support for backwards compatibility with VT52
            let index = byte.wrapping_sub(b'0');
            Some(Color::Base(index as u8))
        } else {
            None
        }
    }

    /// Parse VT52 hex color code from ASCII byte
    #[inline]
    fn parse_vt52_color_atari(byte: u8) -> Option<Color> {
        if byte <= 0x0F {
            // ATARI ST extension
            Some(Color::Base(byte as u8))
        } else {
            None
        }
    }

    /// Parse cursor position for Mixed mode (auto-detect format)
    /// Returns (line, column) as 1-based coordinates
    #[inline]
    fn read_cursor_position_mixed(line_byte: u8, row_byte: u8) -> Option<(u16, u16)> {
        // Auto-detect format: if line_byte >= b' ' it's standard space-based format
        if let Some(pos) = Self::read_cursor_position_standard(line_byte, row_byte) {
            return Some(pos);
        } else {
            // Atari format: direct byte values (0-25 for line, 0-132 for row)
            Self::read_cursor_position_atari(line_byte, row_byte)
        }
    }

    /// Parse cursor position for Standard mode
    /// Returns (line, column) as 1-based coordinates
    #[inline]
    fn read_cursor_position_standard(line_byte: u8, row_byte: u8) -> Option<(u16, u16)> {
        // Original VT-52: space-based encoding
        if line_byte >= b' ' && line_byte <= b'8' && row_byte >= b' ' && row_byte <= b'p' {
            let line = (line_byte - b' ') as u16 + 1;
            let row = (row_byte - b' ') as u16 + 1;
            Some((line, row))
        } else {
            None
        }
    }

    /// Parse cursor position for Atari mode
    /// Returns (line, column) as 1-based coordinates
    #[inline]
    fn read_cursor_position_atari(line_byte: u8, row_byte: u8) -> Option<(u16, u16)> {
        // Atari ST: direct byte values
        if line_byte <= 25 && row_byte <= 132 {
            let line = line_byte as u16 + 1;
            let row = row_byte as u16 + 1;
            Some((line, row))
        } else {
            None
        }
    }

    /// Helper function to emit color commands with proper mode and reverse video handling
    fn emit_color_command(&self, byte: u8, get_foreground: bool, sink: &mut dyn CommandSink) {
        let color = match self.vt52 {
            VT52Mode::Mixed => Self::parse_vt52_color_mixed(byte),
            VT52Mode::Atari => Self::parse_vt52_color_atari(byte),
            VT52Mode::Standard => Self::parse_vt52_color_standard(byte),
        };

        if let Some(color) = color {
            // XOR get_foreground with reverse_video to determine actual attribute
            let sgr = if get_foreground ^ self.reverse_video {
                SgrAttribute::Foreground(color)
            } else {
                SgrAttribute::Background(color)
            };
            sink.emit(TerminalCommand::CsiSelectGraphicRendition(sgr));
        } else {
            let color_type = if get_foreground { "foreground" } else { "background" };
            sink.report_errror(
                crate::ParseError::InvalidParameter {
                    command: "VT52 color",
                    value: format!("0x{:02X}", byte),
                    expected: Some(format!("valid {} color code (mode: {:?})", color_type, self.vt52)),
                },
                crate::ErrorLevel::Warning,
            );
        }
    }
}

impl Default for Vt52Parser {
    fn default() -> Self {
        Self::new(VT52Mode::Mixed)
    }
}

impl CommandParser for Vt52Parser {
    fn parse(&mut self, data: &[u8], sink: &mut dyn CommandSink) {
        for &byte in data {
            match self.state {
                State::Default => match byte {
                    0x1B => {
                        // ESC - VT52 escape sequence
                        self.state = State::Escape;
                    }
                    0x08 | 0x0B | 0x0C => {
                        // Backspace
                        sink.emit(TerminalCommand::Backspace);
                    }
                    0x0D => {
                        // Carriage return
                        sink.emit(TerminalCommand::CarriageReturn);
                    }
                    0x0A => {
                        // Line feed
                        sink.emit(TerminalCommand::LineFeed);
                    }
                    0x00..=0x0F => {
                        if self.vt52 != VT52Mode::Standard {
                            let color = Color::Base(byte);
                            // ATARI ST extension - direct foreground color codes (0x00-0x0F)
                            let sgr: SgrAttribute = if self.reverse_video {
                                SgrAttribute::Background(color)
                            } else {
                                SgrAttribute::Foreground(color)
                            };
                            sink.emit(TerminalCommand::CsiSelectGraphicRendition(sgr));
                        }
                    }
                    0x0E..=0x1A | 0x1C..=0x1F => {
                        // Ignore control characters
                    }
                    _ => {
                        // Regular character
                        sink.print(&[byte]);
                    }
                },

                State::Escape => {
                    let ch = byte as char;
                    match ch {
                        'A' => {
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Up, 1));
                            self.state = State::Default;
                        }
                        'B' => {
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Down, 1));
                            self.state = State::Default;
                        }
                        'C' => {
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Right, 1));
                            self.state = State::Default;
                        }
                        'D' => {
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Left, 1));
                            self.state = State::Default;
                        }
                        'E' => {
                            sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::All));
                            sink.emit(TerminalCommand::CsiCursorPosition(1, 1));
                            self.state = State::Default;
                        }
                        'H' => {
                            sink.emit(TerminalCommand::CsiCursorPosition(1, 1));
                            self.state = State::Default;
                        }
                        'I' => {
                            // VT52 Reverse line feed (cursor up and insert)
                            sink.emit(TerminalCommand::EscReverseIndex);
                            self.state = State::Default;
                        }
                        'J' => {
                            sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::CursorToEnd));
                            self.state = State::Default;
                        }
                        'K' => {
                            sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::CursorToEnd));
                            self.state = State::Default;
                        }
                        'Y' => {
                            self.state = State::ReadCursorLine;
                        }
                        '3' | 'b' => {
                            self.state = State::ReadFgColor;
                        }
                        '4' | 'c' => {
                            self.state = State::ReadBgColor;
                        }
                        'e' => {
                            sink.emit(TerminalCommand::CsiDecPrivateModeSet(DecPrivateMode::CursorVisible));
                            self.state = State::Default;
                        }
                        'f' => {
                            sink.emit(TerminalCommand::CsiDecPrivateModeReset(DecPrivateMode::CursorVisible));
                            self.state = State::Default;
                        }
                        'j' => {
                            sink.emit(TerminalCommand::CsiSaveCursorPosition);
                            self.state = State::Default;
                        }
                        'k' => {
                            sink.emit(TerminalCommand::CsiRestoreCursorPosition);
                            self.state = State::Default;
                        }
                        'L' => {
                            // VT52 Insert Line
                            sink.emit(TerminalCommand::CsiInsertLine(1));
                            self.state = State::Default;
                        }
                        'M' => {
                            // VT52 Delete Line
                            sink.emit(TerminalCommand::CsiDeleteLine(1));
                            self.state = State::Default;
                        }
                        'p' => {
                            // VT52 Reverse video
                            self.reverse_video = true;
                            self.state = State::Default;
                        }
                        'q' => {
                            // VT52 Normal video
                            self.reverse_video = false;
                            self.state = State::Default;
                        }
                        'v' => {
                            // VT52 Wrap on
                            sink.emit(TerminalCommand::CsiDecPrivateModeSet(DecPrivateMode::AutoWrap));
                            self.state = State::Default;
                        }
                        'w' => {
                            // VT52 Wrap off
                            sink.emit(TerminalCommand::CsiDecPrivateModeReset(DecPrivateMode::AutoWrap));
                            self.state = State::Default;
                        }
                        'd' => {
                            // VT52 Clear to start of screen
                            sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::StartToCursor));
                            self.state = State::Default;
                        }
                        'o' => {
                            // VT52 Clear to start of line
                            sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::StartToCursor));
                            self.state = State::Default;
                        }
                        'i' => {
                            // Insert line ESC form: mode implicitly 0, next byte is count
                            self.state = State::ReadInsertLineCount;
                        }
                        'l' => {
                            // Clear line ESC form: mode implicitly 0
                            sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::All));
                            self.state = State::Default;
                        }
                        _ => {
                            // Unknown escape sequence, report warning
                            sink.report_errror(
                                crate::ParseError::InvalidParameter {
                                    command: "VT52 escape sequence",
                                    value: format!("ESC {}", ch),
                                    expected: Some("valid VT52 command character".to_string()),
                                },
                                crate::ErrorLevel::Warning,
                            );
                            self.state = State::Default;
                        }
                    }
                }

                State::ReadFgColor => {
                    if byte == 0x1B {
                        // ESC starts new escape sequence
                        self.state = State::Escape;
                    } else {
                        self.emit_color_command(byte, true, sink);
                        self.state = State::Default;
                    }
                }

                State::ReadBgColor => {
                    if byte == 0x1B {
                        // ESC starts new escape sequence
                        self.state = State::Escape;
                    } else {
                        self.emit_color_command(byte, false, sink);
                        self.state = State::Default;
                    }
                }

                State::ReadCursorLine => {
                    // Check if we got an ESC character - this means incomplete sequence
                    if byte == 0x1B {
                        sink.report_errror(
                            crate::ParseError::InvalidParameter {
                                command: "VT52 cursor position",
                                value: "incomplete sequence (got ESC instead of line)".to_string(),
                                expected: Some("line coordinate after ESC Y".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        self.state = State::Escape;
                    } else {
                        self.state = State::ReadCursorRow(byte);
                    }
                }

                State::ReadCursorRow(line_byte) => {
                    // Check if we got an ESC character - this means incomplete cursor position sequence
                    if byte == 0x1B {
                        // Incomplete cursor position - treat as error and start new escape sequence
                        sink.report_errror(
                            crate::ParseError::InvalidParameter {
                                command: "VT52 cursor position",
                                value: format!("line=0x{:02X}, incomplete sequence (got ESC)", line_byte),
                                expected: Some("complete cursor position sequence (ESC Y line row)".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        self.state = State::Escape; // ← Nur hier bei ESC
                    } else {
                        let position = if self.vt52 == VT52Mode::Mixed {
                            Self::read_cursor_position_mixed(line_byte, byte)
                        } else if self.vt52 == VT52Mode::Standard {
                            Self::read_cursor_position_standard(line_byte, byte)
                        } else {
                            Self::read_cursor_position_atari(line_byte, byte)
                        };

                        if let Some((line, row)) = position {
                            sink.emit(TerminalCommand::CsiCursorPosition(line, row));
                        } else {
                            let mode_desc = match self.vt52 {
                                VT52Mode::Standard => "standard mode: line 0x20-0x38, row 0x20-0x70",
                                VT52Mode::Atari => "Atari mode: line 0-25, row 0-132",
                                VT52Mode::Mixed => "mixed mode: space-based (≥0x20) or direct byte values",
                            };
                            sink.report_errror(
                                crate::ParseError::InvalidParameter {
                                    command: "VT52 cursor position",
                                    value: format!("line=0x{:02X}, row=0x{:02X}", line_byte, byte),
                                    expected: Some(format!("valid cursor position for {}", mode_desc)),
                                },
                                crate::ErrorLevel::Warning,
                            );
                        }
                        self.state = State::Default; // ← Zurück zu Default nach der Verarbeitung
                    }
                }

                State::ReadInsertLineCount => {
                    let count = byte as u16;
                    if count > 0 {
                        sink.emit(TerminalCommand::CsiInsertLine(count));
                    }
                    self.state = State::Default;
                }
            }
        }
    }
}
