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

impl From<i32> for PolymarkerKind {
    fn from(value: i32) -> Self {
        match value {
            1 => Self::Point,
            2 => Self::Plus,
            3 => Self::Star,
            4 => Self::Square,
            5 => Self::DiagonalCross,
            6 => Self::Diamond,
            _ => Self::Point,
        }
    }
}
