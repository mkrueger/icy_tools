use super::PolymarkerKind;

/// Line style type for LineStyle command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Solid line
    Solid = 0,
    /// Long dash line
    LongDash = 1,
    /// Dotted line
    Dotted = 2,
    /// Dash-dot line
    DashDot = 3,
    /// Dashed line
    Dashed = 4,
    /// Dash-dot-dot line
    DashDotDot = 5,
    /// User defined line
    UserDefined = 6,
}

impl From<i32> for LineKind {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Solid,
            1 => Self::LongDash,
            2 => Self::Dotted,
            3 => Self::DashDot,
            4 => Self::Dashed,
            5 => Self::DashDotDot,
            6 => Self::UserDefined,
            _ => Self::Solid,
        }
    }
}

/// Line style kind for LineStyle command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyleKind {
    /// Polymarker configuration
    Polymarker(PolymarkerKind),
    /// Line configuration
    Line(LineKind),
}
