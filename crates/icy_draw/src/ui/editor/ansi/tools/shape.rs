//! Shape Tools (Rectangle, Ellipse - Outline and Filled)
//!
//! Draws geometric shapes between two drag points.
//! Supports half-block mode for higher resolution.

use iced::Element;
use iced::widget::{column, text};
use icy_engine::{MouseButton, Position};
use icy_engine_edit::tools::Tool;

use super::{ToolContext, ToolHandler, ToolInput, ToolMessage, ToolResult};

/// Shape type to draw
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShapeType {
    #[default]
    RectangleOutline,
    RectangleFilled,
    EllipseOutline,
    EllipseFilled,
}

impl ShapeType {
    /// Get the corresponding Tool enum value
    pub fn to_tool(self) -> Tool {
        match self {
            ShapeType::RectangleOutline => Tool::RectangleOutline,
            ShapeType::RectangleFilled => Tool::RectangleFilled,
            ShapeType::EllipseOutline => Tool::EllipseOutline,
            ShapeType::EllipseFilled => Tool::EllipseFilled,
        }
    }

    /// Check if this is a filled shape
    pub fn is_filled(self) -> bool {
        matches!(self, ShapeType::RectangleFilled | ShapeType::EllipseFilled)
    }

    /// Check if this is an ellipse
    pub fn is_ellipse(self) -> bool {
        matches!(self, ShapeType::EllipseOutline | ShapeType::EllipseFilled)
    }
}

/// Shape tool state
#[derive(Clone, Debug, Default)]
pub struct ShapeTool {
    /// Type of shape to draw
    shape_type: ShapeType,
    /// Start position of the shape
    start_pos: Option<Position>,
    /// Current end position (during drag)
    current_pos: Option<Position>,
    /// Start position in half-block coordinates
    start_half_block: Option<Position>,
    /// Current position in half-block coordinates
    current_half_block: Option<Position>,
    /// Whether currently dragging
    is_dragging: bool,
    /// Mouse button used for drawing
    draw_button: MouseButton,
    /// Whether to clear/erase instead of draw (Shift modifier)
    clear_mode: bool,
}

impl ShapeTool {
    pub fn new(shape_type: ShapeType) -> Self {
        Self {
            shape_type,
            ..Default::default()
        }
    }

    pub fn rectangle_outline() -> Self {
        Self::new(ShapeType::RectangleOutline)
    }

    pub fn rectangle_filled() -> Self {
        Self::new(ShapeType::RectangleFilled)
    }

    pub fn ellipse_outline() -> Self {
        Self::new(ShapeType::EllipseOutline)
    }

    pub fn ellipse_filled() -> Self {
        Self::new(ShapeType::EllipseFilled)
    }

    /// Get the shape type
    pub fn shape_type(&self) -> ShapeType {
        self.shape_type
    }

    /// Set the shape type
    pub fn set_shape_type(&mut self, shape_type: ShapeType) {
        self.shape_type = shape_type;
    }
}

impl ToolHandler for ShapeTool {
    fn handle_event(&mut self, _ctx: &mut ToolContext, event: ToolInput) -> ToolResult {
        match event {
            ToolInput::MouseDown {
                pos,
                pos_half_block,
                button,
                modifiers,
                ..
            } => {
                self.start_pos = Some(pos);
                self.current_pos = Some(pos);
                self.start_half_block = Some(pos_half_block);
                self.current_half_block = Some(pos_half_block);
                self.is_dragging = true;
                self.draw_button = button;
                self.clear_mode = modifiers.shift;

                ToolResult::StartCapture.and(ToolResult::Redraw)
            }

            ToolInput::MouseMove {
                pos,
                pos_half_block,
                is_dragging,
                ..
            } => {
                if is_dragging && self.is_dragging {
                    self.current_pos = Some(pos);
                    self.current_half_block = Some(pos_half_block);
                    ToolResult::Redraw
                } else {
                    ToolResult::None
                }
            }

            ToolInput::MouseUp { pos, .. } => {
                if self.is_dragging {
                    self.current_pos = Some(pos);
                    self.is_dragging = false;

                    let start = self.start_pos.unwrap_or_default();
                    let end = pos;

                    // Reset state
                    self.start_pos = None;
                    self.current_pos = None;
                    self.start_half_block = None;
                    self.current_half_block = None;

                    let shape_name = match self.shape_type {
                        ShapeType::RectangleOutline => "Rectangle",
                        ShapeType::RectangleFilled => "Filled Rectangle",
                        ShapeType::EllipseOutline => "Ellipse",
                        ShapeType::EllipseFilled => "Filled Ellipse",
                    };

                    ToolResult::EndCapture.and(ToolResult::Commit(format!(
                        "{} from ({},{}) to ({},{})",
                        shape_name, start.x, start.y, end.x, end.y
                    )))
                } else {
                    ToolResult::None
                }
            }

            ToolInput::Deactivate => {
                self.start_pos = None;
                self.current_pos = None;
                self.start_half_block = None;
                self.current_half_block = None;
                self.is_dragging = false;
                ToolResult::Redraw
            }

            _ => ToolResult::None,
        }
    }

    fn view_toolbar<'a>(&'a self, _ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        column![].into()
    }

    fn view_status<'a>(&'a self, _ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        let shape_name = match self.shape_type {
            ShapeType::RectangleOutline => "Rectangle",
            ShapeType::RectangleFilled => "Filled Rect",
            ShapeType::EllipseOutline => "Ellipse",
            ShapeType::EllipseFilled => "Filled Ellipse",
        };

        let status = if let (Some(start), Some(end)) = (self.start_pos, self.current_pos) {
            let w = (end.x - start.x).abs() + 1;
            let h = (end.y - start.y).abs() + 1;
            format!("{} | ({},{}) → ({},{}) [{}x{}] | Shift=Erase", shape_name, start.x, start.y, end.x, end.y, w, h)
        } else {
            format!("{} | Click and drag to draw", shape_name)
        };
        text(status).into()
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        iced::mouse::Interaction::Crosshair
    }

    fn show_caret(&self) -> bool {
        false
    }
}
