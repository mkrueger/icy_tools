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
    pub fn mask(&self, user_mask: u16) -> u16 {
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

/// Arrow end style for line endpoints
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowEnd {
    /// Square end
    Square = 0,
    /// Arrow end
    Arrow = 1,
    /// Rounded end
    Rounded = 2,
}

impl Default for ArrowEnd {
    fn default() -> Self {
        Self::Square
    }
}

/// Line and marker style for LineStyle command (T command)
///
/// The T command has complex parsing based on C code:
/// - Parameter 1: 1=polymarker, 2=line
/// - Parameter 2: type (1-6 for polymarker, 1-7 for line)
/// - Parameter 3: size/thickness/endpoints (complex interpretation)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineMarkerStyle {
    /// Polymarker with size (height = size * 11)
    /// Size range: 1-8
    PolyMarkerSize(PolymarkerKind, u8),

    /// Line with thickness
    /// Thickness range: 1-41 (only for solid lines, others forced to 1)
    LineThickness(LineKind, u8),

    /// Line with endpoint decorations
    /// Left and right arrow ends (square, arrow, or rounded)
    LineEndpoints(LineKind, ArrowEnd, ArrowEnd),
}
