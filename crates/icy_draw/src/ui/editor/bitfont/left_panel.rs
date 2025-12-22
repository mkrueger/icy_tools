//! Left panel for BitFont Editor
//!
//! Contains the tool panel (grid for BitFont tools)
//! Uses the shared GPU-accelerated tool panel component

use iced::{Color, Element};

use super::{BITFONT_TOOL_SLOTS, BitFontTool};
use crate::ui::tool_panel::{GenericToolPanel, ToolPanelMessage};

// ═══════════════════════════════════════════════════════════════════════════
// BitFont Tool Panel (wrapper around generic tool panel)
// ═══════════════════════════════════════════════════════════════════════════

/// Messages from the BitFont tool panel
#[derive(Clone, Debug)]
pub enum BitFontToolPanelMessage {
    /// Clicked on a tool slot
    ClickSlot(usize),
    /// Animation tick
    Tick(f32),
}

/// BitFont tool panel state (wraps the generic tool panel)
pub struct BitFontToolPanel {
    /// Currently selected tool
    pub current_tool: BitFontTool,
    /// Generic tool panel for rendering
    inner: GenericToolPanel,
}

impl Default for BitFontToolPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl BitFontToolPanel {
    pub fn new() -> Self {
        // Create panel with 5 slots using standard atlas
        let mut inner = GenericToolPanel::new_with_slots(5);
        // Set initial display indices (map to standard atlas positions)
        // Standard atlas: 0=Click, 1=Select, 3=Line, 6=RectOutline, 10=Fill
        inner.set_slot_display(0, 0); // Click -> atlas index 0
        inner.set_slot_display(1, 1); // Select -> atlas index 1
        inner.set_slot_display(2, 3); // Line -> atlas index 3
        inner.set_slot_display(3, 6); // RectangleOutline -> atlas index 6
        inner.set_slot_display(4, 10); // Fill -> atlas index 10

        Self {
            current_tool: BitFontTool::Click,
            inner,
        }
    }

    /// Set the current tool
    pub fn set_tool(&mut self, tool: BitFontTool) {
        self.current_tool = tool;
        // Update selected slot based on tool
        if let Some(slot) = self.tool_to_slot(tool) {
            self.inner.set_selected_slot(slot);
            // Update display for toggle tools (rectangle outline/filled)
            self.update_slot_displays();
        }
    }

    /// Update animation state
    pub fn tick(&mut self, delta: f32) {
        self.inner.tick(delta);
    }

    /// Update slot displays for toggle tools
    fn update_slot_displays(&mut self) {
        for (slot, (primary, secondary)) in BITFONT_TOOL_SLOTS.iter().enumerate() {
            let display_tool = if self.current_tool == *primary {
                *primary
            } else if secondary.map_or(false, |s| self.current_tool == s) {
                secondary.unwrap()
            } else {
                *primary
            };
            // Map tool to icon index
            if let Some(icon_idx) = self.tool_to_icon_index(display_tool) {
                self.inner.set_slot_display(slot, icon_idx);
            }
        }
    }

    /// Map tool to slot index
    fn tool_to_slot(&self, tool: BitFontTool) -> Option<usize> {
        for (slot, (primary, secondary)) in BITFONT_TOOL_SLOTS.iter().enumerate() {
            if *primary == tool || secondary.map_or(false, |s| s == tool) {
                return Some(slot);
            }
        }
        None
    }

    /// Map tool to icon index in standard atlas
    fn tool_to_icon_index(&self, tool: BitFontTool) -> Option<usize> {
        // Standard atlas icon order:
        // 0: Click, 1: Select, 2: Pencil, 3: Line, 4: RectangleOutline, 5: RectangleFilled,
        // 6: EllipseOutline, 7: EllipseFilled, 8: Fill, 9: Pipette, 10: Font, 11: Tag
        match tool {
            BitFontTool::Click => Some(0),
            BitFontTool::Select => Some(1),
            BitFontTool::Line => Some(3),
            BitFontTool::RectangleOutline => Some(4),
            BitFontTool::RectangleFilled => Some(5),
            BitFontTool::Fill => Some(8),
        }
    }

    /// Handle a slot click - returns the new tool
    pub fn click_slot(&mut self, slot: usize) -> BitFontTool {
        if slot < BITFONT_TOOL_SLOTS.len() {
            let (primary, secondary) = &BITFONT_TOOL_SLOTS[slot];

            // If already selected and has secondary, toggle
            if self.current_tool == *primary {
                if let Some(sec) = secondary {
                    self.current_tool = *sec;
                }
            } else if secondary.map_or(false, |s| self.current_tool == s) {
                self.current_tool = *primary;
            } else {
                self.current_tool = *primary;
            }

            self.inner.set_selected_slot(slot);
            self.update_slot_displays();
        }
        self.current_tool
    }

    /// Render the tool panel with the given available width and background color
    pub fn view_with_config(&self, available_width: f32, bg_color: Color, icon_color: Color) -> Element<'_, BitFontToolPanelMessage> {
        self.inner.view(available_width, bg_color, icon_color).map(|msg| match msg {
            ToolPanelMessage::ClickSlot(slot) => BitFontToolPanelMessage::ClickSlot(slot),
            ToolPanelMessage::Tick(delta) => BitFontToolPanelMessage::Tick(delta),
        })
    }
}
