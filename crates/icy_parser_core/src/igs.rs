//! IGS (Interactive Graphics System) parser
//!
//! IGS is a graphics system developed for Atari ST BBS systems.
//! Commands start with 'G#' and use single-letter command codes followed by parameters.

use crate::{CommandParser, CommandSink, TerminalCommand};

#[derive(Debug, Clone, PartialEq)]
pub enum IgsCommand {
    // Drawing commands
    Box {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        rounded: bool,
    },
    Line {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
    },
    LineDrawTo {
        x: i32,
        y: i32,
    },
    Circle {
        x: i32,
        y: i32,
        radius: i32,
    },
    Ellipse {
        x: i32,
        y: i32,
        x_radius: i32,
        y_radius: i32,
    },
    Arc {
        x: i32,
        y: i32,
        start_angle: i32,
        end_angle: i32,
        radius: i32,
    },
    PolyLine {
        points: Vec<i32>,
    },
    PolyFill {
        points: Vec<i32>,
    },
    FloodFill {
        x: i32,
        y: i32,
    },

    // Color/Style commands
    ColorSet {
        pen: u8,
        color: u8,
    },
    AttributeForFills {
        pattern_type: u8,
        pattern_index: u8,
        border: bool,
    },
    LineStyle {
        style: u8,
        thickness: u8,
    },

    // Text commands
    WriteText {
        x: i32,
        y: i32,
        justification: u8,
        text: String,
    },
    TextEffects {
        effects: u8,
        size: u8,
        rotation: u8,
    },

    // Special commands
    BellsAndWhistles {
        sound_number: u8,
    },
    GraphicScaling {
        enabled: bool,
    },
    LoopCommand {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        command: String,
        parameters: Vec<Vec<String>>,
    },

    // VT52 commands (for compatibility)
    CursorUp,
    CursorDown,
    CursorRight,
    CursorLeft,
    CursorHome,
    ClearScreen,
    ClearToEOL,
    ClearToEOS,
    SetCursorPos {
        x: i32,
        y: i32,
    },
    SetForeground {
        color: u8,
    },
    SetBackground {
        color: u8,
    },
    ShowCursor,
    HideCursor,
    SaveCursorPos,
    RestoreCursorPos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IgsCommandType {
    AttributeForFills,  // A
    BellsAndWhistles,   // b
    Box,                // B
    ColorSet,           // C
    LineDrawTo,         // D
    TextEffects,        // E
    FloodFill,          // F
    PolyFill,           // f
    GraphicScaling,     // g
    LineStyle,          // I
    PolyLine,           // J
    Circle,             // K
    Line,               // L
    Ellipse,            // M
    PolyMarker,         // P
    Arc,                // Q
    SetResolution,      // R
    Scroll,             // S
    LineType,           // T
    GetGraphicInfo,     // U
    EnvString,          // V
    WriteText,          // W
    UserDefinedPattern, // X
    GetMouseClick,      // Y
    Zoom,               // Z
    LoopCommand,        // &
}

impl IgsCommandType {
    fn from_char(ch: char) -> Option<Self> {
        match ch {
            'A' => Some(Self::AttributeForFills),
            'b' => Some(Self::BellsAndWhistles),
            'B' => Some(Self::Box),
            'C' => Some(Self::ColorSet),
            'D' => Some(Self::LineDrawTo),
            'E' => Some(Self::TextEffects),
            'F' => Some(Self::FloodFill),
            'f' => Some(Self::PolyFill),
            'g' => Some(Self::GraphicScaling),
            'I' => Some(Self::LineStyle),
            'J' => Some(Self::PolyLine),
            'K' => Some(Self::Circle),
            'L' => Some(Self::Line),
            'M' => Some(Self::Ellipse),
            'P' => Some(Self::PolyMarker),
            'Q' => Some(Self::Arc),
            'R' => Some(Self::SetResolution),
            'S' => Some(Self::Scroll),
            'T' => Some(Self::LineType),
            'U' => Some(Self::GetGraphicInfo),
            'V' => Some(Self::EnvString),
            'W' => Some(Self::WriteText),
            'X' => Some(Self::UserDefinedPattern),
            'Y' => Some(Self::GetMouseClick),
            'Z' => Some(Self::Zoom),
            '&' => Some(Self::LoopCommand),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Default,
    GotG,
    GotIgsStart,
    _ReadCommandChar,
    ReadParams(IgsCommandType),
    ReadTextString(i32, i32, u8), // x, y, justification
    _ReadLoopParams,

    // VT52 states
    Escape,
    ReadFgColor,
    ReadBgColor,
    ReadCursorX,
    ReadCursorY(i32), // x position
}

pub struct IgsParser {
    state: State,
    params: Vec<i32>,
    current_param: i32,
    text_buffer: String,

    loop_command: String,
    loop_parameters: Vec<Vec<String>>,
    loop_param_count: i32,
    loop_state: LoopParseState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopParseState {
    ReadingInitialParams,
    _ReadingCommand,
    _ReadingCount,
    _ReadingParameters,
}

impl IgsParser {
    pub fn new() -> Self {
        Self {
            state: State::Default,
            params: Vec::new(),
            current_param: 0,
            text_buffer: String::new(),
            loop_command: String::new(),
            loop_parameters: Vec::new(),
            loop_param_count: 0,
            loop_state: LoopParseState::ReadingInitialParams,
        }
    }

    fn reset_params(&mut self) {
        self.params.clear();
        self.current_param = 0;
        self.text_buffer.clear();
    }

    fn push_current_param(&mut self) {
        self.params.push(self.current_param);
        self.current_param = 0;
    }

    fn emit_command(&mut self, cmd_type: IgsCommandType, sink: &mut dyn CommandSink) {
        let command = match cmd_type {
            IgsCommandType::Box => {
                if self.params.len() >= 5 {
                    Some(IgsCommand::Box {
                        x1: self.params[0],
                        y1: self.params[1],
                        x2: self.params[2],
                        y2: self.params[3],
                        rounded: self.params[4] != 0,
                    })
                } else {
                    None
                }
            }
            IgsCommandType::Line => {
                if self.params.len() >= 4 {
                    Some(IgsCommand::Line {
                        x1: self.params[0],
                        y1: self.params[1],
                        x2: self.params[2],
                        y2: self.params[3],
                    })
                } else {
                    None
                }
            }
            IgsCommandType::LineDrawTo => {
                if self.params.len() >= 2 {
                    Some(IgsCommand::LineDrawTo {
                        x: self.params[0],
                        y: self.params[1],
                    })
                } else {
                    None
                }
            }
            IgsCommandType::Circle => {
                if self.params.len() >= 3 {
                    Some(IgsCommand::Circle {
                        x: self.params[0],
                        y: self.params[1],
                        radius: self.params[2],
                    })
                } else {
                    None
                }
            }
            IgsCommandType::Ellipse => {
                if self.params.len() >= 4 {
                    Some(IgsCommand::Ellipse {
                        x: self.params[0],
                        y: self.params[1],
                        x_radius: self.params[2],
                        y_radius: self.params[3],
                    })
                } else {
                    None
                }
            }
            IgsCommandType::Arc => {
                if self.params.len() >= 5 {
                    Some(IgsCommand::Arc {
                        x: self.params[0],
                        y: self.params[1],
                        start_angle: self.params[2],
                        end_angle: self.params[3],
                        radius: self.params[4],
                    })
                } else {
                    None
                }
            }
            IgsCommandType::ColorSet => {
                if self.params.len() >= 2 {
                    Some(IgsCommand::ColorSet {
                        pen: self.params[0] as u8,
                        color: self.params[1] as u8,
                    })
                } else {
                    None
                }
            }
            IgsCommandType::AttributeForFills => {
                if self.params.len() >= 3 {
                    Some(IgsCommand::AttributeForFills {
                        pattern_type: self.params[0] as u8,
                        pattern_index: self.params[1] as u8,
                        border: self.params[2] != 0,
                    })
                } else {
                    None
                }
            }
            IgsCommandType::TextEffects => {
                if self.params.len() >= 3 {
                    Some(IgsCommand::TextEffects {
                        effects: self.params[0] as u8,
                        size: self.params[1] as u8,
                        rotation: self.params[2] as u8,
                    })
                } else {
                    None
                }
            }
            IgsCommandType::FloodFill => {
                if self.params.len() >= 2 {
                    Some(IgsCommand::FloodFill {
                        x: self.params[0],
                        y: self.params[1],
                    })
                } else {
                    None
                }
            }
            IgsCommandType::PolyLine | IgsCommandType::PolyFill => {
                if !self.params.is_empty() {
                    let count = self.params[0] as usize;
                    if self.params.len() >= 1 + count * 2 {
                        let points = self.params[1..].to_vec();
                        if cmd_type == IgsCommandType::PolyLine {
                            Some(IgsCommand::PolyLine { points })
                        } else {
                            Some(IgsCommand::PolyFill { points })
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            IgsCommandType::BellsAndWhistles => {
                if !self.params.is_empty() {
                    Some(IgsCommand::BellsAndWhistles {
                        sound_number: self.params[0] as u8,
                    })
                } else {
                    None
                }
            }
            IgsCommandType::GraphicScaling => {
                if !self.params.is_empty() {
                    Some(IgsCommand::GraphicScaling { enabled: self.params[0] != 0 })
                } else {
                    None
                }
            }
            IgsCommandType::WriteText => {
                if self.params.len() >= 3 {
                    Some(IgsCommand::WriteText {
                        x: self.params[0],
                        y: self.params[1],
                        justification: self.params[2] as u8,
                        text: self.text_buffer.clone(),
                    })
                } else {
                    None
                }
            }
            IgsCommandType::LoopCommand => {
                if self.params.len() >= 4 {
                    Some(IgsCommand::LoopCommand {
                        x1: self.params[0],
                        y1: self.params[1],
                        x2: self.params[2],
                        y2: self.params[3],
                        command: self.loop_command.clone(),
                        parameters: self.loop_parameters.clone(),
                    })
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(cmd) = command {
            sink.emit_igs(cmd);
        }

        self.reset_params();
    }
}

impl Default for IgsParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandParser for IgsParser {
    fn parse(&mut self, data: &[u8], sink: &mut dyn CommandSink) {
        for &byte in data {
            let ch = byte as char;

            match self.state {
                State::Default => {
                    match byte {
                        b'G' => {
                            self.state = State::GotG;
                        }
                        0x1B => {
                            // ESC - VT52 escape sequence
                            self.state = State::Escape;
                        }
                        0x08 | 0x0B | 0x0C => {
                            // Backspace
                            sink.emit(TerminalCommand::Backspace);
                        }
                        0x0D => {
                            // Carriage return / Line feed
                            sink.emit(TerminalCommand::CarriageReturn);
                        }
                        0x0A => {
                            sink.emit(TerminalCommand::LineFeed);
                        }
                        0x07 => {
                            sink.emit(TerminalCommand::Bell);
                        }
                        0x00..=0x06 | 0x0E..=0x1A | 0x1C..=0x1F => {
                            // Ignore control characters
                        }
                        _ => {
                            // Regular character
                            sink.print(&[byte]);
                        }
                    }
                }
                State::GotG => {
                    if ch == '#' {
                        self.state = State::GotIgsStart;
                        self.reset_params();
                    } else {
                        // False alarm, just 'G' followed by something else
                        sink.print(b"G");
                        if byte >= 0x20 {
                            sink.print(&[byte]);
                        }
                        self.state = State::Default;
                    }
                }
                State::GotIgsStart => {
                    if ch == '&' {
                        // Loop command
                        self.state = State::ReadParams(IgsCommandType::LoopCommand);
                        self.loop_state = LoopParseState::ReadingInitialParams;
                        self.loop_parameters.clear();
                        self.loop_command.clear();
                        self.loop_param_count = 0;
                    } else if let Some(cmd_type) = IgsCommandType::from_char(ch) {
                        self.state = State::ReadParams(cmd_type);
                    } else {
                        // Unknown command, go back to default
                        self.state = State::Default;
                    }
                }
                State::_ReadCommandChar => {
                    if let Some(cmd_type) = IgsCommandType::from_char(ch) {
                        self.state = State::ReadParams(cmd_type);
                    } else {
                        self.state = State::Default;
                    }
                }
                State::ReadParams(cmd_type) => {
                    match ch {
                        '0'..='9' => {
                            self.current_param = self.current_param.wrapping_mul(10).wrapping_add((byte - b'0') as i32);
                        }
                        ',' => {
                            self.push_current_param();

                            // Check if we need to read text string for WriteText command
                            if cmd_type == IgsCommandType::WriteText && self.params.len() == 3 {
                                self.state = State::ReadTextString(self.params[0], self.params[1], self.params[2] as u8);
                            }
                        }
                        ':' | '\n' => {
                            // Command terminator
                            self.push_current_param();
                            self.emit_command(cmd_type, sink);

                            if ch == '\n' {
                                self.state = State::Default;
                            } else {
                                self.state = State::GotIgsStart;
                            }
                        }
                        ' ' | '>' | '\r' | '_' => {
                            // Whitespace/formatting - ignore
                        }
                        _ => {
                            // Invalid character, abort command
                            self.reset_params();
                            self.state = State::Default;
                        }
                    }
                }
                State::ReadTextString(_x, _y, _just) => {
                    if ch == '@' || ch == '\n' {
                        // End of text string
                        self.emit_command(IgsCommandType::WriteText, sink);
                        self.state = if ch == '\n' { State::Default } else { State::GotIgsStart };
                    } else {
                        self.text_buffer.push(ch);
                    }
                }
                State::_ReadLoopParams => {
                    // Loop command parsing - simplified for now
                    // Full implementation would parse the loop syntax
                    if ch == ':' || ch == '\n' {
                        self.emit_command(IgsCommandType::LoopCommand, sink);
                        self.state = if ch == '\n' { State::Default } else { State::GotIgsStart };
                    }
                }

                // VT52 escape sequences
                State::Escape => {
                    match ch {
                        'A' => {
                            sink.emit_igs(IgsCommand::CursorUp);
                            self.state = State::Default;
                        }
                        'B' => {
                            sink.emit_igs(IgsCommand::CursorDown);
                            self.state = State::Default;
                        }
                        'C' => {
                            sink.emit_igs(IgsCommand::CursorRight);
                            self.state = State::Default;
                        }
                        'D' => {
                            sink.emit_igs(IgsCommand::CursorLeft);
                            self.state = State::Default;
                        }
                        'E' => {
                            sink.emit_igs(IgsCommand::ClearScreen);
                            self.state = State::Default;
                        }
                        'H' => {
                            sink.emit_igs(IgsCommand::CursorHome);
                            self.state = State::Default;
                        }
                        'J' => {
                            sink.emit_igs(IgsCommand::ClearToEOS);
                            self.state = State::Default;
                        }
                        'K' => {
                            sink.emit_igs(IgsCommand::ClearToEOL);
                            self.state = State::Default;
                        }
                        'Y' => {
                            self.state = State::ReadCursorX;
                        }
                        'b' => {
                            self.state = State::ReadFgColor;
                        }
                        'c' => {
                            self.state = State::ReadBgColor;
                        }
                        'e' => {
                            sink.emit_igs(IgsCommand::ShowCursor);
                            self.state = State::Default;
                        }
                        'f' => {
                            sink.emit_igs(IgsCommand::HideCursor);
                            self.state = State::Default;
                        }
                        'j' => {
                            sink.emit_igs(IgsCommand::SaveCursorPos);
                            self.state = State::Default;
                        }
                        'k' => {
                            sink.emit_igs(IgsCommand::RestoreCursorPos);
                            self.state = State::Default;
                        }
                        _ => {
                            // Unknown escape sequence, ignore
                            self.state = State::Default;
                        }
                    }
                }
                State::ReadFgColor => {
                    let color = (byte.wrapping_sub(b'0')) as u8;
                    sink.emit_igs(IgsCommand::SetForeground { color });
                    self.state = State::Default;
                }
                State::ReadBgColor => {
                    let color = (byte.wrapping_sub(b'0')) as u8;
                    sink.emit_igs(IgsCommand::SetBackground { color });
                    self.state = State::Default;
                }
                State::ReadCursorX => {
                    let x = (byte.wrapping_sub(b' ')) as i32;
                    self.state = State::ReadCursorY(x);
                }
                State::ReadCursorY(x) => {
                    let y = (byte.wrapping_sub(b' ')) as i32;
                    sink.emit_igs(IgsCommand::SetCursorPos { x, y });
                    self.state = State::Default;
                }
            }
        }
    }
}
