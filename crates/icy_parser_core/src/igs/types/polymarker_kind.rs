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

impl PolymarkerKind {
    pub fn points(&self) -> &'static [i32] {
        // The first two values are control values (line count, points per line)
        // The rest are coordinate pairs that need to be scaled
        match self {
            PolymarkerKind::Point => &[1, 2, 0, 0, 0, 0],
            PolymarkerKind::Plus => &[2, 2, 0, -3, 0, 3, 2, -4, 0, 4, 0],
            PolymarkerKind::Star => &[3, 2, 0, -3, 0, 3, 2, 3, 2, -3, -2, 2, 3, -2, -3, 2],
            PolymarkerKind::Square => &[1, 5, -4, -3, 4, -3, 4, 3, -4, 3, -4, -3],
            PolymarkerKind::DiagonalCross => &[2, 2, -4, -3, 4, 3, 2, -4, 3, 4, -3],
            PolymarkerKind::Diamond => &[1, 5, -4, 0, 0, -3, 4, 0, 0, 3, -4, 0],
        }
    }
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
