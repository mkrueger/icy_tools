use super::PolymarkerKind;

/// Line style type for LineStyle command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Solid line
    Solid = 1,
    /// Long dash line
    LongDash = 2,
    /// Dotted line
    Dotted = 3,
    /// Dash-dot line
    DashDot = 4,
    /// Dashed line
    Dashed = 5,
    /// Dash-dot-dot line
    DashDotDot = 6,
    /// User defined line
    UserDefined = 7,
}

impl Default for LineKind {
    fn default() -> Self {
        Self::Solid
    }
}

impl LineKind {
    /// Get the line style index used in IGS patterns
    pub fn get_mask(&self, user_mask: u16) -> u16 {
        println!("Getting mask for line kind {:?} with user mask {:04X}", self, user_mask);
        match self {
            LineKind::Solid => 0xFFFF,
            LineKind::LongDash => 0xFFF0,
            LineKind::Dotted => 0xC0C0,
            LineKind::DashDot => 0xFF18,
            LineKind::Dashed => 0xFF00,
            LineKind::DashDotDot => 0xF191,
            LineKind::UserDefined => user_mask,
        }
    }
}

impl TryFrom<i32> for LineKind {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Solid),
            2 => Ok(Self::LongDash),
            3 => Ok(Self::Dotted),
            4 => Ok(Self::DashDot),
            5 => Ok(Self::Dashed),
            6 => Ok(Self::DashDotDot),
            7 => Ok(Self::UserDefined),
            _ => Err(format!("Invalid LineKind value: {}", value)),
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
