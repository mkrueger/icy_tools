use std::fmt;

/// Stop type for ChipMusic command - controls how notes end
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StopType {
    /// No effect, sound continues (0)
    #[default]
    NoEffect = 0,
    /// Move voice to release phase (soft stop) (1)
    SndOff = 1,
    /// Stop voice immediately (hard stop) (2)
    StopSnd = 2,
    /// Move all voices to release phase (3)
    SndOffAll = 3,
    /// Stop all voices immediately (4)
    StopSndAll = 4,
}

impl TryFrom<u8> for StopType {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(StopType::NoEffect),
            1 => Ok(StopType::SndOff),
            2 => Ok(StopType::StopSnd),
            3 => Ok(StopType::SndOffAll),
            4 => Ok(StopType::StopSndAll),
            _ => Err(format!("Invalid StopType value: {}", value)),
        }
    }
}

impl TryFrom<i32> for StopType {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        if value < 0 || value > 255 {
            return Err(format!("Invalid StopType value: {}", value));
        }
        Self::try_from(value as u8)
    }
}

impl fmt::Display for StopType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", *self as u8)
    }
}
