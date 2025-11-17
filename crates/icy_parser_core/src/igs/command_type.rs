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
}
