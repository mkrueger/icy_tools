//! Avatar (Advanced Video Attribute Terminal Assembler and Recreator) parser
//!
//! Avatar is a video control language similar to ANSI but more compact.
//! It uses ^V (0x16) to introduce commands and ^Y (0x19) for character repetition.

use crate::{AnsiParser, Color, CommandParser, CommandSink, DecPrivateMode, Direction, EraseInLineMode, ParseError, SgrAttribute, TerminalCommand};

const AVT_CMD: u8 = 0x16; // ^V - Avatar command introducer
const AVT_CLR: u8 = 0x0C; // ^L - Clear screen
const AVT_REP: u8 = 0x19; // ^Y - Repeat character

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AvatarState {
    /// Normal character processing
    Ground,
    /// Reading Avatar command after ^V
    ReadCommand,
    /// Reading color byte after ^V^A
    ReadColor,
    /// Reading row byte for GOTO_XY after ^V^H
    ReadGotoRow,
    /// Reading col byte for GOTO_XY after ^V^H
    ReadGotoCol { row: u8 },
    /// Reading character to repeat after ^Y
    ReadRepeatChar,
    /// Reading repeat count after ^Y{char}
    ReadRepeatCount { ch: u8 },
}

/// Avatar parser that delegates to ANSI parser for non-Avatar sequences
#[derive(Default)]
pub struct AvatarParser {
    state: AvatarState,
    ansi_parser: AnsiParser,
}

impl Default for AvatarState {
    fn default() -> Self {
        AvatarState::Ground
    }
}

impl AvatarParser {
    pub fn new() -> Self {
        Self::default()
    }

    fn reset(&mut self) {
        self.state = AvatarState::Ground;
    }
}

impl CommandParser for AvatarParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        let mut i = 0;
        let mut printable_start = 0;

        while i < input.len() {
            let byte = input[i];

            match self.state {
                AvatarState::Ground => {
                    match byte {
                        AVT_CLR => {
                            // Clear screen and reset attributes
                            if i > printable_start {
                                sink.print(&input[printable_start..i]);
                            }
                            sink.emit(TerminalCommand::FormFeed);
                            i += 1;
                            printable_start = i;
                        }
                        AVT_CMD => {
                            // Avatar command introducer
                            if i > printable_start {
                                sink.print(&input[printable_start..i]);
                            }
                            self.state = AvatarState::ReadCommand;
                            i += 1;
                            printable_start = i;
                        }
                        AVT_REP => {
                            // Repeat character
                            if i > printable_start {
                                sink.print(&input[printable_start..i]);
                            }
                            self.state = AvatarState::ReadRepeatChar;
                            i += 1;
                            printable_start = i;
                        }
                        _ => {
                            // Process through ANSI parser for other control codes
                            // We need to find the end of this run
                            i += 1;
                        }
                    }
                }

                AvatarState::ReadCommand => {
                    match byte {
                        1 => {
                            // Set color - next byte is color value
                            self.state = AvatarState::ReadColor;
                            i += 1;
                            printable_start = i;
                        }
                        2 => {
                            // Blink on - maps to disabling cursor blinking
                            sink.emit(TerminalCommand::CsiDecPrivateModeReset(DecPrivateMode::CursorBlinking));
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        3 => {
                            // Move up - maps to ANSI CUU (Cursor Up)
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Up, 1));
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        4 => {
                            // Move down - maps to ANSI CUD (Cursor Down)
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Down, 1));
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        5 => {
                            // Move left - maps to ANSI CUB (Cursor Back)
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Left, 1));
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        6 => {
                            // Move right - maps to ANSI CUF (Cursor Forward)
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Right, 1));
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        7 => {
                            // Clear to end of line - maps to ANSI EL (Erase in Line)
                            sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::CursorToEnd));
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                        8 => {
                            // GOTO_XY - next two bytes are row and column
                            self.state = AvatarState::ReadGotoRow;
                            i += 1;
                            printable_start = i;
                        }
                        _ => {
                            // Unknown Avatar command
                            sink.report_error(ParseError::MalformedSequence {
                                description: "Unknown or malformed Avatar command",
                            });
                            self.reset();
                            i += 1;
                            printable_start = i;
                        }
                    }
                }

                AvatarState::ReadColor => {
                    // Color byte - convert DOS attribute to SGR
                    emit_dos_color_as_sgr(sink, byte);
                    self.reset();
                    i += 1;
                    printable_start = i;
                }

                AvatarState::ReadGotoRow => {
                    // Row byte for GOTO_XY
                    self.state = AvatarState::ReadGotoCol { row: byte };
                    i += 1;
                    printable_start = i;
                }

                AvatarState::ReadGotoCol { row } => {
                    // Column byte for GOTO_XY - maps to ANSI CUP (Cursor Position)
                    // Note: Avatar uses 1-based row/col like ANSI
                    sink.emit(TerminalCommand::CsiCursorPosition(row as u16, byte as u16));
                    self.reset();
                    i += 1;
                    printable_start = i;
                }

                AvatarState::ReadRepeatChar => {
                    // Character to repeat
                    self.state = AvatarState::ReadRepeatCount { ch: byte };
                    i += 1;
                    printable_start = i;
                }

                AvatarState::ReadRepeatCount { ch } => {
                    // Repeat count
                    sink.print(&vec![ch; byte as usize]);
                    self.reset();
                    i += 1;
                    printable_start = i;
                }
            }
        }

        // Handle any remaining printable bytes
        if i > printable_start && self.state == AvatarState::Ground {
            // Use ANSI parser for remaining bytes
            self.ansi_parser.parse(&input[printable_start..i], sink);
        }
    }

    fn flush(&mut self, _sink: &mut dyn CommandSink) {
        self.reset();
    }
}
