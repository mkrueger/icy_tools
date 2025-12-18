use std::any::TypeId;
use std::collections::HashMap;

use icy_engine_edit::tools::{Tool, ToolPair};

use super::tools::{self, ToolHandler, ToolId};
use crate::SharedFontLibrary;

/// Standard tool slots for the ANSI editor (all tools including Tag)
pub const ANSI_TOOL_SLOTS: &[ToolPair] = &[
    ToolPair::single(Tool::Click),
    ToolPair::single(Tool::Select),
    ToolPair::single(Tool::Pencil),
    ToolPair::single(Tool::Line),
    ToolPair::new(Tool::RectangleOutline, Tool::RectangleFilled),
    ToolPair::new(Tool::EllipseOutline, Tool::EllipseFilled),
    ToolPair::single(Tool::Fill),
    ToolPair::single(Tool::Pipette),
    ToolPair::single(Tool::Font),
    ToolPair::single(Tool::Tag),
];

/// Tool slots for the CharFont editor (no Tag tool)
pub const CHARFONT_TOOL_SLOTS: &[ToolPair] = &[
    ToolPair::single(Tool::Click),
    ToolPair::single(Tool::Select),
    ToolPair::single(Tool::Pencil),
    ToolPair::single(Tool::Line),
    ToolPair::new(Tool::RectangleOutline, Tool::RectangleFilled),
    ToolPair::new(Tool::EllipseOutline, Tool::EllipseFilled),
    ToolPair::single(Tool::Fill),
    ToolPair::single(Tool::Pipette),
    ToolPair::single(Tool::Font),
];

/// Tool slots for Outline font editing (minimal - only Click and Select)
pub const OUTLINE_TOOL_SLOTS: &[ToolPair] = &[ToolPair::single(Tool::Click), ToolPair::single(Tool::Select)];

pub struct ToolRegistry {
    tools: HashMap<TypeId, Box<dyn ToolHandler>>,
    /// The tool slots configuration for this registry
    slots: Vec<ToolPair>,
    /// Whether this registry uses the outline click tool
    use_outline_click: bool,
}

impl ToolRegistry {
    /// Create a new ToolRegistry with the given tool slots
    pub fn new(slots: &[ToolPair], font_library: SharedFontLibrary) -> Self {
        Self::new_internal(slots, font_library, false)
    }

    /// Create a new ToolRegistry for outline font editing
    /// Uses OutlineClickTool instead of ClickTool
    pub fn new_for_outline(slots: &[ToolPair], font_library: SharedFontLibrary) -> Self {
        Self::new_internal(slots, font_library, true)
    }

    fn new_internal(slots: &[ToolPair], font_library: SharedFontLibrary, use_outline_click: bool) -> Self {
        let mut tools: HashMap<TypeId, Box<dyn ToolHandler>> = HashMap::new();

        // Collect which tools are needed based on the slots
        let mut needs_click = false;
        let mut needs_select = false;
        let mut needs_pencil = false;
        let mut needs_shape = false;
        let mut needs_fill = false;
        let mut needs_pipette = false;
        let mut needs_font = false;
        let mut needs_tag = false;

        for pair in slots {
            for tool in [pair.primary, pair.secondary] {
                match tool {
                    Tool::Click => needs_click = true,
                    Tool::Select => needs_select = true,
                    Tool::Pencil => needs_pencil = true,
                    Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled => needs_shape = true,
                    Tool::Fill => needs_fill = true,
                    Tool::Pipette => needs_pipette = true,
                    Tool::Font => needs_font = true,
                    Tool::Tag => needs_tag = true,
                }
            }
        }

        if needs_click {
            if use_outline_click {
                let click = Box::new(tools::OutlineClickTool::new()) as Box<dyn ToolHandler>;
                tools.insert(click.as_any().type_id(), click);
            } else {
                let click = Box::new(tools::ClickTool::new()) as Box<dyn ToolHandler>;
                tools.insert(click.as_any().type_id(), click);
            }
        }

        if needs_select {
            let select = Box::new(tools::SelectTool::new()) as Box<dyn ToolHandler>;
            tools.insert(select.as_any().type_id(), select);
        }

        if needs_pencil {
            let pencil = Box::new(tools::PencilTool::new()) as Box<dyn ToolHandler>;
            tools.insert(pencil.as_any().type_id(), pencil);
        }

        if needs_shape {
            let shape = Box::new(tools::ShapeTool::new()) as Box<dyn ToolHandler>;
            tools.insert(shape.as_any().type_id(), shape);
        }

        if needs_fill {
            let fill = Box::new(tools::FillTool::new()) as Box<dyn ToolHandler>;
            tools.insert(fill.as_any().type_id(), fill);
        }

        if needs_pipette {
            let pipette = Box::new(tools::PipetteTool::new()) as Box<dyn ToolHandler>;
            tools.insert(pipette.as_any().type_id(), pipette);
        }

        if needs_font {
            let font = Box::new(tools::FontTool::new(font_library)) as Box<dyn ToolHandler>;
            tools.insert(font.as_any().type_id(), font);
        }

        if needs_tag {
            let tag = Box::new(tools::TagTool::new()) as Box<dyn ToolHandler>;
            tools.insert(tag.as_any().type_id(), tag);
        }

        Self {
            tools,
            slots: slots.to_vec(),
            use_outline_click,
        }
    }

    /// Get the tool slots for this registry
    pub fn slots(&self) -> &[ToolPair] {
        &self.slots
    }

    /// Get the number of tool slots
    pub fn num_slots(&self) -> usize {
        self.slots.len()
    }

    /// Check if this registry uses the outline click tool
    pub fn uses_outline_click(&self) -> bool {
        self.use_outline_click
    }

    /// Get the display tool for a slot given the current tool
    pub fn get_slot_display_tool(&self, slot: usize, current_tool: Tool) -> Tool {
        if slot >= self.slots.len() {
            return Tool::Click;
        }
        let pair = &self.slots[slot];
        if pair.contains(current_tool) { current_tool } else { pair.primary }
    }

    /// Click on a tool slot - returns the new tool to use
    pub fn click_tool_slot(&self, slot: usize, current_tool: Tool) -> Tool {
        if slot >= self.slots.len() {
            return current_tool;
        }
        let pair = &self.slots[slot];
        if pair.contains(current_tool) {
            pair.toggle(current_tool)
        } else {
            pair.primary
        }
    }

    /// Find which slot contains this tool
    pub fn tool_to_slot(&self, tool: Tool) -> Option<usize> {
        for (slot, pair) in self.slots.iter().enumerate() {
            if pair.contains(tool) {
                return Some(slot);
            }
        }
        None
    }

    pub fn put_back(&mut self, tool: Box<dyn ToolHandler>) {
        self.tools.insert(tool.as_any().type_id(), tool);
    }

    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        let boxed = self.tools.get_mut(&TypeId::of::<T>())?;
        boxed.as_any_mut().downcast_mut::<T>()
    }

    pub fn get_ref<T: 'static>(&self) -> Option<&T> {
        let boxed = self.tools.get(&TypeId::of::<T>())?;
        boxed.as_any().downcast_ref::<T>()
    }

    pub fn with_mut<T: 'static, R>(&mut self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        let t = self.get_mut::<T>()?;
        Some(f(t))
    }

    #[allow(dead_code)]
    pub fn with_ref<T: 'static, R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        let t = self.get_ref::<T>()?;
        Some(f(t))
    }

    pub fn take_for(&mut self, id: ToolId) -> Box<dyn ToolHandler> {
        let type_id = match id {
            ToolId::Tool(Tool::Click) => {
                if self.use_outline_click {
                    TypeId::of::<tools::OutlineClickTool>()
                } else {
                    TypeId::of::<tools::ClickTool>()
                }
            }
            ToolId::Tool(Tool::Select) => TypeId::of::<tools::SelectTool>(),
            ToolId::Tool(Tool::Pencil) => TypeId::of::<tools::PencilTool>(),
            ToolId::Tool(Tool::Pipette) => TypeId::of::<tools::PipetteTool>(),
            ToolId::Tool(Tool::Fill) => TypeId::of::<tools::FillTool>(),
            ToolId::Tool(Tool::Font) => TypeId::of::<tools::FontTool>(),
            ToolId::Tool(Tool::Tag) => TypeId::of::<tools::TagTool>(),
            ToolId::Tool(Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled) => {
                TypeId::of::<tools::ShapeTool>()
            }
            ToolId::Paste => {
                log::warn!("ToolRegistry: requested Paste; returning Click");
                if self.use_outline_click {
                    TypeId::of::<tools::OutlineClickTool>()
                } else {
                    TypeId::of::<tools::ClickTool>()
                }
            }
        };

        let mut tool = self
            .tools
            .remove(&type_id)
            .unwrap_or_else(|| panic!("ToolRegistry: missing tool for {type_id:?}"));

        // Configure shape variant if needed.
        if let ToolId::Tool(Tool::Line | Tool::RectangleOutline | Tool::RectangleFilled | Tool::EllipseOutline | Tool::EllipseFilled) = id {
            if let Some(shape) = tool.as_any_mut().downcast_mut::<tools::ShapeTool>() {
                if let ToolId::Tool(t) = id {
                    shape.set_tool(t);
                }
            }
        }

        tool
    }
}
