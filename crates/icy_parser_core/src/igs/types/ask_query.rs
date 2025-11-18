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

impl Default for MousePointerType {
    fn default() -> Self {
        Self::Immediate
    }
}

impl TryFrom<i32> for MousePointerType {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Immediate),
            1 => Ok(Self::Polymarker),
            2 => Ok(Self::Polymarker2),
            3 => Ok(Self::Arrow),
            4 => Ok(Self::HourGlass),
            5 => Ok(Self::BumbleBee),
            6 => Ok(Self::PointingFinger),
            7 => Ok(Self::FlatHand),
            8 => Ok(Self::ThinCrossHair),
            9 => Ok(Self::ThickCrossHair),
            10 => Ok(Self::OutlinedCrossHair),
            _ => Err(format!("Invalid MousePointerType value: {}", value)),
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
