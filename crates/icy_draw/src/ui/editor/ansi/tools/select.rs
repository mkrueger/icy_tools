//! Select Tool
//!
//! Rectangle selection with move/resize support.
//! Supports add (Shift), remove (Ctrl), and replace modes.

use iced::Element;
use iced::widget::{column, text};
use icy_engine::{Position, Rectangle, Selection};

use super::{ToolContext, ToolHandler, ToolInput, ToolMessage, ToolResult};

/// Selection drag mode
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SelectDragMode {
    #[default]
    None,
    /// Creating a new selection rectangle
    Create,
    /// Moving existing selection
    Move,
    /// Resizing from edges/corners
    ResizeLeft,
    ResizeRight,
    ResizeTop,
    ResizeBottom,
    ResizeTopLeft,
    ResizeTopRight,
    ResizeBottomLeft,
    ResizeBottomRight,
}

/// Selection modifier mode
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectModifier {
    #[default]
    Replace,
    Add,
    Remove,
}

/// Select tool state
#[derive(Clone, Debug, Default)]
pub struct SelectTool {
    /// Current drag mode
    drag_mode: SelectDragMode,
    /// Start position of drag
    start_pos: Option<Position>,
    /// Current position during drag
    current_pos: Option<Position>,
    /// Selection at start of resize operation
    start_selection: Option<Rectangle>,
    /// Whether currently dragging
    is_dragging: bool,
    /// Current modifier (from keyboard)
    modifier: SelectModifier,
}

impl SelectTool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Determine what kind of drag to start based on position relative to selection
    fn get_drag_mode_at(&self, pos: Position, selection: Option<Rectangle>) -> SelectDragMode {
        if let Some(rect) = selection {
            if rect.contains_pt(pos) {
                // Check edges/corners (within 2 chars)
                let left = pos.x - rect.left() < 2;
                let top = pos.y - rect.top() < 2;
                let right = rect.right() - pos.x < 2;
                let bottom = rect.bottom() - pos.y < 2;

                // Corners first
                if left && top {
                    return SelectDragMode::ResizeTopLeft;
                }
                if right && top {
                    return SelectDragMode::ResizeTopRight;
                }
                if left && bottom {
                    return SelectDragMode::ResizeBottomLeft;
                }
                if right && bottom {
                    return SelectDragMode::ResizeBottomRight;
                }

                // Edges
                if left {
                    return SelectDragMode::ResizeLeft;
                }
                if right {
                    return SelectDragMode::ResizeRight;
                }
                if top {
                    return SelectDragMode::ResizeTop;
                }
                if bottom {
                    return SelectDragMode::ResizeBottom;
                }

                // Inside - move
                return SelectDragMode::Move;
            }
        }
        SelectDragMode::Create
    }

    /// Calculate selection rectangle from start and current positions
    fn calculate_selection_rect(&self) -> Option<Rectangle> {
        let start = self.start_pos?;
        let end = self.current_pos?;
        Some(Rectangle::from_pt(start, end))
    }

    /// Apply resize operation to the start selection
    fn apply_resize(&self, delta: Position) -> Option<Rectangle> {
        let rect = self.start_selection?;
        let (mut left, mut top, mut right, mut bottom) = (rect.left(), rect.top(), rect.right(), rect.bottom());

        match self.drag_mode {
            SelectDragMode::ResizeLeft => {
                left += delta.x;
            }
            SelectDragMode::ResizeRight => {
                right += delta.x;
            }
            SelectDragMode::ResizeTop => {
                top += delta.y;
            }
            SelectDragMode::ResizeBottom => {
                bottom += delta.y;
            }
            SelectDragMode::ResizeTopLeft => {
                left += delta.x;
                top += delta.y;
            }
            SelectDragMode::ResizeTopRight => {
                right += delta.x;
                top += delta.y;
            }
            SelectDragMode::ResizeBottomLeft => {
                left += delta.x;
                bottom += delta.y;
            }
            SelectDragMode::ResizeBottomRight => {
                right += delta.x;
                bottom += delta.y;
            }
            _ => {}
        }

        // Ensure valid rectangle
        if left > right {
            std::mem::swap(&mut left, &mut right);
        }
        if top > bottom {
            std::mem::swap(&mut top, &mut bottom);
        }

        Some(Rectangle::from_coords(left, top, right, bottom))
    }

    /// Apply move operation to the start selection
    fn apply_move(&self, delta: Position) -> Option<Rectangle> {
        let rect = self.start_selection?;
        Some(Rectangle::from_coords(
            rect.left() + delta.x,
            rect.top() + delta.y,
            rect.right() + delta.x,
            rect.bottom() + delta.y,
        ))
    }

    /// Update the selection in the edit state based on current drag
    fn update_selection(&self, ctx: &mut ToolContext) {
        let new_rect = match self.drag_mode {
            SelectDragMode::Create => self.calculate_selection_rect(),
            SelectDragMode::Move => {
                if let (Some(start), Some(current)) = (self.start_pos, self.current_pos) {
                    let delta = current - start;
                    self.apply_move(delta)
                } else {
                    None
                }
            }
            SelectDragMode::ResizeLeft
            | SelectDragMode::ResizeRight
            | SelectDragMode::ResizeTop
            | SelectDragMode::ResizeBottom
            | SelectDragMode::ResizeTopLeft
            | SelectDragMode::ResizeTopRight
            | SelectDragMode::ResizeBottomLeft
            | SelectDragMode::ResizeBottomRight => {
                if let (Some(start), Some(current)) = (self.start_pos, self.current_pos) {
                    let delta = current - start;
                    self.apply_resize(delta)
                } else {
                    None
                }
            }
            SelectDragMode::None => None,
        };

        if let Some(rect) = new_rect {
            let _ = ctx.state.set_selection(Selection::from(rect));
        }
    }
}

impl ToolHandler for SelectTool {
    fn handle_event(&mut self, ctx: &mut ToolContext, event: ToolInput) -> ToolResult {
        match event {
            ToolInput::MouseDown { pos, modifiers, .. } => {
                // Determine modifier from keyboard state
                self.modifier = if modifiers.shift {
                    SelectModifier::Add
                } else if modifiers.ctrl || modifiers.meta {
                    SelectModifier::Remove
                } else {
                    SelectModifier::Replace
                };

                // Get current selection
                let current_selection = ctx.state.selection().map(|s| s.as_rectangle());

                // In Add/Remove mode, commit current selection to mask and start fresh
                if self.modifier != SelectModifier::Replace {
                    let _ = ctx.state.add_selection_to_mask();
                    let _ = ctx.state.deselect();
                    self.drag_mode = SelectDragMode::Create;
                    self.start_selection = None;
                } else {
                    // Replace mode - check for move/resize of existing selection
                    let _ = ctx.state.clear_selection_mask();
                    self.drag_mode = self.get_drag_mode_at(pos, current_selection);
                    self.start_selection = current_selection;

                    // If creating new selection in Replace mode, clear existing
                    if self.drag_mode == SelectDragMode::Create {
                        let _ = ctx.state.clear_selection();
                    }
                }

                self.start_pos = Some(pos);
                self.current_pos = Some(pos);
                self.is_dragging = true;

                ToolResult::StartCapture.and(ToolResult::Redraw)
            }

            ToolInput::MouseMove { pos, .. } => {
                if self.is_dragging {
                    self.current_pos = Some(pos);
                    self.update_selection(ctx);
                    ToolResult::Redraw
                } else {
                    // Update cursor based on hover position
                    ToolResult::None
                }
            }

            ToolInput::MouseUp { pos, .. } => {
                if self.is_dragging {
                    self.current_pos = Some(pos);
                    self.update_selection(ctx);
                    self.is_dragging = false;

                    let start = self.start_pos.unwrap_or_default();
                    let end = pos;

                    // Reset state
                    self.drag_mode = SelectDragMode::None;
                    self.start_pos = None;
                    self.current_pos = None;
                    self.start_selection = None;

                    ToolResult::EndCapture.and(ToolResult::Commit(format!("Selection ({},{}) to ({},{})", start.x, start.y, end.x, end.y)))
                } else {
                    ToolResult::None
                }
            }

            ToolInput::KeyDown { key, .. } => {
                // Handle Delete/Backspace to erase selection
                use iced::keyboard::key::Named;
                if let iced::keyboard::Key::Named(named) = key {
                    match named {
                        Named::Delete | Named::Backspace => {
                            if ctx.state.is_something_selected() {
                                let _ = ctx.state.erase_selection();
                                return ToolResult::Commit("Delete selection".to_string());
                            }
                        }
                        Named::Escape => {
                            let _ = ctx.state.clear_selection();
                            return ToolResult::Redraw;
                        }
                        _ => {}
                    }
                }
                ToolResult::None
            }

            ToolInput::Deactivate => {
                self.drag_mode = SelectDragMode::None;
                self.start_pos = None;
                self.current_pos = None;
                self.start_selection = None;
                self.is_dragging = false;
                ToolResult::Redraw
            }

            _ => ToolResult::None,
        }
    }

    fn view_toolbar<'a>(&'a self, _ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        // TODO: Selection mode options (Normal, Row, Column, Line, Lasso)
        column![].into()
    }

    fn view_status<'a>(&'a self, _ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        let status = if let (Some(start), Some(end)) = (self.start_pos, self.current_pos) {
            let w = (end.x - start.x).abs() + 1;
            let h = (end.y - start.y).abs() + 1;
            format!(
                "Select | ({},{}) → ({},{}) [{}x{}] | Shift=Add, Ctrl=Remove",
                start.x, start.y, end.x, end.y, w, h
            )
        } else {
            "Select | Click and drag to select".to_string()
        };
        text(status).into()
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        match self.drag_mode {
            SelectDragMode::Move => iced::mouse::Interaction::Grabbing,
            SelectDragMode::ResizeLeft | SelectDragMode::ResizeRight => iced::mouse::Interaction::ResizingHorizontally,
            SelectDragMode::ResizeTop | SelectDragMode::ResizeBottom => iced::mouse::Interaction::ResizingVertically,
            SelectDragMode::ResizeTopLeft | SelectDragMode::ResizeBottomRight => iced::mouse::Interaction::Crosshair,
            SelectDragMode::ResizeTopRight | SelectDragMode::ResizeBottomLeft => iced::mouse::Interaction::Crosshair,
            _ => iced::mouse::Interaction::Crosshair,
        }
    }

    fn show_caret(&self) -> bool {
        false
    }

    fn show_selection(&self) -> bool {
        true
    }
}
