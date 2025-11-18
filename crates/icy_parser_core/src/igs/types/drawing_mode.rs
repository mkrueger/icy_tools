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

impl Default for DrawingMode {
    fn default() -> Self {
        Self::Replace
    }
}

impl TryFrom<i32> for DrawingMode {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Replace),
            2 => Ok(Self::Transparent),
            3 => Ok(Self::Xor),
            4 => Ok(Self::ReverseTransparent),
            _ => Err(format!("Invalid DrawingMode value: {}", value)),
        }
    }
}
