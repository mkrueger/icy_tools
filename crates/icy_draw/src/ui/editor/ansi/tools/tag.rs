//! Tag Tool (Annotations)
//!
//! Creates and manages annotation tags on the canvas.
//! Tags are rectangular regions with optional labels.

use iced::Element;
use iced::widget::{column, text};
use icy_engine::Position;

use super::{ToolContext, ToolHandler, ToolInput, ToolMessage, ToolResult};

/// Tag tool state
#[derive(Clone, Debug, Default)]
pub struct TagTool {
    /// Index of tag being dragged (if any)
    dragging_tag: Option<usize>,
    /// Start position of new tag (if creating)
    new_tag_start: Option<Position>,
    /// Current position during drag
    current_pos: Option<Position>,
    /// Indices of selected tags (multi-select with Shift)
    selected_tags: Vec<usize>,
    /// Start positions of selected tags (for multi-drag)
    drag_start_positions: Vec<Position>,
    /// Whether currently in add mode (dragging new tag)
    add_mode: bool,
}

impl TagTool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any tag is selected
    #[allow(dead_code)]
    pub fn has_selection(&self) -> bool {
        !self.selected_tags.is_empty()
    }

    /// Get selected tag indices
    #[allow(dead_code)]
    pub fn selected_tags(&self) -> &[usize] {
        &self.selected_tags
    }

    /// Clear selection
    #[allow(dead_code)]
    pub fn clear_selection(&mut self) {
        self.selected_tags.clear();
        self.drag_start_positions.clear();
    }
}

impl ToolHandler for TagTool {
    fn handle_event(&mut self, ctx: &mut ToolContext, event: ToolInput) -> ToolResult {
        match event {
            ToolInput::MouseDown { pos, modifiers, .. } => {
                // Check if clicking on existing tag
                let tag_at_pos = ctx
                    .state
                    .get_buffer()
                    .tags
                    .iter()
                    .enumerate()
                    .find(|(_, t)| t.contains(pos))
                    .map(|(i, t)| (i, t.position));

                if let Some((tag_idx, _tag_pos)) = tag_at_pos {
                    // Clicking on existing tag
                    if modifiers.shift {
                        // Add/remove from selection
                        if let Some(pos) = self.selected_tags.iter().position(|&i| i == tag_idx) {
                            self.selected_tags.remove(pos);
                            self.drag_start_positions.remove(pos);
                        } else {
                            self.selected_tags.push(tag_idx);
                            let tag = &ctx.state.get_buffer().tags[tag_idx];
                            self.drag_start_positions.push(tag.position);
                        }
                    } else {
                        // Single select
                        self.selected_tags.clear();
                        self.drag_start_positions.clear();
                        self.selected_tags.push(tag_idx);
                        let tag = &ctx.state.get_buffer().tags[tag_idx];
                        self.drag_start_positions.push(tag.position);
                    }

                    self.dragging_tag = Some(tag_idx);
                    self.current_pos = Some(pos);
                    ToolResult::StartCapture.and(ToolResult::Redraw)
                } else {
                    // Start new tag
                    self.add_mode = true;
                    self.new_tag_start = Some(pos);
                    self.current_pos = Some(pos);
                    self.selected_tags.clear();
                    self.drag_start_positions.clear();
                    ToolResult::StartCapture.and(ToolResult::Redraw)
                }
            }

            ToolInput::MouseMove { pos, is_dragging, .. } => {
                if !is_dragging {
                    return ToolResult::None;
                }

                self.current_pos = Some(pos);

                if self.add_mode {
                    // Update new tag preview
                    ToolResult::Redraw
                } else if self.dragging_tag.is_some() {
                    // Move tag(s)
                    ToolResult::Redraw
                } else {
                    ToolResult::None
                }
            }

            ToolInput::MouseUp { pos, .. } => {
                self.current_pos = Some(pos);

                let result = if self.add_mode && self.new_tag_start.is_some() {
                    let start = self.new_tag_start.unwrap();
                    self.add_mode = false;
                    self.new_tag_start = None;

                    // Create new tag
                    let min_x = start.x.min(pos.x);
                    let min_y = start.y.min(pos.y);
                    let max_x = start.x.max(pos.x);
                    let max_y = start.y.max(pos.y);

                    let width = (max_x - min_x + 1) as usize;
                    let height = (max_y - min_y + 1) as usize;

                    if width > 0 && height > 0 {
                        // TODO: Create tag via ctx.state
                        ToolResult::EndCapture.and(ToolResult::Commit(format!("Create tag at ({},{}) size {}x{}", min_x, min_y, width, height)))
                    } else {
                        ToolResult::EndCapture
                    }
                } else if self.dragging_tag.is_some() {
                    self.dragging_tag = None;
                    ToolResult::EndCapture.and(ToolResult::Commit("Move tag".to_string()))
                } else {
                    ToolResult::EndCapture
                };

                self.current_pos = None;
                result
            }

            ToolInput::KeyDown { key, .. } => {
                use iced::keyboard::key::Named;

                if let iced::keyboard::Key::Named(named) = key {
                    match named {
                        Named::Delete | Named::Backspace => {
                            if !self.selected_tags.is_empty() {
                                // Delete selected tags (in reverse order to preserve indices)
                                // TODO: Implement via ctx.state
                                self.selected_tags.clear();
                                self.drag_start_positions.clear();
                                return ToolResult::Commit("Delete tag(s)".to_string());
                            }
                        }
                        Named::Escape => {
                            self.selected_tags.clear();
                            self.drag_start_positions.clear();
                            self.add_mode = false;
                            self.new_tag_start = None;
                            return ToolResult::Redraw;
                        }
                        _ => {}
                    }
                }
                ToolResult::None
            }

            ToolInput::Deactivate => {
                self.dragging_tag = None;
                self.new_tag_start = None;
                self.current_pos = None;
                self.selected_tags.clear();
                self.drag_start_positions.clear();
                self.add_mode = false;
                ToolResult::Redraw
            }

            _ => ToolResult::None,
        }
    }

    fn view_toolbar<'a>(&'a self, _ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        column![].into()
    }

    fn view_status<'a>(&'a self, ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        let tag_count = ctx.state.get_buffer().tags.len();
        let selected_count = self.selected_tags.len();

        let status = if self.add_mode {
            if let (Some(start), Some(end)) = (self.new_tag_start, self.current_pos) {
                let w = (end.x - start.x).abs() + 1;
                let h = (end.y - start.y).abs() + 1;
                format!("Tag | Creating: ({},{}) [{}x{}]", start.x, start.y, w, h)
            } else {
                "Tag | Click and drag to create".to_string()
            }
        } else if selected_count > 0 {
            format!("Tag | {} selected of {} | Del=Delete, Shift+Click=Multi-select", selected_count, tag_count)
        } else {
            format!("Tag | {} tags | Click tag to select, drag empty area to create", tag_count)
        };
        text(status).into()
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        if self.dragging_tag.is_some() {
            iced::mouse::Interaction::Grabbing
        } else if self.add_mode {
            iced::mouse::Interaction::Crosshair
        } else {
            iced::mouse::Interaction::Pointer
        }
    }

    fn show_caret(&self) -> bool {
        false
    }

    fn show_selection(&self) -> bool {
        false
    }
}
