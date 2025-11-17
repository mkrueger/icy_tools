/// Mouse pointer type for AskIG queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MousePointerType {
    /// No pointer change (immediate response)
    Immediate = 0,
    /// Polymarker pointer (selected with T command)
    Polymarker = 1,
    /// Another polymarker variant
    Polymarker2 = 2,
    /// GEM Arrow pointer
    Arrow = 3,
    /// GEM Hour Glass pointer
    HourGlass = 4,
    /// GEM Bumble Bee pointer
    BumbleBee = 5,
    /// GEM Pointing Finger pointer
    PointingFinger = 6,
    /// GEM Flat Hand pointer
    FlatHand = 7,
    /// GEM Thin Cross Hair pointer
    ThinCrossHair = 8,
    /// GEM Thick Cross Hair pointer
    ThickCrossHair = 9,
    /// GEM Outlined Cross Hair pointer
    OutlinedCrossHair = 10,
}

impl From<i32> for MousePointerType {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Immediate,
            1 => Self::Polymarker,
            2 => Self::Polymarker2,
            3 => Self::Arrow,
            4 => Self::HourGlass,
            5 => Self::BumbleBee,
            6 => Self::PointingFinger,
            7 => Self::FlatHand,
            8 => Self::ThinCrossHair,
            9 => Self::ThickCrossHair,
            10 => Self::OutlinedCrossHair,
            _ => Self::Immediate,
        }
    }
}

/// Query type for AskIG command
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskQuery {
    /// Query version number
    ///
    /// IGS: `G#?>0:`
    ///
    /// Asks IG to transmit its version number in ASCII to the host system
    VersionNumber,

    /// Ask IG where the cursor is and the mouse button state
    ///
    /// IGS: `G#?>1,pointer_type:`
    ///
    /// When pointer_type is 0, returns cursor location immediately.
    /// When pointer_type is 1+, the user can move the cursor with the mouse
    /// until a button is pressed (point and click cursor).
    ///
    /// Response format: Three ASCII characters (subtract 32 from each):
    /// - COLUMN number (0-79)
    /// - ROW (0-24)  
    /// - BUTTON (0-3)
    ///
    /// Note: Cursor should be enabled with `G#k>1:` command
    CursorPositionAndMouseButton { pointer_type: MousePointerType },

    /// Ask IG where the mouse is and button state
    ///
    /// IGS: `G#?>2,pointer_type:`
    ///
    /// Similar to CursorPositionAndMouseButton but returns pixel coordinates
    /// instead of character cell coordinates.
    ///
    /// Response format: ASCII string like "420,150,1:"
    /// - X coordinate (pixels)
    /// - Y coordinate (pixels)
    /// - Button state (0-3)
    MousePositionAndButton { pointer_type: MousePointerType },

    /// Query current resolution
    ///
    /// IGS: `G#?>3:`
    ///
    /// Asks IG what resolution the terminal is in:
    /// - 0: low resolution (320x200)
    /// - 1: medium resolution (640x200)
    /// - 2: high resolution (640x400)
    CurrentResolution,
}
