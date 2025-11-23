use std::fmt;

/// Screen clear mode for the ScreenClear command
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenClearMode {
    /// Clear screen and home cursor
    ClearAndHome = 0,
    /// Clear from home to cursor
    ClearHomeToToCursor = 1,
    /// Clear from cursor to bottom of screen
    ClearCursorToBottom = 2,
    /// Clear WHOLE screen with VDI
    ClearWholeScreen = 3,
    /// Clear WHOLE screen with VDI and VT52 cursor will be set to home
    ClearWholeScreenAndHome = 4,
    /// Clear, Home, ReverseOff, Text Background to reg 0, Text Color to register 3
    /// All done with VT52, a VT52 quick reset of sorts
    QuickVt52Reset = 5,
}

impl Default for ScreenClearMode {
    fn default() -> Self {
        Self::ClearAndHome
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidScreenClearMode(pub u8);

impl TryFrom<u8> for ScreenClearMode {
    type Error = InvalidScreenClearMode;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::ClearAndHome),
            1 => Ok(Self::ClearHomeToToCursor),
            2 => Ok(Self::ClearCursorToBottom),
            3 => Ok(Self::ClearWholeScreen),
            4 => Ok(Self::ClearWholeScreenAndHome),
            5 => Ok(Self::QuickVt52Reset),
            other => Err(InvalidScreenClearMode(other)),
        }
    }
}

impl fmt::Display for ScreenClearMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", *self as u8)
    }
}
