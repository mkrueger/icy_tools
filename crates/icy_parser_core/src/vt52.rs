//! VT52 (Video Terminal 52) parser
//!
//! VT52 is a simple terminal protocol used by DEC terminals and later adopted
//! by the Atari ST. It uses ESC sequences for cursor control, screen clearing,
//! and basic text formatting.
//!
//! ## Supported VT52 Commands
//!
//! ### Cursor Movement
//! - `ESC A` - Move cursor up one line
//! - `ESC B` - Move cursor down one line
//! - `ESC C` - Move cursor right one column
//! - `ESC D` - Move cursor left one column
//! - `ESC H` - Move cursor to home position (1,1)
//! - `ESC Y <line> <column>` - Set cursor position (space-based: 0x20+n)
//! - `ESC I` - Reverse line feed (cursor up with scroll)
//!
//! ### Screen Clearing
//! - `ESC E` - Clear screen and home cursor
//! - `ESC J` - Clear from cursor to end of screen
//! - `ESC d` - Clear from start of screen to cursor
//!
//! ### Line Clearing
//! - `ESC K` - Clear from cursor to end of line
//! - `ESC o` - Clear from start of line to cursor
//! - `ESC l` - Clear entire line
//!
//! ### Line Operations
//! - `ESC L` - Insert line at cursor position
//! - `ESC M` - Delete line at cursor position
//!
//! ### Display Attributes
//! - `ESC p` - Enable reverse video
//! - `ESC q` - Disable reverse video (normal video)
//!
//! ### Cursor Visibility
//! - `ESC e` - Show cursor
//! - `ESC f` - Hide cursor
//!
//! ### Cursor Save/Restore
//! - `ESC j` - Save cursor position
//! - `ESC k` - Restore cursor position
//!
//! ### Line Wrapping
//! - `ESC v` - Enable line wrap
//! - `ESC w` - Disable line wrap
//!
//! ## Atari ST Extensions
//!
//! The Atari ST extended the VT52 protocol with color support and additional features:
//!
//! ### Color Commands
//! - `ESC b <color>` - Set foreground color (0-15)
//! - `ESC c <color>` - Set background color (0-15)
//! - Direct color codes: Bytes 0x00-0x0F set foreground color directly (Atari mode only)
//!
//! ### Additional Line Operations (Atari ST specific)
//! - `ESC i <count>` - Insert multiple lines
//!
//! ### Alternative Command Codes
//! The Atari ST also recognizes:
//! - `ESC 3 <color>` - Alternative for `ESC b` (set foreground)
//! - `ESC 4 <color>` - Alternative for `ESC c` (set background)
//!
//! ## Color Encoding Modes
//!
//! The parser supports three color encoding modes:
//!
//! ### Standard Mode
//! - Space-based encoding: `0x20` (space) = color 0, `0x21` = color 1, ... `0x2F` = color 15
//! - Alternative digits: '0'-'9' for colors 0-9, 'A'-'F' for colors 10-15
//!
//! ### Atari Mode  
//! - Direct byte values: 0x00-0x0F represent colors 0-15
//! - Supports direct foreground color codes in text stream (0x00-0x0F)
//!
//! ### Mixed Mode (Default)
//! - Auto-detects encoding format
//! - Tries standard space-based first, then falls back to Atari direct values
//! - Most compatible mode for unknown sources
//!
//! ## Cursor Position Encoding
//!
//! ### Standard VT52
//! - Line and column are encoded as space + offset
//! - Line: 0x20 (space) = line 1, 0x21 = line 2, ... up to 0x38 = line 25
//! - Column: 0x20 (space) = column 1, 0x21 = column 2, ... up to 0x70 = column 81
//!
//! ### Atari ST
//! - Direct byte values: 0 = line/column 1, 1 = line/column 2, etc.
//! - Supports up to 26 lines (0-25) and 133 columns (0-132)
//!
//! ### Mixed Mode
//! - Auto-detects format: values >= 0x20 use standard encoding, < 0x20 use Atari encoding
//!
//! ## Control Characters
//!
//! The following control characters are handled outside of escape sequences:
//! - `0x08` (BS) - Backspace
//! - `0x0A` (LF) - Line feed
//! - `0x0B` (VT) - Vertical tab (treated as backspace)
//! - `0x0C` (FF) - Form feed (treated as backspace)
//! - `0x0D` (CR) - Carriage return
//!
//! ## Implementation Notes
//!
//! - Reverse video affects color commands: when enabled, foreground/background are swapped
//! - Invalid escape sequences generate warnings but don't terminate parsing
//! - The parser maintains state to handle multi-byte sequences correctly
//! - Escape (0x1B) during a multi-byte sequence starts a new escape sequence

use crate::{
    Color, CommandParser, CommandSink, DecMode, Direction, EraseInDisplayMode, EraseInLineMode, SgrAttribute, TerminalCommand, Wrapping, flush_input,
    print_char_value,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Default,
    Escape,
    ReadColor(bool),
    ReadCursorLine,
    ReadCursorRow(u8), // Stores the line byte for cursor positioning
    ReadInsertLineCount,
}

/// Escape sequence action for LUT
#[repr(u8)]
#[derive(Clone)]
enum EscAction {
    /// No action / unknown sequence
    None,
    /// Emit command and return to default state
    /// Note: boxing makes sense, makes the table size smaller and gives a bit better performance.
    Command(Box<TerminalCommand>),
    /// Position cursor (Y)
    PositionCursor,
    /// Read foreground color (3, b)
    ReadColor(bool),
    /// Read insert line count
    ReadInsertLineCount,
}

// Build escape sequence lookup table at compile time
fn build_escape_lut() -> Vec<EscAction> {
    let mut lut = vec![EscAction::None; 256];
    lut[b'A' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiMoveCursor(Direction::Up, 1, Wrapping::Always)));
    lut[b'B' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiMoveCursor(Direction::Down, 1, Wrapping::Always)));
    lut[b'C' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiMoveCursor(Direction::Right, 1, Wrapping::Always)));
    lut[b'D' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiMoveCursor(Direction::Left, 1, Wrapping::Always)));
    lut[b'E' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::All)));
    lut[b'H' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiCursorPosition(1, 1)));
    lut[b'I' as usize] = EscAction::Command(Box::new(TerminalCommand::EscReverseIndex));
    lut[b'J' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::CursorToEnd)));
    lut[b'K' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiEraseInLine(EraseInLineMode::CursorToEnd)));
    lut[b'Y' as usize] = EscAction::PositionCursor;
    lut[b'3' as usize] = EscAction::ReadColor(true);
    lut[b'b' as usize] = EscAction::ReadColor(true);
    lut[b'4' as usize] = EscAction::ReadColor(false);
    lut[b'c' as usize] = EscAction::ReadColor(false);
    lut[b'e' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiDecSetMode(DecMode::CursorVisible, true)));
    lut[b'f' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiDecSetMode(DecMode::CursorVisible, false)));
    lut[b'j' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiSaveCursorPosition));
    lut[b'k' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiRestoreCursorPosition));
    lut[b'L' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiInsertLine(1)));
    lut[b'M' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiDeleteLine(1)));
    lut[b'p' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Inverse(true))));
    lut[b'q' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Inverse(false))));
    lut[b'v' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiDecSetMode(DecMode::AutoWrap, true)));
    lut[b'w' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiDecSetMode(DecMode::AutoWrap, false)));
    lut[b'd' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::StartToCursor)));
    lut[b'o' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiEraseInLine(EraseInLineMode::StartToCursor)));
    lut[b'i' as usize] = EscAction::ReadInsertLineCount;
    lut[b'l' as usize] = EscAction::Command(Box::new(TerminalCommand::CsiEraseInLine(EraseInLineMode::All)));
    lut
}

lazy_static::lazy_static! {
    static ref ESCAPE_LUT: Vec<EscAction> = build_escape_lut();
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
}

impl Vt52Parser {
    pub fn new(vt52: VT52Mode) -> Self {
        Self { vt52, state: State::Default }
    }
    pub fn is_default_state(&self) -> bool {
        self.state == State::Default
    }

    /// Parse VT52 color code from ASCII byte for Mixed mode (auto-detect format)
    #[inline]
    fn parse_vt52_color_mixed(byte: u8) -> Option<Color> {
        // Try standard space-based first (0x20-0x2F)
        if let Some(color) = Self::parse_vt52_color_standard(byte) {
            return Some(color);
        }
        if byte >= b'0' && byte <= b'9' + 15 {
            let index = byte.wrapping_sub(b'0');
            Some(Color::Base(index as u8))
        } else if byte >= b'a' && byte <= b'f' {
            let index = byte.wrapping_sub(b'a') + 10;
            return Some(Color::Base(index as u8));
        } else if byte >= b'A' && byte <= b'F' {
            let index = byte.wrapping_sub(b'A') + 10;
            return Some(Color::Base(index as u8));
        } else {
            // Try ATARI ST direct byte values last
            Self::parse_vt52_color_atari(byte)
        }
    }

    /// Parse VT52 color code from ASCII byte for Standard mode (space-based)
    #[inline]
    fn parse_vt52_color_standard(byte: u8) -> Option<Color> {
        if byte >= b' ' && byte < b' ' + 16 {
            // Standard VT52: space-based encoding (0x20-0x2F)
            let index = byte.wrapping_sub(b' ');
            Some(Color::Base(index as u8))
        } else {
            None
        }
    }

    /// Parse VT52 hex color code from ASCII byte
    #[inline]
    fn parse_vt52_color_atari(byte: u8) -> Option<Color> {
        if byte <= 0x0F {
            // ATARI ST extension: direct byte values
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
            let sgr = if get_foreground {
                SgrAttribute::Foreground(color)
            } else {
                SgrAttribute::Background(color)
            };
            sink.emit(TerminalCommand::CsiSelectGraphicRendition(sgr));
        } else {
            let color_type = if get_foreground { "foreground" } else { "background" };
            sink.report_error(
                crate::ParseError::InvalidParameter {
                    command: "VT52 color",
                    value: print_char_value(byte),
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
        let mut start = 0;

        for (i, &byte) in data.iter().enumerate() {
            match self.state {
                State::Default => match byte {
                    0x1B => {
                        // ESC - VT52 escape sequence
                        flush_input(data, sink, i, start);
                        self.state = State::Escape;
                        start = i + 1;
                    }
                    0x08 | 0x0B | 0x0C => {
                        // Backspace
                        flush_input(data, sink, i, start);
                        sink.emit(TerminalCommand::Backspace);
                        start = i + 1;
                    }
                    0x0D => {
                        // Carriage return
                        flush_input(data, sink, i, start);
                        sink.emit(TerminalCommand::CarriageReturn);
                        start = i + 1;
                    }
                    0x0A => {
                        // Line feed
                        flush_input(data, sink, i, start);
                        sink.emit(TerminalCommand::LineFeed);
                        start = i + 1;
                    }
                    0x00..=0x0F => {
                        flush_input(data, sink, i, start);
                        if self.vt52 != VT52Mode::Standard {
                            let color = Color::Base(byte);
                            // ATARI ST extension - direct foreground color codes (0x00-0x0F)
                            let sgr = SgrAttribute::Foreground(color);
                            sink.emit(TerminalCommand::CsiSelectGraphicRendition(sgr));
                        }
                        start = i + 1;
                    }
                    0x0E..=0x1A | 0x1C..=0x1F => {
                        // Ignore control characters
                        flush_input(data, sink, i, start);
                        start = i + 1;
                    }
                    _ => {
                        // Regular character - will be handled in bulk
                    }
                },

                State::Escape => {
                    match unsafe { ESCAPE_LUT.get_unchecked(byte as usize) } {
                        EscAction::Command(cmd) => {
                            sink.emit((**cmd).clone());
                            self.state = State::Default;
                        }
                        EscAction::PositionCursor => {
                            self.state = State::ReadCursorLine;
                        }
                        EscAction::ReadColor(fore) => {
                            self.state = State::ReadColor(*fore);
                        }
                        EscAction::ReadInsertLineCount => {
                            self.state = State::ReadInsertLineCount;
                        }
                        EscAction::None => {
                            // Unknown escape sequence, report warning
                            sink.report_error(
                                crate::ParseError::InvalidParameter {
                                    command: "VT52 escape sequence",
                                    value: format!("ESC {}", byte as char),
                                    expected: Some("valid VT52 command character".to_string()),
                                },
                                crate::ErrorLevel::Warning,
                            );
                            self.state = State::Default;
                        }
                    }
                    start = i + 1;
                }

                State::ReadColor(fore) => {
                    if byte == 0x1B {
                        // ESC starts new escape sequence
                        self.state = State::Escape;
                    } else {
                        self.emit_color_command(byte, fore, sink);
                        self.state = State::Default;
                    }
                    start = i + 1;
                }

                State::ReadCursorLine => {
                    // Check if we got an ESC character - this means incomplete sequence
                    if byte == 0x1B {
                        sink.report_error(
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
                    start = i + 1;
                }

                State::ReadCursorRow(line_byte) => {
                    // Check if we got an ESC character - this means incomplete cursor position sequence
                    if byte == 0x1B {
                        // Incomplete cursor position - treat as error and start new escape sequence
                        sink.report_error(
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
                            sink.report_error(
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
                    start = i + 1;
                }

                State::ReadInsertLineCount => {
                    let count = byte as u16;
                    if count > 0 {
                        sink.emit(TerminalCommand::CsiInsertLine(count));
                    }
                    self.state = State::Default;
                    start = i + 1;
                }
            }
        }

        // Emit any remaining text
        if start < data.len() && self.state == State::Default {
            sink.print(&data[start..]);
        }
    }
}
