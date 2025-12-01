use crate::{
    BACKSPACE, BELL, Blink, CARRIAGE_RETURN, Color, CommandParser, CommandSink, DELETE, Direction, EraseInDisplayMode, EraseInLineMode, FORM_FEED, Intensity,
    LINE_FEED, SgrAttribute, TAB, TerminalCommand, VERTICAL_TAB, Wrapping, flush_input,
};

mod commands;
pub use commands::{CrcTransferMode, DisplayMode, FillMode, SkypixCommand, command_numbers};

/// ANSI color to Amiga/Skypix color mapping (normal intensity)
/// ANSI: Black=0, Red=1, Green=2, Yellow=3, Blue=4, Magenta=5, Cyan=6, White=7
/// Maps to Skypix palette indices for normal colors
const AMIGA_COLOR_OFFSETS: [u8; 8] = [0, 3, 4, 6, 1, 7, 5, 2];

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
    is_negative: bool,
    cmd_num: i32,
    string_param: String,
}

impl CommandBuilder {
    fn new() -> Self {
        Self {
            params: Vec::new(),
            current_param: 0,
            has_param: false,
            is_negative: false,
            cmd_num: 0,
            string_param: String::new(),
        }
    }

    fn reset(&mut self) {
        self.params.clear();
        self.current_param = 0;
        self.has_param = false;
        self.is_negative = false;
        self.cmd_num = 0;
        self.string_param.clear();
    }

    fn push_param(&mut self) {
        if self.has_param {
            let value = if self.is_negative { -self.current_param } else { self.current_param };
            self.params.push(value);
            self.current_param = 0;
            self.has_param = false;
            self.is_negative = false;
        }
    }

    fn add_digit(&mut self, digit: i32) {
        self.current_param = self.current_param.wrapping_mul(10).wrapping_add(digit);
        self.has_param = true;
    }

    fn set_negative(&mut self) {
        self.is_negative = true;
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

    #[inline]
    fn check_params(&self, sink: &mut dyn CommandSink, cmd_name: &'static str, required: usize) -> bool {
        if self.builder.params.len() < required {
            sink.report_error(
                crate::ParseError::InvalidParameter {
                    command: cmd_name,
                    value: format!(
                        "{} parameter{}",
                        self.builder.params.len(),
                        if self.builder.params.len() == 1 { "" } else { "s" }
                    ),
                    expected: Some(format!("{} parameter{}", required, if required == 1 { "" } else { "s" })),
                },
                crate::ErrorLevel::Error,
            );
            false
        } else {
            true
        }
    }

    fn emit_skypix_command(&mut self, sink: &mut dyn CommandSink) {
        // Finalize any pending parameter
        self.builder.push_param();
        use commands::command_numbers::*;
        let cmd = match self.builder.cmd_num {
            COMMENT => {
                // Command 0: Comment - just ignore the text, no command emitted
                // The comment text is in self.builder.string_param but we discard it
                Some(SkypixCommand::Comment {
                    text: self.builder.string_param.clone(),
                })
            }
            SET_PIXEL => {
                if !self.check_params(sink, "SetPixel", 2) {
                    return;
                }
                Some(SkypixCommand::SetPixel {
                    x: self.builder.params[0],
                    y: self.builder.params[1],
                })
            }
            DRAW_LINE => {
                if !self.check_params(sink, "DrawLine", 2) {
                    return;
                }
                Some(SkypixCommand::DrawLine {
                    x: self.builder.params[0],
                    y: self.builder.params[1],
                })
            }
            AREA_FILL => {
                if !self.check_params(sink, "AreaFill", 3) {
                    return;
                }
                match FillMode::try_from(self.builder.params[0]) {
                    Ok(mode) => Some(SkypixCommand::AreaFill {
                        mode,
                        x: self.builder.params[1],
                        y: self.builder.params[2],
                    }),
                    Err(_e) => {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "AreaFill",
                                value: format!("{}", self.builder.params[0]),
                                expected: Some("0 (Outline) or 1 (Color)".to_string()),
                            },
                            crate::ErrorLevel::Error,
                        );
                        None
                    }
                }
            }
            RECTANGLE_FILL => {
                if !self.check_params(sink, "RectangleFill", 4) {
                    return;
                }
                Some(SkypixCommand::RectangleFill {
                    x1: self.builder.params[0],
                    y1: self.builder.params[1],
                    x2: self.builder.params[2],
                    y2: self.builder.params[3],
                })
            }
            ELLIPSE => {
                if !self.check_params(sink, "Ellipse", 4) {
                    return;
                }
                Some(SkypixCommand::Ellipse {
                    x: self.builder.params[0],
                    y: self.builder.params[1],
                    a: self.builder.params[2],
                    b: self.builder.params[3],
                })
            }
            GRAB_BRUSH => {
                if !self.check_params(sink, "GrabBrush", 4) {
                    return;
                }
                Some(SkypixCommand::GrabBrush {
                    x1: self.builder.params[0],
                    y1: self.builder.params[1],
                    width: self.builder.params[2],
                    height: self.builder.params[3],
                })
            }
            USE_BRUSH => {
                if !self.check_params(sink, "UseBrush", 8) {
                    return;
                }
                Some(SkypixCommand::UseBrush {
                    src_x: self.builder.params[0],
                    src_y: self.builder.params[1],
                    dst_x: self.builder.params[2],
                    dst_y: self.builder.params[3],
                    width: self.builder.params[4],
                    height: self.builder.params[5],
                    minterm: self.builder.params[6],
                    mask: self.builder.params[7],
                })
            }
            MOVE_PEN => {
                if !self.check_params(sink, "MovePen", 2) {
                    return;
                }
                Some(SkypixCommand::MovePen {
                    x: self.builder.params[0],
                    y: self.builder.params[1],
                })
            }
            PLAY_SAMPLE => {
                if !self.check_params(sink, "PlaySample", 4) {
                    return;
                }
                Some(SkypixCommand::PlaySample {
                    speed: self.builder.params[0],
                    start: self.builder.params[1],
                    end: self.builder.params[2],
                    loops: self.builder.params[3],
                })
            }
            SET_FONT => {
                if !self.check_params(sink, "SetFont", 1) {
                    return;
                }
                let size = self.builder.params[0];
                if size == 0 {
                    // Special case: font reset
                    Some(SkypixCommand::ResetFont)
                } else {
                    Some(SkypixCommand::SetFont {
                        size,
                        name: self.builder.string_param.clone(),
                    })
                }
            }
            NEW_PALETTE => {
                if !self.check_params(sink, "NewPalette", 16) {
                    return;
                }
                Some(SkypixCommand::NewPalette {
                    colors: self.builder.params.clone(),
                })
            }
            RESET_PALETTE => Some(SkypixCommand::ResetPalette),
            FILLED_ELLIPSE => {
                if !self.check_params(sink, "FilledEllipse", 4) {
                    return;
                }
                Some(SkypixCommand::FilledEllipse {
                    x: self.builder.params[0],
                    y: self.builder.params[1],
                    a: self.builder.params[2],
                    b: self.builder.params[3],
                })
            }
            DELAY => {
                if !self.check_params(sink, "Delay", 1) {
                    return;
                }
                Some(SkypixCommand::Delay {
                    jiffies: self.builder.params[0],
                })
            }
            SET_PEN_A => {
                // If no parameter provided, default to color 7 (white/default foreground)
                let color = self.builder.params.get(0).copied().unwrap_or(2);
                Some(SkypixCommand::SetPenA { color })
            }
            CRC_TRANSFER => {
                if !self.check_params(sink, "CrcTransfer", 3) {
                    return;
                }
                // Convert mode parameter to CrcTransferMode enum
                match CrcTransferMode::try_from(self.builder.params[0]) {
                    Ok(mode) => Some(SkypixCommand::CrcTransfer {
                        mode,
                        width: self.builder.params[1],
                        height: self.builder.params[2],
                        filename: self.builder.string_param.clone(),
                    }),
                    Err(_) => {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "CrcTransfer",
                                value: format!("{}", self.builder.params[0]),
                                expected: Some("1 (IFF Brush), 2 (IFF Sound), 3 (FutureSound), or 20 (General Purpose)".to_string()),
                            },
                            crate::ErrorLevel::Error,
                        );
                        None
                    }
                }
            }
            SET_DISPLAY_MODE => {
                if !self.check_params(sink, "SetDisplayMode", 1) {
                    return;
                }
                // Convert mode parameter to DisplayMode enum
                match DisplayMode::try_from(self.builder.params[0]) {
                    Ok(mode) => Some(SkypixCommand::SetDisplayMode { mode }),
                    Err(_) => {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "SetDisplayMode",
                                value: format!("{}", self.builder.params[0]),
                                expected: Some("1 (8 colors) or 2 (16 colors)".to_string()),
                            },
                            crate::ErrorLevel::Error,
                        );
                        None
                    }
                }
            }
            SET_PEN_B => {
                // If no parameter provided, default to color 0 (black/default background)
                let color = self.builder.params.get(0).copied().unwrap_or(0);
                Some(SkypixCommand::SetPenB { color })
            }
            POSITION_CURSOR => {
                if !self.check_params(sink, "PositionCursor", 2) {
                    return;
                }
                Some(SkypixCommand::PositionCursor {
                    x: self.builder.params[0],
                    y: self.builder.params[1],
                })
            }
            CONTROLLER_RETURN => {
                if !self.check_params(sink, "ControllerReturn", 3) {
                    return;
                }
                Some(SkypixCommand::ControllerReturn {
                    c: self.builder.params[0],
                    x: self.builder.params[1],
                    y: self.builder.params[2],
                })
            }
            DEFINE_GADGET => {
                if !self.check_params(sink, "DefineGadget", 6) {
                    return;
                }
                Some(SkypixCommand::DefineGadget {
                    num: self.builder.params[0],
                    cmd: self.builder.params[1],
                    x1: self.builder.params[2],
                    y1: self.builder.params[3],
                    x2: self.builder.params[4],
                    y2: self.builder.params[5],
                })
            }
            END_SKYPIX => {
                // Unofficial extension: End SkyPix mode and return to ANSI
                Some(SkypixCommand::EndSkypix)
            }
            _ => {
                if self.builder.cmd_num > 0 {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "SkypixCommand",
                            value: format!("command {}", self.builder.cmd_num),
                            expected: Some("valid command number".to_string()),
                        },
                        crate::ErrorLevel::Error,
                    );
                }
                None
            }
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
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Up, n as u16, Wrapping::Never));
            }
            b'B' => {
                // Cursor Down
                let n = self.builder.params.get(0).copied().unwrap_or(1).max(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Down, n as u16, Wrapping::Never));
            }
            b'C' => {
                // Cursor Forward
                let n = self.builder.params.get(0).copied().unwrap_or(1).max(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Right, n as u16, Wrapping::Never));
            }
            b'D' => {
                // Cursor Backward
                let n = self.builder.params.get(0).copied().unwrap_or(1).max(1);
                sink.emit(TerminalCommand::CsiMoveCursor(Direction::Left, n as u16, Wrapping::Never));
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
                    2 => {
                        sink.emit(TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::All));
                    }
                    _ => {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "EraseDisplay",
                                value: format!("{}", n),
                                expected: Some("0 (cursor to end), 1 (start to cursor), or 2 (entire display)".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                    }
                }
            }
            b'K' => {
                // Erase Line
                let n = self.builder.params.get(0).copied().unwrap_or(0);
                match n {
                    0 => sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::CursorToEnd)),
                    1 => sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::StartToCursor)),
                    2 => sink.emit(TerminalCommand::CsiEraseInLine(EraseInLineMode::All)),
                    _ => {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "EraseLine",
                                value: format!("{}", n),
                                expected: Some("0 (cursor to end), 1 (start to cursor), or 2 (entire line)".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                    }
                }
            }
            b'm' => {
                // SGR - Select Graphic Rendition (colors and text effects)
                // Note: This is a simplified ANSI subset for SkyPix compatibility.
                // Bold mode persists until reset with SGR 0.
                if self.builder.params.is_empty() {
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Reset));
                } else {
                    for &param in &self.builder.params {
                        match param {
                            0 => {
                                sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Reset));
                                sink.emit(TerminalCommand::SetFontPage(0));
                            }
                            1 => {
                                sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Intensity(Intensity::Bold)));
                            }
                            3 => sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Italic(true))),
                            5 => sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(Blink::Slow))),
                            7 => sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Inverse(true))),
                            30..=37 => {
                                // Use bold color table if Bold mode is active
                                sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(
                                    AMIGA_COLOR_OFFSETS[(param - 30) as usize],
                                ))));
                            }
                            40..=47 => {
                                // Background colors don't have bold variants in Skypix
                                sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(
                                    AMIGA_COLOR_OFFSETS[(param - 40) as usize],
                                ))));
                            }
                            _ => {
                                sink.report_error(
                                    crate::ParseError::InvalidParameter {
                                        command: "SGR",
                                        value: format!("{}", param),
                                        expected: Some(
                                            "0 (reset), 1 (bold), 3 (italic), 5 (blink), 7 (inverse), 30-37 (foreground), or 40-47 (background)".to_string(),
                                        ),
                                    },
                                    crate::ErrorLevel::Warning,
                                );
                            }
                        }
                    }
                }
            }
            b'E' => {
                // CNL - Cursor Next Line: Move cursor down n lines and to column 1
                let n = self.builder.params.get(0).copied().unwrap_or(1).max(1);
                sink.emit(TerminalCommand::CsiCursorNextLine(n as u16));
            }
            b'F' => {
                // CPL - Cursor Previous Line: Move cursor up n lines and to column 1
                let n = self.builder.params.get(0).copied().unwrap_or(1).max(1);
                sink.emit(TerminalCommand::CsiCursorPreviousLine(n as u16));
            }
            b'G' => {
                // CHA - Cursor Horizontal Absolute: Move cursor to column n
                let n = self.builder.params.get(0).copied().unwrap_or(1).max(1);
                sink.emit(TerminalCommand::CsiCursorHorizontalAbsolute(n as u16 - 1));
            }
            b'L' => {
                // IL - Insert Line: Insert n blank lines at cursor position
                let n = self.builder.params.get(0).copied().unwrap_or(1).max(1);
                sink.emit(TerminalCommand::CsiInsertLine(n as u16));
            }
            b'M' => {
                // DL - Delete Line: Delete n lines at cursor position
                let n = self.builder.params.get(0).copied().unwrap_or(1).max(1);
                sink.emit(TerminalCommand::CsiDeleteLine(n as u16));
            }
            b'P' => {
                // DCH - Delete Character: Delete n characters at cursor position
                let n = self.builder.params.get(0).copied().unwrap_or(1).max(1);
                sink.emit(TerminalCommand::CsiDeleteCharacter(n as u16));
            }
            b'S' => {
                // SU - Scroll Up: Scroll display up n lines
                let n = self.builder.params.get(0).copied().unwrap_or(1).max(1);
                sink.emit(TerminalCommand::CsiScroll(Direction::Up, n as u16));
            }
            b'T' => {
                // SD - Scroll Down: Scroll display down n lines
                let n = self.builder.params.get(0).copied().unwrap_or(1).max(1);
                sink.emit(TerminalCommand::CsiScroll(Direction::Down, n as u16));
            }
            b'@' => {
                // ICH - Insert Character: Insert n blank characters at cursor position
                let n = self.builder.params.get(0).copied().unwrap_or(1).max(1);
                sink.emit(TerminalCommand::CsiInsertCharacter(n as u16));
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
        let mut start = 0;

        for (i, &ch) in input.iter().enumerate() {
            match self.state {
                State::Default => {
                    match ch {
                        0x1B => {
                            // ESC
                            flush_input(input, sink, i, start);
                            self.state = State::GotEscape;
                            self.builder.reset();
                            start = i + 1;
                        }
                        BELL => {
                            flush_input(input, sink, i, start);
                            sink.emit(TerminalCommand::Bell);
                            start = i + 1;
                        }
                        BACKSPACE => {
                            flush_input(input, sink, i, start);
                            sink.emit(TerminalCommand::Backspace);
                            start = i + 1;
                        }
                        TAB => {
                            flush_input(input, sink, i, start);
                            sink.emit(TerminalCommand::Tab);
                            start = i + 1;
                        }
                        LINE_FEED => {
                            flush_input(input, sink, i, start);
                            sink.emit(TerminalCommand::LineFeed);
                            start = i + 1;
                        }
                        VERTICAL_TAB => {
                            // CTRL-K - Cursor Up (as per ANSI.TXT spec for SkyPix)
                            flush_input(input, sink, i, start);
                            sink.emit(TerminalCommand::CsiMoveCursor(Direction::Up, 1, Wrapping::Never));
                            start = i + 1;
                        }
                        FORM_FEED => {
                            flush_input(input, sink, i, start);
                            sink.emit(TerminalCommand::FormFeed);
                            start = i + 1;
                        }
                        CARRIAGE_RETURN => {
                            flush_input(input, sink, i, start);
                            sink.emit(TerminalCommand::CarriageReturn);
                            start = i + 1;
                        }
                        DELETE => {
                            flush_input(input, sink, i, start);
                            sink.emit(TerminalCommand::Delete);
                            start = i + 1;
                        }
                        _ => {
                            // Regular character - will be handled in bulk
                        }
                    }
                }
                State::GotEscape => {
                    if ch == b'[' {
                        self.state = State::GotBracket;
                        start = i + 1;
                    } else {
                        // Not a valid sequence, emit ESC and current char
                        sink.print(b"\x1B");
                        flush_input(input, sink, i + 1, start);
                        self.state = State::Default;
                        start = i + 1;
                    }
                }
                State::GotBracket => {
                    if ch.is_ascii_digit() {
                        self.builder.add_digit((ch - b'0') as i32);
                        self.state = State::ReadingParams;
                    } else if ch == b'-' {
                        // Handle negative number at start
                        self.builder.set_negative();
                        self.state = State::ReadingParams;
                    } else if ch == b'!' {
                        // SkyPix command with no params
                        self.emit_skypix_command(sink);
                        self.state = State::Default;
                        start = i + 1;
                    } else if ch.is_ascii_alphabetic() {
                        // ANSI command with no params
                        self.emit_ansi_command(sink, ch);
                        self.state = State::Default;
                        start = i + 1;
                    } else {
                        // Invalid character after CSI
                        sink.report_error(
                            crate::ParseError::MalformedSequence {
                                description: "Invalid character after CSI",
                                sequence: Some(format!("ESC[{}", if ch.is_ascii_graphic() { ch as char } else { '?' })),
                                context: Some("Expected digit, '!', or letter".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        self.state = State::Default;
                        start = i + 1;
                    }
                }
                State::ReadingParams => {
                    if ch.is_ascii_digit() {
                        self.builder.add_digit((ch - b'0') as i32);
                    } else if ch == b'-' {
                        // Handle negative number sign
                        self.builder.set_negative();
                    } else if ch == b';' {
                        self.builder.push_param();
                    } else if ch == b'!' {
                        // SkyPix command terminator - check if we need to read string
                        self.builder.push_param();

                        // Commands 10 and 16 have string parameters after the !
                        if self.builder.params.is_empty() {
                            self.emit_skypix_command(sink);
                            self.state = State::Default;
                            start = i + 1;
                        } else {
                            self.builder.cmd_num = self.builder.params[0];
                            self.builder.params.remove(0);

                            if self.builder.cmd_num == command_numbers::COMMENT
                                || self.builder.cmd_num == command_numbers::SET_FONT
                                || self.builder.cmd_num == command_numbers::CRC_TRANSFER
                            {
                                // Commands that have string parameters
                                // Special case: SET_FONT with size 0 is a font reset and has no string parameter
                                if self.builder.cmd_num == command_numbers::SET_FONT && !self.builder.params.is_empty() && self.builder.params[0] == 0 {
                                    self.emit_skypix_command(sink);
                                    self.state = State::Default;
                                    start = i + 1;
                                } else {
                                    self.state = State::ReadingString;
                                }
                            } else {
                                self.emit_skypix_command(sink);
                                self.state = State::Default;
                                start = i + 1;
                            }
                        }
                    } else if ch.is_ascii_alphabetic() {
                        // ANSI command terminator
                        self.emit_ansi_command(sink, ch);
                        self.state = State::Default;
                        start = i + 1;
                    } else {
                        // Invalid character in parameter sequence
                        sink.report_error(
                            crate::ParseError::MalformedSequence {
                                description: "Invalid character in CSI parameter sequence",
                                sequence: Some(format!("ESC[...{}", if ch.is_ascii_graphic() { ch as char } else { '?' })),
                                context: Some("Expected digit, ';', '!', or letter".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        self.state = State::Default;
                        start = i + 1;
                    }
                }
                State::ReadingString => {
                    if ch == b'!' {
                        // End of string parameter
                        self.emit_skypix_command(sink);
                        self.state = State::Default;
                        start = i + 1;
                    } else {
                        self.builder.string_param.push(ch as char);
                    }
                }
            }
        }

        // Flush any remaining input at the end
        flush_input(input, sink, input.len(), start);
    }
}
