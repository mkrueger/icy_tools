//! ATASCII (Atari Standard Code for Information Interchange) parser
//!
//! ATASCII is the character encoding used by Atari 8-bit computers.
//! Key features:
//! - Characters 0-127: Standard ASCII with some special graphics characters
//! - Characters 128-255: Inverse video versions (foreground/background swapped)
//! - Special control codes for cursor movement, line operations, and tab management
//! - ESC prefix for literal character printing

use crate::{CommandParser, CommandSink, Direction, TerminalCommand};

/// ATASCII parser for Atari 8-bit computer systems
pub struct AtasciiParser {
    got_escape: bool,
}

impl Default for AtasciiParser {
    fn default() -> Self {
        Self::new()
    }
}

impl AtasciiParser {
    pub fn new() -> Self {
        Self { got_escape: false }
    }
}

impl CommandParser for AtasciiParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        let mut start = 0;

        for (i, &byte) in input.iter().enumerate() {
            // Handle escape sequence
            if self.got_escape {
                self.got_escape = false;
                // Emit any text before the escape
                if start < i - 1 {
                    sink.print(&input[start..i - 1]);
                }
                // Emit the literal character
                sink.print(&[byte]);
                start = i + 1;
                continue;
            }

            // Check for control characters and special ATASCII codes
            match byte {
                0x1B => {
                    // ESC - next character is literal
                    if start < i {
                        sink.print(&input[start..i]);
                    }
                    self.got_escape = true;
                    start = i + 1;
                }
                0x1C => {
                    // Cursor up
                    if start < i {
                        sink.print(&input[start..i]);
                    }
                    sink.emit(TerminalCommand::CsiMoveCursor(Direction::Up, 1));
                    start = i + 1;
                }
                0x1D => {
                    // Cursor down
                    if start < i {
                        sink.print(&input[start..i]);
                    }
                    sink.emit(TerminalCommand::CsiMoveCursor(Direction::Down, 1));
                    start = i + 1;
                }
                0x1E => {
                    // Cursor left
                    if start < i {
                        sink.print(&input[start..i]);
                    }
                    sink.emit(TerminalCommand::CsiMoveCursor(Direction::Left, 1));
                    start = i + 1;
                }
                0x1F => {
                    // Cursor right
                    if start < i {
                        sink.print(&input[start..i]);
                    }
                    sink.emit(TerminalCommand::CsiMoveCursor(Direction::Right, 1));
                    start = i + 1;
                }
                0x7D => {
                    // Clear screen
                    if start < i {
                        sink.print(&input[start..i]);
                    }
                    sink.emit(TerminalCommand::CsiEraseInDisplay(crate::EraseInDisplayMode::All));
                    start = i + 1;
                }
                0x7E => {
                    // Backspace
                    if start < i {
                        sink.print(&input[start..i]);
                    }
                    sink.emit(TerminalCommand::Backspace);
                    start = i + 1;
                }
                0x7F => {
                    // Tab - move to next tab stop
                    if start < i {
                        sink.print(&input[start..i]);
                    }
                    sink.emit(TerminalCommand::Tab);
                    start = i + 1;
                }
                0x9B => {
                    // Line feed (End Of Line in ATASCII)
                    if start < i {
                        sink.print(&input[start..i]);
                    }
                    sink.emit(TerminalCommand::LineFeed);
                    start = i + 1;
                }
                0x9C => {
                    // Delete line
                    if start < i {
                        sink.print(&input[start..i]);
                    }
                    sink.emit(TerminalCommand::CsiDeleteLine(1));
                    start = i + 1;
                }
                0x9D => {
                    // Insert line
                    if start < i {
                        sink.print(&input[start..i]);
                    }
                    sink.emit(TerminalCommand::CsiInsertLine(1));
                    start = i + 1;
                }
                0x9E => {
                    // Clear tab stop at current position
                    if start < i {
                        sink.print(&input[start..i]);
                    }
                    sink.emit(TerminalCommand::CsiClearTabulation);
                    start = i + 1;
                }
                0x9F => {
                    // Set tab stop at current position
                    if start < i {
                        sink.print(&input[start..i]);
                    }
                    sink.emit(TerminalCommand::EscSetTab);
                    start = i + 1;
                }
                0xFD => {
                    // Bell (beep)
                    if start < i {
                        sink.print(&input[start..i]);
                    }
                    sink.emit(TerminalCommand::Bell);
                    start = i + 1;
                }
                0xFE => {
                    // Delete character
                    if start < i {
                        sink.print(&input[start..i]);
                    }
                    sink.emit(TerminalCommand::Delete);
                    start = i + 1;
                }
                0xFF => {
                    // Insert blank character (space) at current position
                    if start < i {
                        sink.print(&input[start..i]);
                    }
                    sink.print(&[b' ']);
                    start = i + 1;
                }
                _ => {
                    // Regular character - will be handled in bulk
                    // Characters >= 0x80 are inverse video and will be
                    // handled by the consumer
                }
            }
        }

        // Emit any remaining text
        if start < input.len() && !self.got_escape {
            sink.print(&input[start..]);
        }
    }
}
