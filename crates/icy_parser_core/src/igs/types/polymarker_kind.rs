/// Polymarker type for LineStyle command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolymarkerKind {
    /// Point marker
    Point = 1,
    /// Plus marker
    Plus = 2,
    /// Star marker
    Star = 3,
    /// Square marker
    Square = 4,
    /// Diagonal cross marker
    DiagonalCross = 5,
    /// Diamond marker
    Diamond = 6,
}

impl Default for PolymarkerKind {
    fn default() -> Self {
        Self::Point
    }
}

impl TryFrom<i32> for PolymarkerKind {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Point),
            2 => Ok(Self::Plus),
            3 => Ok(Self::Star),
            4 => Ok(Self::Square),
            5 => Ok(Self::DiagonalCross),
            6 => Ok(Self::Diamond),
            _ => Err(format!("Invalid PolymarkerKind value: {}", value)),
        }
    }
}
