//! IGS (Interactive Graphics System) parser
//!
//! IGS is a graphics system developed for Atari ST BBS systems.
//! Commands start with 'G#' and use single-letter command codes followed by parameters.

use crate::{Color, CommandParser, CommandSink, DecPrivateMode, Direction, EraseInDisplayMode, EraseInLineMode, SgrAttribute, TerminalCommand};

mod types;
pub use types::*;

mod command_type;
pub use command_type::IgsCommandType;

mod command;
pub use command::*;

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Default,
    GotG,
    GotIgsStart,
    ReadParams(IgsCommandType),
    ReadTextString(i32, i32, u8), // x, y, justification
    ReadLoopTokens,               // specialized loop command token parsing
    ReadZoneString(Vec<i32>),     // extended command X 4 zone string reading after numeric params
    ReadFillPattern(i32),         // extended command X 7 pattern data reading after id,pattern

    // VT52 states
    Escape,
    ReadFgColor,
    ReadBgColor,
    ReadCursorX,
    ReadCursorY(i32), // row position
    ReadInsertLineCount,
}

pub struct IgsParser {
    state: State,
    params: Vec<i32>,
    current_param: i32,
    text_buffer: String,

    loop_command: String,
    loop_parameters: Vec<Vec<String>>,
    _loop_param_count: i32,
    _loop_state: LoopParseState,
    loop_tokens: Vec<String>,
    loop_token_buffer: String,
    reading_chain_gang: bool, // True when reading >XXX@ chain-gang identifier

    skip_next_lf: bool, // used for skipping LF in igs line G>....\n otherwise screen would scroll.
}

static ATARI_COLOR_MAP: [u8; 16] = [0x00, 0x02, 0x03, 0x01, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F];

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
            _loop_param_count: 0,
            _loop_state: LoopParseState::ReadingInitialParams,
            loop_tokens: Vec::new(),
            loop_token_buffer: String::new(),
            reading_chain_gang: false,
            skip_next_lf: false,
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

    #[inline(always)]
    const fn get_parameter_name(expected: i32) -> Option<&'static str> {
        match expected {
            0 => Some("No parameters"),
            1 => Some("1 parameter"),
            2 => Some("2 parameter"),
            3 => Some("3 parameter"),
            4 => Some("4 parameter"),
            5 => Some("5 parameter"),
            6 => Some("6 parameter"),
            7 => Some("7 parameter"),
            8 => Some("8 parameter"),
            _ => None,
        }
    }

    #[inline(always)]
    fn check_parameters<F, T>(&self, sink: &mut dyn CommandSink, command: &'static str, expected: usize, cmd: F) -> Option<T>
    where
        F: FnOnce() -> T,
    {
        if self.params.len() < expected {
            sink.report_errror(
                crate::ParseError::InvalidParameter {
                    command,
                    value: self.params.len() as u16,
                    expected: Self::get_parameter_name(expected as i32),
                },
                crate::ErrorLevel::Error,
            );
            None
        } else {
            if self.params.len() > expected {
                sink.report_errror(
                    crate::ParseError::InvalidParameter {
                        command,
                        value: self.params.len() as u16,
                        expected: Self::get_parameter_name(expected as i32),
                    },
                    crate::ErrorLevel::Warning,
                );
            }
            Some(cmd())
        }
    }

    fn emit_command(&mut self, cmd_type: IgsCommandType, sink: &mut dyn CommandSink) {
        let command = match cmd_type {
            IgsCommandType::Box => self.check_parameters(sink, "Box", 5, || IgsCommand::Box {
                x1: self.params[0],
                y1: self.params[1],
                x2: self.params[2],
                y2: self.params[3],
                rounded: self.params[4] != 0,
            }),
            IgsCommandType::Line => self.check_parameters(sink, "Line", 4, || IgsCommand::Line {
                x1: self.params[0],
                y1: self.params[1],
                x2: self.params[2],
                y2: self.params[3],
            }),
            IgsCommandType::LineDrawTo => self.check_parameters(sink, "LineDrawTo", 2, || IgsCommand::LineDrawTo {
                x: self.params[0],
                y: self.params[1],
            }),
            IgsCommandType::Circle => self.check_parameters(sink, "Circle", 3, || IgsCommand::Circle {
                x: self.params[0],
                y: self.params[1],
                radius: self.params[2],
            }),
            IgsCommandType::Ellipse => self.check_parameters(sink, "Ellipse", 4, || IgsCommand::Ellipse {
                x: self.params[0],
                y: self.params[1],
                x_radius: self.params[2],
                y_radius: self.params[3],
            }),
            IgsCommandType::Arc => self.check_parameters(sink, "Arc", 5, || IgsCommand::Arc {
                x: self.params[0],
                y: self.params[1],
                radius: self.params[2],
                start_angle: self.params[3],
                end_angle: self.params[4],
            }),
            IgsCommandType::ColorSet => self.check_parameters(sink, "ColorSet", 2, || IgsCommand::ColorSet {
                pen: self.params[0].into(),
                color: self.params[1] as u8,
            }),
            IgsCommandType::AttributeForFills => self.check_parameters(sink, "AttributeForFills", 3, || {
                let pattern_type = match self.params[0] {
                    0 => PatternType::Hollow,
                    1 => PatternType::Solid,
                    2 => PatternType::Pattern(self.params[1] as u8),
                    3 => PatternType::Hatch(self.params[1] as u8),
                    4 => PatternType::UserDefined(self.params[1] as u8),
                    _ => PatternType::Solid,
                };
                IgsCommand::AttributeForFills {
                    pattern_type,
                    border: self.params[2] != 0,
                }
            }),
            IgsCommandType::TextEffects => self.check_parameters(sink, "TextEffects", 3, || IgsCommand::TextEffects {
                effects: TextEffects::from_bits_truncate(self.params[0] as u8),
                size: self.params[1] as u8,
                rotation: self.params[2].into(),
            }),
            IgsCommandType::FloodFill => self.check_parameters(sink, "FloodFill", 2, || IgsCommand::FloodFill {
                x: self.params[0],
                y: self.params[1],
            }),
            IgsCommandType::PolyMarker => self.check_parameters(sink, "PolyMarker", 2, || IgsCommand::PolymarkerPlot {
                x: self.params[0],
                y: self.params[1],
            }),
            IgsCommandType::SetPenColor => self.check_parameters(sink, "SetPenColor", 4, || IgsCommand::SetPenColor {
                pen: self.params[0] as u8,
                red: self.params[1] as u8,
                green: self.params[2] as u8,
                blue: self.params[3] as u8,
            }),
            IgsCommandType::DrawingMode => self.check_parameters(sink, "DrawingMode", 1, || IgsCommand::DrawingMode { mode: self.params[0].into() }),
            IgsCommandType::HollowSet => self.check_parameters(sink, "HollowSet", 1, || IgsCommand::HollowSet { enabled: self.params[0] != 0 }),
            IgsCommandType::Initialize => self.check_parameters(sink, "Initialize", 1, || IgsCommand::Initialize { mode: self.params[0].into() }),
            IgsCommandType::EllipticalArc => self.check_parameters(sink, "EllipticalArc", 6, || IgsCommand::EllipticalArc {
                x: self.params[0],
                y: self.params[1],
                x_radius: self.params[2],
                y_radius: self.params[3],
                start_angle: self.params[4],
                end_angle: self.params[5],
            }),
            IgsCommandType::Cursor => self.check_parameters(sink, "Cursor", 1, || IgsCommand::Cursor { mode: self.params[0].into() }),
            IgsCommandType::ChipMusic => self.check_parameters(sink, "ChipMusic", 6, || IgsCommand::ChipMusic {
                sound_effect: self.params[0].into(),
                voice: self.params[1] as u8,
                volume: self.params[2] as u8,
                pitch: self.params[3] as u8,
                timing: self.params[4],
                stop_type: self.params[5] as u8,
            }),
            IgsCommandType::ScreenClear => self.check_parameters(sink, "ScreenClear", 1, || IgsCommand::ScreenClear { mode: self.params[0] as u8 }),
            IgsCommandType::SetResolution => self.check_parameters(sink, "SetResolution", 2, || IgsCommand::SetResolution {
                resolution: self.params[0] as u8,
                palette: self.params[1] as u8,
            }),
            IgsCommandType::LineType => self.check_parameters(sink, "LineType", 3, || {
                let kind = if self.params[0] == 1 {
                    LineStyleKind::Polymarker(self.params[1].into())
                } else {
                    LineStyleKind::Line(self.params[1].into())
                };
                IgsCommand::LineStyle {
                    kind,
                    value: self.params[2] as u16,
                }
            }),
            IgsCommandType::PauseSeconds => self.check_parameters(sink, "PauseSeconds", 1, || IgsCommand::PauseSeconds { seconds: self.params[0] as u8 }),
            IgsCommandType::VsyncPause => self.check_parameters(sink, "VsyncPause", 1, || IgsCommand::VsyncPause { vsyncs: self.params[0] }),
            IgsCommandType::PolyLine | IgsCommandType::PolyFill => {
                if self.params.is_empty() {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: if cmd_type == IgsCommandType::PolyLine { "PolyLine" } else { "PolyFill" },
                            value: 0,
                            expected: Some("1 parameter"),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let count = self.params[0] as usize;
                    let expected = 1 + count * 2;
                    if self.params.len() < expected {
                        sink.report_errror(
                            crate::ParseError::InvalidParameter {
                                command: if cmd_type == IgsCommandType::PolyLine { "PolyLine" } else { "PolyFill" },
                                value: self.params.len() as u16,
                                expected: None,
                            },
                            crate::ErrorLevel::Error,
                        );
                        None
                    } else {
                        if self.params.len() > expected {
                            sink.report_errror(
                                crate::ParseError::InvalidParameter {
                                    command: if cmd_type == IgsCommandType::PolyLine { "PolyLine" } else { "PolyFill" },
                                    value: self.params.len() as u16,
                                    expected: None,
                                },
                                crate::ErrorLevel::Warning,
                            );
                        }
                        let points = self.params[1..].to_vec();
                        if cmd_type == IgsCommandType::PolyLine {
                            Some(IgsCommand::PolyLine { points })
                        } else {
                            Some(IgsCommand::PolyFill { points })
                        }
                    }
                }
            }
            IgsCommandType::BellsAndWhistles => {
                if self.params.is_empty() {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: "BellsAndWhistles",
                            value: 0,
                            expected: Some("1 parameter"),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let cmd_id = self.params[0];
                    match cmd_id {
                        20 => {
                            // b>20,play_flag,snd_num,element_num,negative_flag,thousands,hundreds:
                            if self.params.len() < 7 {
                                sink.report_errror(
                                    crate::ParseError::InvalidParameter {
                                        command: "BellsAndWhistles:AlterSoundEffect",
                                        value: self.params.len() as u16,
                                        expected: Some("7 parameter"),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                None
                            } else {
                                if self.params.len() > 7 {
                                    sink.report_errror(
                                        crate::ParseError::InvalidParameter {
                                            command: "BellsAndWhistles:AlterSoundEffect",
                                            value: self.params.len() as u16,
                                            expected: Some("7 parameter"),
                                        },
                                        crate::ErrorLevel::Warning,
                                    );
                                }
                                Some(IgsCommand::AlterSoundEffect {
                                    play_flag: self.params[1] as u8,
                                    sound_effect: self.params[2].into(),
                                    element_num: self.params[3] as u8,
                                    negative_flag: self.params[4] as u8,
                                    thousands: self.params[5] as u16,
                                    hundreds: self.params[6] as u16,
                                })
                            }
                        }
                        21 => {
                            // b>21: - Stop all sounds
                            Some(IgsCommand::StopAllSound)
                        }
                        22 => {
                            // b>22,snd_num: - Restore sound effect
                            if self.params.len() < 2 {
                                sink.report_errror(
                                    crate::ParseError::InvalidParameter {
                                        command: "BellsAndWhistles:RestoreSoundEffect",
                                        value: self.params.len() as u16,
                                        expected: Some("2 parameter"),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                None
                            } else {
                                if self.params.len() > 2 {
                                    sink.report_errror(
                                        crate::ParseError::InvalidParameter {
                                            command: "BellsAndWhistles:RestoreSoundEffect",
                                            value: self.params.len() as u16,
                                            expected: Some("2 parameter"),
                                        },
                                        crate::ErrorLevel::Warning,
                                    );
                                }
                                Some(IgsCommand::RestoreSoundEffect {
                                    sound_effect: self.params[1].into(),
                                })
                            }
                        }
                        23 => {
                            // b>23,count: - Set effect loops
                            if self.params.len() < 2 {
                                sink.report_errror(
                                    crate::ParseError::InvalidParameter {
                                        command: "BellsAndWhistles:SetEffectLoops",
                                        value: self.params.len() as u16,
                                        expected: Some("2 parameter"),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                None
                            } else {
                                if self.params.len() > 2 {
                                    sink.report_errror(
                                        crate::ParseError::InvalidParameter {
                                            command: "BellsAndWhistles:SetEffectLoops",
                                            value: self.params.len() as u16,
                                            expected: Some("2 parameter"),
                                        },
                                        crate::ErrorLevel::Warning,
                                    );
                                }
                                Some(IgsCommand::SetEffectLoops { count: self.params[1] as u32 })
                            }
                        }
                        _ => {
                            // b>0-19: - Play sound effect
                            Some(IgsCommand::BellsAndWhistles { sound_effect: cmd_id.into() })
                        }
                    }
                }
            }
            IgsCommandType::GraphicScaling => self.check_parameters(sink, "GraphicScaling", 1, || IgsCommand::GraphicScaling { mode: self.params[0] as u8 }),
            IgsCommandType::GrabScreen => {
                if self.params.len() < 2 {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: "GrabScreen",
                            value: self.params.len() as u16,
                            expected: Some("2 parameter"),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let blit_type_id = self.params[0];
                    let mode: BlitMode = self.params[1].into();

                    let operation = match blit_type_id {
                        0 => {
                            // Screen to screen: needs 6 params
                            self.check_parameters(sink, "GrabScreen:ScreenToScreen", 8, || BlitOperation::ScreenToScreen {
                                src_x1: self.params[2],
                                src_y1: self.params[3],
                                src_x2: self.params[4],
                                src_y2: self.params[5],
                                dest_x: self.params[6],
                                dest_y: self.params[7],
                            })
                        }
                        1 => {
                            // Screen to memory: needs 4 params (6 total with blit_type_id and mode)
                            self.check_parameters(sink, "GrabScreen:ScreenToMemory", 6, || BlitOperation::ScreenToMemory {
                                src_x1: self.params[2],
                                src_y1: self.params[3],
                                src_x2: self.params[4],
                                src_y2: self.params[5],
                            })
                        }
                        2 => {
                            // Memory to screen: needs 2 params (4 total with blit_type_id and mode)
                            self.check_parameters(sink, "GrabScreen:MemoryToScreen", 4, || BlitOperation::MemoryToScreen {
                                dest_x: self.params[2],
                                dest_y: self.params[3],
                            })
                        }
                        3 => {
                            // Piece of memory to screen: needs 6 params (8 total with blit_type_id and mode)
                            self.check_parameters(sink, "GrabScreen:PieceOfMemoryToScreen", 8, || BlitOperation::PieceOfMemoryToScreen {
                                src_x1: self.params[2],
                                src_y1: self.params[3],
                                src_x2: self.params[4],
                                src_y2: self.params[5],
                                dest_x: self.params[6],
                                dest_y: self.params[7],
                            })
                        }
                        4 => {
                            // Memory to memory: needs 6 params (8 total with blit_type_id and mode)
                            self.check_parameters(sink, "GrabScreen:MemoryToMemory", 8, || BlitOperation::MemoryToMemory {
                                src_x1: self.params[2],
                                src_y1: self.params[3],
                                src_x2: self.params[4],
                                src_y2: self.params[5],
                                dest_x: self.params[6],
                                dest_y: self.params[7],
                            })
                        }
                        _ => {
                            sink.report_errror(
                                crate::ParseError::InvalidParameter {
                                    command: "GrabScreen",
                                    value: blit_type_id as u16,
                                    expected: Some("valid blit_type_id (0-4)"),
                                },
                                crate::ErrorLevel::Error,
                            );
                            None
                        }
                    };

                    operation.map(|op| IgsCommand::GrabScreen { operation: op, mode })
                }
            }
            IgsCommandType::WriteText => self.check_parameters(sink, "WriteText", 2, || IgsCommand::WriteText {
                x: self.params[0],
                y: self.params[1],
                text: self.text_buffer.clone(),
            }),
            IgsCommandType::LoopCommand => {
                // & from,to,step,delay,cmd,param_count,(params...)
                if self.params.len() < 6 {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: "LoopCommand",
                            value: self.params.len() as u16,
                            expected: Some("6 parameter"),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let from = self.params[0];
                    let to = self.params[1];
                    let step = self.params[2];
                    let delay = self.params[3];
                    let command_identifier = self.loop_command.clone();
                    let param_count = self.params[4] as u16;

                    let mut params_tokens: Vec<LoopParamToken> = Vec::new();
                    // Remaining numeric params are converted to tokens unless already substituted tokens in loop_parameters
                    if self.params.len() > 5 {
                        for p in &self.params[5..] {
                            params_tokens.push(LoopParamToken::Number(*p));
                        }
                    }
                    // Add any textual parameter tokens captured (x,y,+n etc.)
                    for token_group in &self.loop_parameters {
                        for token in token_group {
                            match token.as_str() {
                                ":" => params_tokens.push(LoopParamToken::GroupSeparator),
                                "x" | "y" => params_tokens.push(LoopParamToken::Symbol(token.chars().next().unwrap())),
                                _ => {
                                    // Check if token starts with a prefix operator (+, -, !)
                                    let has_prefix = token.starts_with('+') || token.starts_with('-') || token.starts_with('!');
                                    if !has_prefix && token.parse::<i32>().is_ok() {
                                        params_tokens.push(LoopParamToken::Number(token.parse::<i32>().unwrap()));
                                    } else {
                                        params_tokens.push(LoopParamToken::Expr(token.clone()));
                                    }
                                }
                            }
                        }
                    }

                    let mut modifiers = LoopModifiers::default();
                    let original_ident = command_identifier.as_str();
                    let mut base_ident = original_ident;
                    if let Some(pos) = base_ident.find(|c| c == '|' || c == '@') {
                        let (ident_part, mod_part) = base_ident.split_at(pos);
                        base_ident = ident_part;
                        for ch in mod_part.chars() {
                            match ch {
                                '|' => modifiers.xor_stepping = true,
                                '@' => modifiers.refresh_text_each_iteration = true,
                                _ => {}
                            }
                        }
                    }

                    let target = if base_ident.starts_with('>') && original_ident.ends_with('@') {
                        let inner: String = base_ident.chars().skip(1).collect();
                        let commands: Vec<char> = inner.chars().collect();
                        LoopTarget::ChainGang {
                            raw: original_ident.to_string(),
                            commands,
                        }
                    } else {
                        let ch = base_ident.chars().next().unwrap_or(' ');
                        LoopTarget::Single(ch)
                    };

                    Some(IgsCommand::Loop(LoopCommandData {
                        from,
                        to,
                        step,
                        delay,
                        target,
                        modifiers,
                        param_count,
                        params: params_tokens,
                    }))
                }
            }
            IgsCommandType::Noise => Some(IgsCommand::Noise { params: self.params.clone() }),
            IgsCommandType::RoundedRectangles => self.check_parameters(sink, "RoundedRectangles", 5, || IgsCommand::RoundedRectangles {
                x1: self.params[0],
                y1: self.params[1],
                x2: self.params[2],
                y2: self.params[3],
                fill: self.params[4] != 0,
            }),
            IgsCommandType::PieSlice => self.check_parameters(sink, "PieSlice", 5, || IgsCommand::PieSlice {
                x: self.params[0],
                y: self.params[1],
                radius: self.params[2],
                start_angle: self.params[3],
                end_angle: self.params[4],
            }),
            IgsCommandType::ExtendedCommand => {
                // X - Extended commands
                if self.params.is_empty() {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: "ExtendedCommand",
                            value: 0,
                            expected: Some("1 parameter"),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let cmd_id = self.params[0];
                    match cmd_id {
                        0 => {
                            // SprayPaint (id,x,y,width,height,density)
                            self.check_parameters(sink, "ExtendedCommand:SprayPaint", 6, || IgsCommand::SprayPaint {
                                x: self.params[1],
                                y: self.params[2],
                                width: self.params[3],
                                height: self.params[4],
                                density: self.params[5],
                            })
                        }
                        1 => {
                            // SetColorRegister
                            self.check_parameters(sink, "ExtendedCommand:SetColorRegister", 3, || IgsCommand::SetColorRegister {
                                register: self.params[1] as u8,
                                value: self.params[2],
                            })
                        }
                        2 => {
                            // SetRandomRange
                            Some(IgsCommand::SetRandomRange {
                                params: self.params[1..].to_vec(),
                            })
                        }
                        3 => {
                            // RightMouseMacro
                            Some(IgsCommand::RightMouseMacro {
                                params: self.params[1..].to_vec(),
                            })
                        }
                        4 => {
                            // DefineZone: Special handling for clear (9999-9997)
                            if self.params.len() == 2 && (9997..=9999).contains(&self.params[1]) {
                                // Clear command or loopback toggle - no additional params needed
                                Some(IgsCommand::DefineZone {
                                    zone_id: self.params[1],
                                    x1: 0,
                                    y1: 0,
                                    x2: 0,
                                    y2: 0,
                                    length: 0,
                                    string: String::new(),
                                })
                            } else if self.params.len() >= 8 && !self.text_buffer.is_empty() {
                                Some(IgsCommand::DefineZone {
                                    zone_id: self.params[1],
                                    x1: self.params[2],
                                    y1: self.params[3],
                                    x2: self.params[4],
                                    y2: self.params[5],
                                    length: self.params[6] as u16,
                                    string: self.text_buffer.clone(),
                                })
                            } else {
                                sink.report_errror(
                                    crate::ParseError::InvalidParameter {
                                        command: "ExtendedCommand:DefineZone",
                                        value: self.params.len() as u16,
                                        expected: Some("2 parameter (9997-9999) or 8+ parameter with text"),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                None
                            }
                        }
                        5 => {
                            // FlowControl
                            self.check_parameters(sink, "ExtendedCommand:FlowControl", 2, || IgsCommand::FlowControl {
                                mode: self.params[1] as u8,
                                params: self.params[2..].to_vec(),
                            })
                        }
                        6 => {
                            // LeftMouseButton
                            self.check_parameters(sink, "ExtendedCommand:LeftMouseButton", 2, || IgsCommand::LeftMouseButton {
                                mode: self.params[1] as u8,
                            })
                        }
                        7 => {
                            // LoadFillPattern
                            self.check_parameters(sink, "ExtendedCommand:LoadFillPattern", 2, || IgsCommand::LoadFillPattern {
                                pattern: self.params[1] as u8,
                                data: self.text_buffer.clone(),
                            })
                        }
                        8 => {
                            // RotateColorRegisters
                            self.check_parameters(sink, "ExtendedCommand:RotateColorRegisters", 5, || IgsCommand::RotateColorRegisters {
                                start_reg: self.params[1] as u8,
                                end_reg: self.params[2] as u8,
                                count: self.params[3],
                                delay: self.params[4],
                            })
                        }
                        9 => {
                            // LoadMidiBuffer
                            Some(IgsCommand::LoadMidiBuffer {
                                params: self.params[1..].to_vec(),
                            })
                        }
                        10 => {
                            // SetDrawtoBegin
                            self.check_parameters(sink, "ExtendedCommand:SetDrawtoBegin", 3, || IgsCommand::SetDrawtoBegin {
                                x: self.params[1],
                                y: self.params[2],
                            })
                        }
                        11 => {
                            // LoadBitblitMemory
                            Some(IgsCommand::LoadBitblitMemory {
                                params: self.params[1..].to_vec(),
                            })
                        }
                        12 => {
                            // LoadColorPalette
                            Some(IgsCommand::LoadColorPalette {
                                params: self.params[1..].to_vec(),
                            })
                        }
                        _ => {
                            sink.report_errror(
                                crate::ParseError::InvalidParameter {
                                    command: "ExtendedCommand",
                                    value: cmd_id as u16,
                                    expected: Some("valid cmd_id (0-12)"),
                                },
                                crate::ErrorLevel::Error,
                            );
                            None
                        }
                    }
                }
            }
            IgsCommandType::EllipticalPieSlice => self.check_parameters(sink, "EllipticalPieSlice", 6, || IgsCommand::EllipticalPieSlice {
                x: self.params[0],
                y: self.params[1],
                x_radius: self.params[2],
                y_radius: self.params[3],
                start_angle: self.params[4],
                end_angle: self.params[5],
            }),
            IgsCommandType::FilledRectangle => self.check_parameters(sink, "FilledRectangle", 4, || IgsCommand::FilledRectangle {
                x1: self.params[0],
                y1: self.params[1],
                x2: self.params[2],
                y2: self.params[3],
            }),
            IgsCommandType::CursorMotion => {
                // m - cursor motion
                // IG form: direction,count
                // ESC form previously provided x,y; map to direction/count
                if self.params.len() < 2 {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: "CursorMotion",
                            value: self.params.len() as u16,
                            expected: Some("2 parameter"),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let a = self.params[0];
                    let b = self.params[1];
                    // Heuristic: if both non-zero prefer horizontal if y==0 else vertical
                    let (direction, count) = if a != 0 && b == 0 {
                        if a > 0 { (Direction::Right, a) } else { (Direction::Left, -a) }
                    } else if b != 0 && a == 0 {
                        if b > 0 { (Direction::Down, b) } else { (Direction::Up, -b) }
                    } else {
                        // Assume IG form already direction,count
                        let dir = match a {
                            0 => Direction::Up,
                            1 => Direction::Down,
                            2 => Direction::Left,
                            _ => Direction::Right,
                        };
                        (dir, b)
                    };
                    Some(IgsCommand::CursorMotion { direction, count })
                }
            }
            IgsCommandType::PositionCursor => self.check_parameters(sink, "PositionCursor", 2, || IgsCommand::PositionCursor {
                x: self.params[0],
                y: self.params[1],
            }),
            IgsCommandType::InverseVideo => self.check_parameters(sink, "InverseVideo", 1, || IgsCommand::InverseVideo { enabled: self.params[0] != 0 }),
            IgsCommandType::LineWrap => self.check_parameters(sink, "LineWrap", 1, || IgsCommand::LineWrap { enabled: self.params[0] != 0 }),
            IgsCommandType::InputCommand => self.check_parameters(sink, "InputCommand", 1, || IgsCommand::InputCommand {
                input_type: self.params[0] as u8,
                params: self.params[1..].to_vec(),
            }),
            IgsCommandType::AskIG => {
                if self.params.is_empty() {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: "AskIG",
                            value: 0,
                            expected: Some("at least 1 parameter required"),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let query = match self.params[0] {
                        0 => Some(AskQuery::VersionNumber),
                        1 => {
                            let pointer_type = if self.params.len() > 1 {
                                MousePointerType::from(self.params[1])
                            } else {
                                MousePointerType::Immediate
                            };
                            Some(AskQuery::CursorPositionAndMouseButton { pointer_type })
                        }
                        2 => {
                            let pointer_type = if self.params.len() > 1 {
                                MousePointerType::from(self.params[1])
                            } else {
                                MousePointerType::Immediate
                            };
                            Some(AskQuery::MousePositionAndButton { pointer_type })
                        }
                        3 => Some(AskQuery::CurrentResolution),
                        _ => {
                            sink.report_errror(
                                crate::ParseError::InvalidParameter {
                                    command: "AskIG",
                                    value: self.params[0] as u16,
                                    expected: Some("valid query type (0-3)"),
                                },
                                crate::ErrorLevel::Error,
                            );
                            None
                        }
                    };
                    query.map(|q| IgsCommand::AskIG { query: q })
                }
            }
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
                            if self.skip_next_lf {
                                self.skip_next_lf = false;
                                continue;
                            }
                            sink.emit(TerminalCommand::LineFeed);
                        } /*
                        0x07 => {
                        sink.emit(TerminalCommand::Bell);
                        }*/
                        0x00..=0x0F => {
                            // TOS direct foreground color codes (0x00-0x0F)
                            // 0x07 (Bell) is excluded to maintain standard ASCII compatibility
                            sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(
                                ATARI_COLOR_MAP[byte as usize],
                            ))));
                        } /*
                        0x09 => {
                        sink.emit(TerminalCommand::Tab);
                        }*/
                        0x0E..=0x1A | 0x1C..=0x1F => {
                            // Ignore control characters
                        }
                        _ => {
                            // Regular character
                            sink.print(&[byte]);
                        }
                    }
                }
                State::GotG => {
                    self.skip_next_lf = true;
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
                        // Use specialized token parser for loop command because parameters include substitution tokens.
                        self.state = State::ReadLoopTokens;
                        self.loop_tokens.clear();
                        self.loop_token_buffer.clear();
                    } else if let Some(cmd_type) = IgsCommandType::from_char(ch) {
                        self.state = State::ReadParams(cmd_type);
                    } else {
                        // Unknown command
                        if !ch.is_control() {
                            sink.report_errror(
                                crate::ParseError::InvalidParameter {
                                    command: "IGS",
                                    value: byte as u16,
                                    expected: Some("valid IGS command character"),
                                },
                                crate::ErrorLevel::Error,
                            );
                        }
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
                            // For WriteText: after 2 params (x, y), next non-separator char starts text
                            if cmd_type == IgsCommandType::WriteText && self.params.len() == 2 {
                                // W>x,y,text@ - text follows immediately after second comma
                                self.state = State::ReadTextString(self.params[0], self.params[1], 0);
                                self.text_buffer.clear();
                            }
                        }
                        '@' if cmd_type == IgsCommandType::WriteText => {
                            // For WriteText: @ starts text after x,y params
                            self.push_current_param();
                            if self.params.len() == 2 {
                                // W>x,y@text@ format
                                self.state = State::ReadTextString(self.params[0], self.params[1], 0);
                                self.text_buffer.clear();
                            } else {
                                // Invalid - WriteText needs exactly 2 params before @
                                self.reset_params();
                                self.state = State::Default;
                            }
                        }
                        ':' => {
                            // Command terminator
                            self.push_current_param();
                            self.emit_command(cmd_type, sink);
                            self.state = State::GotIgsStart;
                        }
                        ' ' | '>' | '\r' | '\n' | '_' => {
                            // Whitespace/formatting - ignore
                            // Special handling: extended command X 4 (DefineZone) starts string after 7 numeric params
                            if let State::ReadParams(IgsCommandType::ExtendedCommand) = self.state {
                                if !self.params.is_empty() && self.params[0] == 4 && self.params.len() == 7 {
                                    // Switch into zone string reading state (length already captured)
                                    self.state = State::ReadZoneString(self.params.clone());
                                    self.text_buffer.clear();
                                } else if !self.params.is_empty() && self.params[0] == 7 && self.params.len() == 2 {
                                    // Switch to fill pattern reading state
                                    let pattern = self.params[1];
                                    self.state = State::ReadFillPattern(pattern);
                                    self.text_buffer.clear();
                                }
                            }
                        }
                        _ => {
                            // Extended command X 4 zone string may contain arbitrary characters until ':'
                            if let State::ReadParams(IgsCommandType::ExtendedCommand) = self.state {
                                if !self.params.is_empty() && self.params[0] == 4 && self.params.len() == 7 {
                                    self.state = State::ReadZoneString(self.params.clone());
                                    self.text_buffer.clear();
                                    self.text_buffer.push(ch);
                                } else if !self.params.is_empty() && self.params[0] == 7 && self.params.len() == 2 {
                                    let pattern = self.params[1];
                                    self.state = State::ReadFillPattern(pattern);
                                    self.text_buffer.clear();
                                    self.text_buffer.push(ch);
                                } else {
                                    // Invalid for other extended commands
                                    sink.report_errror(
                                        crate::ParseError::InvalidParameter {
                                            command: "ExtendedCommand",
                                            value: ch as u16,
                                            expected: Some("digit, ',', ':' oder gültiger Text für X4/X7"),
                                        },
                                        crate::ErrorLevel::Error,
                                    );
                                    self.reset_params();
                                    self.state = State::Default;
                                }
                            } else {
                                // Invalid character in numeric parameter phase for non-extended commands
                                sink.report_errror(
                                    crate::ParseError::InvalidParameter {
                                        command: "IGS:ReadParams",
                                        value: ch as u16,
                                        expected: Some("Ziffer, ',', ':' oder Whitespace"),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                self.reset_params();
                                self.state = State::Default;
                            }
                        }
                    }
                }
                State::ReadZoneString(ref zone_params) => {
                    match ch {
                        ':' | '\n' => {
                            // Terminator: build DefineZone command (X 4)
                            if zone_params.len() == 7 {
                                let zone_id = zone_params[1];
                                let x1 = zone_params[2];
                                let y1 = zone_params[3];
                                let x2 = zone_params[4];
                                let y2 = zone_params[5];
                                let length = zone_params[6] as u16;
                                let string = self.text_buffer.clone();
                                sink.emit_igs(IgsCommand::DefineZone {
                                    zone_id,
                                    x1,
                                    y1,
                                    x2,
                                    y2,
                                    length,
                                    string,
                                });
                            }
                            self.reset_params();
                            self.state = if ch == '\n' { State::Default } else { State::GotIgsStart };
                        }
                        _ => {
                            self.text_buffer.push(ch);
                        }
                    }
                }
                State::ReadLoopTokens => {
                    match ch {
                        ':' => {
                            if !self.loop_token_buffer.is_empty() {
                                self.loop_tokens.push(self.loop_token_buffer.clone());
                                self.loop_token_buffer.clear();
                            }

                            // Check if we have enough tokens and if we've collected all expected parameters
                            if self.loop_tokens.len() >= 6 {
                                let parse_i32 = |s: &str| s.parse::<i32>().unwrap_or(0);
                                let param_count = parse_i32(&self.loop_tokens[5]) as usize;
                                // Count actual parameters (excluding ':' markers)
                                let current_param_count = self.loop_tokens[6..].iter().filter(|s| *s != ":").count();

                                // If we have collected all parameters, emit the command
                                if current_param_count >= param_count {
                                    let from = parse_i32(&self.loop_tokens[0]);
                                    let to = parse_i32(&self.loop_tokens[1]);
                                    let step = parse_i32(&self.loop_tokens[2]);
                                    let delay = parse_i32(&self.loop_tokens[3]);
                                    let raw_identifier = self.loop_tokens[4].clone();

                                    // Parse target and modifiers from command identifier
                                    // For chain-gangs (>XXX@), the @ is part of the identifier, not a modifier
                                    // Modifiers come AFTER the chain-gang's closing @
                                    let mut modifiers = LoopModifiers::default();
                                    let original_ident = raw_identifier.as_str();
                                    let mut base_ident = original_ident;
                                    let mut target = LoopTarget::Single(' ');

                                    // Check if this is a chain-gang command (>...@)
                                    let is_chain_gang = base_ident.starts_with('>') && base_ident.contains('@');

                                    if is_chain_gang {
                                        // For chain-gangs, find the closing @ of the chain
                                        if let Some(chain_end_pos) = base_ident.find('@') {
                                            let after_chain = &base_ident[chain_end_pos + 1..];
                                            // Parse modifiers that come after the chain-gang's @
                                            for ch in after_chain.chars() {
                                                match ch {
                                                    '|' => modifiers.xor_stepping = true,
                                                    '@' => modifiers.refresh_text_each_iteration = true,
                                                    _ => {}
                                                }
                                            }
                                            // base_ident includes the chain-gang with its closing @
                                            base_ident = &base_ident[..=chain_end_pos];
                                            // Create ChainGang target with the base_ident (which includes @)
                                            let inner: String = base_ident.chars().skip(1).take(base_ident.len().saturating_sub(2)).collect();
                                            let commands: Vec<char> = inner.chars().collect();
                                            target = LoopTarget::ChainGang {
                                                raw: base_ident.to_string(),
                                                commands,
                                            };
                                        }
                                    } else {
                                        // For single commands, parse modifiers normally
                                        if let Some(pos) = base_ident.find(|c| c == '|' || c == '@') {
                                            let (ident_part, mod_part) = base_ident.split_at(pos);
                                            base_ident = ident_part;
                                            for ch in mod_part.chars() {
                                                match ch {
                                                    '|' => modifiers.xor_stepping = true,
                                                    '@' => modifiers.refresh_text_each_iteration = true,
                                                    _ => {}
                                                }
                                            }
                                        }
                                    }

                                    if matches!(target, LoopTarget::Single(' ')) {
                                        target = if base_ident.starts_with('>') && original_ident.ends_with('@') {
                                            let inner: String = base_ident.chars().skip(1).collect();
                                            let commands: Vec<char> = inner.chars().collect();
                                            LoopTarget::ChainGang {
                                                raw: original_ident.to_string(),
                                                commands,
                                            }
                                        } else {
                                            let ch = base_ident.chars().next().unwrap_or(' ');
                                            LoopTarget::Single(ch)
                                        };
                                    }

                                    // Convert parameters into typed tokens, preserving ':' position
                                    let mut params: Vec<LoopParamToken> = Vec::new();
                                    for token in &self.loop_tokens[6..] {
                                        if token == ":" {
                                            params.push(LoopParamToken::GroupSeparator);
                                        } else if token == "x" || token == "y" {
                                            params.push(LoopParamToken::Symbol(token.chars().next().unwrap()));
                                        } else {
                                            // Check if token starts with a prefix operator (+, -, !)
                                            let has_prefix = token.starts_with('+') || token.starts_with('-') || token.starts_with('!');
                                            if !has_prefix && token.parse::<i32>().is_ok() {
                                                params.push(LoopParamToken::Number(token.parse::<i32>().unwrap()));
                                            } else {
                                                params.push(LoopParamToken::Expr(token.clone()));
                                            }
                                        }
                                    }

                                    let data = LoopCommandData {
                                        from,
                                        to,
                                        step,
                                        delay,
                                        target,
                                        modifiers,
                                        param_count: param_count as u16,
                                        params,
                                    };

                                    sink.emit_igs(IgsCommand::Loop(data));
                                    self.loop_tokens.clear();
                                    self.loop_token_buffer.clear();
                                    self.state = State::GotIgsStart;
                                } else {
                                    // Add ':' as a marker and continue reading
                                    self.loop_tokens.push(":".to_string());
                                }
                            }
                        }
                        '\n' => {
                            if !self.loop_token_buffer.is_empty() {
                                self.loop_tokens.push(self.loop_token_buffer.clone());
                                self.loop_token_buffer.clear();
                            }
                            // Process tokens even if incomplete on newline
                            if self.loop_tokens.len() >= 6 {
                                use crate::igs::{LoopCommandData, LoopModifiers, LoopParamToken, LoopTarget};

                                let parse_i32 = |s: &str| s.parse::<i32>().unwrap_or(0);
                                let from = parse_i32(&self.loop_tokens[0]);
                                let to = parse_i32(&self.loop_tokens[1]);
                                let step = parse_i32(&self.loop_tokens[2]);
                                let delay = parse_i32(&self.loop_tokens[3]);
                                let raw_identifier = self.loop_tokens[4].clone();
                                let param_count = parse_i32(&self.loop_tokens[5]) as usize;

                                let mut modifiers = LoopModifiers::default();
                                let original_ident = raw_identifier.as_str();
                                let mut base_ident = original_ident;
                                if let Some(pos) = base_ident.find(|c| c == '|' || c == '@') {
                                    let (ident_part, mod_part) = base_ident.split_at(pos);
                                    base_ident = ident_part;
                                    for ch in mod_part.chars() {
                                        match ch {
                                            '|' => modifiers.xor_stepping = true,
                                            '@' => modifiers.refresh_text_each_iteration = true,
                                            _ => {}
                                        }
                                    }
                                }

                                let target = if base_ident.starts_with('>') && original_ident.ends_with('@') {
                                    let inner: String = base_ident.chars().skip(1).collect();
                                    let commands: Vec<char> = inner.chars().collect();
                                    LoopTarget::ChainGang {
                                        raw: original_ident.to_string(),
                                        commands,
                                    }
                                } else {
                                    let ch = base_ident.chars().next().unwrap_or(' ');
                                    LoopTarget::Single(ch)
                                };

                                let mut params: Vec<LoopParamToken> = Vec::new();
                                for token in &self.loop_tokens[6..] {
                                    if token == ":" {
                                        params.push(LoopParamToken::GroupSeparator);
                                    } else if token == "x" || token == "y" {
                                        params.push(LoopParamToken::Symbol(token.chars().next().unwrap()));
                                    } else {
                                        // Check if token starts with a prefix operator (+, -, !)
                                        let has_prefix = token.starts_with('+') || token.starts_with('-') || token.starts_with('!');
                                        if !has_prefix && token.parse::<i32>().is_ok() {
                                            params.push(LoopParamToken::Number(token.parse::<i32>().unwrap()));
                                        } else {
                                            params.push(LoopParamToken::Expr(token.clone()));
                                        }
                                    }
                                }

                                let data = LoopCommandData {
                                    from,
                                    to,
                                    step,
                                    delay,
                                    target,
                                    modifiers,
                                    param_count: param_count as u16,
                                    params,
                                };

                                sink.emit_igs(IgsCommand::Loop(data));
                            }
                            self.loop_tokens.clear();
                            self.loop_token_buffer.clear();
                            self.reading_chain_gang = false;
                            self.state = State::Default;
                        }
                        ',' => {
                            // Comma acts as parameter separator
                            if self.reading_chain_gang {
                                self.loop_token_buffer.push(ch);
                            } else if !self.loop_token_buffer.is_empty() {
                                self.loop_tokens.push(self.loop_token_buffer.clone());
                                self.loop_token_buffer.clear();
                            }
                        }
                        ')' => {
                            // Closing paren marks command index in chain-gang parameters
                            // Keep it as part of the token for display purposes
                            if !self.loop_token_buffer.is_empty() {
                                self.loop_token_buffer.push(ch);
                                self.loop_tokens.push(self.loop_token_buffer.clone());
                                self.loop_token_buffer.clear();
                            }
                        }
                        '@' => {
                            // @ can end a chain-gang identifier or be a modifier
                            self.loop_token_buffer.push(ch);
                            if self.reading_chain_gang {
                                // This @ ends the chain-gang identifier
                                // Push token and clear flag
                                self.loop_tokens.push(self.loop_token_buffer.clone());
                                self.loop_token_buffer.clear();
                                self.reading_chain_gang = false;
                            }
                        }
                        ' ' | '\r' | '_' => {
                            // ignore these formatting chars entirely for loop tokens
                        }
                        '>' => {
                            // '>' can be part of chain-gang identifier (e.g., >CL@) or a formatting char
                            // If buffer is empty and we're at the command identifier position, it starts a chain-gang
                            if self.loop_token_buffer.is_empty() && self.loop_tokens.len() == 4 {
                                // We're at the command identifier position (5th token, index 4)
                                self.loop_token_buffer.push(ch);
                                self.reading_chain_gang = true;
                            }
                            // Otherwise ignore as formatting
                        }
                        _ => {
                            self.loop_token_buffer.push(ch);
                        }
                    }
                }
                State::ReadFillPattern(pattern) => match ch {
                    ':' | '\n' => {
                        sink.emit_igs(IgsCommand::LoadFillPattern {
                            pattern: pattern as u8,
                            data: self.text_buffer.clone(),
                        });
                        self.reset_params();
                        self.state = if ch == '\n' { State::Default } else { State::GotIgsStart };
                    }
                    _ => self.text_buffer.push(ch),
                },
                State::ReadTextString(_x, _y, _just) => {
                    if ch == '@' || ch == '\n' {
                        // End of text string
                        self.emit_command(IgsCommandType::WriteText, sink);
                        self.state = if ch == '\n' { State::Default } else { State::GotIgsStart };
                    } else {
                        self.text_buffer.push(ch);
                    }
                }

                // VT52 escape sequences
                State::Escape => {
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
                            self.state = State::ReadCursorX;
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
                            sink.emit(TerminalCommand::CsiDecPrivateModeSet(DecPrivateMode::Inverse));
                            self.state = State::Default;
                        }
                        'q' => {
                            // VT52 Normal video
                            sink.emit(TerminalCommand::CsiDecPrivateModeReset(DecPrivateMode::Inverse));
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
                        'r' => {
                            // Remember cursor ESC form: value implicitly 0
                            sink.emit_igs(IgsCommand::RememberCursor { value: 0 });
                            self.state = State::Default;
                        }
                        'm' => {
                            // IGS command that can be invoked with ESC prefix instead of G#
                            // ESC m x,y:  - cursor motion
                            if let Some(cmd_type) = IgsCommandType::from_char(ch) {
                                self.state = State::ReadParams(cmd_type);
                            } else {
                                self.state = State::Default;
                            }
                        }
                        _ => {
                            // Unknown escape sequence, ignore
                            self.state = State::Default;
                        }
                    }
                }
                State::ReadFgColor => {
                    // VT52 foreground color uses ASCII digits/hex: '0'-'9' (0-9), 'A'-'F' or 'a'-'f' (10-15)
                    let color = match byte {
                        b'0'..=b'9' => byte - b'0',
                        b'A'..=b'F' => byte - b'A' + 10,
                        b'a'..=b'f' => byte - b'a' + 10,
                        _ => byte, // Fallback for non-standard values
                    };
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(
                        ATARI_COLOR_MAP[color as usize],
                    ))));
                    self.state = State::Default;
                }
                State::ReadBgColor => {
                    // VT52 background color uses ASCII digits/hex: '0'-'9' (0-9), 'A'-'F' or 'a'-'f' (10-15)
                    let color = match byte {
                        b'0'..=b'9' => byte - b'0',
                        b'A'..=b'F' => byte - b'A' + 10,
                        b'a'..=b'f' => byte - b'a' + 10,
                        _ => byte, // Fallback for non-standard values
                    };
                    sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(
                        ATARI_COLOR_MAP[color as usize],
                    ))));
                    self.state = State::Default;
                }
                State::ReadCursorX => {
                    let row = (byte.wrapping_sub(32)) as i32;
                    self.state = State::ReadCursorY(row);
                }
                State::ReadCursorY(row) => {
                    let col = (byte.wrapping_sub(32)) as i32;
                    // VT52 uses 0-based coordinates, but CsiCursorPosition uses 1-based
                    sink.emit(TerminalCommand::CsiCursorPosition((row + 1) as u16, (col + 1) as u16));
                    self.state = State::Default;
                }
                State::ReadInsertLineCount => {
                    let count = byte;
                    sink.emit(TerminalCommand::CsiInsertLine(count as u16));
                    self.state = State::Default;
                }
            }
        }
        // Flush pending ESC-style parameter commands without explicit ':' terminator (e.g. ESC m1,20)
        if let State::ReadParams(cmd_type) = self.state {
            match cmd_type {
                IgsCommandType::CursorMotion | IgsCommandType::InverseVideo | IgsCommandType::LineWrap => {
                    // Ensure last param captured
                    if self.current_param != 0 || !self.params.is_empty() {
                        self.push_current_param();
                    }
                    if !self.params.is_empty() {
                        self.emit_command(cmd_type, sink);
                        self.state = State::Default;
                    }
                }
                _ => {}
            }
        }
    }
}
