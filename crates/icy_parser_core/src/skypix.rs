use crate::{Blink, Color, CommandParser, CommandSink, Direction, EraseInDisplayMode, EraseInLineMode, Intensity, SgrAttribute, TerminalCommand};

/// SkyPix-specific commands (those with ! terminator)
#[derive(Debug, Clone, PartialEq)]
pub enum SkypixCommand {
    /// Command 1: Set pixel at (x, y) to Pen A color
    SetPixel { x: i32, y: i32 },

    /// Command 2: Draw line from current pen position to (x, y)
    DrawLine { x: i32, y: i32 },

    /// Command 3: Area fill starting at (x, y) in mode m
    AreaFill { mode: i32, x: i32, y: i32 },

    /// Command 4: Draw filled rectangle
    RectangleFill { x1: i32, y1: i32, x2: i32, y2: i32 },

    /// Command 5: Draw ellipse with center (x, y), major axis a, minor axis b
    Ellipse { x: i32, y: i32, a: i32, b: i32 },

    /// Command 6: Grab brush from screen
    GrabBrush { x1: i32, y1: i32, width: i32, height: i32 },

    /// Command 7: Blit brush to screen
    UseBrush {
        src_x: i32,
        src_y: i32,
        dst_x: i32,
        dst_y: i32,
        width: i32,
        height: i32,
        minterm: i32,
        mask: i32,
    },

    /// Command 8: Move drawing pen to (x, y)
    MovePen { x: i32, y: i32 },

    /// Command 9: Play sound sample
    PlaySample { speed: i32, start: i32, end: i32, loops: i32 },

    /// Command 10: Set font
    SetFont { size: i32, name: String },

    /// Command 11: Set new palette (16 colors)
    NewPalette { colors: Vec<i32> },

    /// Command 12: Reset to default SkyPix palette
    ResetPalette,

    /// Command 13: Draw filled ellipse
    FilledEllipse { x: i32, y: i32, a: i32, b: i32 },

    /// Command 14: Delay/pause for specified jiffies (1/60th second)
    Delay { jiffies: i32 },

    /// Command 15: Set Pen A color (0-15)
    SetPenA { color: i32 },

    /// Command 16: CRC XMODEM transfer
    CrcTransfer { mode: i32, width: i32, height: i32, filename: String },

    /// Command 17: Select display mode (1=8 colors, 2=16 colors)
    SetDisplayMode { mode: i32 },

    /// Command 18: Set Pen B (background) color
    SetPenB { color: i32 },

    /// Command 19: Position cursor at pixel coordinates (x, y)
    PositionCursor { x: i32, y: i32 },

    /// Command 21: Controller return (mouse click or menu selection)
    ControllerReturn { c: i32, x: i32, y: i32 },

    /// Command 22: Define gadget
    DefineGadget { num: i32, cmd: i32, x1: i32, y1: i32, x2: i32, y2: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Default,
    GotEscape,
    GotBracket,
    ReadingParams,
    ReadingString,
}

struct CommandBuilder {
    params: Vec<i32>,
    current_param: i32,
    has_param: bool,
    cmd_num: i32,
    string_param: String,
}

impl CommandBuilder {
    fn new() -> Self {
        Self {
            params: Vec::new(),
            current_param: 0,
            has_param: false,
            cmd_num: 0,
            string_param: String::new(),
        }
    }

    fn reset(&mut self) {
        self.params.clear();
        self.current_param = 0;
        self.has_param = false;
        self.cmd_num = 0;
        self.string_param.clear();
    }

    fn push_param(&mut self) {
        if self.has_param {
            self.params.push(self.current_param);
            self.current_param = 0;
            self.has_param = false;
        }
    }

    fn add_digit(&mut self, digit: i32) {
        self.current_param = self.current_param.wrapping_mul(10).wrapping_add(digit);
        self.has_param = true;
    }
}

pub struct SkypixParser {
    state: State,
    builder: CommandBuilder,
}

impl SkypixParser {
    pub fn new() -> Self {
        Self {
            state: State::Default,
            builder: CommandBuilder::new(),
        }
    }

    fn emit_skypix_command(&mut self, sink: &mut dyn CommandSink) {
        // Finalize any pending parameter
        self.builder.push_param();

        let cmd = match self.builder.cmd_num {
            1 if self.builder.params.len() >= 2 => Some(SkypixCommand::SetPixel {
                x: self.builder.params[0],
                y: self.builder.params[1],
            }),
            2 if self.builder.params.len() >= 2 => Some(SkypixCommand::DrawLine {
                x: self.builder.params[0],
                y: self.builder.params[1],
            }),
            3 if self.builder.params.len() >= 3 => Some(SkypixCommand::AreaFill {
                mode: self.builder.params[0],
                x: self.builder.params[1],
                y: self.builder.params[2],
            }),
            4 if self.builder.params.len() >= 4 => Some(SkypixCommand::RectangleFill {
                x1: self.builder.params[0],
                y1: self.builder.params[1],
                x2: self.builder.params[2],
                y2: self.builder.params[3],
            }),
            5 if self.builder.params.len() >= 4 => Some(SkypixCommand::Ellipse {
                x: self.builder.params[0],
                y: self.builder.params[1],
                a: self.builder.params[2],
                b: self.builder.params[3],
            }),
            6 if self.builder.params.len() >= 4 => Some(SkypixCommand::GrabBrush {
                x1: self.builder.params[0],
                y1: self.builder.params[1],
                width: self.builder.params[2],
                height: self.builder.params[3],
            }),
            7 if self.builder.params.len() >= 8 => Some(SkypixCommand::UseBrush {
                src_x: self.builder.params[0],
                src_y: self.builder.params[1],
                dst_x: self.builder.params[2],
                dst_y: self.builder.params[3],
                width: self.builder.params[4],
                height: self.builder.params[5],
                minterm: self.builder.params[6],
                mask: self.builder.params[7],
            }),
            8 if self.builder.params.len() >= 2 => Some(SkypixCommand::MovePen {
                x: self.builder.params[0],
                y: self.builder.params[1],
            }),
            9 if self.builder.params.len() >= 4 => Some(SkypixCommand::PlaySample {
                speed: self.builder.params[0],
                start: self.builder.params[1],
                end: self.builder.params[2],
                loops: self.builder.params[3],
            }),
            10 if self.builder.params.len() >= 1 => Some(SkypixCommand::SetFont {
                size: self.builder.params[0],
                name: self.builder.string_param.clone(),
            }),
            11 if self.builder.params.len() >= 16 => Some(SkypixCommand::NewPalette {
                colors: self.builder.params.clone(),
            }),
            12 => Some(SkypixCommand::ResetPalette),
            13 if self.builder.params.len() >= 4 => Some(SkypixCommand::FilledEllipse {
                x: self.builder.params[0],
                y: self.builder.params[1],
                a: self.builder.params[2],
                b: self.builder.params[3],
            }),
            14 if self.builder.params.len() >= 1 => Some(SkypixCommand::Delay {
                jiffies: self.builder.params[0],
            }),
            15 if self.builder.params.len() >= 1 => Some(SkypixCommand::SetPenA { color: self.builder.params[0] }),
            16 if self.builder.params.len() >= 3 => Some(SkypixCommand::CrcTransfer {
                mode: self.builder.params[0],
                width: self.builder.params[1],
                height: self.builder.params[2],
                filename: self.builder.string_param.clone(),
            }),
            17 if self.builder.params.len() >= 1 => Some(SkypixCommand::SetDisplayMode { mode: self.builder.params[0] }),
            18 if self.builder.params.len() >= 1 => Some(SkypixCommand::SetPenB { color: self.builder.params[0] }),
            19 if self.builder.params.len() >= 2 => Some(SkypixCommand::PositionCursor {
                x: self.builder.params[0],
                y: self.builder.params[1],
            }),
            21 if self.builder.params.len() >= 3 => Some(SkypixCommand::ControllerReturn {
                c: self.builder.params[0],
                x: self.builder.params[1],
                y: self.builder.params[2],
            }),
            22 if self.builder.params.len() >= 6 => Some(SkypixCommand::DefineGadget {
                num: self.builder.params[0],
                cmd: self.builder.params[1],
                x1: self.builder.params[2],
                y1: self.builder.params[3],
                x2: self.builder.params[4],
                y2: self.builder.params[5],
            }),
            _ => None,
        };

        if let Some(command) = cmd {
            sink.emit_skypix(command);
        }
    }

    fn emit_ansi_command(&mut self, sink: &mut dyn CommandSink, terminator: u8) {
        // Finalize any pending parameter
        self.builder.push_param();

        // Handle ANSI subset commands based on terminator
        match terminator {
            b'A' => {
                // Cursor Up
                let n = self.builder.params.get(0).copied().unwrap_or(1).max(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Up, n as u16));
            }
            b'B' => {
                // Cursor Down
                let n = self.builder.params.get(0).copied().unwrap_or(1).max(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Down, n as u16));
            }
            b'C' => {
                // Cursor Forward
                let n = self.builder.params.get(0).copied().unwrap_or(1).max(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Right, n as u16));
            }
            b'D' => {
                // Cursor Backward
                let n = self.builder.params.get(0).copied().unwrap_or(1).max(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Left, n as u16));
            }
            b'H' | b'f' => {
                // Cursor Position
                let row = self.builder.params.get(0).copied().unwrap_or(1).max(1) as u16;
                let col = self.builder.params.get(1).copied().unwrap_or(1).max(1) as u16;
                sink.emit(TerminalCommand::CsiCursorPosition(row - 1, col - 1));
            }
            b'J' => {
                // Erase Display
                let n = self.builder.params.get(0).copied().unwrap_or(0);
                match n {
                    0 => sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::CursorToEnd)),
                    1 => sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::StartToCursor)),
                    2 => sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::All)),
                    _ => {}
                }
            }
            b'K' => {
                // Erase Line
                let n = self.builder.params.get(0).copied().unwrap_or(0);
                match n {
                    0 => sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::CursorToEnd)),
                    1 => sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::StartToCursor)),
                    2 => sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::All)),
                    _ => {}
                }
            }
            b'm' => {
                // SGR - Select Graphic Rendition (colors and text effects)
                if self.builder.params.is_empty() {
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Reset));
                } else {
                    for &param in &self.builder.params {
                        match param {
                            0 => sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Reset)),
                            1 => sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Intensity(Intensity::Bold))),
                            3 => sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Italic(true))),
                            5 => sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(Blink::Slow))),
                            7 => sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Inverse(true))),
                            30..=37 => sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(
                                (param - 30) as u8,
                            )))),
                            40..=47 => sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(
                                (param - 40) as u8,
                            )))),
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl Default for SkypixParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandParser for SkypixParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        for &ch in input {
            match self.state {
                State::Default => {
                    if ch == 0x1B {
                        // ESC
                        self.state = State::GotEscape;
                        self.builder.reset();
                    } else {
                        // Regular character - pass through
                        sink.print(&[ch]);
                    }
                }
                State::GotEscape => {
                    if ch == b'[' {
                        self.state = State::GotBracket;
                    } else {
                        // Not a valid sequence, emit ESC and current char
                        sink.print(b"\x1B");
                        sink.print(&[ch]);
                        self.state = State::Default;
                    }
                }
                State::GotBracket => {
                    if ch.is_ascii_digit() {
                        self.builder.add_digit((ch - b'0') as i32);
                        self.state = State::ReadingParams;
                    } else if ch == b'!' {
                        // SkyPix command with no params
                        self.emit_skypix_command(sink);
                        self.state = State::Default;
                    } else if ch.is_ascii_alphabetic() {
                        // ANSI command with no params
                        self.emit_ansi_command(sink, ch);
                        self.state = State::Default;
                    } else {
                        // Invalid sequence
                        self.state = State::Default;
                    }
                }
                State::ReadingParams => {
                    if ch.is_ascii_digit() {
                        self.builder.add_digit((ch - b'0') as i32);
                    } else if ch == b';' {
                        self.builder.push_param();
                    } else if ch == b'!' {
                        // SkyPix command terminator - check if we need to read string
                        self.builder.push_param();

                        // Commands 10 and 16 have string parameters after the !
                        if self.builder.params.is_empty() {
                            self.emit_skypix_command(sink);
                            self.state = State::Default;
                        } else {
                            self.builder.cmd_num = self.builder.params[0];
                            self.builder.params.remove(0);

                            if self.builder.cmd_num == 10 || self.builder.cmd_num == 16 {
                                // Commands that have string parameters
                                self.state = State::ReadingString;
                            } else {
                                self.emit_skypix_command(sink);
                                self.state = State::Default;
                            }
                        }
                    } else if ch.is_ascii_alphabetic() {
                        // ANSI command terminator
                        self.emit_ansi_command(sink, ch);
                        self.state = State::Default;
                    } else {
                        // Invalid character
                        self.state = State::Default;
                    }
                }
                State::ReadingString => {
                    if ch == b'!' {
                        // End of string parameter
                        self.emit_skypix_command(sink);
                        self.state = State::Default;
                    } else {
                        self.builder.string_param.push(ch as char);
                    }
                }
            }
        }
    }
}
