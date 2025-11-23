use crate::{
    ArrowEnd, AskQuery, BlitMode, BlitOperation, CommandSink, CursorMode, Direction, DrawingMode, GraphicsScalingMode, IgsCommand, IgsParameter,
    InitializationType, LineKind, LineMarkerStyle, MousePointerType, PaletteMode, PatternType, PenType, PolymarkerKind, RandomRangeType, ScreenClearMode,
    SoundEffect, TerminalResolution, TextColorLayer, TextEffects, TextRotation,
};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
pub enum IgsCommandType {
    // Command           ASCII
    AttributeForFills    = b'A',
    Box                  = b'B',
    ColorSet             = b'C',
    LineDrawTo           = b'D',
    TextEffects          = b'E',
    FloodFill            = b'F',
    GrabScreen           = b'G',
    HollowSet            = b'H',
    Initialize           = b'I',
    EllipticalArc        = b'J',
    Arc                  = b'K',
    Line                 = b'L',
    DrawingMode          = b'M',
    Noise                = b'N',
    Circle               = b'O',
    PolyMarker           = b'P',
    Ellipse              = b'Q',
    SetResolution        = b'R',
    SetPenColor          = b'S',
    LineType             = b'T',
    RoundedRectangles    = b'U',
    PieSlice             = b'V',
    WriteText            = b'W',
    ExtendedCommand      = b'X',
    EllipticalPieSlice   = b'Y',
    FilledRectangle      = b'Z',
    BellsAndWhistles     = b'b',
    PolyFill             = b'f',
    GraphicScaling       = b'g',
    Cursor               = b'k',
    CursorMotion         = b'm',
    ChipMusic            = b'n',
    PositionCursor       = b'p',
    VsyncPause           = b'q',
    ScreenClear          = b's',
    PauseSeconds         = b't',
    InverseVideo         = b'v',
    LineWrap             = b'w',
    PolyLine             = b'z',
    LoopCommand          = b'&',
    InputCommand         = b'<',
    AskIG                = b'?',
    SetTextColor         = b'c',  // IGS extension: G#c>0/1,color: for fg/bg color
    DeleteLines          = b'd',  // IGS extension: G#d>count: to delete lines
    InsertLine           = b'i',  // IGS/VT52: G#i>mode,count: to insert lines
    ClearLine            = b'l',  // IGS/VT52: G#l>mode: to clear line
    RememberCursor       = b'r',  // IGS/VT52: G#r>value: to remember cursor position
}

impl TryFrom<u8> for IgsCommandType {
    type Error = ();

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        match byte {
            b'A' => Ok(Self::AttributeForFills),
            b'b' => Ok(Self::BellsAndWhistles),
            b'B' => Ok(Self::Box),
            b'C' => Ok(Self::ColorSet),
            b'D' => Ok(Self::LineDrawTo),
            b'E' => Ok(Self::TextEffects),
            b'F' => Ok(Self::FloodFill),
            b'f' => Ok(Self::PolyFill),
            b'g' => Ok(Self::GraphicScaling),
            b'G' => Ok(Self::GrabScreen),
            b'H' => Ok(Self::HollowSet),
            b'I' => Ok(Self::Initialize),
            b'J' => Ok(Self::EllipticalArc),
            b'k' => Ok(Self::Cursor),
            b'K' => Ok(Self::Arc),
            b'L' => Ok(Self::Line),
            b'M' => Ok(Self::DrawingMode),
            b'm' => Ok(Self::CursorMotion),
            b'n' => Ok(Self::ChipMusic),
            b'N' => Ok(Self::Noise),
            b'O' => Ok(Self::Circle),
            b'P' => Ok(Self::PolyMarker),
            b'p' => Ok(Self::PositionCursor),
            b'Q' => Ok(Self::Ellipse),
            b'R' => Ok(Self::SetResolution),
            b's' => Ok(Self::ScreenClear),
            b'S' => Ok(Self::SetPenColor),
            b'T' => Ok(Self::LineType),
            b't' => Ok(Self::PauseSeconds),
            b'q' => Ok(Self::VsyncPause),
            b'U' => Ok(Self::RoundedRectangles),
            b'V' => Ok(Self::PieSlice),
            b'v' => Ok(Self::InverseVideo),
            b'W' => Ok(Self::WriteText),
            b'w' => Ok(Self::LineWrap),
            b'X' => Ok(Self::ExtendedCommand),
            b'Y' => Ok(Self::EllipticalPieSlice),
            b'z' => Ok(Self::PolyLine),
            b'Z' => Ok(Self::FilledRectangle),
            b'&' => Ok(Self::LoopCommand),
            b'<' => Ok(Self::InputCommand),
            b'?' => Ok(Self::AskIG),
            b'c' => Ok(Self::SetTextColor),
            b'd' => Ok(Self::DeleteLines),
            b'i' => Ok(Self::InsertLine),
            b'l' => Ok(Self::ClearLine),
            b'r' => Ok(Self::RememberCursor),
            _ => Err(()),
        }
    }
}

impl IgsCommandType {
    pub fn to_char(self) -> char {
        self as u8 as char
    }

    /// Parses a 16x17 character pattern buffer into Vec<u16>
    ///
    /// Format: 16 lines × 17 characters = 272 bytes total (or 288 with newlines)
    /// Each line: 16 pattern chars + '@' delimiter (optionally followed by '\n')
    /// Pattern chars: 'X' or 'x' = 1, anything else = 0
    ///
    /// Returns Vec<u16> where each u16 represents one line (16 bits)
    /// Returns None if buffer format is invalid
    pub fn parse_pattern_buffer(buffer: &[u8], sink: &mut dyn CommandSink) -> Option<Vec<u16>> {
        const NUM_LINES: usize = 16;

        let mut result = Vec::with_capacity(NUM_LINES);
        let mut pos = 0;

        for line_idx in 0..NUM_LINES {
            // Need at least 17 chars: 16 pattern chars + '@'
            if pos + 16 > buffer.len() {
                sink.report_error(
                    crate::ParseError::MalformedSequence {
                        description: "LoadFillPattern: Buffer too short",
                        sequence: Some(format!("Line {} incomplete at position {}", line_idx + 1, pos)),
                        context: Some(format!("Expected at least {} more bytes", pos + 17 - buffer.len())),
                    },
                    crate::ErrorLevel::Error,
                );
                return None;
            }

            let line = &buffer[pos..pos + 16];

            // Parse 16 pattern characters into u16
            let mut word: u16 = 0;

            for bit_idx in 0..16 {
                let is_set = matches!(line[bit_idx], b'X' | b'x');
                if is_set {
                    word |= 1 << (15 - bit_idx);
                }
            }

            result.push(word);

            pos += 16;

            // Validate '@' delimiter
            if pos >= buffer.len() || buffer[pos] != b'@' {
                sink.report_error(
                    crate::ParseError::MalformedSequence {
                        description: "LoadFillPattern: Missing '@' delimiter",
                        sequence: Some(format!("Line {} at position {}", line_idx + 1, pos)),
                        context: Some("Expected '@' after 16 pattern characters".to_string()),
                    },
                    crate::ErrorLevel::Error,
                );
                return None;
            }
            pos += 1; // Skip '@'

            // Skip optional newline
            if pos < buffer.len() && buffer[pos] == b'\n' {
                pos += 1;
            }
        }

        Some(result)
    }

    #[inline(always)]
    fn get_parameter_name(expected: i32) -> Option<String> {
        match expected {
            0 => Some("No parameters".to_string()),
            1 => Some("1 parameter".to_string()),
            2 => Some("2 parameter".to_string()),
            3 => Some("3 parameter".to_string()),
            4 => Some("4 parameter".to_string()),
            5 => Some("5 parameter".to_string()),
            6 => Some("6 parameter".to_string()),
            7 => Some("7 parameter".to_string()),
            8 => Some("8 parameter".to_string()),
            _ => None,
        }
    }

    #[inline(always)]
    fn check_parameters<F, T>(params: &[IgsParameter], sink: &mut dyn CommandSink, command: &'static str, expected: usize, cmd: F) -> Option<T>
    where
        F: FnOnce(&mut dyn CommandSink) -> Option<T>,
    {
        if params.len() < expected {
            sink.report_error(
                crate::ParseError::InvalidParameter {
                    command,
                    value: format!("{}", params.len()),
                    expected: Self::get_parameter_name(expected as i32),
                },
                crate::ErrorLevel::Error,
            );
            None
        } else {
            if params.len() > expected {
                sink.report_error(
                    crate::ParseError::InvalidParameter {
                        command,
                        value: format!("{}", params.len()),
                        expected: Self::get_parameter_name(expected as i32),
                    },
                    crate::ErrorLevel::Warning,
                );
            }
            cmd(sink)
        }
    }

    /// Returns the number of parameters required by this command type.
    /// Returns None for variable-length commands (like PolyLine, PolyFill, etc.)
    pub fn parameter_count(&self) -> Option<usize> {
        use IgsCommandType::*;

        match self {
            Line => Some(4),               // x1, y1, x2, y2
            LineDrawTo => Some(2),         // x, y
            Circle => Some(3),             // x, y, radius
            Box => Some(5),                // x1, y1, x2, y2, rounded
            RoundedRectangles => Some(5),  // x1, y1, x2, y2, fill
            FilledRectangle => Some(4),    // x1, y1, x2, y2
            PolyMarker => Some(2),         // x, y
            Ellipse => Some(4),            // x, y, x_radius, y_radius
            Arc => Some(5),                // x, y, radius, start_angle, end_angle
            EllipticalArc => Some(6),      // x, y, x_radius, y_radius, start_angle, end_angle
            PieSlice => Some(5),           // x, y, radius, start_angle, end_angle
            EllipticalPieSlice => Some(6), // x, y, x_radius, y_radius, start_angle, end_angle
            ColorSet => Some(2),           // pen, color
            AttributeForFills => Some(3),  // type, pattern, border
            FloodFill => Some(2),          // x, y
            SetPenColor => Some(4),        // pen, red, green, blue
            DrawingMode => Some(1),        // mode
            HollowSet => Some(1),          // enabled
            WriteText => Some(2),          // x, y (text comes separately)
            LineType => Some(3),           // type, kind, value
            TextEffects => Some(3),        // effects, size, rotation
            Initialize => Some(1),         // mode
            SetResolution => Some(2),      // resolution, palette
            GraphicScaling => Some(1),     // mode
            Cursor => Some(1),             // mode
            ChipMusic => Some(6),          // sound_effect, voice, volume, pitch, timing, stop_type
            ScreenClear => Some(1),        // mode
            PauseSeconds => Some(1),       // seconds
            VsyncPause => Some(1),         // vsyncs
            CursorMotion => Some(2),       // direction, count
            PositionCursor => Some(2),     // x, y
            InverseVideo => Some(1),       // enabled
            LineWrap => Some(1),           // enabled
            SetTextColor => Some(2),       // layer, color
            DeleteLines => Some(1),        // count
            InsertLine => Some(2),         // mode, count (mode optional, defaults to 0)
            ClearLine => Some(1),          // mode (optional, defaults to 0)
            RememberCursor => Some(1),     // value (optional, defaults to 0)

            // Variable-length commands - return None
            PolyLine => None,         // count, then count*2 coordinates
            PolyFill => None,         // count, then count*2 coordinates
            BellsAndWhistles => None, // variable based on subcommand
            ExtendedCommand => None,  // variable based on subcommand
            GrabScreen => None,       // variable based on blit type (2-8 params)
            Noise => None,            // variable
            InputCommand => None,     // variable
            AskIG => None,            // variable (1-2 params)
            LoopCommand => None,      // complex variable structure
        }
    }

    pub fn create_command(self, sink: &mut dyn CommandSink, params: &[IgsParameter], text_buffer: &[u8]) -> Option<IgsCommand> {
        match self {
            IgsCommandType::Box => {
                if params.len() == 4 {
                    // There are files with invalid boxes.
                    Some(IgsCommand::Box {
                        x1: params[0],
                        y1: params[1],
                        x2: params[2],
                        y2: params[3],
                        rounded: false,
                    })
                } else {
                    Self::check_parameters(params, sink, "Box", 5, |_sink| {
                        Some(IgsCommand::Box {
                            x1: params[0],
                            y1: params[1],
                            x2: params[2],
                            y2: params[3],
                            rounded: params[4].value() != 0,
                        })
                    })
                }
            }
            IgsCommandType::Line => Self::check_parameters(params, sink, "Line", 4, |_sink| {
                Some(IgsCommand::Line {
                    x1: params[0],
                    y1: params[1],
                    x2: params[2],
                    y2: params[3],
                })
            }),
            IgsCommandType::LineDrawTo => Self::check_parameters(params, sink, "LineDrawTo", 2, |_sink| {
                Some(IgsCommand::LineDrawTo { x: params[0], y: params[1] })
            }),
            IgsCommandType::Circle => Self::check_parameters(params, sink, "Circle", 3, |_sink| {
                Some(IgsCommand::Circle {
                    x: params[0],
                    y: params[1],
                    radius: params[2],
                })
            }),
            IgsCommandType::Ellipse => Self::check_parameters(params, sink, "Ellipse", 4, |_sink| {
                Some(IgsCommand::Ellipse {
                    x: params[0],
                    y: params[1],
                    x_radius: params[2],
                    y_radius: params[3],
                })
            }),
            IgsCommandType::Arc => Self::check_parameters(params, sink, "Arc", 5, |_sink| {
                Some(IgsCommand::Arc {
                    x: params[0],
                    y: params[1],
                    radius: params[2],
                    start_angle: params[3],
                    end_angle: params[4],
                })
            }),
            IgsCommandType::ColorSet => {
                let pen = PenType::try_from(params.get(0).map(|p| p.value()).unwrap_or(0)).unwrap_or_else(|_| {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "ColorSet",
                            value: format!("{}", params.get(0).map(|p| p.value()).unwrap_or(0)),
                            expected: Some("valid PenType (0-3)".to_string()),
                        },
                        crate::ErrorLevel::Warning,
                    );
                    PenType::default()
                });
                Self::check_parameters(params, sink, "ColorSet", 2, |_sink| {
                    Some(IgsCommand::ColorSet {
                        pen,
                        color: params[1].value() as u8,
                    })
                })
            }
            IgsCommandType::AttributeForFills => Self::check_parameters(params, sink, "AttributeForFills", 3, |sink| {
                let pattern_type = match params[0].value() {
                    0 => PatternType::Hollow,
                    1 => PatternType::Solid,
                    2 => {
                        let pattern_index = params[1].value() as u8;
                        if !(1..=24).contains(&pattern_index) {
                            sink.report_error(
                                crate::ParseError::InvalidParameter {
                                    command: "AttributeForFills",
                                    value: format!("{}", pattern_index),
                                    expected: Some("valid pattern index (1-24)".to_string()),
                                },
                                crate::ErrorLevel::Warning,
                            );
                            return None;
                        }
                        PatternType::Pattern(pattern_index)
                    }
                    3 => {
                        let hatch_index = params[1].value() as u8;
                        if !(1..=12).contains(&hatch_index) {
                            sink.report_error(
                                crate::ParseError::InvalidParameter {
                                    command: "AttributeForFills",
                                    value: format!("{}", hatch_index),
                                    expected: Some("valid hatch index (1-12)".to_string()),
                                },
                                crate::ErrorLevel::Warning,
                            );
                            return None;
                        }
                        PatternType::Hatch(hatch_index)
                    }
                    4 => match params[1].value() {
                        9 => PatternType::StarTrek,
                        8 => PatternType::Random,
                        val if (0..=7).contains(&val) => PatternType::UserDefined(val as u8),
                        _ => {
                            sink.report_error(
                                crate::ParseError::InvalidParameter {
                                    command: "AttributeForFills",
                                    value: format!("{}", params[1].value()),
                                    expected: Some("valid user pattern type (0-9)".to_string()),
                                },
                                crate::ErrorLevel::Warning,
                            );
                            return None;
                        }
                    },
                    _ => {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "AttributeForFills",
                                value: format!("{}", params[0].value()),
                                expected: Some("valid pattern type (0-4)".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        return None;
                    }
                };
                Some(IgsCommand::AttributeForFills {
                    pattern_type,
                    border: params[2].value() != 0,
                })
            }),
            IgsCommandType::TextEffects => {
                let rotation = TextRotation::try_from(params.get(2).map(|p| p.value()).unwrap_or(0)).unwrap_or_else(|_| {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "TextEffects",
                            value: format!("{}", params.get(2).map(|p| p.value()).unwrap_or(0)),
                            expected: Some("valid TextRotation (0-3)".to_string()),
                        },
                        crate::ErrorLevel::Warning,
                    );
                    TextRotation::default()
                });
                Self::check_parameters(params, sink, "TextEffects", 3, |_sink| {
                    Some(IgsCommand::TextEffects {
                        effects: TextEffects::from_bits_truncate(params[0].value() as u8),
                        size: params[1].value() as u8,
                        rotation,
                    })
                })
            }
            IgsCommandType::FloodFill => {
                Self::check_parameters(params, sink, "FloodFill", 2, |_sink| Some(IgsCommand::FloodFill { x: params[0], y: params[1] }))
            }
            IgsCommandType::PolyMarker => Self::check_parameters(params, sink, "PolyMarker", 2, |_sink| {
                Some(IgsCommand::PolymarkerPlot { x: params[0], y: params[1] })
            }),
            IgsCommandType::SetPenColor => Self::check_parameters(params, sink, "SetPenColor", 4, |_sink| {
                Some(IgsCommand::SetPenColor {
                    pen: params[0].value() as u8,
                    red: params[1].value() as u8,
                    green: params[2].value() as u8,
                    blue: params[3].value() as u8,
                })
            }),
            IgsCommandType::DrawingMode => {
                let mode = DrawingMode::try_from(params.get(0).map(|p| p.value()).unwrap_or(1)).unwrap_or_else(|_| {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "DrawingMode",
                            value: format!("{}", params.get(0).map(|p| p.value()).unwrap_or(1)),
                            expected: Some("valid DrawingMode (1-4)".to_string()),
                        },
                        crate::ErrorLevel::Warning,
                    );
                    DrawingMode::default()
                });
                Self::check_parameters(params, sink, "DrawingMode", 1, |_sink| Some(IgsCommand::DrawingMode { mode }))
            }
            IgsCommandType::HollowSet => Self::check_parameters(params, sink, "HollowSet", 1, |_sink| {
                Some(IgsCommand::HollowSet {
                    enabled: params[0].value() != 0,
                })
            }),
            IgsCommandType::Initialize => {
                let mode = InitializationType::try_from(params.get(0).map(|p| p.value()).unwrap_or(0)).unwrap_or_else(|_| {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "Initialize",
                            value: format!("{}", params.get(0).map(|p| p.value()).unwrap_or(0)),
                            expected: Some("valid InitializationType (0-5)".to_string()),
                        },
                        crate::ErrorLevel::Warning,
                    );
                    InitializationType::default()
                });
                Self::check_parameters(params, sink, "Initialize", 1, |_sink| Some(IgsCommand::Initialize { mode }))
            }
            IgsCommandType::EllipticalArc => Self::check_parameters(params, sink, "EllipticalArc", 6, |_sink| {
                Some(IgsCommand::EllipticalArc {
                    x: params[0],
                    y: params[1],
                    x_radius: params[2],
                    y_radius: params[3],
                    start_angle: params[4],
                    end_angle: params[5],
                })
            }),
            IgsCommandType::Cursor => {
                let mode = CursorMode::try_from(params.get(0).map(|p| p.value()).unwrap_or(0)).unwrap_or_else(|_| {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "Cursor",
                            value: format!("{}", params.get(0).map(|p| p.value()).unwrap_or(0)),
                            expected: Some("valid CursorMode (0-3)".to_string()),
                        },
                        crate::ErrorLevel::Warning,
                    );
                    CursorMode::default()
                });
                Self::check_parameters(params, sink, "Cursor", 1, |_sink| Some(IgsCommand::Cursor { mode }))
            }
            IgsCommandType::ChipMusic => {
                let sound_effect = SoundEffect::try_from(params.get(0).map(|p| p.value()).unwrap_or(0)).unwrap_or_else(|_| {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "ChipMusic",
                            value: format!("{}", params.get(0).map(|p| p.value()).unwrap_or(0)),
                            expected: Some("valid SoundEffect (0-19)".to_string()),
                        },
                        crate::ErrorLevel::Warning,
                    );
                    SoundEffect::default()
                });
                Self::check_parameters(params, sink, "ChipMusic", 6, |_sink| {
                    Some(IgsCommand::ChipMusic {
                        sound_effect,
                        voice: params[1].value() as u8,
                        volume: params[2].value() as u8,
                        pitch: params[3].value() as u8,
                        timing: params[4].value(),
                        stop_type: params[5].value() as u8,
                    })
                })
            }
            IgsCommandType::ScreenClear => {
                let mode_val = params.get(0).map(|p| p.value()).unwrap_or(0);
                let mode = ScreenClearMode::try_from(mode_val as u8).unwrap_or_else(|_| {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "ScreenClear",
                            value: format!("{}", mode_val),
                            expected: Some("valid ScreenClearMode (0-5)".to_string()),
                        },
                        crate::ErrorLevel::Error,
                    );
                    ScreenClearMode::default()
                });
                Self::check_parameters(params, sink, "ScreenClear", 1, |_sink| Some(IgsCommand::ScreenClear { mode }))
            }
            IgsCommandType::SetResolution => {
                let resolution = TerminalResolution::try_from(params.get(0).map(|p| p.value()).unwrap_or(0)).unwrap_or_else(|_| {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "SetResolution",
                            value: format!("{}", params.get(0).map(|p| p.value()).unwrap_or(0)),
                            expected: Some("resolution (0=Low, 1=Medium, 2=High)".to_string()),
                        },
                        crate::ErrorLevel::Error,
                    );
                    TerminalResolution::default()
                });
                let palette = PaletteMode::try_from(params.get(1).map(|p| p.value()).unwrap_or(0)).unwrap_or_else(|_| {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "SetResolution",
                            value: format!("{}", params.get(1).map(|p| p.value()).unwrap_or(0)),
                            expected: Some("palette (0=NoChange, 1=Desktop, 2=IgDefault, 3=VdiDefault)".to_string()),
                        },
                        crate::ErrorLevel::Error,
                    );
                    PaletteMode::default()
                });
                Self::check_parameters(params, sink, "SetResolution", 2, |_sink| {
                    Some(IgsCommand::SetResolution { resolution, palette })
                })
            }
            IgsCommandType::LineType => {
                // Parse according to C code logic:
                // p1 = type (1=polymarker, 2=line)
                // p2 = style (1-6 for polymarker, 1-7 for line)
                // p3 = size/thickness/endpoints

                let m_type = params.get(0).map(|p| p.value()).unwrap_or(0);
                let style = params.get(1).map(|p| p.value()).unwrap_or(1);
                let value = params.get(2).map(|p| p.value()).unwrap_or(1);
                if m_type < 1 || m_type > 2 {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "LineType",
                            value: format!("{}", m_type),
                            expected: Some("type: 1=polymarker, 2=line".to_string()),
                        },
                        crate::ErrorLevel::Error,
                    );
                    return None;
                }

                let style = if m_type == 1 {
                    // Polymarker: p2=type(1-6), p3=size(1-8)
                    let mut poly_kind = style;
                    if poly_kind < 1 || poly_kind > 6 {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "LineType",
                                value: format!("{}", poly_kind),
                                expected: Some("valid PolymarkerKind (1-6)".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        poly_kind = 1;
                    }

                    let mut poly_size = value;
                    if poly_size < 1 || poly_size > 8 {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "LineType",
                                value: format!("{}", value),
                                expected: Some("polymarker size (1-8)".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        poly_size = 1;
                    }

                    let kind = PolymarkerKind::try_from(poly_kind).unwrap_or_default();
                    LineMarkerStyle::PolyMarkerSize(kind, poly_size as u8)
                } else {
                    // Line: p2=type(1-7), p3=thickness(1-41) or endpoints(0,50-54,60-64)
                    let mut line_kind = style;
                    if line_kind < 1 || line_kind > 7 {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "LineType",
                                value: format!("{}", style),
                                expected: Some("valid LineKind (1-7)".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        line_kind = 1;
                    }

                    let kind = LineKind::try_from(line_kind).unwrap_or_default();

                    // Parse p3 based on C code logic
                    if value > 0 && value < 42 {
                        // Thickness mode: force non-solid lines to thickness 1
                        let thickness = if line_kind > 1 { 1 } else { value as u8 };
                        LineMarkerStyle::LineThickness(kind, thickness)
                    } else {
                        // Endpoint mode
                        let (left, right) = match value {
                            0 => (ArrowEnd::Square, ArrowEnd::Square),
                            50 => (ArrowEnd::Arrow, ArrowEnd::Arrow),
                            51 => (ArrowEnd::Arrow, ArrowEnd::Square),
                            52 => (ArrowEnd::Square, ArrowEnd::Arrow),
                            53 => (ArrowEnd::Arrow, ArrowEnd::Rounded),
                            54 => (ArrowEnd::Rounded, ArrowEnd::Arrow),
                            60 => (ArrowEnd::Rounded, ArrowEnd::Rounded),
                            61 => (ArrowEnd::Rounded, ArrowEnd::Square),
                            62 => (ArrowEnd::Square, ArrowEnd::Rounded),
                            63 => (ArrowEnd::Rounded, ArrowEnd::Arrow),
                            64 => (ArrowEnd::Arrow, ArrowEnd::Rounded),
                            _ => {
                                sink.report_error(
                                    crate::ParseError::InvalidParameter {
                                        command: "LineType",
                                        value: format!("{}", value),
                                        expected: Some("thickness (1-41) or endpoints (0,50-54,60-64)".to_string()),
                                    },
                                    crate::ErrorLevel::Warning,
                                );
                                (ArrowEnd::Square, ArrowEnd::Square)
                            }
                        };
                        LineMarkerStyle::LineEndpoints(kind, left, right)
                    }
                };

                Self::check_parameters(params, sink, "LineType", 3, |_sink| Some(IgsCommand::SetLineOrMarkerStyle { style }))
            }
            IgsCommandType::PauseSeconds => Self::check_parameters(params, sink, "PauseSeconds", 1, |_sink| {
                Some(IgsCommand::PauseSeconds {
                    seconds: params[0].value() as u8,
                })
            }),
            IgsCommandType::VsyncPause => Self::check_parameters(params, sink, "VsyncPause", 1, |_sink| {
                Some(IgsCommand::VsyncPause { vsyncs: params[0].value() })
            }),
            IgsCommandType::PolyLine | IgsCommandType::PolyFill => {
                if params.is_empty() {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: if self == IgsCommandType::PolyLine { "PolyLine" } else { "PolyFill" },
                            value: format!("{}", 0).to_string(),
                            expected: Some("1 parameter".to_string()),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let count = params[0].value() as usize;
                    let expected = 1 + count * 2;
                    if params.len() < expected {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: if self == IgsCommandType::PolyLine { "PolyLine" } else { "PolyFill" },
                                value: format!("{}", params.len()),
                                expected: None,
                            },
                            crate::ErrorLevel::Error,
                        );
                        None
                    } else {
                        if params.len() > expected {
                            sink.report_error(
                                crate::ParseError::InvalidParameter {
                                    command: if self == IgsCommandType::PolyLine { "PolyLine" } else { "PolyFill" },
                                    value: format!("{}", params.len()),
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
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "BellsAndWhistles",
                            value: format!("{}", 0).to_string(),
                            expected: Some("1 parameter".to_string()),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let cmd_id = params[0].value();
                    match cmd_id {
                        20 => {
                            // b>20,play_flag,snd_num,element_num,negative_flag,thousands,hundreds:
                            if params.len() < 7 {
                                sink.report_error(
                                    crate::ParseError::InvalidParameter {
                                        command: "BellsAndWhistles:AlterSoundEffect",
                                        value: format!("{}", params.len()),
                                        expected: Some("7 parameter".to_string()),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                None
                            } else {
                                if params.len() > 7 {
                                    sink.report_error(
                                        crate::ParseError::InvalidParameter {
                                            command: "BellsAndWhistles:AlterSoundEffect",
                                            value: format!("{}", params.len()),
                                            expected: Some("7 parameter".to_string()),
                                        },
                                        crate::ErrorLevel::Warning,
                                    );
                                }
                                Some(IgsCommand::AlterSoundEffect {
                                    play_flag: params[1].value() as u8,
                                    sound_effect: SoundEffect::try_from(params[2].value()).unwrap_or_else(|_| {
                                        sink.report_error(
                                            crate::ParseError::InvalidParameter {
                                                command: "BellsAndWhistles:AlterSoundEffect",
                                                value: format!("{}", params[2].value()),
                                                expected: Some("valid SoundEffect (0-19)".to_string()),
                                            },
                                            crate::ErrorLevel::Warning,
                                        );
                                        SoundEffect::default()
                                    }),
                                    element_num: params[3].value() as u8,
                                    negative_flag: params[4].value() as u8,
                                    thousands: params[5].value() as u16,
                                    hundreds: params[6].value() as u16,
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
                                sink.report_error(
                                    crate::ParseError::InvalidParameter {
                                        command: "BellsAndWhistles:RestoreSoundEffect",
                                        value: format!("{}", params.len()),
                                        expected: Some("2 parameter".to_string()),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                None
                            } else {
                                if params.len() > 2 {
                                    sink.report_error(
                                        crate::ParseError::InvalidParameter {
                                            command: "BellsAndWhistles:RestoreSoundEffect",
                                            value: format!("{}", params.len()),
                                            expected: Some("2 parameter".to_string()),
                                        },
                                        crate::ErrorLevel::Warning,
                                    );
                                }
                                Some(IgsCommand::RestoreSoundEffect {
                                    sound_effect: SoundEffect::try_from(params[1].value()).unwrap_or_else(|_| {
                                        sink.report_error(
                                            crate::ParseError::InvalidParameter {
                                                command: "BellsAndWhistles:RestoreSoundEffect",
                                                value: format!("{}", params[1].value()),
                                                expected: Some("valid SoundEffect (0-19)".to_string()),
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
                                sink.report_error(
                                    crate::ParseError::InvalidParameter {
                                        command: "BellsAndWhistles:SetEffectLoops",
                                        value: format!("{}", params.len()),
                                        expected: Some("2 parameter".to_string()),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                None
                            } else {
                                if params.len() > 2 {
                                    sink.report_error(
                                        crate::ParseError::InvalidParameter {
                                            command: "BellsAndWhistles:SetEffectLoops",
                                            value: format!("{}", params.len()),
                                            expected: Some("2 parameter".to_string()),
                                        },
                                        crate::ErrorLevel::Warning,
                                    );
                                }
                                Some(IgsCommand::SetEffectLoops {
                                    count: params[1].value() as u32,
                                })
                            }
                        }
                        _ => {
                            // b>0-19: - Play sound effect
                            Some(IgsCommand::BellsAndWhistles {
                                sound_effect: SoundEffect::try_from(cmd_id).unwrap_or_else(|_| {
                                    sink.report_error(
                                        crate::ParseError::InvalidParameter {
                                            command: "BellsAndWhistles",
                                            value: format!("{}", cmd_id).to_string(),
                                            expected: Some("valid SoundEffect (0-19)".to_string()),
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
            IgsCommandType::GraphicScaling => Self::check_parameters(params, sink, "GraphicScaling", 1, |sink| {
                let raw_mode = params[0].value() as u8;
                match GraphicsScalingMode::try_from(raw_mode) {
                    Ok(mode) => Some(IgsCommand::GraphicScaling { mode }),
                    Err(_) => {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "GraphicScaling",
                                value: format!("{}", raw_mode),
                                expected: Some("valid GraphicsScaling mode (0-2)".to_string()),
                            },
                            crate::ErrorLevel::Warning,
                        );
                        None
                    }
                }
            }),
            IgsCommandType::GrabScreen => {
                if params.len() < 2 {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "GrabScreen",
                            value: format!("{}", params.len()),
                            expected: Some("2 parameter".to_string()),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let blit_type_id = params[0].value();
                    let mode: BlitMode = params[1].value().into();

                    let operation = match blit_type_id {
                        0 => {
                            // Screen to screen: needs 6 params
                            Self::check_parameters(params, sink, "GrabScreen:ScreenToScreen", 8, |_sink| {
                                Some(BlitOperation::ScreenToScreen {
                                    src_x1: params[2].value(),
                                    src_y1: params[3].value(),
                                    src_x2: params[4].value(),
                                    src_y2: params[5].value(),
                                    dest_x: params[6].value(),
                                    dest_y: params[7].value(),
                                })
                            })
                        }
                        1 => {
                            // Screen to memory: needs 4 params (6 total with blit_type_id and mode)
                            Self::check_parameters(params, sink, "GrabScreen:ScreenToMemory", 6, |_sink| {
                                Some(BlitOperation::ScreenToMemory {
                                    src_x1: params[2].value(),
                                    src_y1: params[3].value(),
                                    src_x2: params[4].value(),
                                    src_y2: params[5].value(),
                                })
                            })
                        }
                        2 => {
                            // Memory to screen: needs 2 params (4 total with blit_type_id and mode)
                            Self::check_parameters(params, sink, "GrabScreen:MemoryToScreen", 4, |_sink| {
                                Some(BlitOperation::MemoryToScreen {
                                    dest_x: params[2].value(),
                                    dest_y: params[3].value(),
                                })
                            })
                        }
                        3 => {
                            // Piece of memory to screen: needs 6 params (8 total with blit_type_id and mode)
                            Self::check_parameters(params, sink, "GrabScreen:PieceOfMemoryToScreen", 8, |_sink| {
                                Some(BlitOperation::PieceOfMemoryToScreen {
                                    src_x1: params[2].value(),
                                    src_y1: params[3].value(),
                                    src_x2: params[4].value(),
                                    src_y2: params[5].value(),
                                    dest_x: params[6].value(),
                                    dest_y: params[7].value(),
                                })
                            })
                        }
                        4 => {
                            // Memory to memory: needs 6 params (8 total with blit_type_id and mode)
                            Self::check_parameters(params, sink, "GrabScreen:MemoryToMemory", 8, |_sink| {
                                Some(BlitOperation::MemoryToMemory {
                                    src_x1: params[2].value(),
                                    src_y1: params[3].value(),
                                    src_x2: params[4].value(),
                                    src_y2: params[5].value(),
                                    dest_x: params[6].value(),
                                    dest_y: params[7].value(),
                                })
                            })
                        }
                        _ => {
                            sink.report_error(
                                crate::ParseError::InvalidParameter {
                                    command: "GrabScreen",
                                    value: format!("{}", blit_type_id).to_string(),
                                    expected: Some("valid blit_type_id (0-4)".to_string()),
                                },
                                crate::ErrorLevel::Error,
                            );
                            None
                        }
                    };

                    operation.map(|op| IgsCommand::GrabScreen { operation: op, mode })
                }
            }
            IgsCommandType::WriteText => Self::check_parameters(params, sink, "WriteText", 2, |_sink| {
                Some(IgsCommand::WriteText {
                    x: params[0],
                    y: params[1],
                    text: text_buffer.to_vec(),
                })
            }),

            IgsCommandType::Noise => Some(IgsCommand::Noise { params: params.to_vec() }),
            IgsCommandType::RoundedRectangles => Self::check_parameters(params, sink, "RoundedRectangles", 5, |_sink| {
                Some(IgsCommand::RoundedRectangles {
                    x1: params[0],
                    y1: params[1],
                    x2: params[2],
                    y2: params[3],
                    fill: params[4].value() != 0,
                })
            }),
            IgsCommandType::PieSlice => Self::check_parameters(params, sink, "PieSlice", 5, |_sink| {
                Some(IgsCommand::PieSlice {
                    x: params[0],
                    y: params[1],
                    radius: params[2],
                    start_angle: params[3],
                    end_angle: params[4],
                })
            }),
            IgsCommandType::ExtendedCommand => {
                // X - Extended commands
                if params.is_empty() {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "ExtendedCommand",
                            value: format!("{}", 0).to_string(),
                            expected: Some("1 parameter".to_string()),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let cmd_id = params[0].value();
                    match cmd_id {
                        0 => {
                            // SprayPaint (id,x,y,width,height,density)
                            Self::check_parameters(params, sink, "ExtendedCommand:SprayPaint", 6, |_sink| {
                                Some(IgsCommand::SprayPaint {
                                    x: params[1],
                                    y: params[2],
                                    width: params[3],
                                    height: params[4],
                                    density: params[5],
                                })
                            })
                        }
                        1 => {
                            // SetColorRegister
                            Self::check_parameters(params, sink, "ExtendedCommand:SetColorRegister", 3, |_sink| {
                                Some(IgsCommand::SetColorRegister {
                                    register: params[1].value() as u8,
                                    value: params[2].value(),
                                })
                            })
                        }
                        2 => {
                            // SetRandomRange: check if Small (2 params) or Big (3 params)
                            if params.len() == 3 {
                                // Small: G#X>2,min,max:
                                Some(IgsCommand::SetRandomRange {
                                    range_type: RandomRangeType::Small {
                                        min: params[1],
                                        max: params[2],
                                    },
                                })
                            } else if params.len() == 4 {
                                // Big: G#X>2,min,min,max: (middle param is duplicate)
                                Some(IgsCommand::SetRandomRange {
                                    range_type: RandomRangeType::Big {
                                        min: params[1],
                                        max: params[3],
                                    },
                                })
                            } else {
                                sink.report_error(
                                    crate::ParseError::InvalidParameter {
                                        command: "SetRandomRange",
                                        value: format!("{}", params.len()),
                                        expected: Some("2 or 3 parameters".to_string()),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                None
                            }
                        }
                        3 => {
                            // RightMouseMacro
                            Some(IgsCommand::RightMouseMacro { params: params[1..].to_vec() })
                        }
                        4 => {
                            // DefineZone: Special handling for clear (9999-9997)
                            if params.len() == 2 && (9997..=9999).contains(&params[1].value()) {
                                // Clear command or loopback toggle - no additional params needed
                                Some(IgsCommand::DefineZone {
                                    zone_id: params[1].value(),
                                    x1: 0.into(),
                                    y1: 0.into(),
                                    x2: 0.into(),
                                    y2: 0.into(),
                                    length: 0,
                                    string: Vec::new(),
                                })
                            } else if params.len() >= 8 && !text_buffer.is_empty() {
                                Some(IgsCommand::DefineZone {
                                    zone_id: params[1].value(),
                                    x1: params[2],
                                    y1: params[3],
                                    x2: params[4],
                                    y2: params[5],
                                    length: params[6].value() as u16,
                                    string: text_buffer.to_vec(),
                                })
                            } else {
                                sink.report_error(
                                    crate::ParseError::InvalidParameter {
                                        command: "ExtendedCommand:DefineZone",
                                        value: format!("{}", params.len()),
                                        expected: Some("2 parameter (9997-9999) or 8+ parameter with text".to_string()),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                None
                            }
                        }
                        5 => {
                            // FlowControl
                            Self::check_parameters(params, sink, "ExtendedCommand:FlowControl", 2, |_sink| {
                                Some(IgsCommand::FlowControl {
                                    mode: params[1].value() as u8,
                                    params: params[2..].to_vec(),
                                })
                            })
                        }
                        6 => {
                            // LeftMouseButton
                            Self::check_parameters(params, sink, "ExtendedCommand:LeftMouseButton", 2, |_sink| {
                                Some(IgsCommand::LeftMouseButton { mode: params[1].value() as u8 })
                            })
                        }
                        7 => {
                            // LoadFillPattern
                            // Format: 16 lines × 16 characters = 272 bytes
                            // Each line: 16 pattern chars + '@' delimiter
                            const EXPECTED_LENGTH: usize = 16 * 16;

                            if text_buffer.len() != EXPECTED_LENGTH {
                                sink.report_error(
                                    crate::ParseError::InvalidParameter {
                                        command: "ExtendedCommand:LoadFillPattern",
                                        value: format!("{} bytes", text_buffer.len()),
                                        expected: Some(format!("{} bytes (16 lines × 17 chars)", EXPECTED_LENGTH)),
                                    },
                                    crate::ErrorLevel::Error,
                                );
                                return None;
                            }

                            let data: Vec<u16> = Self::parse_pattern_buffer(text_buffer, sink)?;

                            Self::check_parameters(params, sink, "ExtendedCommand:LoadFillPattern", 2, move |_sink| {
                                Some(IgsCommand::LoadFillPattern {
                                    pattern: params[1].value() as u8,
                                    data,
                                })
                            })
                        }
                        8 => {
                            // RotateColorRegisters
                            Self::check_parameters(params, sink, "ExtendedCommand:RotateColorRegisters", 5, |_sink| {
                                Some(IgsCommand::RotateColorRegisters {
                                    start_reg: params[1].value() as u8,
                                    end_reg: params[2].value() as u8,
                                    count: params[3].value(),
                                    delay: params[4].value(),
                                })
                            })
                        }
                        9 => {
                            // LoadMidiBuffer
                            Some(IgsCommand::LoadMidiBuffer { params: params[1..].to_vec() })
                        }
                        10 => {
                            // SetDrawtoBegin
                            Self::check_parameters(params, sink, "ExtendedCommand:SetDrawtoBegin", 3, |_sink| {
                                Some(IgsCommand::SetDrawtoBegin { x: params[1], y: params[2] })
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
                            sink.report_error(
                                crate::ParseError::InvalidParameter {
                                    command: "ExtendedCommand",
                                    value: format!("{}", cmd_id).to_string(),
                                    expected: Some("valid cmd_id (0-12)".to_string()),
                                },
                                crate::ErrorLevel::Error,
                            );
                            None
                        }
                    }
                }
            }
            IgsCommandType::EllipticalPieSlice => Self::check_parameters(params, sink, "EllipticalPieSlice", 6, |_sink| {
                Some(IgsCommand::EllipticalPieSlice {
                    x: params[0],
                    y: params[1],
                    x_radius: params[2],
                    y_radius: params[3],
                    start_angle: params[4],
                    end_angle: params[5],
                })
            }),
            IgsCommandType::FilledRectangle => Self::check_parameters(params, sink, "FilledRectangle", 4, |_sink| {
                Some(IgsCommand::FilledRectangle {
                    x1: params[0],
                    y1: params[1],
                    x2: params[2],
                    y2: params[3],
                })
            }),
            IgsCommandType::CursorMotion => {
                // m - cursor motion
                // IG form: direction,count
                // ESC form previously provided x,y; map to direction/count
                if params.len() < 2 {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "CursorMotion",
                            value: format!("{}", params.len()),
                            expected: Some("2 parameter".to_string()),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let a = params[0].value();
                    let b = params[1].value();
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
            IgsCommandType::PositionCursor => Self::check_parameters(params, sink, "PositionCursor", 2, |_sink| {
                Some(IgsCommand::PositionCursor { x: params[0], y: params[1] })
            }),
            IgsCommandType::InverseVideo => Self::check_parameters(params, sink, "InverseVideo", 1, |_sink| {
                Some(IgsCommand::InverseVideo {
                    enabled: params[0].value() != 0,
                })
            }),
            IgsCommandType::LineWrap => Self::check_parameters(params, sink, "LineWrap", 1, |_sink| {
                Some(IgsCommand::LineWrap {
                    enabled: params[0].value() != 0,
                })
            }),
            IgsCommandType::InputCommand => Self::check_parameters(params, sink, "InputCommand", 1, |_sink| {
                Some(IgsCommand::InputCommand {
                    input_type: params[0].value() as u8,
                    params: params[1..].to_vec(),
                })
            }),
            IgsCommandType::AskIG => {
                if params.is_empty() {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "AskIG",
                            value: format!("{}", 0).to_string(),
                            expected: Some("at least 1 parameter required".to_string()),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let query = match params[0].value() {
                        0 => Some(AskQuery::VersionNumber),
                        1 => {
                            let pointer_type = if params.len() > 1 {
                                match MousePointerType::try_from(params[1].value()) {
                                    Ok(pt) => pt,
                                    Err(_) => {
                                        sink.report_error(
                                            crate::ParseError::InvalidParameter {
                                                command: "AskIG",
                                                value: format!("{}", params[1].value()),
                                                expected: Some("valid MousePointerType (0-10)".to_string()),
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
                                match MousePointerType::try_from(params[1].value()) {
                                    Ok(pt) => pt,
                                    Err(_) => {
                                        sink.report_error(
                                            crate::ParseError::InvalidParameter {
                                                command: "AskIG",
                                                value: format!("{}", params[1].value()),
                                                expected: Some("valid MousePointerType (0-10)".to_string()),
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
                            sink.report_error(
                                crate::ParseError::InvalidParameter {
                                    command: "AskIG",
                                    value: format!("{}", params[0].value()),
                                    expected: Some("valid query type (0-3)".to_string()),
                                },
                                crate::ErrorLevel::Error,
                            );
                            None
                        }
                    };
                    query.map(|q| IgsCommand::AskIG { query: q })
                }
            }
            IgsCommandType::SetTextColor => {
                // G#c>layer,color: where layer is 0 (background) or 1 (foreground)
                if params.len() < 2 {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "SetTextColor",
                            value: format!("{}", params.len()),
                            expected: Some("2 parameters (layer, color)".to_string()),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else {
                    let layer_value = params[0].value();
                    let color = params[1].value() as u8;
                    let layer = if layer_value == 0 {
                        TextColorLayer::Background
                    } else if layer_value == 1 {
                        TextColorLayer::Foreground
                    } else {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "SetTextColor",
                                value: format!("{}", layer_value),
                                expected: Some("0 (background) or 1 (foreground)".to_string()),
                            },
                            crate::ErrorLevel::Error,
                        );
                        return None;
                    };
                    Some(IgsCommand::SetTextColor { layer, color })
                }
            }
            IgsCommandType::DeleteLines => Self::check_parameters(params, sink, "DeleteLines", 1, |_sink| {
                Some(IgsCommand::DeleteLine {
                    count: params[0].value() as u8,
                })
            }),
            IgsCommandType::InsertLine => {
                // G#i>mode,count: - mode is optional, defaults to 0
                if params.is_empty() {
                    sink.report_error(
                        crate::ParseError::InvalidParameter {
                            command: "InsertLine",
                            value: format!("{}", 0),
                            expected: Some("1 or 2 parameters (count or mode,count)".to_string()),
                        },
                        crate::ErrorLevel::Error,
                    );
                    None
                } else if params.len() == 1 {
                    // ESC i form: just count, mode defaults to 0
                    Some(IgsCommand::InsertLine {
                        mode: 0,
                        count: params[0].value() as u8,
                    })
                } else {
                    // G# form: mode,count
                    Some(IgsCommand::InsertLine {
                        mode: params[0].value() as u8,
                        count: params[1].value() as u8,
                    })
                }
            }
            IgsCommandType::ClearLine => {
                // G#l>mode: - mode defaults to 0 if not provided
                let mode = params.get(0).map(|p| p.value() as u8).unwrap_or(0);
                Some(IgsCommand::ClearLine { mode })
            }
            IgsCommandType::RememberCursor => {
                // G#r>value: - value defaults to 0 if not provided
                let value = params.get(0).map(|p| p.value() as u8).unwrap_or(0);
                Some(IgsCommand::RememberCursor { value })
            }
            IgsCommandType::LoopCommand => {
                // Handled in parser (No inner loops allowed)
                None
            }
        }
    }
}
