//! Tool definitions for ANSI art editing
//!
//! This module provides an enum-based tool system. Tools are organized in
//! toggle pairs - clicking on an already-selected tool switches to its partner.

/// Available editing tools
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tool {
    // === Toggle Pair 1: Click / Select ===
    /// Keyboard input mode - type characters, navigate with cursor
    #[default]
    Click,
    /// Rectangle selection mode
    Select,

    // === Toggle Pair 2: Pencil ===
    /// Draw single characters (freehand)
    Pencil,

    // === Single Tool: Line ===
    /// Draw straight lines
    Line,

    // === Toggle Pair 4: Rectangle Outline / Filled ===
    /// Draw rectangle outline
    RectangleOutline,
    /// Draw filled rectangle
    RectangleFilled,

    // === Toggle Pair 5: Ellipse Outline / Filled ===
    /// Draw ellipse outline
    EllipseOutline,
    /// Draw filled ellipse
    EllipseFilled,

    // === Single Tools (no toggle partner) ===
    /// Pick color/character from canvas
    Pipette,
    /// Flood fill area
    Fill,
    /// TDF/Figlet font rendering
    Font,
    /// Tag tool for annotations
    Tag,
}

/// A toggle pair of tools sharing one icon slot
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolPair {
    pub primary: Tool,
    pub secondary: Tool,
}

impl ToolPair {
    pub const fn new(primary: Tool, secondary: Tool) -> Self {
        Self { primary, secondary }
    }

    pub const fn single(tool: Tool) -> Self {
        Self {
            primary: tool,
            secondary: tool,
        }
    }

    /// Check if this pair contains the given tool
    pub fn contains(&self, tool: Tool) -> bool {
        self.primary == tool || self.secondary == tool
    }

    /// Get the other tool in the pair (toggle)
    pub fn toggle(&self, current: Tool) -> Tool {
        if current == self.primary { self.secondary } else { self.primary }
    }

    /// Check if this is a single tool (no toggle partner)
    pub fn is_single(&self) -> bool {
        self.primary == self.secondary
    }
}

/// The 8 tool slots in the toolbar (each can be a pair or single)
pub const TOOL_SLOTS: [ToolPair; 8] = [
    ToolPair::new(Tool::Click, Tool::Select),
    ToolPair::single(Tool::Pencil),
    ToolPair::single(Tool::Line),
    ToolPair::new(Tool::RectangleOutline, Tool::RectangleFilled),
    ToolPair::new(Tool::EllipseOutline, Tool::EllipseFilled),
    ToolPair::single(Tool::Fill),
    ToolPair::single(Tool::Pipette),
    ToolPair::single(Tool::Font),
];

impl Tool {
    /// Get the icon filename (without extension) for this tool
    pub fn icon(&self) -> &'static str {
        match self {
            Tool::Click => "cursor",
            Tool::Select => "select",
            Tool::Pencil => "pencil",
            Tool::Line => "line",
            Tool::RectangleOutline => "rectangle_outline",
            Tool::RectangleFilled => "rectangle_filled",
            Tool::EllipseOutline => "ellipse_outline",
            Tool::EllipseFilled => "ellipse_filled",
            Tool::Fill => "fill",
            Tool::Pipette => "dropper",
            Tool::Font => "font",
            Tool::Tag => "tag",
        }
    }

    /// Get the display name
    pub fn name(&self) -> &'static str {
        match self {
            Tool::Click => "Click",
            Tool::Select => "Select",
            Tool::Pencil => "Pencil",
            Tool::Line => "Line",
            Tool::RectangleOutline => "Rectangle",
            Tool::RectangleFilled => "Filled Rectangle",
            Tool::EllipseOutline => "Ellipse",
            Tool::EllipseFilled => "Filled Ellipse",
            Tool::Fill => "Fill",
            Tool::Pipette => "Pipette",
            Tool::Font => "Font",
            Tool::Tag => "Tag",
        }
    }

    /// Get the tooltip text
    pub fn tooltip(&self) -> &'static str {
        match self {
            Tool::Click => "Keyboard input and cursor navigation (click again for Select)",
            Tool::Select => "Rectangle selection (click again for Click)",
            Tool::Pencil => "Draw single characters",
            Tool::Line => "Draw straight lines",
            Tool::RectangleOutline => "Draw rectangle outline (click again for Filled)",
            Tool::RectangleFilled => "Draw filled rectangle (click again for Outline)",
            Tool::EllipseOutline => "Draw ellipse outline (click again for Filled)",
            Tool::EllipseFilled => "Draw filled ellipse (click again for Outline)",
            Tool::Fill => "Flood fill area",
            Tool::Pipette => "Pick color/character",
            Tool::Font => "TDF/Figlet font rendering",
            Tool::Tag => "Add annotation tags",
        }
    }

    /// Get the keyboard shortcut
    pub fn shortcut(&self) -> Option<char> {
        match self {
            Tool::Click => Some('c'),
            Tool::Select => Some('s'),
            Tool::Pencil => Some('p'),
            Tool::Line => Some('l'),
            Tool::RectangleOutline | Tool::RectangleFilled => Some('r'),
            Tool::EllipseOutline | Tool::EllipseFilled => Some('o'),
            Tool::Fill => Some('f'),
            Tool::Pipette => Some('i'),
            Tool::Font => Some('t'),
            Tool::Tag => Some('g'),
        }
    }

    /// Find which slot this tool belongs to
    pub fn slot_index(&self) -> usize {
        TOOL_SLOTS.iter().position(|pair| pair.contains(*self)).unwrap_or(0)
    }

    /// Check if this tool uses the caret for input
    pub fn uses_caret(&self) -> bool {
        matches!(self, Tool::Click | Tool::Font)
    }

    /// Check if this tool supports selection
    pub fn uses_selection(&self) -> bool {
        matches!(self, Tool::Select | Tool::Click)
    }

    /// Check if this tool draws shapes (needs overlay preview)
    pub fn is_shape_tool(&self) -> bool {
        matches!(
            self,
            Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled
        )
    }

    /// Check if this is a filled variant
    pub fn is_filled(&self) -> bool {
        matches!(self, Tool::RectangleFilled | Tool::EllipseFilled)
    }

    /// Check if this tool needs drag tracking
    pub fn needs_drag(&self) -> bool {
        matches!(
            self,
            Tool::Select | Tool::Pencil | Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled
        )
    }
}

/// Handle clicking on a tool slot - returns the new tool
pub fn click_tool_slot(slot: usize, current_tool: Tool) -> Tool {
    if slot >= TOOL_SLOTS.len() {
        return current_tool;
    }

    let pair = &TOOL_SLOTS[slot];
    if pair.contains(current_tool) {
        // Already selected - toggle to partner
        pair.toggle(current_tool)
    } else {
        // Not selected - switch to primary
        pair.primary
    }
}

/// Get the tool that should be displayed for a slot
pub fn get_slot_display_tool(slot: usize, current_tool: Tool) -> Tool {
    if slot >= TOOL_SLOTS.len() {
        return Tool::Click;
    }

    let pair = &TOOL_SLOTS[slot];
    if pair.contains(current_tool) { current_tool } else { pair.primary }
}

/// Tool event returned from tool operations
#[derive(Clone, Debug)]
pub enum ToolEvent {
    /// No action needed
    None,
    /// Request a redraw (e.g., after cursor move)
    Redraw,
    /// Commit an operation to undo stack
    Commit(String),
    /// Status message to display
    Status(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toggle_click_select() {
        assert_eq!(click_tool_slot(0, Tool::Click), Tool::Select);
        assert_eq!(click_tool_slot(0, Tool::Select), Tool::Click);
    }

    #[test]
    fn test_toggle_rectangle() {
        assert_eq!(click_tool_slot(3, Tool::RectangleOutline), Tool::RectangleFilled);
        assert_eq!(click_tool_slot(3, Tool::RectangleFilled), Tool::RectangleOutline);
    }

    #[test]
    fn test_single_tool_no_toggle() {
        // Fill has no partner, clicking again returns Fill
        assert_eq!(click_tool_slot(5, Tool::Fill), Tool::Fill);
    }

    #[test]
    fn test_switch_to_new_slot() {
        // From Click, click on Pencil slot -> Pencil (primary)
        assert_eq!(click_tool_slot(1, Tool::Click), Tool::Pencil);
    }

    #[test]
    fn test_slot_index() {
        assert_eq!(Tool::Click.slot_index(), 0);
        assert_eq!(Tool::Select.slot_index(), 0);
        assert_eq!(Tool::Pencil.slot_index(), 1);
        assert_eq!(Tool::Line.slot_index(), 2);
        assert_eq!(Tool::Fill.slot_index(), 5);
    }

    #[test]
    fn test_all_tools_have_icons() {
        for slot in &TOOL_SLOTS {
            assert!(!slot.primary.icon().is_empty());
            assert!(!slot.secondary.icon().is_empty());
        }
    }

    #[test]
    fn test_display_tool_for_slot() {
        // When Click is selected, slot 0 shows Click
        assert_eq!(get_slot_display_tool(0, Tool::Click), Tool::Click);
        // When Select is selected, slot 0 shows Select
        assert_eq!(get_slot_display_tool(0, Tool::Select), Tool::Select);
        // When Pencil is selected, slot 0 shows primary (Click)
        assert_eq!(get_slot_display_tool(0, Tool::Pencil), Tool::Click);
    }
}
