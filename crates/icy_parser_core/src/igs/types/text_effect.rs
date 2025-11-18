use bitflags::bitflags;

bitflags! {
    /// Text effect flags for VDI text rendering
    ///
    /// These flags can be combined using bitwise OR to apply multiple effects.
    /// For example: `THICKENED | UNDERLINED` = 9 (1 | 8)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TextEffects: u8 {
        const NORMAL = 0;
        const THICKENED = 1;
        const GHOSTED = 2;
        const SKEWED = 4;
        const UNDERLINED = 8;
        const OUTLINED = 16;
    }
}

/// Text effects (bit flags, can be combined)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEffect {
    /// Normal text (no effects)
    Normal = 0,
    /// Thickened (bold)
    Thickened = 1,
    /// Ghosted
    Ghosted = 2,
    /// Skewed (italic)
    Skewed = 4,
    /// Underlined
    Underlined = 8,
    /// Outlined
    Outlined = 16,
}

/// Text rotation angle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRotation {
    /// 0 degrees (normal)
    Degrees0 = 0,
    /// 90 degrees
    Degrees90 = 1,
    /// 180 degrees
    Degrees180 = 2,
    /// 270 degrees
    Degrees270 = 3,
}

impl Default for TextRotation {
    fn default() -> Self {
        Self::Degrees0
    }
}

impl TryFrom<i32> for TextRotation {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Degrees0),
            1 => Ok(Self::Degrees90),
            2 => Ok(Self::Degrees180),
            3 => Ok(Self::Degrees270),
            _ => Err(format!("Invalid TextRotation value: {}", value)),
        }
    }
}
