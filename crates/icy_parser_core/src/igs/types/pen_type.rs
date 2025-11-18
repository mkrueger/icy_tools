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

impl Default for PenType {
    fn default() -> Self {
        Self::Polymarker
    }
}

impl TryFrom<i32> for PenType {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Polymarker),
            1 => Ok(Self::Line),
            2 => Ok(Self::Fill),
            3 => Ok(Self::Text),
            _ => Err(format!("Invalid PenType value: {}", value)),
        }
    }
}
