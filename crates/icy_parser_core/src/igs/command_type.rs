use crate::{
    AskQuery, BlitMode, BlitOperation, CommandSink, CursorMode, Direction, DrawingMode, IgsCommand, InitializationType, LineKind, LineStyleKind,
    MousePointerType, PaletteMode, PatternType, PenType, PolymarkerKind, SoundEffect, TerminalResolution, TextEffects, TextRotation,
};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgsCommandType {
    AttributeForFills,  // A
    BellsAndWhistles,   // b
    Box,                // B
    ColorSet,           // C
    LineDrawTo,         // D
    TextEffects,        // E
    FloodFill,          // F
    PolyFill,           // f
    GraphicScaling,     // g
    GrabScreen,         // G
    HollowSet,          // H
    Initialize,         // I
    EllipticalArc,      // J
    Cursor,             // k
    Arc,                // K (Arc for circle)
    Line,               // L
    DrawingMode,        // M
    CursorMotion,       // m (VT52 cursor motion)
    ChipMusic,          // n
    Noise,              // N
    Circle,             // O (Circle/Disk)
    PolyMarker,         // P
    PositionCursor,     // p (VT52 position cursor)
    Ellipse,            // Q (Ellipse/Oval)
    SetResolution,      // R
    ScreenClear,        // s
    SetPenColor,        // S
    LineType,           // T
    PauseSeconds,       // t (seconds pause)
    VsyncPause,         // q (vsync pause)
    RoundedRectangles,  // U
    PieSlice,           // V
    InverseVideo,       // v (VT52 inverse video)
    WriteText,          // W
    LineWrap,           // w (VT52 line wrap)
    ExtendedCommand,    // X
    EllipticalPieSlice, // Y
    FilledRectangle,    // Z
    PolyLine,           // z
    LoopCommand,        // &
    InputCommand,       // <
    AskIG,              // ?
}

impl IgsCommandType {
    pub fn from_char(ch: char) -> Option<Self> {
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
            'G' => Some(Self::GrabScreen),
            'H' => Some(Self::HollowSet),
            'I' => Some(Self::Initialize),
            'J' => Some(Self::EllipticalArc),
            'k' => Some(Self::Cursor),
            'K' => Some(Self::Arc),
            'L' => Some(Self::Line),
            'M' => Some(Self::DrawingMode),
            'm' => Some(Self::CursorMotion),
            'n' => Some(Self::ChipMusic),
            'N' => Some(Self::Noise),
            'O' => Some(Self::Circle),
            'P' => Some(Self::PolyMarker),
            'p' => Some(Self::PositionCursor),
            'Q' => Some(Self::Ellipse),
            'R' => Some(Self::SetResolution),
            's' => Some(Self::ScreenClear),
            'S' => Some(Self::SetPenColor),
            'T' => Some(Self::LineType),
            't' => Some(Self::PauseSeconds),
            'q' => Some(Self::VsyncPause),
            'U' => Some(Self::RoundedRectangles),
            'V' => Some(Self::PieSlice),
            'v' => Some(Self::InverseVideo),
            'W' => Some(Self::WriteText),
            'w' => Some(Self::LineWrap),
            'X' => Some(Self::ExtendedCommand),
            'Y' => Some(Self::EllipticalPieSlice),
            'z' => Some(Self::PolyLine),
            'Z' => Some(Self::FilledRectangle),
            '&' => Some(Self::LoopCommand),
            '<' => Some(Self::InputCommand),
            '?' => Some(Self::AskIG),
            _ => None,
        }
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
    fn check_parameters<F, T>(params: &[i32], sink: &mut dyn CommandSink, command: &'static str, expected: usize, cmd: F) -> Option<T>
    where
        F: FnOnce() -> T,
    {
        if params.len() < expected {
            sink.report_errror(
                crate::ParseError::InvalidParameter {
                    command,
                    value: params.len() as u16,
                    expected: Self::get_parameter_name(expected as i32),
                },
                crate::ErrorLevel::Error,
            );
            None
        } else {
            if params.len() > expected {
                sink.report_errror(
                    crate::ParseError::InvalidParameter {
                        command,
                        value: params.len() as u16,
                        expected: Self::get_parameter_name(expected as i32),
                    },
                    crate::ErrorLevel::Warning,
                );
            }
            Some(cmd())
        }
    }

    pub fn create_command(self, sink: &mut dyn CommandSink, params: &[i32], text_buffer: &String) -> Option<IgsCommand> {
        match self {
            IgsCommandType::Box => Self::check_parameters(params, sink, "Box", 5, || IgsCommand::Box {
                x1: params[0],
                y1: params[1],
                x2: params[2],
                y2: params[3],
                rounded: params[4] != 0,
            }),
            IgsCommandType::Line => Self::check_parameters(params, sink, "Line", 4, || IgsCommand::Line {
                x1: params[0],
                y1: params[1],
                x2: params[2],
                y2: params[3],
            }),
            IgsCommandType::LineDrawTo => Self::check_parameters(params, sink, "LineDrawTo", 2, || IgsCommand::LineDrawTo { x: params[0], y: params[1] }),
            IgsCommandType::Circle => Self::check_parameters(params, sink, "Circle", 3, || IgsCommand::Circle {
                x: params[0],
                y: params[1],
                radius: params[2],
            }),
            IgsCommandType::Ellipse => Self::check_parameters(params, sink, "Ellipse", 4, || IgsCommand::Ellipse {
                x: params[0],
                y: params[1],
                x_radius: params[2],
                y_radius: params[3],
            }),
            IgsCommandType::Arc => Self::check_parameters(params, sink, "Arc", 5, || IgsCommand::Arc {
                x: params[0],
                y: params[1],
                radius: params[2],
                start_angle: params[3],
                end_angle: params[4],
            }),
            IgsCommandType::ColorSet => {
                let pen = PenType::try_from(params.get(0).copied().unwrap_or(0)).unwrap_or_else(|_| {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: "ColorSet",
                            value: params.get(0).copied().unwrap_or(0) as u16,
                            expected: Some("valid PenType (0-3)"),
                        },
                        crate::ErrorLevel::Warning,
                    );
                    PenType::default()
                });
                Self::check_parameters(params, sink, "ColorSet", 2, || IgsCommand::ColorSet { pen, color: params[1] as u8 })
            }
            IgsCommandType::AttributeForFills => Self::check_parameters(params, sink, "AttributeForFills", 3, || {
                let pattern_type = match params[0] {
                    0 => PatternType::Hollow,
                    1 => PatternType::Solid,
                    2 => PatternType::Pattern(params[1] as u8),
                    3 => PatternType::Hatch(params[1] as u8),
                    4 => PatternType::UserDefined(params[1] as u8),
                    _ => PatternType::Solid,
                };
                IgsCommand::AttributeForFills {
                    pattern_type,
                    border: params[2] != 0,
                }
            }),
            IgsCommandType::TextEffects => {
                let rotation = TextRotation::try_from(params.get(2).copied().unwrap_or(0)).unwrap_or_else(|_| {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: "TextEffects",
                            value: params.get(2).copied().unwrap_or(0) as u16,
                            expected: Some("valid TextRotation (0-3)"),
                        },
                        crate::ErrorLevel::Warning,
                    );
                    TextRotation::default()
                });
                Self::check_parameters(params, sink, "TextEffects", 3, || IgsCommand::TextEffects {
                    effects: TextEffects::from_bits_truncate(params[0] as u8),
                    size: params[1] as u8,
                    rotation,
                })
            }
            IgsCommandType::FloodFill => Self::check_parameters(params, sink, "FloodFill", 2, || IgsCommand::FloodFill { x: params[0], y: params[1] }),
            IgsCommandType::PolyMarker => Self::check_parameters(params, sink, "PolyMarker", 2, || IgsCommand::PolymarkerPlot { x: params[0], y: params[1] }),
            IgsCommandType::SetPenColor => Self::check_parameters(params, sink, "SetPenColor", 4, || IgsCommand::SetPenColor {
                pen: params[0] as u8,
                red: params[1] as u8,
                green: params[2] as u8,
                blue: params[3] as u8,
            }),
            IgsCommandType::DrawingMode => {
                let mode = DrawingMode::try_from(params.get(0).copied().unwrap_or(1)).unwrap_or_else(|_| {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: "DrawingMode",
                            value: params.get(0).copied().unwrap_or(1) as u16,
                            expected: Some("valid DrawingMode (1-4)"),
                        },
                        crate::ErrorLevel::Warning,
                    );
                    DrawingMode::default()
                });
                Self::check_parameters(params, sink, "DrawingMode", 1, || IgsCommand::DrawingMode { mode })
            }
            IgsCommandType::HollowSet => Self::check_parameters(params, sink, "HollowSet", 1, || IgsCommand::HollowSet { enabled: params[0] != 0 }),
            IgsCommandType::Initialize => {
                let mode = InitializationType::try_from(params.get(0).copied().unwrap_or(0)).unwrap_or_else(|_| {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: "Initialize",
                            value: params.get(0).copied().unwrap_or(0) as u16,
                            expected: Some("valid InitializationType (0-5)"),
                        },
                        crate::ErrorLevel::Warning,
                    );
                    InitializationType::default()
                });
                Self::check_parameters(params, sink, "Initialize", 1, || IgsCommand::Initialize { mode })
            }
            IgsCommandType::EllipticalArc => Self::check_parameters(params, sink, "EllipticalArc", 6, || IgsCommand::EllipticalArc {
                x: params[0],
                y: params[1],
                x_radius: params[2],
                y_radius: params[3],
                start_angle: params[4],
                end_angle: params[5],
            }),
            IgsCommandType::Cursor => {
                let mode = CursorMode::try_from(params.get(0).copied().unwrap_or(0)).unwrap_or_else(|_| {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: "Cursor",
                            value: params.get(0).copied().unwrap_or(0) as u16,
                            expected: Some("valid CursorMode (0-3)"),
                        },
                        crate::ErrorLevel::Warning,
                    );
                    CursorMode::default()
                });
                Self::check_parameters(params, sink, "Cursor", 1, || IgsCommand::Cursor { mode })
            }
            IgsCommandType::ChipMusic => {
                let sound_effect = SoundEffect::try_from(params.get(0).copied().unwrap_or(0)).unwrap_or_else(|_| {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: "ChipMusic",
                            value: params.get(0).copied().unwrap_or(0) as u16,
                            expected: Some("valid SoundEffect (0-19)"),
                        },
                        crate::ErrorLevel::Warning,
                    );
                    SoundEffect::default()
                });
                Self::check_parameters(params, sink, "ChipMusic", 6, || IgsCommand::ChipMusic {
                    sound_effect,
                    voice: params[1] as u8,
                    volume: params[2] as u8,
                    pitch: params[3] as u8,
                    timing: params[4],
                    stop_type: params[5] as u8,
                })
            }
            IgsCommandType::ScreenClear => Self::check_parameters(params, sink, "ScreenClear", 1, || IgsCommand::ScreenClear { mode: params[0] as u8 }),
            IgsCommandType::SetResolution => {
                let resolution = TerminalResolution::try_from(params.get(0).copied().unwrap_or(0)).unwrap_or_else(|_| {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: "SetResolution",
                            value: params.get(0).copied().unwrap_or(0) as u16,
                            expected: Some("resolution (0=Low, 1=Medium, 2=High)"),
                        },
                        crate::ErrorLevel::Error,
                    );
                    TerminalResolution::default()
                });
                let palette = PaletteMode::try_from(params.get(1).copied().unwrap_or(0)).unwrap_or_else(|_| {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: "SetResolution",
                            value: params.get(1).copied().unwrap_or(0) as u16,
                            expected: Some("palette (0=NoChange, 1=Desktop, 2=IgDefault, 3=VdiDefault)"),
                        },
                        crate::ErrorLevel::Error,
                    );
                    PaletteMode::default()
                });
                Self::check_parameters(params, sink, "SetResolution", 2, || IgsCommand::SetResolution { resolution, palette })
            }
            IgsCommandType::LineType => {
                let param1 = params.get(1).copied().unwrap_or(1);
                let kind = if params.get(0).copied().unwrap_or(0) == 1 {
                    LineStyleKind::Polymarker(PolymarkerKind::try_from(param1).unwrap_or_else(|_| {
                        sink.report_errror(
                            crate::ParseError::InvalidParameter {
                                command: "LineType",
                                value: param1 as u16,
                                expected: Some("valid PolymarkerKind (1-6)"),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        PolymarkerKind::default()
                    }))
                } else {
                    LineStyleKind::Line(LineKind::try_from(param1).unwrap_or_else(|_| {
                        sink.report_errror(
                            crate::ParseError::InvalidParameter {
                                command: "LineType",
                                value: param1 as u16,
                                expected: Some("valid LineKind (1-7)"),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        LineKind::default()
                    }))
                };
                Self::check_parameters(params, sink, "LineType", 3, || IgsCommand::LineStyle { kind, value: params[2] as u16 })
            }
            IgsCommandType::PauseSeconds => Self::check_parameters(params, sink, "PauseSeconds", 1, || IgsCommand::PauseSeconds { seconds: params[0] as u8 }),
            IgsCommandType::VsyncPause => Self::check_parameters(params, sink, "VsyncPause", 1, || IgsCommand::VsyncPause { vsyncs: params[0] }),
            IgsCommandType::PolyLine | IgsCommandType::PolyFill => {
                if params.is_empty() {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: if self == IgsCommandType::PolyLine { "PolyLine" } else { "PolyFill" },
                            value: 0,
                            expected: Some("1 parameter"),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let count = params[0] as usize;
                    let expected = 1 + count * 2;
                    if params.len() < expected {
                        sink.report_errror(
                            crate::ParseError::InvalidParameter {
                                command: if self == IgsCommandType::PolyLine { "PolyLine" } else { "PolyFill" },
                                value: params.len() as u16,
                                expected: None,
                            },
                            crate::ErrorLevel::Error,
                        );
                        None
                    } else {
                        if params.len() > expected {
                            sink.report_errror(
                                crate::ParseError::InvalidParameter {
                                    command: if self == IgsCommandType::PolyLine { "PolyLine" } else { "PolyFill" },
                                    value: params.len() as u16,
                                    expected: None,
                                },
                                crate::ErrorLevel::Warning,
                            );
                        }
                        let points = params[1..].to_vec();
                        if self == IgsCommandType::PolyLine {
                            Some(IgsCommand::PolyLine { points })
                        } else {
                            Some(IgsCommand::PolyFill { points })
                        }
                    }
                }
            }
            IgsCommandType::BellsAndWhistles => {
                if params.is_empty() {
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
                    let cmd_id = params[0];
                    match cmd_id {
                        20 => {
                            // b>20,play_flag,snd_num,element_num,negative_flag,thousands,hundreds:
                            if params.len() < 7 {
                                sink.report_errror(
                                    crate::ParseError::InvalidParameter {
                                        command: "BellsAndWhistles:AlterSoundEffect",
                                        value: params.len() as u16,
                                        expected: Some("7 parameter"),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                None
                            } else {
                                if params.len() > 7 {
                                    sink.report_errror(
                                        crate::ParseError::InvalidParameter {
                                            command: "BellsAndWhistles:AlterSoundEffect",
                                            value: params.len() as u16,
                                            expected: Some("7 parameter"),
                                        },
                                        crate::ErrorLevel::Warning,
                                    );
                                }
                                Some(IgsCommand::AlterSoundEffect {
                                    play_flag: params[1] as u8,
                                    sound_effect: SoundEffect::try_from(params[2]).unwrap_or_else(|_| {
                                        sink.report_errror(
                                            crate::ParseError::InvalidParameter {
                                                command: "BellsAndWhistles:AlterSoundEffect",
                                                value: params[2] as u16,
                                                expected: Some("valid SoundEffect (0-19)"),
                                            },
                                            crate::ErrorLevel::Warning,
                                        );
                                        SoundEffect::default()
                                    }),
                                    element_num: params[3] as u8,
                                    negative_flag: params[4] as u8,
                                    thousands: params[5] as u16,
                                    hundreds: params[6] as u16,
                                })
                            }
                        }
                        21 => {
                            // b>21: - Stop all sounds
                            Some(IgsCommand::StopAllSound)
                        }
                        22 => {
                            // b>22,snd_num: - Restore sound effect
                            if params.len() < 2 {
                                sink.report_errror(
                                    crate::ParseError::InvalidParameter {
                                        command: "BellsAndWhistles:RestoreSoundEffect",
                                        value: params.len() as u16,
                                        expected: Some("2 parameter"),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                None
                            } else {
                                if params.len() > 2 {
                                    sink.report_errror(
                                        crate::ParseError::InvalidParameter {
                                            command: "BellsAndWhistles:RestoreSoundEffect",
                                            value: params.len() as u16,
                                            expected: Some("2 parameter"),
                                        },
                                        crate::ErrorLevel::Warning,
                                    );
                                }
                                Some(IgsCommand::RestoreSoundEffect {
                                    sound_effect: SoundEffect::try_from(params[1]).unwrap_or_else(|_| {
                                        sink.report_errror(
                                            crate::ParseError::InvalidParameter {
                                                command: "BellsAndWhistles:RestoreSoundEffect",
                                                value: params[1] as u16,
                                                expected: Some("valid SoundEffect (0-19)"),
                                            },
                                            crate::ErrorLevel::Warning,
                                        );
                                        SoundEffect::default()
                                    }),
                                })
                            }
                        }
                        23 => {
                            // b>23,count: - Set effect loops
                            if params.len() < 2 {
                                sink.report_errror(
                                    crate::ParseError::InvalidParameter {
                                        command: "BellsAndWhistles:SetEffectLoops",
                                        value: params.len() as u16,
                                        expected: Some("2 parameter"),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                None
                            } else {
                                if params.len() > 2 {
                                    sink.report_errror(
                                        crate::ParseError::InvalidParameter {
                                            command: "BellsAndWhistles:SetEffectLoops",
                                            value: params.len() as u16,
                                            expected: Some("2 parameter"),
                                        },
                                        crate::ErrorLevel::Warning,
                                    );
                                }
                                Some(IgsCommand::SetEffectLoops { count: params[1] as u32 })
                            }
                        }
                        _ => {
                            // b>0-19: - Play sound effect
                            Some(IgsCommand::BellsAndWhistles {
                                sound_effect: SoundEffect::try_from(cmd_id).unwrap_or_else(|_| {
                                    sink.report_errror(
                                        crate::ParseError::InvalidParameter {
                                            command: "BellsAndWhistles",
                                            value: cmd_id as u16,
                                            expected: Some("valid SoundEffect (0-19)"),
                                        },
                                        crate::ErrorLevel::Warning,
                                    );
                                    SoundEffect::default()
                                }),
                            })
                        }
                    }
                }
            }
            IgsCommandType::GraphicScaling => {
                Self::check_parameters(params, sink, "GraphicScaling", 1, || IgsCommand::GraphicScaling { mode: params[0] as u8 })
            }
            IgsCommandType::GrabScreen => {
                if params.len() < 2 {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: "GrabScreen",
                            value: params.len() as u16,
                            expected: Some("2 parameter"),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let blit_type_id = params[0];
                    let mode: BlitMode = params[1].into();

                    let operation = match blit_type_id {
                        0 => {
                            // Screen to screen: needs 6 params
                            Self::check_parameters(params, sink, "GrabScreen:ScreenToScreen", 8, || BlitOperation::ScreenToScreen {
                                src_x1: params[2],
                                src_y1: params[3],
                                src_x2: params[4],
                                src_y2: params[5],
                                dest_x: params[6],
                                dest_y: params[7],
                            })
                        }
                        1 => {
                            // Screen to memory: needs 4 params (6 total with blit_type_id and mode)
                            Self::check_parameters(params, sink, "GrabScreen:ScreenToMemory", 6, || BlitOperation::ScreenToMemory {
                                src_x1: params[2],
                                src_y1: params[3],
                                src_x2: params[4],
                                src_y2: params[5],
                            })
                        }
                        2 => {
                            // Memory to screen: needs 2 params (4 total with blit_type_id and mode)
                            Self::check_parameters(params, sink, "GrabScreen:MemoryToScreen", 4, || BlitOperation::MemoryToScreen {
                                dest_x: params[2],
                                dest_y: params[3],
                            })
                        }
                        3 => {
                            // Piece of memory to screen: needs 6 params (8 total with blit_type_id and mode)
                            Self::check_parameters(params, sink, "GrabScreen:PieceOfMemoryToScreen", 8, || BlitOperation::PieceOfMemoryToScreen {
                                src_x1: params[2],
                                src_y1: params[3],
                                src_x2: params[4],
                                src_y2: params[5],
                                dest_x: params[6],
                                dest_y: params[7],
                            })
                        }
                        4 => {
                            // Memory to memory: needs 6 params (8 total with blit_type_id and mode)
                            Self::check_parameters(params, sink, "GrabScreen:MemoryToMemory", 8, || BlitOperation::MemoryToMemory {
                                src_x1: params[2],
                                src_y1: params[3],
                                src_x2: params[4],
                                src_y2: params[5],
                                dest_x: params[6],
                                dest_y: params[7],
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
            IgsCommandType::WriteText => Self::check_parameters(params, sink, "WriteText", 2, || IgsCommand::WriteText {
                x: params[0],
                y: params[1],
                text: text_buffer.clone(),
            }),

            IgsCommandType::Noise => Some(IgsCommand::Noise { params: params.to_vec() }),
            IgsCommandType::RoundedRectangles => Self::check_parameters(params, sink, "RoundedRectangles", 5, || IgsCommand::RoundedRectangles {
                x1: params[0],
                y1: params[1],
                x2: params[2],
                y2: params[3],
                fill: params[4] != 0,
            }),
            IgsCommandType::PieSlice => Self::check_parameters(params, sink, "PieSlice", 5, || IgsCommand::PieSlice {
                x: params[0],
                y: params[1],
                radius: params[2],
                start_angle: params[3],
                end_angle: params[4],
            }),
            IgsCommandType::ExtendedCommand => {
                // X - Extended commands
                if params.is_empty() {
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
                    let cmd_id = params[0];
                    match cmd_id {
                        0 => {
                            // SprayPaint (id,x,y,width,height,density)
                            Self::check_parameters(params, sink, "ExtendedCommand:SprayPaint", 6, || IgsCommand::SprayPaint {
                                x: params[1],
                                y: params[2],
                                width: params[3],
                                height: params[4],
                                density: params[5],
                            })
                        }
                        1 => {
                            // SetColorRegister
                            Self::check_parameters(params, sink, "ExtendedCommand:SetColorRegister", 3, || IgsCommand::SetColorRegister {
                                register: params[1] as u8,
                                value: params[2],
                            })
                        }
                        2 => {
                            // SetRandomRange
                            Some(IgsCommand::SetRandomRange { params: params[1..].to_vec() })
                        }
                        3 => {
                            // RightMouseMacro
                            Some(IgsCommand::RightMouseMacro { params: params[1..].to_vec() })
                        }
                        4 => {
                            // DefineZone: Special handling for clear (9999-9997)
                            if params.len() == 2 && (9997..=9999).contains(&params[1]) {
                                // Clear command or loopback toggle - no additional params needed
                                Some(IgsCommand::DefineZone {
                                    zone_id: params[1],
                                    x1: 0,
                                    y1: 0,
                                    x2: 0,
                                    y2: 0,
                                    length: 0,
                                    string: String::new(),
                                })
                            } else if params.len() >= 8 && !text_buffer.is_empty() {
                                Some(IgsCommand::DefineZone {
                                    zone_id: params[1],
                                    x1: params[2],
                                    y1: params[3],
                                    x2: params[4],
                                    y2: params[5],
                                    length: params[6] as u16,
                                    string: text_buffer.clone(),
                                })
                            } else {
                                sink.report_errror(
                                    crate::ParseError::InvalidParameter {
                                        command: "ExtendedCommand:DefineZone",
                                        value: params.len() as u16,
                                        expected: Some("2 parameter (9997-9999) or 8+ parameter with text"),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                None
                            }
                        }
                        5 => {
                            // FlowControl
                            Self::check_parameters(params, sink, "ExtendedCommand:FlowControl", 2, || IgsCommand::FlowControl {
                                mode: params[1] as u8,
                                params: params[2..].to_vec(),
                            })
                        }
                        6 => {
                            // LeftMouseButton
                            Self::check_parameters(params, sink, "ExtendedCommand:LeftMouseButton", 2, || IgsCommand::LeftMouseButton {
                                mode: params[1] as u8,
                            })
                        }
                        7 => {
                            // LoadFillPattern
                            Self::check_parameters(params, sink, "ExtendedCommand:LoadFillPattern", 2, || IgsCommand::LoadFillPattern {
                                pattern: params[1] as u8,
                                data: text_buffer.clone(),
                            })
                        }
                        8 => {
                            // RotateColorRegisters
                            Self::check_parameters(params, sink, "ExtendedCommand:RotateColorRegisters", 5, || IgsCommand::RotateColorRegisters {
                                start_reg: params[1] as u8,
                                end_reg: params[2] as u8,
                                count: params[3],
                                delay: params[4],
                            })
                        }
                        9 => {
                            // LoadMidiBuffer
                            Some(IgsCommand::LoadMidiBuffer { params: params[1..].to_vec() })
                        }
                        10 => {
                            // SetDrawtoBegin
                            Self::check_parameters(params, sink, "ExtendedCommand:SetDrawtoBegin", 3, || IgsCommand::SetDrawtoBegin {
                                x: params[1],
                                y: params[2],
                            })
                        }
                        11 => {
                            // LoadBitblitMemory
                            Some(IgsCommand::LoadBitblitMemory { params: params[1..].to_vec() })
                        }
                        12 => {
                            // LoadColorPalette
                            Some(IgsCommand::LoadColorPalette { params: params[1..].to_vec() })
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
            IgsCommandType::EllipticalPieSlice => Self::check_parameters(params, sink, "EllipticalPieSlice", 6, || IgsCommand::EllipticalPieSlice {
                x: params[0],
                y: params[1],
                x_radius: params[2],
                y_radius: params[3],
                start_angle: params[4],
                end_angle: params[5],
            }),
            IgsCommandType::FilledRectangle => Self::check_parameters(params, sink, "FilledRectangle", 4, || IgsCommand::FilledRectangle {
                x1: params[0],
                y1: params[1],
                x2: params[2],
                y2: params[3],
            }),
            IgsCommandType::CursorMotion => {
                // m - cursor motion
                // IG form: direction,count
                // ESC form previously provided x,y; map to direction/count
                if params.len() < 2 {
                    sink.report_errror(
                        crate::ParseError::InvalidParameter {
                            command: "CursorMotion",
                            value: params.len() as u16,
                            expected: Some("2 parameter"),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let a = params[0];
                    let b = params[1];
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
            IgsCommandType::PositionCursor => {
                Self::check_parameters(params, sink, "PositionCursor", 2, || IgsCommand::PositionCursor { x: params[0], y: params[1] })
            }
            IgsCommandType::InverseVideo => Self::check_parameters(params, sink, "InverseVideo", 1, || IgsCommand::InverseVideo { enabled: params[0] != 0 }),
            IgsCommandType::LineWrap => Self::check_parameters(params, sink, "LineWrap", 1, || IgsCommand::LineWrap { enabled: params[0] != 0 }),
            IgsCommandType::InputCommand => Self::check_parameters(params, sink, "InputCommand", 1, || IgsCommand::InputCommand {
                input_type: params[0] as u8,
                params: params[1..].to_vec(),
            }),
            IgsCommandType::AskIG => {
                if params.is_empty() {
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
                    let query = match params[0] {
                        0 => Some(AskQuery::VersionNumber),
                        1 => {
                            let pointer_type = if params.len() > 1 {
                                match MousePointerType::try_from(params[1]) {
                                    Ok(pt) => pt,
                                    Err(_) => {
                                        sink.report_errror(
                                            crate::ParseError::InvalidParameter {
                                                command: "AskIG",
                                                value: params[1] as u16,
                                                expected: Some("valid MousePointerType (0-10)"),
                                            },
                                            crate::ErrorLevel::Warning,
                                        );
                                        MousePointerType::default()
                                    }
                                }
                            } else {
                                MousePointerType::Immediate
                            };
                            Some(AskQuery::CursorPositionAndMouseButton { pointer_type })
                        }
                        2 => {
                            let pointer_type = if params.len() > 1 {
                                match MousePointerType::try_from(params[1]) {
                                    Ok(pt) => pt,
                                    Err(_) => {
                                        sink.report_errror(
                                            crate::ParseError::InvalidParameter {
                                                command: "AskIG",
                                                value: params[1] as u16,
                                                expected: Some("valid MousePointerType (0-10)"),
                                            },
                                            crate::ErrorLevel::Warning,
                                        );
                                        MousePointerType::default()
                                    }
                                }
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
                                    value: params[0] as u16,
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
            IgsCommandType::LoopCommand => {
                // Handled in parser (No inner loops allowed)
                None
            }
        }
    }
}
