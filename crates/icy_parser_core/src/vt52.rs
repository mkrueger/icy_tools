use crate::{Color, CommandParser, CommandSink, DecPrivateMode, Direction, EraseInDisplayMode, EraseInLineMode, SgrAttribute, TerminalCommand};

// TODO: TosWin2 extensions not yet implemented:
// - ESC F/G: Enter/Exit graphics mode (alternate character set)
// - ESC y/z: Set/Clear text effects
// - ESC u: Original colors
// - ESC R <cols,rows> CR: Set window size
// - ESC S <title> CR: Set window title
// - ESC Z: Identify (sends response ESC / Z)

#[derive(Debug, Clone)]
enum State {
    Default,
    GotEscape,
    GotY,
    GotYRow { row: u8 },
    Gotb,
    Gotc,
    Got3,
    Got4,
}

pub struct Vt52Parser {
    state: State,
}

impl Vt52Parser {
    pub fn new() -> Self {
        Self { state: State::Default }
    }

    fn handle_ascii_control(&self, ch: u8, sink: &mut dyn CommandSink) {
        match ch {
            0x07 => sink.emit(TerminalCommand::Bell),
            0x08 => sink.emit(TerminalCommand::Backspace),
            0x09 => sink.emit(TerminalCommand::Tab),
            0x0A => sink.emit(TerminalCommand::LineFeed),
            0x0B => sink.emit(TerminalCommand::Tab), // Vertical tab - treat as tab
            0x0C => sink.emit(TerminalCommand::FormFeed),
            0x0D => sink.emit(TerminalCommand::CarriageReturn),
            _ => {}
        }
    }
}

impl Default for Vt52Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandParser for Vt52Parser {
    fn parse(&mut self, data: &[u8], sink: &mut dyn CommandSink) {
        let mut i = 0;
        while i < data.len() {
            let ch = data[i];

            match &self.state {
                State::Default => match ch {
                    0x1B => {
                        self.state = State::GotEscape;
                    }
                    0x07 | 0x08 | 0x09 | 0x0A | 0x0B | 0x0C | 0x0D => {
                        self.handle_ascii_control(ch, sink);
                    }
                    _ => {
                        sink.print(&[ch]);
                    }
                },
                State::GotEscape => {
                    match ch {
                        b'A' => {
                            // Cursor up
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Up, 1));
                            self.state = State::Default;
                        }
                        b'B' => {
                            // Cursor down
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Down, 1));
                            self.state = State::Default;
                        }
                        b'C' => {
                            // Cursor right
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Right, 1));
                            self.state = State::Default;
                        }
                        b'D' => {
                            // Cursor left
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Left, 1));
                            self.state = State::Default;
                        }
                        b'E' => {
                            // Clear screen and home cursor
                            sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::All));
                            sink.emit(TerminalCommand::CsiCursorPosition(1, 1));
                            self.state = State::Default;
                        }
                        b'H' => {
                            // Cursor home (move to upper left corner)
                            sink.emit(TerminalCommand::CsiCursorPosition(1, 1));
                            self.state = State::Default;
                        }
                        b'I' => {
                            // Cursor up and insert (reverse index with scroll)
                            sink.emit(TerminalCommand::EscReverseIndex);
                            self.state = State::Default;
                        }
                        b'J' => {
                            // Clear from cursor to end of screen
                            sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::CursorToEnd));
                            self.state = State::Default;
                        }
                        b'K' => {
                            // Clear from cursor to end of line
                            sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::CursorToEnd));
                            self.state = State::Default;
                        }
                        b'L' => {
                            // Insert line
                            sink.emit(TerminalCommand::CsiInsertLine(1));
                            self.state = State::Default;
                        }
                        b'M' => {
                            // Delete line
                            sink.emit(TerminalCommand::CsiDeleteLine(1));
                            self.state = State::Default;
                        }
                        b'Y' => {
                            self.state = State::GotY;
                        }
                        b'b' => {
                            self.state = State::Gotb;
                        }
                        b'c' => {
                            self.state = State::Gotc;
                        }
                        b'd' => {
                            // Clear from beginning of screen to cursor
                            sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::StartToCursor));
                            self.state = State::Default;
                        }
                        b'e' => {
                            // Show cursor
                            sink.emit(TerminalCommand::CsiDecPrivateModeSet(DecPrivateMode::CursorVisible));
                            self.state = State::Default;
                        }
                        b'f' => {
                            // Hide cursor
                            sink.emit(TerminalCommand::CsiDecPrivateModeReset(DecPrivateMode::CursorVisible));
                            self.state = State::Default;
                        }
                        b'j' => {
                            // Save cursor position
                            sink.emit(TerminalCommand::CsiSaveCursorPosition);
                            self.state = State::Default;
                        }
                        b'k' => {
                            // Restore cursor position
                            sink.emit(TerminalCommand::CsiRestoreCursorPosition);
                            self.state = State::Default;
                        }
                        b'l' => {
                            // Clear entire line
                            sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::All));
                            self.state = State::Default;
                        }
                        b'o' => {
                            // Clear from beginning of line to cursor
                            sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::StartToCursor));
                            self.state = State::Default;
                        }
                        b'p' => {
                            // Reverse video (VT-52 uses DecPrivateMode::Inverse)
                            sink.emit(TerminalCommand::CsiDecPrivateModeSet(DecPrivateMode::Inverse));
                            self.state = State::Default;
                        }
                        b'q' => {
                            // Normal video (VT-52 uses DecPrivateMode::Inverse)
                            sink.emit(TerminalCommand::CsiDecPrivateModeReset(DecPrivateMode::Inverse));
                            self.state = State::Default;
                        }
                        b'v' => {
                            // Wrap on
                            sink.emit(TerminalCommand::CsiDecPrivateModeSet(DecPrivateMode::AutoWrap));
                            self.state = State::Default;
                        }
                        b'w' => {
                            // Wrap off
                            sink.emit(TerminalCommand::CsiDecPrivateModeReset(DecPrivateMode::AutoWrap));
                            self.state = State::Default;
                        }
                        // TosWin2 extensions - ANSI color support
                        b'3' => {
                            self.state = State::Got3;
                        }
                        b'4' => {
                            self.state = State::Got4;
                        }
                        // TosWin2 extensions - TODO: Not yet implemented
                        b'F' | b'G' | b'R' | b'S' | b'Z' | b'a' | b'h' | b'i' | b'u' | b'y' | b'z' => {
                            // Silently ignore unimplemented TosWin2 extensions
                            self.state = State::Default;
                        }
                        _ => {
                            // Unknown escape sequence, reset
                            self.state = State::Default;
                        }
                    }
                }
                State::GotY => {
                    // ESC Y <row> <col> - coordinates are offset by 32 (space character)
                    self.state = State::GotYRow { row: ch };
                }
                State::GotYRow { row } => {
                    // VT-52 uses 1-based coordinates (offset by 32 for printable ASCII)
                    let y = (row.wrapping_sub(32) as u16).wrapping_add(1);
                    let x = (ch.wrapping_sub(32) as u16).wrapping_add(1);
                    sink.emit(TerminalCommand::CsiCursorPosition(y, x));
                    self.state = State::Default;
                }
                State::Gotb => {
                    // ESC b <color> - Set foreground color (VT-52 colors are 0-15)
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(ch & 0x0F))));
                    self.state = State::Default;
                }
                State::Gotc => {
                    // ESC c <color> - Set background color (VT-52 colors are 0-15)
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(ch & 0x0F))));
                    self.state = State::Default;
                }
                State::Got3 => {
                    // ESC 3 <color> - ANSI foreground color (TosWin2 extension)
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(ch & 0x0F))));
                    self.state = State::Default;
                }
                State::Got4 => {
                    // ESC 4 <color> - ANSI background color (TosWin2 extension)
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(ch & 0x0F))));
                    self.state = State::Default;
                }
            }

            i += 1;
        }
    }
}
