//! Tool system for BitFont editor

/// Available tools in the BitFont editor
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BitFontTool {
    /// Click tool - draw/erase single pixels, keyboard cursor navigation
    #[default]
    Click,
    /// Selection tool - select rectangular areas
    Select,
    /// Line tool - draw straight lines
    Line,
    /// Rectangle outline tool
    RectangleOutline,
    /// Filled rectangle tool
    RectangleFilled,
    /// Flood fill tool
    Fill,
}

impl BitFontTool {
    /// Get the display name for this tool
    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            BitFontTool::Click => "Click",
            BitFontTool::Select => "Select",
            BitFontTool::Line => "Line",
            BitFontTool::RectangleOutline => "Rectangle",
            BitFontTool::RectangleFilled => "Filled Rect",
            BitFontTool::Fill => "Fill",
        }
    }

    /// Get the icon character for this tool
    #[allow(dead_code)]
    pub fn icon(&self) -> &'static str {
        match self {
            BitFontTool::Click => "✎",
            BitFontTool::Select => "▢",
            BitFontTool::Line => "╱",
            BitFontTool::RectangleOutline => "□",
            BitFontTool::RectangleFilled => "■",
            BitFontTool::Fill => "◧",
        }
    }

    /// Get keyboard shortcut
    #[allow(dead_code)]
    pub fn shortcut(&self) -> char {
        match self {
            BitFontTool::Click => 'C',
            BitFontTool::Select => 'S',
            BitFontTool::Line => 'L',
            BitFontTool::RectangleOutline | BitFontTool::RectangleFilled => 'R',
            BitFontTool::Fill => 'F',
        }
    }
}

/// Tool slots for the toolbar (some tools toggle between variants)
pub const BITFONT_TOOL_SLOTS: &[(BitFontTool, Option<BitFontTool>)] = &[
    (BitFontTool::Click, None),
    (BitFontTool::Select, None),
    (BitFontTool::Line, None),
    (BitFontTool::RectangleOutline, Some(BitFontTool::RectangleFilled)),
    (BitFontTool::Fill, None),
];
