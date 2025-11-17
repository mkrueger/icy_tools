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

impl From<i32> for TextRotation {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Degrees0,
            1 => Self::Degrees90,
            2 => Self::Degrees180,
            3 => Self::Degrees270,
            _ => Self::Degrees0,
        }
    }
}
