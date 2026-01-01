//! ANSI Editor Tool Panel wrapper
//!
//! Wraps the shared GenericToolPanel for use with the ANSI editor's Tool enum.
//! Provides the same interface as the old tool_panel_gpu.rs but uses the shared
//! GPU rendering backend.
//!
//! The ToolPanel now takes a reference to the ToolRegistry and uses it to
//! determine which tool slots to display and how to handle clicks.

use icy_ui::{Color, Element};
use icy_engine_edit::tools::Tool;

use crate::ui::editor::ansi::tool_registry::ToolRegistry;
use crate::ui::tool_panel::{GenericToolPanel, ToolPanelMessage as SharedToolPanelMessage};

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
    /// Tool registry reference (used to get slot configuration)
    pub registry: ToolRegistry,
}

impl ToolPanel {
    /// Create a tool panel with the given tool registry
    pub fn new(registry: ToolRegistry) -> Self {
        let num_slots = registry.num_slots();
        let mut inner = GenericToolPanel::new_with_slots(num_slots);

        // Initialize each slot with its default tool's atlas index
        {
            for slot in 0..num_slots {
                let display_tool = registry.get_slot_display_tool(slot, Tool::Click);
                inner.set_slot_display(slot, tool_to_index(display_tool));
            }
        }

        Self {
            current_tool: Tool::Click,
            inner,
            registry,
        }
    }

    /// Set the current tool
    pub fn set_tool(&mut self, tool: Tool) {
        self.current_tool = tool;
        self.update_button_states();
    }

    /// Update animation state
    pub fn tick(&mut self, delta: f32) {
        self.inner.tick(delta);
    }

    /// Update button states based on current tool
    fn update_button_states(&mut self) {
        let num_slots = self.registry.num_slots();

        for slot in 0..num_slots {
            let display_tool = self.registry.get_slot_display_tool(slot, self.current_tool);
            self.inner.set_slot_display(slot, tool_to_index(display_tool));
        }

        // Update selected slot
        if let Some(slot) = self.registry.tool_to_slot(self.current_tool) {
            self.inner.set_selected_slot(slot);
        }
    }

    /// Update the tool panel state (only handles Tick, ClickSlot is handled by the editor)
    pub fn update(&mut self, message: ToolPanelMessage) -> icy_ui::Task<ToolPanelMessage> {
        match message {
            ToolPanelMessage::ClickSlot(_) => {
                // Click handling is done by the editor, not here
            }
            ToolPanelMessage::Tick(delta) => {
                self.tick(delta);
            }
        }
        icy_ui::Task::none()
    }

    /// Render the tool panel with the given available width and background color
    pub fn view_with_config(&self, available_width: f32, bg_color: Color, icon_color: Color) -> Element<'_, ToolPanelMessage> {
        self.inner.view(available_width, bg_color, icon_color).map(|msg| match msg {
            SharedToolPanelMessage::ClickSlot(slot) => ToolPanelMessage::ClickSlot(slot),
            SharedToolPanelMessage::Tick(delta) => ToolPanelMessage::Tick(delta),
        })
    }
}
