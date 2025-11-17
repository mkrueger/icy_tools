/// Drawing mode for pixel operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawingMode {
    /// Replace (normal drawing)
    Replace = 1,
    /// Transparent (skip background pixels)
    Transparent = 2,
    /// XOR (reversible drawing)
    Xor = 3,
    /// Reverse transparent
    ReverseTransparent = 4,
}

impl From<i32> for DrawingMode {
    fn from(value: i32) -> Self {
        match value {
            1 => Self::Replace,
            2 => Self::Transparent,
            3 => Self::Xor,
            4 => Self::ReverseTransparent,
            _ => Self::Replace,
        }
    }
}
