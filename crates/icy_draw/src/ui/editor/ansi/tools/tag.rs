//! Tag Tool (Annotations)
//!
//! Creates and manages annotation tags on the canvas.
//! Tags are rectangular regions with optional labels.

use iced::Element;
use iced::widget::{Space, button, row, text};
use icy_engine::Position;
use icy_engine::Rectangle;
use icy_engine_edit::AtomicUndoGuard;
use icy_engine_edit::EditState;
use icy_engine_edit::UndoState;
use icy_engine_gui::TerminalMessage;
use icy_engine_gui::ui::{SPACE_8, SPACE_16, TEXT_SIZE_SMALL};

use super::{ToolContext, ToolHandler, ToolMessage, ToolResult};
use crate::ui::editor::ansi::dialog::tag::TagDialog;
use crate::ui::editor::ansi::dialog::tag::TagDialogMessage;
use crate::ui::editor::ansi::dialog::tag_list::TagListDialog;
use crate::ui::editor::ansi::dialog::tag_list::TagListDialogMessage;
use crate::ui::editor::ansi::dialog::tag_list::TagListItem;
use icy_engine::{Tag, TagPlacement, TagRole, TextPane};

/// Consolidated state for the Tag tool system.
///
/// This structure holds all tag-related state that was previously scattered
/// across AnsiEditor fields. It manages:
/// - Tag dialogs (edit and list)
/// - Drag operations (single and multi-select)
/// - Selection state
/// - Context menu state
/// - Undo guards for atomic operations
#[derive(Default)]
pub struct TagToolState {
    /// Tag edit dialog (when editing a single tag's properties)
    pub dialog: Option<TagDialog>,
    /// Tag list dialog (shows all tags in the document)
    pub list_dialog: Option<TagListDialog>,
    /// If true, we are dragging one or more tags
    pub drag_active: bool,
    /// Indices of tags being dragged (supports multi-selection)
    pub drag_indices: Vec<usize>,
    /// Tag positions at start of drag (parallel to drag_indices)
    pub drag_start_positions: Vec<Position>,
    /// Selected tag indices for multi-selection
    pub selection: Vec<usize>,
    /// Context menu state: Some((tag_index, screen_position)) when open
    pub context_menu: Option<(usize, Position)>,
    /// If Some(index), we are adding a new tag and dragging it
    pub add_new_index: Option<usize>,
    /// If true, we are doing a selection rectangle drag to select multiple tags
    pub selection_drag_active: bool,
    /// Atomic undo guard for tag drag operations
    pub drag_undo: Option<AtomicUndoGuard>,
    /// Drag start position (text coordinates)
    pub drag_start: Position,
    /// Drag current position (text coordinates)
    pub drag_cur: Position,
}

impl TagToolState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn collect_overlay_data(&self, state: &EditState) -> Vec<(Position, usize, bool)> {
        state
            .get_buffer()
            .tags
            .iter()
            .enumerate()
            .map(|(idx, tag)| (tag.position, tag.len(), self.selection.contains(&idx)))
            .collect()
    }

    pub fn snapshot_tags(state: &EditState) -> Vec<TagListItem> {
        state
            .get_buffer()
            .tags
            .iter()
            .enumerate()
            .map(|(index, tag)| TagListItem {
                index,
                is_enabled: tag.is_enabled,
                preview: tag.preview.clone(),
                replacement_value: tag.replacement_value.clone(),
                position: tag.position,
                placement: tag.tag_placement,
            })
            .collect()
    }

    pub fn open_list_dialog(&mut self, state: &EditState) {
        // Avoid stacked modals.
        self.dialog = None;
        self.list_dialog = Some(TagListDialog::new(Self::snapshot_tags(state)));
    }

    pub fn handle_list_dialog_message(&mut self, state: &mut EditState, msg: TagListDialogMessage) -> ToolResult {
        match msg {
            TagListDialogMessage::Close => {
                self.list_dialog = None;
                ToolResult::None
            }
            TagListDialogMessage::Delete(index) => {
                if let Err(err) = state.remove_tag(index) {
                    log::warn!("Failed to remove tag: {}", err);
                    return ToolResult::None;
                }

                // Keep selection consistent.
                self.selection.retain(|&i| i != index);
                for i in &mut self.selection {
                    if *i > index {
                        *i -= 1;
                    }
                }

                // Refresh dialog contents.
                if let Some(dialog) = self.list_dialog.as_mut() {
                    dialog.items = Self::snapshot_tags(state);
                }

                ToolResult::Commit("Remove tag".to_string())
            }
        }
    }

    pub fn open_edit_dialog_for_tag(&mut self, state: &EditState, index: usize) {
        self.close_context_menu();
        let tag = state.get_buffer().tags.get(index).cloned();
        if let Some(tag) = tag {
            self.list_dialog = None;
            self.dialog = Some(TagDialog::edit(&tag, index));
        }
    }

    pub fn delete_tag(&mut self, state: &mut EditState, index: usize) -> ToolResult {
        self.close_context_menu();
        if let Err(err) = state.remove_tag(index) {
            log::warn!("Failed to remove tag: {}", err);
            return ToolResult::None;
        }

        self.selection.retain(|&i| i != index);
        for i in &mut self.selection {
            if *i > index {
                *i -= 1;
            }
        }

        ToolResult::Commit("Remove tag".to_string())
    }

    pub fn clone_tag(&mut self, state: &mut EditState, index: usize) -> ToolResult {
        self.close_context_menu();
        if let Err(err) = state.clone_tag(index) {
            log::warn!("Failed to clone tag: {}", err);
            return ToolResult::None;
        }
        ToolResult::Commit("Clone tag".to_string())
    }

    pub fn delete_selected_tags(&mut self, state: &mut EditState) -> ToolResult {
        self.close_context_menu();

        // Delete tags in reverse order to keep indices valid.
        let mut indices: Vec<usize> = self.selection.clone();
        indices.sort_by(|a, b| b.cmp(a));

        for index in indices {
            if let Err(err) = state.remove_tag(index) {
                log::warn!("Failed to remove tag {}: {}", index, err);
            }
        }

        let count = self.selection.len();
        self.selection.clear();

        ToolResult::Commit(format!("Remove {} tags", count))
    }

    pub fn generate_next_tag_name(state: &EditState) -> String {
        let tag_count = state.get_buffer().tags.len();
        format!("TAG{}", tag_count + 1)
    }

    pub fn start_add_mode(&mut self, state: &mut EditState) -> ToolResult {
        self.close_context_menu();

        let next_tag_name = Self::generate_next_tag_name(state);
        let attribute = state.get_caret().attribute;

        let new_tag = Tag {
            position: Position::default(),
            length: next_tag_name.len(),
            preview: next_tag_name,
            is_enabled: true,
            alignment: std::fmt::Alignment::Left,
            replacement_value: String::new(),
            tag_placement: TagPlacement::InText,
            tag_role: TagRole::Displaycode,
            attribute,
        };

        self.drag_undo = Some(state.begin_atomic_undo("Add tag"));

        if let Err(err) = (|| {
            if !state.get_buffer().show_tags {
                let _ = state.show_tags(true);
            }
            state.add_new_tag(new_tag)
        })() {
            log::warn!("Failed to add tag: {}", err);
            self.drag_undo = None;
            return ToolResult::None;
        }

        let new_index = state.get_buffer().tags.len().saturating_sub(1);
        self.add_new_index = Some(new_index);
        self.selection.clear();
        self.selection.push(new_index);
        self.drag_active = true;
        self.drag_indices = vec![new_index];
        self.drag_start_positions = vec![Position::default()];
        self.drag_start = Position::default();
        self.drag_cur = Position::default();

        ToolResult::StartCapture.and(ToolResult::Redraw)
    }

    pub fn cancel_add_mode(&mut self, state: &mut EditState) -> ToolResult {
        if self.add_new_index.is_some() {
            self.add_new_index = None;
            self.end_drag();
            let _ = state.undo();
            return ToolResult::EndCapture.and(ToolResult::Redraw);
        }
        ToolResult::None
    }

    pub fn handle_dialog_message(&mut self, state: &mut EditState, msg: TagDialogMessage) -> ToolResult {
        let Some(dialog) = &mut self.dialog else {
            return ToolResult::None;
        };

        match msg {
            TagDialogMessage::SetPreview(s) => {
                dialog.preview = s;
                ToolResult::None
            }
            TagDialogMessage::SetReplacement(s) => {
                dialog.replacement_value = s;
                ToolResult::None
            }
            TagDialogMessage::SetPosX(s) => {
                dialog.pos_x = s;
                ToolResult::None
            }
            TagDialogMessage::SetPosY(s) => {
                dialog.pos_y = s;
                ToolResult::None
            }
            TagDialogMessage::SetPlacement(p) => {
                dialog.placement = p;
                ToolResult::None
            }
            TagDialogMessage::Cancel => {
                self.dialog = None;
                ToolResult::None
            }
            TagDialogMessage::Ok => {
                let mut position = dialog.position;
                let preview = dialog.preview.trim().to_string();
                let replacement_value = dialog.replacement_value.clone();
                let placement = dialog.placement;
                let pos_x = dialog.pos_x.trim().to_string();
                let pos_y = dialog.pos_y.trim().to_string();
                let edit_index = dialog.edit_index;
                let tag_length = dialog.length.unwrap_or(0);
                let from_selection = dialog.from_selection;
                self.dialog = None;

                if preview.is_empty() {
                    return ToolResult::None;
                }

                if let Ok(x) = pos_x.parse::<i32>() {
                    position.x = x;
                }
                if let Ok(y) = pos_y.parse::<i32>() {
                    position.y = y;
                }

                let attribute = state.get_caret().attribute;
                let new_tag = Tag {
                    is_enabled: true,
                    preview,
                    replacement_value,
                    position,
                    length: tag_length,
                    alignment: std::fmt::Alignment::Left,
                    tag_placement: placement.to_engine(),
                    tag_role: TagRole::Displaycode,
                    attribute,
                };

                let commit_message = if edit_index.is_some() { "Edit tag" } else { "Add tag" };

                let size = state.get_buffer().size();
                let max_x = (size.width - 1).max(0);
                let max_y = (size.height - 1).max(0);

                let mut new_tag = new_tag;
                new_tag.position.x = new_tag.position.x.clamp(0, max_x);
                new_tag.position.y = new_tag.position.y.clamp(0, max_y);

                if let Some(index) = edit_index {
                    if let Err(err) = state.update_tag(new_tag, index) {
                        log::warn!("Failed to update tag: {}", err);
                        return ToolResult::None;
                    }
                } else {
                    if let Err(err) = (|| {
                        if !state.get_buffer().show_tags {
                            state.show_tags(true)?;
                        }
                        state.add_new_tag(new_tag)?;
                        if from_selection {
                            let _ = state.clear_selection();
                        }
                        Ok::<(), icy_engine::EngineError>(())
                    })() {
                        log::warn!("Failed to add tag: {}", err);
                        return ToolResult::None;
                    }
                }

                ToolResult::Commit(commit_message.to_string())
            }
        }
    }

    /// End the current drag operation, clearing all drag state.
    pub fn end_drag(&mut self) {
        self.drag_active = false;
        self.drag_indices.clear();
        self.drag_start_positions.clear();
        // Drop the guard to finalize/commit the atomic undo.
        self.drag_undo = None;
    }

    /// Cancel selection drag operation.
    pub fn cancel_selection_drag(&mut self) {
        if !self.selection_drag_active {
            return;
        }
        self.selection_drag_active = false;
    }

    /// Close the context menu.
    pub fn close_context_menu(&mut self) {
        self.context_menu = None;
    }

    pub fn view_context_menu_overlay(
        &self,
        font_width: f32,
        font_height: f32,
        scroll_x: f32,
        scroll_y: f32,
        display_scale: f32,
    ) -> Option<Element<'_, ToolMessage>> {
        use iced::Length;
        use iced::Theme;
        use iced::widget::{button, column, container, mouse_area, text};
        use icy_engine_gui::ui::TEXT_SIZE_NORMAL;

        let Some((tag_index, pos)) = self.context_menu else {
            return None;
        };

        let edit_btn = button(text("Edit").size(TEXT_SIZE_NORMAL))
            .padding([4, 12])
            .style(iced::widget::button::text)
            .on_press(ToolMessage::TagEdit(tag_index));

        let clone_btn = button(text("Clone").size(TEXT_SIZE_NORMAL))
            .padding([4, 12])
            .style(iced::widget::button::text)
            .on_press(ToolMessage::TagClone(tag_index));

        let delete_btn = button(text("Delete").size(TEXT_SIZE_NORMAL))
            .padding([4, 12])
            .style(iced::widget::button::text)
            .on_press(ToolMessage::TagDelete(tag_index));

        let mut menu_items: Vec<Element<'_, ToolMessage>> = vec![edit_btn.into(), clone_btn.into(), delete_btn.into()];

        // Add "Delete Selected" option if multiple tags are selected
        if self.selection.len() > 1 {
            let delete_selected_btn = button(text(format!("Delete {} Selected", self.selection.len())).size(TEXT_SIZE_NORMAL))
                .padding([4, 12])
                .style(iced::widget::button::text)
                .on_press(ToolMessage::TagDeleteSelected);
            menu_items.push(delete_selected_btn.into());
        }

        let menu_content = container(column(menu_items).spacing(2).width(Length::Fixed(150.0)))
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(iced::Background::Color(palette.background.weak.color)),
                    border: iced::Border {
                        color: palette.background.strong.color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                }
            })
            .padding(4);

        // Convert buffer position to screen pixels.
        let scale = display_scale.max(0.001);
        let font_w = font_width.max(1.0);
        let font_h = font_height.max(1.0);
        let menu_x = ((pos.x as f32 - scroll_x) * font_w * scale) as f32;
        let menu_y = ((pos.y as f32 - scroll_y + 1.0) * font_h * scale) as f32;

        let menu_positioned = container(menu_content).padding(iced::Padding {
            top: menu_y,
            left: menu_x,
            right: 0.0,
            bottom: 0.0,
        });

        // Full-screen clickable area that closes the menu when clicked outside.
        let backdrop = mouse_area(container(menu_positioned).width(Length::Fill).height(Length::Fill)).on_press(ToolMessage::TagContextMenuClose);

        Some(backdrop.into())
    }

    /// Check if any dialog is open.
    pub fn has_open_dialog(&self) -> bool {
        self.dialog.is_some() || self.list_dialog.is_some()
    }

    /// Check if context menu is open.
    pub fn has_context_menu(&self) -> bool {
        self.context_menu.is_some()
    }

    /// Check if any drag operation is active
    pub fn is_dragging(&self) -> bool {
        self.drag_active || self.selection_drag_active || self.add_new_index.is_some()
    }
}

/// Tag tool handler
///
/// Owns toolbar message dispatch and (eventually) mouse interaction logic.
/// Persistent state is still in the editor via `TagToolState`.
#[derive(Default)]
pub struct TagTool {}

impl TagTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_overlay_mask_in_state(state: &mut EditState) {
        let tag_rects: Vec<(Position, usize)> = state.get_buffer().tags.iter().map(|tag| (tag.position, tag.len())).collect();

        let overlays = state.get_tool_overlay_mask_mut();
        overlays.clear();

        for (pos, len) in tag_rects {
            let rect = Rectangle::new(pos, (len as i32, 1).into());
            overlays.add_rectangle(rect);
        }

        state.mark_dirty();
    }

    pub fn overlay_mask_for_tags(
        font_width: f32,
        font_height: f32,
        tag_data: &[(Position, usize, bool)],
    ) -> (Option<(Vec<u8>, u32, u32)>, Option<(f32, f32, f32, f32)>) {
        let overlay_rects: Vec<(f32, f32, f32, f32, bool)> = tag_data
            .iter()
            .map(|(pos, len, is_selected)| {
                let x = pos.x as f32 * font_width;
                let y = pos.y as f32 * font_height;
                let w = *len as f32 * font_width;
                let h = font_height;
                (x, y, w, h, *is_selected)
            })
            .collect();

        if overlay_rects.is_empty() {
            return (None, None);
        }

        // Find bounding box of all tags
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for (x, y, w, h, _) in &overlay_rects {
            min_x = min_x.min(*x);
            min_y = min_y.min(*y);
            max_x = max_x.max(x + w);
            max_y = max_y.max(y + h);
        }

        let total_w = (max_x - min_x).ceil() as u32;
        let total_h = (max_y - min_y).ceil() as u32;

        if total_w == 0 || total_h == 0 {
            return (None, None);
        }

        // Create RGBA buffer for overlay
        let mut rgba = vec![0u8; (total_w * total_h * 4) as usize];

        for (x, y, w, h, is_selected) in &overlay_rects {
            let local_x = (x - min_x) as u32;
            let local_y = (y - min_y) as u32;
            let rect_w = *w as u32;
            let rect_h = *h as u32;

            // Different colors for selected vs non-selected tags
            let (r, g, b, a) = if *is_selected {
                (255, 200, 50, 255) // Yellow/orange for selected
            } else {
                (100, 150, 255, 200) // Translucent blue for normal
            };

            // Draw border
            for py in local_y..(local_y + rect_h).min(total_h) {
                for px in local_x..(local_x + rect_w).min(total_w) {
                    // Border pixels only
                    let is_border =
                        px == local_x || px == (local_x + rect_w - 1).min(total_w - 1) || py == local_y || py == (local_y + rect_h - 1).min(total_h - 1);

                    if is_border {
                        let idx = ((py * total_w + px) * 4) as usize;
                        if idx + 3 < rgba.len() {
                            rgba[idx] = r;
                            rgba[idx + 1] = g;
                            rgba[idx + 2] = b;
                            rgba[idx + 3] = a;
                        }
                    }
                }
            }
        }

        (Some((rgba, total_w, total_h)), Some((min_x, min_y, total_w as f32, total_h as f32)))
    }

    pub fn overlay_mask_for_selection_drag(
        font_width: f32,
        font_height: f32,
        start: Position,
        cur: Position,
    ) -> (Option<(Vec<u8>, u32, u32)>, Option<(f32, f32, f32, f32)>) {
        // Calculate selection rectangle in pixel coordinates
        let min_x = start.x.min(cur.x) as f32 * font_width;
        let max_x = (start.x.max(cur.x) + 1) as f32 * font_width;
        let min_y = start.y.min(cur.y) as f32 * font_height;
        let max_y = (start.y.max(cur.y) + 1) as f32 * font_height;

        let w = (max_x - min_x).ceil() as u32;
        let h = (max_y - min_y).ceil() as u32;

        if w == 0 || h == 0 {
            return (None, None);
        }

        // Create RGBA buffer for selection rectangle overlay (dashed border, translucent fill)
        let mut rgba = vec![0u8; (w * h * 4) as usize];

        // Draw translucent fill and dashed border
        for py in 0..h {
            for px in 0..w {
                let idx = ((py * w + px) * 4) as usize;
                if idx + 3 >= rgba.len() {
                    continue;
                }

                let is_border = px == 0 || px == w - 1 || py == 0 || py == h - 1;

                if is_border {
                    // Dashed border pattern (every 4 pixels)
                    let is_dash = ((px + py) / 4) % 2 == 0;
                    if is_dash {
                        rgba[idx] = 255; // R - white
                        rgba[idx + 1] = 255; // G
                        rgba[idx + 2] = 255; // B
                        rgba[idx + 3] = 200; // A
                    }
                } else {
                    // Translucent fill
                    rgba[idx] = 100; // R
                    rgba[idx + 1] = 150; // G
                    rgba[idx + 2] = 255; // B - blue tint
                    rgba[idx + 3] = 30; // A - very translucent
                }
            }
        }

        (Some((rgba, w, h)), Some((min_x, min_y, w as f32, h as f32)))
    }
}

impl ToolHandler for TagTool {
    fn handle_terminal_message(&mut self, ctx: &mut ToolContext, msg: &TerminalMessage) -> ToolResult {
        let Some(tag_state) = ctx.tag_state.as_deref_mut() else {
            return ToolResult::None;
        };

        match msg {
            TerminalMessage::Press(evt) => {
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };
                Self::handle_mouse_down(ctx.state, tag_state, pos, evt.modifiers.clone(), evt.button)
            }
            TerminalMessage::Move(evt) | TerminalMessage::Drag(evt) => {
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };
                let cursor = Some(Self::cursor_for_state(tag_state));
                let result = Self::handle_mouse_drag(ctx.state, tag_state, pos);
                ToolResult::SetCursorIcon(cursor).and(result)
            }
            TerminalMessage::Release(evt) => {
                let pos = evt.text_position.unwrap_or_default();
                Self::handle_mouse_up(ctx.state, tag_state, pos)
            }
            _ => ToolResult::None,
        }
    }

    fn handle_event(&mut self, ctx: &mut ToolContext, event: &iced::Event) -> ToolResult {
        let Some(tag_state) = ctx.tag_state.as_deref_mut() else {
            return ToolResult::None;
        };

        match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                use iced::keyboard::key::Named;
                if matches!(key, iced::keyboard::Key::Named(Named::Escape)) {
                    // Cancel add-tag mode (undo the newly created tag)
                    if tag_state.add_new_index.is_some() {
                        tag_state.add_new_index = None;
                        tag_state.selection_drag_active = false;
                        tag_state.end_drag();
                        let _ = ctx.state.undo();
                        return ToolResult::EndCapture.and(ToolResult::Redraw);
                    }

                    // Cancel selection-drag rectangle
                    if tag_state.selection_drag_active {
                        tag_state.cancel_selection_drag();
                        return ToolResult::EndCapture.and(ToolResult::Redraw);
                    }
                }
                ToolResult::None
            }
            _ => ToolResult::None,
        }
    }

    fn handle_message(&mut self, ctx: &mut ToolContext, msg: &ToolMessage) -> ToolResult {
        let Some(tag_state) = ctx.tag_state.as_deref_mut() else {
            return ToolResult::None;
        };

        match *msg {
            ToolMessage::TagEdit(index) => {
                tag_state.open_edit_dialog_for_tag(ctx.state, index);
                ToolResult::Redraw
            }
            ToolMessage::TagDelete(index) => tag_state.delete_tag(ctx.state, index),
            ToolMessage::TagClone(index) => tag_state.clone_tag(ctx.state, index),
            ToolMessage::TagContextMenuClose => {
                tag_state.close_context_menu();
                ToolResult::Redraw
            }
            ToolMessage::TagOpenList => {
                tag_state.open_list_dialog(ctx.state);
                ToolResult::Redraw
            }
            ToolMessage::TagStartAdd => {
                if tag_state.add_new_index.is_some() {
                    tag_state.cancel_add_mode(ctx.state)
                } else {
                    tag_state.start_add_mode(ctx.state)
                }
            }
            ToolMessage::TagEditSelected => {
                if tag_state.selection.len() == 1 {
                    let idx = tag_state.selection[0];
                    tag_state.open_edit_dialog_for_tag(ctx.state, idx);
                    ToolResult::Redraw
                } else {
                    ToolResult::None
                }
            }
            ToolMessage::TagDeleteSelected => tag_state.delete_selected_tags(ctx.state),
            _ => ToolResult::None,
        }
    }

    fn view_toolbar<'a>(&'a self, ctx: &super::ToolViewContext<'_>) -> Element<'a, ToolMessage> {
        let add_button = if ctx.tag_add_mode {
            button(text("Add").size(TEXT_SIZE_SMALL))
                .on_press(ToolMessage::TagStartAdd)
                .style(button::primary)
        } else {
            button(text("Add").size(TEXT_SIZE_SMALL))
                .on_press(ToolMessage::TagStartAdd)
                .style(button::secondary)
        };

        let tags_button = button(text("Tags…").size(TEXT_SIZE_SMALL))
            .on_press(ToolMessage::TagOpenList)
            .style(button::secondary);

        let left_side = row![add_button, tags_button].spacing(SPACE_8);

        let has_selected_tags = ctx.tag_selection_count > 0;

        let middle: Element<'a, ToolMessage> = if ctx.tag_selection_count > 1 {
            text(format!("({} tags)", ctx.tag_selection_count)).size(TEXT_SIZE_SMALL).into()
        } else if let Some(tag_info) = ctx.selected_tag.clone() {
            let edit_button = button(text("Edit").size(TEXT_SIZE_SMALL))
                .on_press(ToolMessage::TagEditSelected)
                .style(button::secondary);

            let pos_text = text(format!("({}, {})", tag_info.position.x, tag_info.position.y)).size(TEXT_SIZE_SMALL);
            let replacement_text = if tag_info.replacement.is_empty() {
                text("(no replacement)").size(TEXT_SIZE_SMALL)
            } else {
                text(tag_info.replacement).size(TEXT_SIZE_SMALL)
            };

            row![edit_button, pos_text, replacement_text].spacing(SPACE_8).into()
        } else if ctx.tag_add_mode {
            text("Click to place tag, ESC to cancel").size(TEXT_SIZE_SMALL).into()
        } else {
            text("").into()
        };

        let right_side: Element<'a, ToolMessage> = if has_selected_tags {
            button(text("Delete").size(TEXT_SIZE_SMALL))
                .on_press(ToolMessage::TagDeleteSelected)
                .style(button::danger)
                .into()
        } else {
            Space::new().width(iced::Length::Shrink).into()
        };

        row![
            left_side,
            Space::new().width(iced::Length::Fixed(SPACE_16)),
            middle,
            Space::new().width(iced::Length::Fill),
            right_side,
        ]
        .spacing(SPACE_8)
        .into()
    }

    fn view_status<'a>(&'a self, _ctx: &super::ToolViewContext<'_>) -> Element<'a, ToolMessage> {
        // Status is managed by editor using TagToolState
        text("Tag").into()
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        iced::mouse::Interaction::Pointer
    }

    fn show_caret(&self) -> bool {
        false
    }

    fn show_selection(&self) -> bool {
        false
    }
}

// =============================================================================
// Tag Tool Mouse Handling (called by editor with TagToolState)
// =============================================================================

impl TagTool {
    /// Handle mouse press for Tag tool
    ///
    /// Returns ToolResult for the editor to process.
    pub fn handle_mouse_down(
        state: &mut icy_engine_edit::EditState,
        tag_state: &mut TagToolState,
        pos: Position,
        modifiers: icy_engine::KeyModifiers,
        button: icy_engine::MouseButton,
    ) -> ToolResult {
        // Close context menu on any click
        tag_state.close_context_menu();

        if button == icy_engine::MouseButton::Left {
            // If in add mode (dragging new tag), do nothing - already dragging
            if tag_state.add_new_index.is_some() {
                return ToolResult::None;
            }

            // Check if clicking on an existing tag
            let tag_at_pos = state
                .get_buffer()
                .tags
                .iter()
                .enumerate()
                .find(|(_, t)| t.contains(pos))
                .map(|(i, t)| (i, t.position));

            if let Some((index, tag_pos)) = tag_at_pos {
                // Handle Ctrl+Click for multi-selection toggle
                if modifiers.ctrl || modifiers.meta {
                    if tag_state.selection.contains(&index) {
                        tag_state.selection.retain(|&i| i != index);
                    } else {
                        tag_state.selection.push(index);
                    }
                    return ToolResult::Redraw;
                }

                // Check if this tag is part of multi-selection
                if tag_state.selection.contains(&index) {
                    // Drag all selected tags
                    let selected_positions: Vec<(usize, Position)> = tag_state
                        .selection
                        .iter()
                        .filter_map(|&i| state.get_buffer().tags.get(i).map(|t| (i, t.position)))
                        .collect();
                    tag_state.drag_indices = selected_positions.iter().map(|(i, _)| *i).collect();
                    tag_state.drag_start_positions = selected_positions.iter().map(|(_, p)| *p).collect();
                } else {
                    // Clear selection and drag single tag
                    tag_state.selection.clear();
                    tag_state.selection.push(index);
                    tag_state.drag_indices = vec![index];
                    tag_state.drag_start_positions = vec![tag_pos];
                }

                tag_state.drag_active = true;
                tag_state.drag_start = pos;
                tag_state.drag_cur = pos;

                // Start atomic undo for drag operation
                let desc = if tag_state.selection.len() > 1 {
                    format!("Move {} tags", tag_state.selection.len())
                } else {
                    "Move tag".to_string()
                };
                tag_state.drag_undo = Some(state.begin_atomic_undo(desc));

                return ToolResult::StartCapture.and(ToolResult::Redraw);
            }

            // No tag at position - start a selection drag to select multiple tags
            tag_state.selection.clear();
            tag_state.list_dialog = None;

            // Start selection rectangle drag
            tag_state.selection_drag_active = true;
            tag_state.drag_start = pos;
            tag_state.drag_cur = pos;

            ToolResult::StartCapture.and(ToolResult::Redraw)
        } else if button == icy_engine::MouseButton::Right {
            // Right-click: open context menu if clicking on a tag
            let tag_at_pos = state.get_buffer().tags.iter().enumerate().find(|(_, t)| t.contains(pos)).map(|(i, _)| i);

            if let Some(index) = tag_at_pos {
                // If the tag is not in selection, select only this tag
                if !tag_state.selection.contains(&index) {
                    tag_state.selection.clear();
                    tag_state.selection.push(index);
                }
                tag_state.context_menu = Some((index, pos));
                return ToolResult::Redraw;
            }
            ToolResult::None
        } else {
            ToolResult::None
        }
    }

    /// Handle mouse drag for Tag tool
    pub fn handle_mouse_drag(state: &mut icy_engine_edit::EditState, tag_state: &mut TagToolState, pos: Position) -> ToolResult {
        tag_state.drag_cur = pos;

        // Handle new tag placement drag
        if tag_state.add_new_index.is_some() && tag_state.drag_active {
            let delta = tag_state.drag_cur - tag_state.drag_start;

            // Move the new tag(s)
            let moves: Vec<(usize, Position)> = tag_state
                .drag_indices
                .iter()
                .zip(tag_state.drag_start_positions.iter())
                .map(|(&tag_idx, &start_pos)| (tag_idx, start_pos + delta))
                .collect();

            for (tag_idx, new_pos) in moves {
                let _ = state.move_tag(tag_idx, new_pos);
            }
            state.mark_dirty();
            return ToolResult::Redraw;
        }

        // Handle tag drag (multi-selection)
        if tag_state.drag_active && !tag_state.drag_indices.is_empty() {
            let delta = tag_state.drag_cur - tag_state.drag_start;

            // Move all selected tags
            let moves: Vec<(usize, Position)> = tag_state
                .drag_indices
                .iter()
                .zip(tag_state.drag_start_positions.iter())
                .map(|(&tag_idx, &start_pos)| (tag_idx, start_pos + delta))
                .collect();

            for (tag_idx, new_pos) in moves {
                let _ = state.move_tag(tag_idx, new_pos);
            }
            state.mark_dirty();
            return ToolResult::Redraw;
        }

        // Handle selection rectangle drag
        if tag_state.selection_drag_active {
            return ToolResult::Redraw;
        }

        ToolResult::None
    }

    /// Handle mouse release for Tag tool
    pub fn handle_mouse_up(state: &mut icy_engine_edit::EditState, tag_state: &mut TagToolState, pos: Position) -> ToolResult {
        tag_state.drag_cur = pos;

        // Handle new tag placement completion
        if let Some(new_tag_index) = tag_state.add_new_index.take() {
            tag_state.end_drag();

            // Open edit dialog for the newly placed tag
            if let Some(tag) = state.get_buffer().tags.get(new_tag_index).cloned() {
                tag_state.dialog = Some(TagDialog::edit(&tag, new_tag_index));
            }
            return ToolResult::EndCapture.and(ToolResult::Redraw);
        }

        // Handle regular tag drag completion
        if tag_state.drag_active {
            tag_state.end_drag();
            return ToolResult::EndCapture;
        }

        // Handle tag selection drag completion
        if tag_state.selection_drag_active {
            tag_state.selection_drag_active = false;

            // Calculate selection rectangle
            let min_x = tag_state.drag_start.x.min(tag_state.drag_cur.x);
            let max_x = tag_state.drag_start.x.max(tag_state.drag_cur.x);
            let min_y = tag_state.drag_start.y.min(tag_state.drag_cur.y);
            let max_y = tag_state.drag_start.y.max(tag_state.drag_cur.y);

            // Find all tags that intersect with the selection rectangle
            let selected_indices: Vec<usize> = state
                .get_buffer()
                .tags
                .iter()
                .enumerate()
                .filter(|(_, tag)| {
                    let tag_min_x = tag.position.x;
                    let tag_max_x = tag.position.x + tag.len() as i32 - 1;
                    let tag_y = tag.position.y;

                    // Check if tag intersects with selection rectangle
                    tag_y >= min_y && tag_y <= max_y && tag_max_x >= min_x && tag_min_x <= max_x
                })
                .map(|(i, _)| i)
                .collect();

            tag_state.selection = selected_indices;

            return ToolResult::EndCapture.and(ToolResult::Redraw);
        }

        ToolResult::EndCapture
    }

    /// Handle keyboard events for Tag tool
    pub fn handle_key(state: &mut icy_engine_edit::EditState, tag_state: &mut TagToolState, key: &iced::keyboard::Key) -> ToolResult {
        use iced::keyboard::key::Named;

        if let iced::keyboard::Key::Named(named) = key {
            match named {
                Named::Delete | Named::Backspace => {
                    if !tag_state.selection.is_empty() {
                        // Delete selected tags in reverse order to preserve indices
                        let mut indices: Vec<usize> = tag_state.selection.clone();
                        indices.sort_by(|a, b| b.cmp(a));
                        for idx in indices {
                            let _ = state.remove_tag(idx);
                        }
                        let count = tag_state.selection.len();
                        tag_state.selection.clear();
                        return ToolResult::Commit(format!("Delete {} tag(s)", count));
                    }
                }
                Named::Escape => {
                    // Cancel current operation
                    if tag_state.add_new_index.is_some() {
                        tag_state.add_new_index = None;
                        tag_state.selection_drag_active = false;
                        return ToolResult::Redraw;
                    }
                    tag_state.selection.clear();
                    return ToolResult::Redraw;
                }
                _ => {}
            }
        }
        ToolResult::None
    }

    /// Get cursor for current tag state
    pub fn cursor_for_state(tag_state: &TagToolState) -> iced::mouse::Interaction {
        if tag_state.drag_active {
            iced::mouse::Interaction::Grabbing
        } else if tag_state.add_new_index.is_some() {
            iced::mouse::Interaction::Crosshair
        } else {
            iced::mouse::Interaction::Pointer
        }
    }
}
