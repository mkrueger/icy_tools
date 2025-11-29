//! Avatar (Advanced Video Attribute Terminal Assembler and Recreator) parser
//!
//! Avatar is a video control language similar to ANSI but more compact.
//! It uses ^V (0x16) to introduce commands and ^Y (0x19) for character repetition.

use crate::{AnsiParser, Color, CommandParser, CommandSink, Direction, EraseInLineMode, ParseError, SgrAttribute, TerminalCommand};

pub mod constants {

    /// Avatar command introducer (^V = 0x16)
    /// All Avatar commands except ^L and ^Y start with this byte
    pub const COMMAND: u8 = 0x16;

    /// Clear screen and reset to default attribute (^L = 0x0C)
    /// Sets current attribute to 3 (cyan on black) and clears the screen
    pub const CLEAR_SCREEN: u8 = 0x0C;

    /// Repeat character command (^Y = 0x19)
    /// Format: ^Y <char> <count> - repeats <char> exactly <count> times
    pub const REPEAT: u8 = 0x19;

    // Basic Avatar commands (FSC-0025)

    /// Set color attribute (^V^A)
    /// Format: ^V^A <attr> - sets current attribute to <attr> & 0x7F
    /// Attribute byte: [blink:1][bg:3][fg:4] following IBM CGA colors
    pub const SET_COLOR: u8 = 0x01;

    /// Enable blink attribute (^V^B)
    /// Sets bit 7 of current attribute (enables blinking text)
    pub const BLINK_ON: u8 = 0x02;

    /// Move cursor up one line (^V^C)
    /// Does nothing if already at top of current window
    pub const CARET_UP: u8 = 0x03;

    /// Move cursor down one line (^V^D)
    /// Does nothing if already at bottom of current window
    pub const CARET_DOWN: u8 = 0x04;

    /// Move cursor left one column (^V^E)
    /// Does nothing if already at leftmost column of current window
    pub const CARET_LEFT: u8 = 0x05;

    /// Move cursor right one column (^V^F)
    /// Does nothing if already at rightmost column of current window
    pub const CARET_RIGHT: u8 = 0x06;

    /// Clear to end of line (^V^G)
    /// Clears from cursor to end of line using current attribute
    pub const CLEAR_EOL: u8 = 0x07;

    /// Position cursor (^V^H)
    /// Format: ^V^H <row> <col> - moves cursor to row,col (1-based)
    pub const GOTO_XY: u8 = 0x08;

    // Avatar level 0 extensions (FSC-0037)

    /// Turn insert mode ON (^V^I)
    /// Insert mode stays on until any AVT/0 command except ^Y and ^V^Y
    /// Characters push existing text right, discarding last char on line
    pub const INSERT_MODE: u8 = 0x09;

    /// Scroll area up (^V^J)
    /// Format: ^V^J <numlines> <upper> <left> <lower> <right>
    /// Scrolls defined area up by numlines, filling gap with spaces using current attr
    pub const SCROLL_UP: u8 = 0x0A;

    /// Scroll area down (^V^K)
    /// Format: ^V^K <numlines> <upper> <left> <lower> <right>
    /// Scrolls defined area down by numlines, filling gap with spaces using current attr
    pub const SCROLL_DOWN: u8 = 0x0B;

    /// Clear rectangular area (^V^L)
    /// Format: ^V^L <attr> <lines> <columns>
    /// Sets current attribute to <attr> & 0x7F and fills area with spaces
    /// Area starts at cursor position which remains unchanged
    pub const CLEAR_AREA: u8 = 0x0C;

    /// Initialize rectangular area (^V^M)
    /// Format: ^V^M <attr> <char> <lines> <columns>
    /// Sets current attribute to <attr> & 0x7F (bit 7 enables blink)
    /// Fills area with <char> starting at cursor position
    pub const INIT_AREA: u8 = 0x0D;

    /// Delete character at cursor (^V^N)
    /// Scrolls rest of line left by one position
    /// Fills gap at end of line with space using current attribute
    pub const DELETE_CHAR: u8 = 0x0E;

    /// Repeat pattern (^V^Y)
    /// Format: ^V^Y <numchars> <char1>...<charN> <count>
    /// Repeats pattern of numchars exactly count times
    /// Example: ^V^Y 3 ABC 4 produces "ABCABCABCABC"
    pub const REPEAT_PATTERN: u8 = 0x19;
}

use constants::*;

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

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrollAreaStage {
    NumLines,
    Top,
    Left,
    Bottom,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    ReadGotoCol {
        row: u8,
    },
    /// Reading character to repeat after ^Y
    ReadRepeatChar,
    /// Reading repeat count after ^Y{char}
    ReadRepeatCount {
        ch: u8,
    },

    // FSC‑0037
    ReadScrollArea {
        direction: Direction,
        stage: ScrollAreaStage,
        num_lines: u8,
        top: u8,
        left: u8,
        bottom: u8,
    },
    ReadClearAreaAttr,
    ReadClearAreaLines {
        attr: u8,
    },
    ReadClearAreaCols {
        attr: u8,
        lines: u8,
    },
    ReadInitAreaAttr,
    ReadInitAreaChar {
        attr: u8,
    },
    ReadInitAreaLines {
        attr: u8,
        ch: u8,
    },
    ReadInitAreaCols {
        attr: u8,
        ch: u8,
        lines: u8,
    },
    ReadPatternLen,
    ReadPatternData {
        len: u8,
    },
    ReadPatternCount,
}

/// Avatar parser that delegates to ANSI parser for non-Avatar sequences
pub struct AvatarParser {
    state: AvatarState,
    ansi_parser: AnsiParser,
    blink_on: bool,
    insert_mode: bool,
    buf: Vec<u8>,
}

impl Default for AvatarParser {
    fn default() -> Self {
        Self {
            state: AvatarState::Ground,
            ansi_parser: AnsiParser::new(),
            blink_on: false,
            insert_mode: false,
            buf: Vec::new(),
        }
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
                        CLEAR_SCREEN => {
                            if i > printable_start {
                                self.ansi_parser.parse(&input[printable_start..i], sink);
                            }
                            // Clear screen
                            sink.emit(TerminalCommand::FormFeed);

                            self.state = AvatarState::Ground;
                            i += 1;
                            printable_start = i;
                        }
                        COMMAND => {
                            if i > printable_start {
                                self.ansi_parser.parse(&input[printable_start..i], sink);
                            }
                            self.state = AvatarState::ReadCommand;
                            i += 1;
                            printable_start = i;
                        }
                        REPEAT => {
                            if i > printable_start {
                                self.ansi_parser.parse(&input[printable_start..i], sink);
                            }
                            self.state = AvatarState::ReadRepeatChar;
                            i += 1;
                            printable_start = i;
                        }
                        _ => {
                            i += 1;
                        }
                    }
                }

                AvatarState::ReadCommand => match byte {
                    SET_COLOR => {
                        self.state = AvatarState::ReadColor;
                        i += 1;
                        printable_start = i;
                    }
                    BLINK_ON => {
                        self.blink_on = true;
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(crate::Blink::Slow)));
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    CARET_UP => {
                        sink.emit(TerminalCommand::CsiMoveCursor(Direction::Up, 1));
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    CARET_DOWN => {
                        sink.emit(TerminalCommand::CsiMoveCursor(Direction::Down, 1));
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    CARET_LEFT => {
                        sink.emit(TerminalCommand::CsiMoveCursor(Direction::Left, 1));
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    CARET_RIGHT => {
                        sink.emit(TerminalCommand::CsiMoveCursor(Direction::Right, 1));
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    CLEAR_EOL => {
                        sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::CursorToEnd));
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    GOTO_XY => {
                        self.state = AvatarState::ReadGotoRow;
                        i += 1;
                        printable_start = i;
                    }
                    INSERT_MODE => {
                        self.insert_mode = true;
                        sink.emit(TerminalCommand::CsiSetMode(crate::AnsiMode::InsertReplace, true));
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    SCROLL_UP => {
                        self.state = AvatarState::ReadScrollArea {
                            direction: Direction::Up,
                            stage: ScrollAreaStage::NumLines,
                            num_lines: 0,
                            top: 0,
                            left: 0,
                            bottom: 0,
                        };
                        i += 1;
                        printable_start = i;
                    }
                    SCROLL_DOWN => {
                        self.state = AvatarState::ReadScrollArea {
                            direction: Direction::Down,
                            stage: ScrollAreaStage::NumLines,
                            num_lines: 0,
                            top: 0,
                            left: 0,
                            bottom: 0,
                        };
                        i += 1;
                        printable_start = i;
                    }
                    CLEAR_AREA => {
                        self.state = AvatarState::ReadClearAreaAttr;
                        i += 1;
                        printable_start = i;
                    }
                    INIT_AREA => {
                        self.state = AvatarState::ReadInitAreaAttr;
                        i += 1;
                        printable_start = i;
                    }
                    DELETE_CHAR => {
                        sink.emit(TerminalCommand::CsiDeleteCharacter(1));
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                    REPEAT_PATTERN => {
                        self.state = AvatarState::ReadPatternLen;
                        i += 1;
                        printable_start = i;
                    }
                    _ => {
                        sink.report_error(
                            ParseError::MalformedSequence {
                                description: "Unknown or malformed Avatar command",
                                sequence: Some(format!("CTRL-V 0x{:02X}", byte)),
                                context: Some(format!("Unknown Avatar command byte: 0x{:02X}", byte)),
                            },
                            crate::ErrorLevel::Error,
                        );
                        self.reset();
                        i += 1;
                        printable_start = i;
                    }
                },

                AvatarState::ReadColor => {
                    let attr = byte & 0x7F;
                    emit_dos_color_as_sgr(sink, attr);
                    if self.blink_on {
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(crate::Blink::Slow)));
                    }
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
                    if byte > 0 {
                        let repeated = vec![ch; byte as usize];
                        self.ansi_parser.parse(&repeated, sink);
                    }
                    self.reset();
                    i += 1;
                    printable_start = i;
                }

                AvatarState::ReadScrollArea {
                    direction,
                    stage,
                    num_lines,
                    top,
                    left,
                    bottom,
                } => {
                    match stage {
                        ScrollAreaStage::NumLines => {
                            let nl = if byte == 0 { 0 } else { byte };
                            self.state = AvatarState::ReadScrollArea {
                                direction,
                                stage: ScrollAreaStage::Top,
                                num_lines: nl,
                                top,
                                left,
                                bottom,
                            };
                        }
                        ScrollAreaStage::Top => {
                            self.state = AvatarState::ReadScrollArea {
                                direction,
                                stage: ScrollAreaStage::Left,
                                num_lines,
                                top: byte.max(1),
                                left,
                                bottom,
                            };
                        }
                        ScrollAreaStage::Left => {
                            self.state = AvatarState::ReadScrollArea {
                                direction,
                                stage: ScrollAreaStage::Bottom,
                                num_lines,
                                top,
                                left: byte.max(1),
                                bottom,
                            };
                        }
                        ScrollAreaStage::Bottom => {
                            self.state = AvatarState::ReadScrollArea {
                                direction,
                                stage: ScrollAreaStage::Right,
                                num_lines,
                                top,
                                left,
                                bottom: byte.max(1),
                            };
                        }
                        ScrollAreaStage::Right => {
                            let right = byte.max(1);
                            sink.emit(TerminalCommand::ScrollArea {
                                direction,
                                num_lines: num_lines as u16,
                                top: top.max(1) as u16,
                                left: left.max(1) as u16,
                                bottom: bottom.max(1) as u16,
                                right: right as u16,
                            });
                            self.reset();
                        }
                    }
                    i += 1;
                    printable_start = i;
                }
                AvatarState::ReadClearAreaAttr => {
                    let attr = byte & 0x7F;
                    emit_dos_color_as_sgr(sink, attr);
                    if self.blink_on {
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(crate::Blink::Slow)));
                    }
                    self.state = AvatarState::ReadClearAreaLines { attr };
                    i += 1;
                    printable_start = i;
                }

                AvatarState::ReadClearAreaLines { attr } => {
                    self.state = AvatarState::ReadClearAreaCols { attr, lines: byte };
                    i += 1;
                    printable_start = i;
                }

                AvatarState::ReadClearAreaCols { attr, lines } => {
                    sink.emit(TerminalCommand::AvatarClearArea { attr, lines, columns: byte });
                    self.reset();
                    i += 1;
                    printable_start = i;
                }

                AvatarState::ReadInitAreaAttr => {
                    let attr = byte;
                    let masked = attr & 0x7F;
                    // FSC‑0037: wenn Bit 7 gesetzt, Blink an
                    if attr & 0x80 != 0 {
                        self.blink_on = true;
                    }
                    emit_dos_color_as_sgr(sink, masked);
                    if self.blink_on {
                        sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(crate::Blink::Slow)));
                    }
                    self.state = AvatarState::ReadInitAreaChar { attr };
                    i += 1;
                    printable_start = i;
                }

                AvatarState::ReadInitAreaChar { attr } => {
                    self.state = AvatarState::ReadInitAreaLines { attr, ch: byte };
                    i += 1;
                    printable_start = i;
                }

                AvatarState::ReadInitAreaLines { attr, ch } => {
                    self.state = AvatarState::ReadInitAreaCols { attr, ch, lines: byte };
                    i += 1;
                    printable_start = i;
                }

                AvatarState::ReadInitAreaCols { attr, ch, lines } => {
                    sink.emit(TerminalCommand::AvatarInitArea {
                        attr,
                        ch,
                        lines,
                        columns: byte,
                    });
                    self.reset();
                    i += 1;
                    printable_start = i;
                }

                AvatarState::ReadPatternLen => {
                    let len = byte;
                    self.state = AvatarState::ReadPatternData { len };
                    self.buf.clear();
                    i += 1;
                    printable_start = i;
                }

                AvatarState::ReadPatternData { len } => {
                    self.buf.push(byte);
                    if self.buf.len() as u8 == len {
                        self.state = AvatarState::ReadPatternCount;
                    }
                    i += 1;
                    printable_start = i;
                }

                AvatarState::ReadPatternCount => {
                    // Repeat the pattern 'byte' times
                    for _ in 0..byte {
                        self.ansi_parser.parse(&self.buf, sink);
                    }
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
}
