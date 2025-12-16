//! ANSI Editor Tool Panel wrapper
//!
//! Wraps the shared GenericToolPanel for use with the ANSI editor's Tool enum.
//! Provides the same interface as the old tool_panel_gpu.rs but uses the shared
//! GPU rendering backend.

use iced::{Color, Element};
use icy_engine_edit::tools::{TOOL_SLOTS, Tool, click_tool_slot, get_slot_display_tool};

use crate::ui::tool_panel::{GenericToolPanel, ToolPanelMessage as SharedToolPanelMessage};

/// Maximum number of tool buttons
const MAX_BUTTONS: usize = 10;

/// Messages from the tool panel
#[derive(Clone, Debug)]
pub enum ToolPanelMessage {
    /// Clicked on a tool slot
    ClickSlot(usize),
    /// Animation tick
    Tick(f32),
}

/// Tool icons in atlas order (must match STANDARD_ICONS in tool_panel/mod.rs)
const TOOL_ICON_ORDER: &[Tool] = &[
    Tool::Click,            // 0
    Tool::Select,           // 1
    Tool::Pencil,           // 2
    Tool::Line,             // 3
    Tool::RectangleOutline, // 4
    Tool::RectangleFilled,  // 5
    Tool::EllipseOutline,   // 6
    Tool::EllipseFilled,    // 7
    Tool::Fill,             // 8
    Tool::Pipette,          // 9
    Tool::Font,             // 10
    Tool::Tag,              // 11
];

/// Map tool to atlas index
fn tool_to_index(tool: Tool) -> usize {
    TOOL_ICON_ORDER.iter().position(|&t| t == tool).unwrap_or(0)
}

/// Tool panel state
pub struct ToolPanel {
    /// Currently selected tool
    current_tool: Tool,
    /// Generic tool panel for GPU rendering
    inner: GenericToolPanel,
}

impl Default for ToolPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolPanel {
    pub fn new() -> Self {
        let num_slots = TOOL_SLOTS.len().min(MAX_BUTTONS);
        let mut inner = GenericToolPanel::new_with_slots(num_slots);

        // Initialize each slot with its default tool's atlas index
        for slot in 0..num_slots {
            let display_tool = get_slot_display_tool(slot, Tool::Click);
            inner.set_slot_display(slot, tool_to_index(display_tool));
        }

        Self {
            current_tool: Tool::Click,
            inner,
        }
    }

    /// Get the current tool
    pub fn current_tool(&self) -> Tool {
        self.current_tool
    }

    /// Set the current tool
    pub fn set_tool(&mut self, tool: Tool) {
        self.current_tool = tool;
        self.update_button_states();
    }

    /// Check if any animation is running
    pub fn needs_animation(&self) -> bool {
        self.inner.needs_animation()
    }

    /// Update animation state
    pub fn tick(&mut self, delta: f32) {
        self.inner.tick(delta);
    }

    /// Update button states based on current tool
    fn update_button_states(&mut self) {
        let num_slots = TOOL_SLOTS.len().min(MAX_BUTTONS);
        for slot in 0..num_slots {
            let display_tool = get_slot_display_tool(slot, self.current_tool);
            self.inner.set_slot_display(slot, tool_to_index(display_tool));
        }

        // Update selected slot
        if let Some(slot) = self.tool_to_slot(self.current_tool) {
            self.inner.set_selected_slot(slot);
        }
    }

    /// Find which slot contains this tool
    fn tool_to_slot(&self, tool: Tool) -> Option<usize> {
        for (slot, tools) in TOOL_SLOTS.iter().enumerate().take(MAX_BUTTONS) {
            if tools.contains(tool) {
                return Some(slot);
            }
        }
        None
    }

    /// Update the tool panel state
    pub fn update(&mut self, message: ToolPanelMessage) -> iced::Task<ToolPanelMessage> {
        match message {
            ToolPanelMessage::ClickSlot(slot) => {
                self.current_tool = click_tool_slot(slot, self.current_tool);
                self.update_button_states();
            }
            ToolPanelMessage::Tick(delta) => {
                self.tick(delta);
            }
        }
        iced::Task::none()
    }

    /// Render the tool panel with the given available width and background color
    pub fn view_with_config(&self, available_width: f32, bg_color: Color) -> Element<'_, ToolPanelMessage> {
        self.inner.view(available_width, bg_color).map(|msg| match msg {
            SharedToolPanelMessage::ClickSlot(slot) => ToolPanelMessage::ClickSlot(slot),
            SharedToolPanelMessage::Tick(delta) => ToolPanelMessage::Tick(delta),
        })
    }
}
