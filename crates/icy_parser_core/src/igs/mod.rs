//! IGS (Interactive Graphics System) parser
//!
//! IGS is a graphics system developed for Atari ST BBS systems.
//! Commands start with 'G#' and use single-letter command codes followed by parameters.

use crate::{CommandParser, CommandSink, TerminalCommand};

mod command;
pub use command::*;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Default,
    GotG,
    GotIgsStart,
    _ReadCommandChar,
    ReadParams(IgsCommandType),
    ReadTextString(i32, i32, u8), // x, y, justification
    _ReadLoopParams,
    ReadLoopTokens,           // specialized loop command token parsing
    ReadZoneString(Vec<i32>), // extended command X 4 zone string reading after numeric params
    ReadFillPattern(i32),     // extended command X 7 pattern data reading after id,pattern

    // VT52 states
    Escape,
    ReadFgColor,
    ReadBgColor,
    ReadCursorX,
    ReadCursorY(i32), // row position
    ReadDeleteLineCount,
    ReadInsertLineCount,
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
    loop_tokens: Vec<String>,
    loop_token_buffer: String,
    reading_chain_gang: bool, // True when reading >XXX@ chain-gang identifier
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
            loop_tokens: Vec::new(),
            loop_token_buffer: String::new(),
            reading_chain_gang: false,
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
                        radius: self.params[2],
                        start_angle: self.params[3],
                        end_angle: self.params[4],
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
            IgsCommandType::PolyMarker => {
                if self.params.len() >= 2 {
                    Some(IgsCommand::PolymarkerPlot {
                        x: self.params[0],
                        y: self.params[1],
                    })
                } else {
                    None
                }
            }
            IgsCommandType::SetPenColor => {
                if self.params.len() >= 4 {
                    Some(IgsCommand::SetPenColor {
                        pen: self.params[0] as u8,
                        red: self.params[1] as u8,
                        green: self.params[2] as u8,
                        blue: self.params[3] as u8,
                    })
                } else {
                    None
                }
            }
            IgsCommandType::DrawingMode => {
                if !self.params.is_empty() {
                    Some(IgsCommand::DrawingMode { mode: self.params[0] as u8 })
                } else {
                    None
                }
            }
            IgsCommandType::HollowSet => {
                if !self.params.is_empty() {
                    Some(IgsCommand::HollowSet { enabled: self.params[0] != 0 })
                } else {
                    None
                }
            }
            IgsCommandType::Initialize => {
                if !self.params.is_empty() {
                    Some(IgsCommand::Initialize { mode: self.params[0] as u8 })
                } else {
                    None
                }
            }
            IgsCommandType::EllipticalArc => {
                if self.params.len() >= 6 {
                    Some(IgsCommand::EllipticalArc {
                        x: self.params[0],
                        y: self.params[1],
                        x_radius: self.params[2],
                        y_radius: self.params[3],
                        start_angle: self.params[4],
                        end_angle: self.params[5],
                    })
                } else {
                    None
                }
            }
            IgsCommandType::Cursor => {
                if !self.params.is_empty() {
                    Some(IgsCommand::Cursor { mode: self.params[0] as u8 })
                } else {
                    None
                }
            }
            IgsCommandType::ChipMusic => {
                if self.params.len() >= 6 {
                    Some(IgsCommand::ChipMusic {
                        effect: self.params[0] as u8,
                        voice: self.params[1] as u8,
                        volume: self.params[2] as u8,
                        pitch: self.params[3] as u8,
                        timing: self.params[4],
                        stop_type: self.params[5] as u8,
                    })
                } else {
                    None
                }
            }
            IgsCommandType::ScreenClear => {
                if !self.params.is_empty() {
                    Some(IgsCommand::ScreenClear { mode: self.params[0] as u8 })
                } else {
                    None
                }
            }
            IgsCommandType::SetResolution => {
                if self.params.len() >= 2 {
                    Some(IgsCommand::SetResolution {
                        resolution: self.params[0] as u8,
                        palette: self.params[1] as u8,
                    })
                } else {
                    None
                }
            }
            IgsCommandType::LineType => {
                if self.params.len() >= 3 {
                    Some(IgsCommand::LineStyle {
                        kind: self.params[0] as u8,
                        style: self.params[1] as u8,
                        value: self.params[2] as u16,
                    })
                } else {
                    None
                }
            }
            IgsCommandType::PauseSeconds => {
                if !self.params.is_empty() {
                    Some(IgsCommand::PauseSeconds { seconds: self.params[0] as u8 })
                } else {
                    None
                }
            }
            IgsCommandType::VsyncPause => {
                if !self.params.is_empty() {
                    Some(IgsCommand::VsyncPause { vsyncs: self.params[0] })
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
                    let cmd_id = self.params[0];
                    match cmd_id {
                        20 => {
                            // b>20,play_flag,snd_num,element_num,negative_flag,thousands,hundreds:
                            if self.params.len() >= 7 {
                                Some(IgsCommand::AlterSoundEffect {
                                    play_flag: self.params[1] as u8,
                                    snd_num: self.params[2] as u8,
                                    element_num: self.params[3] as u8,
                                    negative_flag: self.params[4] as u8,
                                    thousands: self.params[5] as u16,
                                    hundreds: self.params[6] as u16,
                                })
                            } else {
                                None
                            }
                        }
                        21 => {
                            // b>21: - Stop all sounds
                            Some(IgsCommand::StopAllSound)
                        }
                        22 => {
                            // b>22,snd_num: - Restore sound effect
                            if self.params.len() >= 2 {
                                Some(IgsCommand::RestoreSoundEffect { snd_num: self.params[1] as u8 })
                            } else {
                                None
                            }
                        }
                        23 => {
                            // b>23,count: - Set effect loops
                            if self.params.len() >= 2 {
                                Some(IgsCommand::SetEffectLoops { count: self.params[1] as u32 })
                            } else {
                                None
                            }
                        }
                        _ => {
                            // b>0-19: - Play sound effect
                            Some(IgsCommand::BellsAndWhistles { sound_number: cmd_id as u8 })
                        }
                    }
                } else {
                    None
                }
            }
            IgsCommandType::GraphicScaling => {
                if !self.params.is_empty() {
                    Some(IgsCommand::GraphicScaling { mode: self.params[0] as u8 })
                } else {
                    None
                }
            }
            IgsCommandType::GrabScreen => {
                if self.params.len() >= 2 {
                    let blit_type = self.params[0] as u8;
                    let mode = self.params[1] as u8;
                    let remaining_params = self.params[2..].to_vec();
                    Some(IgsCommand::GrabScreen {
                        blit_type,
                        mode,
                        params: remaining_params,
                    })
                } else {
                    None
                }
            }
            IgsCommandType::WriteText => {
                if self.params.len() >= 2 {
                    Some(IgsCommand::WriteText {
                        x: self.params[0],
                        y: self.params[1],
                        text: self.text_buffer.clone(),
                    })
                } else {
                    None
                }
            }
            IgsCommandType::LoopCommand => {
                // & from,to,step,delay,cmd,param_count,(params...)
                if self.params.len() >= 6 {
                    let from = self.params[0];
                    let to = self.params[1];
                    let step = self.params[2];
                    let delay = self.params[3];
                    let command_identifier = self.loop_command.clone();
                    let param_count = self.params[4] as u16;
                    use crate::igs::command::{LoopCommandData, LoopModifiers, LoopParamToken, LoopTarget};

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
                                    if let Ok(n) = token.parse::<i32>() {
                                        params_tokens.push(LoopParamToken::Number(n));
                                    } else {
                                        params_tokens.push(LoopParamToken::Expr(token.clone()));
                                    }
                                }
                            }
                        }
                    }

                    let mut modifiers = LoopModifiers::default();
                    let mut base_ident = command_identifier.as_str();
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

                    let target = if base_ident.starts_with('>') && base_ident.ends_with('@') {
                        let inner: String = base_ident.chars().skip(1).take(base_ident.len().saturating_sub(2)).collect();
                        let commands: Vec<char> = inner.chars().collect();
                        LoopTarget::ChainGang {
                            raw: base_ident.to_string(),
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
                } else {
                    None
                }
            }
            IgsCommandType::Noise => Some(IgsCommand::Noise { params: self.params.clone() }),
            IgsCommandType::RoundedRectangles => {
                // U - RoundedRectangles
                if self.params.len() >= 5 {
                    Some(IgsCommand::RoundedRectangles {
                        x1: self.params[0],
                        y1: self.params[1],
                        x2: self.params[2],
                        y2: self.params[3],
                        fill: self.params[4] != 0,
                    })
                } else {
                    None
                }
            }
            IgsCommandType::PieSlice => {
                // V - PieSlice
                if self.params.len() >= 5 {
                    Some(IgsCommand::PieSlice {
                        x: self.params[0],
                        y: self.params[1],
                        radius: self.params[2],
                        start_angle: self.params[3],
                        end_angle: self.params[4],
                    })
                } else {
                    None
                }
            }
            IgsCommandType::ExtendedCommand => {
                // X - Extended commands
                if !self.params.is_empty() {
                    let cmd_id = self.params[0];
                    match cmd_id {
                        0 => {
                            // SprayPaint (id,x,y,width,height,density)
                            if self.params.len() >= 6 {
                                Some(IgsCommand::SprayPaint {
                                    x: self.params[1],
                                    y: self.params[2],
                                    width: self.params[3],
                                    height: self.params[4],
                                    density: self.params[5],
                                })
                            } else {
                                None
                            }
                        }
                        1 => {
                            // SetColorRegister
                            if self.params.len() >= 3 {
                                Some(IgsCommand::SetColorRegister {
                                    register: self.params[1] as u8,
                                    value: self.params[2],
                                })
                            } else {
                                None
                            }
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
                                None
                            }
                        }
                        5 => {
                            // FlowControl
                            if self.params.len() >= 2 {
                                Some(IgsCommand::FlowControl {
                                    mode: self.params[1] as u8,
                                    params: self.params[2..].to_vec(),
                                })
                            } else {
                                None
                            }
                        }
                        6 => {
                            // LeftMouseButton
                            if self.params.len() >= 2 {
                                Some(IgsCommand::LeftMouseButton { mode: self.params[1] as u8 })
                            } else {
                                None
                            }
                        }
                        7 => {
                            // LoadFillPattern
                            if self.params.len() >= 2 {
                                Some(IgsCommand::LoadFillPattern {
                                    pattern: self.params[1] as u8,
                                    data: self.text_buffer.clone(),
                                })
                            } else {
                                None
                            }
                        }
                        8 => {
                            // RotateColorRegisters
                            if self.params.len() >= 5 {
                                Some(IgsCommand::RotateColorRegisters {
                                    start_reg: self.params[1] as u8,
                                    end_reg: self.params[2] as u8,
                                    count: self.params[3],
                                    delay: self.params[4],
                                })
                            } else {
                                None
                            }
                        }
                        9 => {
                            // LoadMidiBuffer
                            Some(IgsCommand::LoadMidiBuffer {
                                params: self.params[1..].to_vec(),
                            })
                        }
                        10 => {
                            // SetDrawtoBegin
                            if self.params.len() >= 3 {
                                Some(IgsCommand::SetDrawtoBegin {
                                    x: self.params[1],
                                    y: self.params[2],
                                })
                            } else {
                                None
                            }
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
                        _ => None,
                    }
                } else {
                    None
                }
            }
            IgsCommandType::EllipticalPieSlice => {
                // Y - EllipticalPieSlice
                if self.params.len() >= 6 {
                    Some(IgsCommand::EllipticalPieSlice {
                        x: self.params[0],
                        y: self.params[1],
                        x_radius: self.params[2],
                        y_radius: self.params[3],
                        start_angle: self.params[4],
                        end_angle: self.params[5],
                    })
                } else {
                    None
                }
            }
            IgsCommandType::FilledRectangle => {
                // Z - FilledRectangle
                if self.params.len() >= 4 {
                    Some(IgsCommand::FilledRectangle {
                        x1: self.params[0],
                        y1: self.params[1],
                        x2: self.params[2],
                        y2: self.params[3],
                    })
                } else {
                    None
                }
            }
            IgsCommandType::CursorMotion => {
                // m - cursor motion
                // IG form: direction,count
                // ESC form previously provided x,y; map to direction/count
                if self.params.len() >= 2 {
                    let a = self.params[0];
                    let b = self.params[1];
                    // Heuristic: if both non-zero prefer horizontal if y==0 else vertical
                    let (direction, count) = if a != 0 && b == 0 {
                        if a > 0 { (3u8, a) } else { (2u8, -a) }
                    } else if b != 0 && a == 0 {
                        if b > 0 { (1u8, b) } else { (0u8, -b) }
                    } else {
                        // Assume IG form already direction,count
                        let dir = a as i32;
                        (dir.clamp(0, 3) as u8, b)
                    };
                    Some(IgsCommand::CursorMotion { direction, count })
                } else {
                    None
                }
            }
            IgsCommandType::PositionCursor => {
                // p - position cursor (VT52)
                if self.params.len() >= 2 {
                    Some(IgsCommand::PositionCursor {
                        x: self.params[0],
                        y: self.params[1],
                    })
                } else {
                    None
                }
            }
            IgsCommandType::InverseVideo => {
                // v - inverse video (VT52)
                if !self.params.is_empty() {
                    Some(IgsCommand::InverseVideo { enabled: self.params[0] != 0 })
                } else {
                    None
                }
            }
            IgsCommandType::LineWrap => {
                // w - line wrap (VT52)
                if !self.params.is_empty() {
                    Some(IgsCommand::LineWrap { enabled: self.params[0] != 0 })
                } else {
                    None
                }
            }
            IgsCommandType::InputCommand => {
                // < - input command
                if !self.params.is_empty() {
                    Some(IgsCommand::InputCommand {
                        input_type: self.params[0] as u8,
                        params: self.params[1..].to_vec(),
                    })
                } else {
                    None
                }
            }
            IgsCommandType::AskIG => {
                // ? - ask IG
                if !self.params.is_empty() {
                    Some(IgsCommand::AskIG { query: self.params[0] as u8 })
                } else {
                    None
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
                        // Use specialized token parser for loop command because parameters include substitution tokens.
                        self.state = State::ReadLoopTokens;
                        self.loop_tokens.clear();
                        self.loop_token_buffer.clear();
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
                                    // Invalid for other commands
                                    self.reset_params();
                                    self.state = State::Default;
                                }
                            } else {
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
                                use crate::igs::command::{LoopCommandData, LoopModifiers, LoopParamToken, LoopTarget};

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
                                    let mut base_ident = raw_identifier.as_str();

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

                                    let target = if base_ident.starts_with('>') && base_ident.ends_with('@') {
                                        let inner: String = base_ident.chars().skip(1).take(base_ident.len().saturating_sub(2)).collect();
                                        let commands: Vec<char> = inner.chars().collect();
                                        LoopTarget::ChainGang {
                                            raw: base_ident.to_string(),
                                            commands,
                                        }
                                    } else {
                                        let ch = base_ident.chars().next().unwrap_or(' ');
                                        LoopTarget::Single(ch)
                                    };

                                    // Convert parameters into typed tokens, preserving ':' position
                                    let mut params: Vec<LoopParamToken> = Vec::new();
                                    for token in &self.loop_tokens[6..] {
                                        if token == ":" {
                                            params.push(LoopParamToken::GroupSeparator);
                                        } else if token == "x" || token == "y" {
                                            params.push(LoopParamToken::Symbol(token.chars().next().unwrap()));
                                        } else if let Ok(n) = token.parse::<i32>() {
                                            params.push(LoopParamToken::Number(n));
                                        } else {
                                            params.push(LoopParamToken::Expr(token.clone()));
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
                                use crate::igs::command::{LoopCommandData, LoopModifiers, LoopParamToken, LoopTarget};

                                let parse_i32 = |s: &str| s.parse::<i32>().unwrap_or(0);
                                let from = parse_i32(&self.loop_tokens[0]);
                                let to = parse_i32(&self.loop_tokens[1]);
                                let step = parse_i32(&self.loop_tokens[2]);
                                let delay = parse_i32(&self.loop_tokens[3]);
                                let raw_identifier = self.loop_tokens[4].clone();
                                let param_count = parse_i32(&self.loop_tokens[5]) as usize;

                                let mut modifiers = LoopModifiers::default();
                                let mut base_ident = raw_identifier.as_str();
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

                                let target = if base_ident.starts_with('>') && base_ident.ends_with('@') {
                                    let inner: String = base_ident.chars().skip(1).take(base_ident.len().saturating_sub(2)).collect();
                                    let commands: Vec<char> = inner.chars().collect();
                                    LoopTarget::ChainGang {
                                        raw: base_ident.to_string(),
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
                                    } else if let Ok(n) = token.parse::<i32>() {
                                        params.push(LoopParamToken::Number(n));
                                    } else {
                                        params.push(LoopParamToken::Expr(token.clone()));
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
                        'd' => {
                            // Delete line - next byte is line count
                            self.state = State::ReadDeleteLineCount;
                        }
                        'i' => {
                            // Insert line ESC form: mode implicitly 0, next byte is count
                            self.state = State::ReadInsertLineCount;
                        }
                        'l' => {
                            // Clear line ESC form: mode implicitly 0
                            sink.emit_igs(IgsCommand::ClearLine { mode: 0 });
                            self.state = State::Default;
                        }
                        'r' => {
                            // Remember cursor ESC form: value implicitly 0
                            sink.emit_igs(IgsCommand::RememberCursor { value: 0 });
                            self.state = State::Default;
                        }
                        'm' | 'p' | 'v' | 'w' => {
                            // IGS commands that can be invoked with ESC prefix instead of G#
                            // ESC m x,y:  - cursor motion
                            // ESC p x,y:  - position cursor
                            // ESC v n:    - inverse video
                            // ESC w n:    - line wrap
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
                    let color = byte;
                    sink.emit_igs(IgsCommand::SetForeground { color });
                    self.state = State::Default;
                }
                State::ReadBgColor => {
                    let color = byte;
                    sink.emit_igs(IgsCommand::SetBackground { color });
                    self.state = State::Default;
                }
                State::ReadCursorX => {
                    let row = (byte.wrapping_sub(32)) as i32;
                    self.state = State::ReadCursorY(row);
                }
                State::ReadCursorY(row) => {
                    let col = (byte.wrapping_sub(32)) as i32;
                    sink.emit_igs(IgsCommand::SetCursorPos { x: col, y: row });
                    self.state = State::Default;
                }
                State::ReadDeleteLineCount => {
                    let count = byte;
                    sink.emit_igs(IgsCommand::DeleteLine { count });
                    self.state = State::Default;
                }
                State::ReadInsertLineCount => {
                    let count = byte;
                    sink.emit_igs(IgsCommand::InsertLine { mode: 0, count });
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
