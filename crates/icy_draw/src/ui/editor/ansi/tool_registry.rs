use std::any::TypeId;
use std::collections::HashMap;

use icy_engine_edit::tools::Tool;

use super::tools::{self, ToolHandler, ToolId};
use crate::SharedFontLibrary;

pub struct ToolRegistry {
    tools: HashMap<TypeId, Box<dyn ToolHandler>>,
}

impl ToolRegistry {
    pub fn new(font_library: SharedFontLibrary) -> Self {
        let mut tools: HashMap<TypeId, Box<dyn ToolHandler>> = HashMap::new();

        let click = Box::new(tools::ClickTool::new()) as Box<dyn ToolHandler>;
        tools.insert(click.as_any().type_id(), click);

        let select = Box::new(tools::SelectTool::new()) as Box<dyn ToolHandler>;
        tools.insert(select.as_any().type_id(), select);

        let pencil = Box::new(tools::PencilTool::new()) as Box<dyn ToolHandler>;
        tools.insert(pencil.as_any().type_id(), pencil);

        let shape = Box::new(tools::ShapeTool::new()) as Box<dyn ToolHandler>;
        tools.insert(shape.as_any().type_id(), shape);

        let fill = Box::new(tools::FillTool::new()) as Box<dyn ToolHandler>;
        tools.insert(fill.as_any().type_id(), fill);

        let pipette = Box::new(tools::PipetteTool::new()) as Box<dyn ToolHandler>;
        tools.insert(pipette.as_any().type_id(), pipette);

        let font = Box::new(tools::FontTool::new(font_library)) as Box<dyn ToolHandler>;
        tools.insert(font.as_any().type_id(), font);

        let tag = Box::new(tools::TagTool::new()) as Box<dyn ToolHandler>;
        tools.insert(tag.as_any().type_id(), tag);

        Self { tools }
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
            ToolId::Tool(Tool::Click) => TypeId::of::<tools::ClickTool>(),
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
                TypeId::of::<tools::ClickTool>()
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
