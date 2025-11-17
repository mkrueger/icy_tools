/// Pen type for ColorSet command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PenType {
    /// Polymarker color (for PolymarkerPlot)
    Polymarker = 0,
    /// Line color
    Line = 1,
    /// Fill color
    Fill = 2,
    /// Text color (for WriteText)
    Text = 3,
}

impl From<i32> for PenType {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Polymarker,
            1 => Self::Line,
            2 => Self::Fill,
            3 => Self::Text,
            _ => Self::Polymarker,
        }
    }
}
