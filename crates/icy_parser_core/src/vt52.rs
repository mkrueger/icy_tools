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
    ReadCursorRow(i32), // row position
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

    /// Parse VT52 cursor row position from ASCII byte (1 based)
    #[inline]
    fn parse_cursor_row(byte: u8) -> Option<i32> {
        // Original VT-52:
        if byte >= b' ' && byte <= b'p' { Some((byte - b' ') as i32 + 1) } else { None }
    }

    /// Parse ATARI cursor line position from ASCII byte (1 based)
    #[inline]
    fn parse_cursor_line(byte: u8) -> Option<i32> {
        // Original VT-52:
        if byte >= b' ' && byte <= b'8' { Some((byte - b' ') as i32 + 1) } else { None }
    }

    /// Parse VT52 cursor row position from ASCII byte (1 based)
    #[inline]
    fn parse_cursor_row_atari(byte: u8) -> Option<i32> {
        // Original VT-52:
        if byte <= 132 { Some(byte as i32 + 1) } else { None }
    }

    /// Parse ATARI cursor line position from ASCII byte (1 based)
    #[inline]
    fn parse_cursor_line_atari(byte: u8) -> Option<i32> {
        // Original VT-52:
        if byte <= 25 { Some(byte as i32 + 1) } else { None }
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
                            if let Some(color) = match self.vt52 {
                                VT52Mode::Mixed => Self::parse_vt52_color_mixed(byte),
                                VT52Mode::Atari => Self::parse_vt52_color_atari(byte),
                                VT52Mode::Standard => None,
                            } {
                                // ATARI ST extension - direct foreground color codes (0x00-0x0F)
                                let sgr: SgrAttribute = if self.reverse_video {
                                    SgrAttribute::Background(color)
                                } else {
                                    SgrAttribute::Foreground(color)
                                };
                                sink.emit(TerminalCommand::CsiSelectGraphicRendition(sgr));
                            }
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
                            // Unknown escape sequence, ignore
                            self.state = State::Default;
                        }
                    }
                }

                State::ReadFgColor => {
                    self.emit_color_command(byte, true, sink);
                    self.state = State::Default;
                }

                State::ReadBgColor => {
                    self.emit_color_command(byte, false, sink);
                    self.state = State::Default;
                }

                State::ReadCursorLine => {
                    if self.vt52 == VT52Mode::Atari {
                        if let Some(line) = Self::parse_cursor_line_atari(byte) {
                            self.state = State::ReadCursorRow(line);
                        } else {
                            self.state = State::Default;
                        }
                    } else {
                        if let Some(line) = Self::parse_cursor_line(byte) {
                            self.state = State::ReadCursorRow(line);
                        } else {
                            self.state = State::Default;
                        }
                    }
                }

                State::ReadCursorRow(line) => {
                    if self.vt52 == VT52Mode::Atari {
                        if let Some(row) = Self::parse_cursor_row_atari(byte) {
                            sink.emit(TerminalCommand::CsiCursorPosition(line as u16, row as u16));
                        }
                    } else {
                        if let Some(row) = Self::parse_cursor_row(byte) {
                            sink.emit(TerminalCommand::CsiCursorPosition(line as u16, row as u16));
                        }
                    }
                    self.state = State::Default;
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
