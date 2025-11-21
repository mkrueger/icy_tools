/// CRC Transfer modes for Command 16
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrcTransferMode {
    /// Mode 1: IFF Brush format
    IffBrush = 1,
    /// Mode 2: IFF Sound format
    IffSound = 2,
    /// Mode 3: FutureSound format
    FutureSound = 3,
    /// Mode 20: General purpose file transfer
    GeneralPurpose = 20,
}

impl TryFrom<i32> for CrcTransferMode {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::IffBrush),
            2 => Ok(Self::IffSound),
            3 => Ok(Self::FutureSound),
            20 => Ok(Self::GeneralPurpose),
            _ => Err(()),
        }
    }
}

impl From<CrcTransferMode> for i32 {
    fn from(mode: CrcTransferMode) -> Self {
        mode as i32
    }
}

impl std::fmt::Display for CrcTransferMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IffBrush => write!(f, "IFF Brush"),
            Self::IffSound => write!(f, "IFF Sound"),
            Self::FutureSound => write!(f, "FutureSound"),
            Self::GeneralPurpose => write!(f, "General Purpose"),
        }
    }
}

/// Display modes for Command 17
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    /// Mode 1: 8 colors (3 bitplanes)
    EightColors = 1,
    /// Mode 2: 16 colors (4 bitplanes)
    SixteenColors = 2,
}

impl TryFrom<i32> for DisplayMode {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::EightColors),
            2 => Ok(Self::SixteenColors),
            _ => Err(()),
        }
    }
}

impl From<DisplayMode> for i32 {
    fn from(mode: DisplayMode) -> Self {
        mode as i32
    }
}

impl std::fmt::Display for DisplayMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EightColors => write!(f, "8 colors"),
            Self::SixteenColors => write!(f, "16 colors"),
        }
    }
}

/// SkyPix command numbers
pub mod command_numbers {
    pub const SET_PIXEL: i32 = 1;
    pub const DRAW_LINE: i32 = 2;
    pub const AREA_FILL: i32 = 3;
    pub const RECTANGLE_FILL: i32 = 4;
    pub const ELLIPSE: i32 = 5;
    pub const GRAB_BRUSH: i32 = 6;
    pub const USE_BRUSH: i32 = 7;
    pub const MOVE_PEN: i32 = 8;
    pub const PLAY_SAMPLE: i32 = 9;
    pub const SET_FONT: i32 = 10;
    pub const NEW_PALETTE: i32 = 11;
    pub const RESET_PALETTE: i32 = 12;
    pub const FILLED_ELLIPSE: i32 = 13;
    pub const DELAY: i32 = 14;
    pub const SET_PEN_A: i32 = 15;
    pub const CRC_TRANSFER: i32 = 16;
    pub const SET_DISPLAY_MODE: i32 = 17;
    pub const SET_PEN_B: i32 = 18;
    pub const POSITION_CURSOR: i32 = 19;
    pub const CONTROLLER_RETURN: i32 = 21;
    pub const DEFINE_GADGET: i32 = 22;
}

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
    CrcTransfer {
        mode: CrcTransferMode,
        width: i32,
        height: i32,
        filename: String,
    },

    /// Command 17: Select display mode (1=8 colors, 2=16 colors)
    SetDisplayMode { mode: DisplayMode },

    /// Command 18: Set Pen B (background) color
    SetPenB { color: i32 },

    /// Command 19: Position cursor at pixel coordinates (x, y)
    PositionCursor { x: i32, y: i32 },

    /// Command 21: Controller return (mouse click or menu selection)
    ControllerReturn { c: i32, x: i32, y: i32 },

    /// Command 22: Define gadget
    DefineGadget { num: i32, cmd: i32, x1: i32, y1: i32, x2: i32, y2: i32 },
}
