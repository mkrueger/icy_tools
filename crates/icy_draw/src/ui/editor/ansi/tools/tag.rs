//! Tag Tool (Annotations)
//!
//! Creates and manages annotation tags on the canvas.
//! Tags are rectangular regions with optional labels.

use std::{fs, path::PathBuf, sync::Arc};

use iced::widget::{button, row, text, Space};
use iced::Element;
use icy_engine::Position;
use icy_engine::Rectangle;
use icy_engine_edit::AtomicUndoGuard;
use icy_engine_edit::EditState;
use icy_engine_edit::UndoState;
use icy_engine_gui::ui::{SPACE_16, SPACE_8, TEXT_SIZE_SMALL};
use icy_engine_gui::DoubleClickDetector;
use icy_engine_gui::TerminalMessage;
use parking_lot::RwLock;

use super::{ToolContext, ToolHandler, ToolMessage, ToolResult};
use crate::fl;
use crate::ui::editor::ansi::dialog::tag::TagDialog;
use crate::ui::editor::ansi::dialog::tag::TagDialogMessage;
use crate::ui::editor::ansi::dialog::tag_list::TagListDialog;
use crate::ui::editor::ansi::dialog::tag_list::TagListDialogMessage;
use crate::ui::editor::ansi::dialog::tag_list::TagListItem;
use crate::util::{get_available_taglists, load_taglist};
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
    /// Pending click: Some((tag_index, position)) when mouse pressed but not yet dragged
    pub pending_click: Option<(usize, Position)>,
    /// Double-click detector for opening edit dialog
    pub double_click_detector: DoubleClickDetector<usize>,
    /// Currently selected taglist (from settings)
    pub selected_taglist: String,
    /// Effective taglist directory (from settings)
    pub taglists_dir: Option<PathBuf>,
}

impl TagToolState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancel any active drag/capture operation
    pub fn cancel_capture(&mut self) {
        self.drag_active = false;
        self.drag_indices.clear();
        self.drag_start_positions.clear();
        self.selection_drag_active = false;
        self.drag_undo = None;
        self.add_new_index = None;
        self.context_menu = None;
        self.pending_click = None;
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

    pub fn open_edit_dialog_for_tag(&mut self, state: &EditState, index: usize, selected_taglist: &str) {
        self.close_context_menu();
        let tag = state.get_buffer().tags.get(index).cloned();
        if let Some(tag) = tag {
            self.list_dialog = None;
            self.dialog = Some(TagDialog::edit(&tag, index, selected_taglist, self.taglists_dir.clone()));
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

    pub fn handle_dialog_message(&mut self, state: &mut EditState, msg: TagDialogMessage, settings: Option<&Arc<RwLock<crate::Settings>>>) -> ToolResult {
        let Some(dialog) = &mut self.dialog else {
            return ToolResult::None;
        };

        let taglists_dir = self.taglists_dir.clone();

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
            TagDialogMessage::ToggleReplacements => {
                dialog.show_replacements = !dialog.show_replacements;

                // On opening the replacement browser: reload taglists on-demand.
                if dialog.show_replacements {
                    if let Some(dir) = taglists_dir.as_deref() {
                        dialog.available_taglists = get_available_taglists(Some(dir));

                        // Ensure selected taglist still exists
                        let selected_id = dialog.selected_taglist.id.clone();
                        if !dialog.available_taglists.iter().any(|t| t.id.eq_ignore_ascii_case(&selected_id)) {
                            if let Some(first) = dialog.available_taglists.first().cloned() {
                                dialog.selected_taglist = first;
                            }
                        }

                        dialog.replacement_list = load_taglist(&dialog.selected_taglist.id, Some(dir));
                    }
                }
                ToolResult::Redraw
            }
            TagDialogMessage::SelectReplacement(example, tag) => {
                dialog.preview = example;
                dialog.replacement_value = tag;
                dialog.show_replacements = false;
                ToolResult::Redraw
            }
            TagDialogMessage::SelectTaglist(name) => {
                let id = name.id.clone();
                dialog.selected_taglist = name;
                dialog.replacement_list = load_taglist(&id, taglists_dir.as_deref());

                // Store in state so it persists for the next dialog
                self.selected_taglist = id.clone();

                // Save to settings (persist immediately)
                if let Some(settings) = settings {
                    settings.write().selected_taglist = id;
                    settings.read().store_persistent();
                }
                ToolResult::Redraw
            }
            TagDialogMessage::ImportTaglist => {
                let Some(dir) = taglists_dir else {
                    log::error!("Cannot import taglist: taglist directory is not configured");
                    return ToolResult::None;
                };

                let file = rfd::FileDialog::new().add_filter("Taglist", &["toml"]).pick_file();
                let Some(src_path) = file else {
                    return ToolResult::None;
                };

                if let Err(err) = fs::create_dir_all(&dir) {
                    log::error!("Failed to create taglists directory {:?}: {}", dir, err);
                    return ToolResult::None;
                }

                let Some(file_name) = src_path.file_name() else {
                    log::error!("Invalid taglist file path (no filename): {:?}", src_path);
                    return ToolResult::None;
                };

                let dest_path = dir.join(file_name);
                match fs::copy(&src_path, &dest_path) {
                    Ok(_) => {
                        // Refresh available lists (on-demand)
                        dialog.available_taglists = get_available_taglists(Some(dir.as_path()));

                        // Ensure selected taglist still exists
                        let selected_id = dialog.selected_taglist.id.clone();
                        if !dialog.available_taglists.iter().any(|t| t.id.eq_ignore_ascii_case(&selected_id)) {
                            if let Some(first) = dialog.available_taglists.first().cloned() {
                                dialog.selected_taglist = first;
                            }
                        }

                        dialog.replacement_list = load_taglist(&dialog.selected_taglist.id, Some(dir.as_path()));
                        ToolResult::Redraw
                    }
                    Err(err) => {
                        log::error!("Failed to import taglist {:?} -> {:?}: {}", src_path, dest_path, err);
                        ToolResult::None
                    }
                }
            }
            TagDialogMessage::SetFilter(s) => {
                dialog.filter = s;
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
        use iced::widget::{button, column, container, mouse_area, text};
        use iced::Length;
        use iced::Theme;
        use icy_engine_gui::ui::TEXT_SIZE_NORMAL;

        let Some((tag_index, pos)) = self.context_menu else {
            return None;
        };

        let edit_btn = button(text(fl!("tag-toolbar-edit")).size(TEXT_SIZE_NORMAL))
            .padding([4, 12])
            .style(iced::widget::button::text)
            .on_press(ToolMessage::TagEdit(tag_index));

        let mut menu_items: Vec<Element<'_, ToolMessage>> = vec![edit_btn.into()];

        // Show "Delete X Selected" if multiple tags selected, otherwise just "Delete"
        if self.selection.len() > 1 {
            let delete_selected_btn = button(text(fl!("tag-toolbar-delete-selected", count = self.selection.len())).size(TEXT_SIZE_NORMAL))
                .padding([4, 12])
                .style(iced::widget::button::text)
                .on_press(ToolMessage::TagDeleteSelected);
            menu_items.push(delete_selected_btn.into());
        } else {
            let delete_btn = button(text(fl!("tag-toolbar-delete")).size(TEXT_SIZE_NORMAL))
                .padding([4, 12])
                .style(iced::widget::button::text)
                .on_press(ToolMessage::TagDelete(tag_index));
            menu_items.push(delete_btn.into());
        }

        let menu_content = container(column(menu_items).spacing(2).width(Length::Fixed(150.0)))
            .style(|theme: &Theme| container::Style {
                background: Some(iced::Background::Color(theme.secondary.base)),
                border: iced::Border {
                    color: theme.primary.divider,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
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
/// Persistent state is owned by the tool.
pub struct TagTool {
    state: TagToolState,
}

impl TagTool {
    pub fn new() -> Self {
        Self { state: TagToolState::new() }
    }

    pub fn state(&self) -> &TagToolState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut TagToolState {
        &mut self.state
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

        // Find bounding box of all tags (with 1px margin for outer border)
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for (x, y, w, h, _) in &overlay_rects {
            min_x = min_x.min(*x - 1.0);
            min_y = min_y.min(*y - 1.0);
            max_x = max_x.max(x + w + 1.0);
            max_y = max_y.max(y + h + 1.0);
        }

        // Clamp to non-negative
        min_x = min_x.max(0.0);
        min_y = min_y.max(0.0);

        let total_w = (max_x - min_x).ceil() as u32;
        let total_h = (max_y - min_y).ceil() as u32;

        if total_w == 0 || total_h == 0 {
            return (None, None);
        }

        // Create RGBA buffer for overlay
        let mut rgba = vec![0u8; (total_w * total_h * 4) as usize];

        // Helper to set a pixel
        let set_pixel = |rgba: &mut [u8], px: u32, py: u32, r: u8, g: u8, b: u8, a: u8| {
            if px < total_w && py < total_h {
                let idx = ((py * total_w + px) * 4) as usize;
                if idx + 3 < rgba.len() {
                    rgba[idx] = r;
                    rgba[idx + 1] = g;
                    rgba[idx + 2] = b;
                    rgba[idx + 3] = a;
                }
            }
        };

        for (x, y, w, h, is_selected) in &overlay_rects {
            let local_x = (*x - min_x) as i32;
            let local_y = (*y - min_y) as i32;
            let rect_w = *w as i32;
            let rect_h = *h as i32;

            // Colors: white/black for selected, light gray/black for non-selected
            let inner_color: (u8, u8, u8) = if *is_selected {
                (255, 255, 255) // White
            } else {
                (180, 180, 180) // Light gray
            };

            // Draw outer border (black) - 1px outside the rect
            for px in (local_x - 1).max(0)..(local_x + rect_w + 1).min(total_w as i32) {
                let px = px as u32;
                // Top outer
                if local_y > 0 {
                    set_pixel(&mut rgba, px, (local_y - 1) as u32, 0, 0, 0, 255);
                }
                // Bottom outer
                let bottom_outer = local_y + rect_h;
                if bottom_outer >= 0 && (bottom_outer as u32) < total_h {
                    set_pixel(&mut rgba, px, bottom_outer as u32, 0, 0, 0, 255);
                }
            }
            for py in local_y.max(0)..(local_y + rect_h).min(total_h as i32) {
                let py = py as u32;
                // Left outer
                if local_x > 0 {
                    set_pixel(&mut rgba, (local_x - 1) as u32, py, 0, 0, 0, 255);
                }
                // Right outer
                let right_outer = local_x + rect_w;
                if right_outer >= 0 && (right_outer as u32) < total_w {
                    set_pixel(&mut rgba, right_outer as u32, py, 0, 0, 0, 255);
                }
            }

            // Draw inner border (white or light gray) - on the rect edge
            for px in local_x.max(0)..(local_x + rect_w).min(total_w as i32) {
                let px = px as u32;
                // Top inner
                if local_y >= 0 && (local_y as u32) < total_h {
                    set_pixel(&mut rgba, px, local_y as u32, inner_color.0, inner_color.1, inner_color.2, 255);
                }
                // Bottom inner
                let bottom_inner = local_y + rect_h - 1;
                if bottom_inner >= 0 && (bottom_inner as u32) < total_h {
                    set_pixel(&mut rgba, px, bottom_inner as u32, inner_color.0, inner_color.1, inner_color.2, 255);
                }
            }
            for py in local_y.max(0)..(local_y + rect_h).min(total_h as i32) {
                let py = py as u32;
                // Left inner
                if local_x >= 0 && (local_x as u32) < total_w {
                    set_pixel(&mut rgba, local_x as u32, py, inner_color.0, inner_color.1, inner_color.2, 255);
                }
                // Right inner
                let right_inner = local_x + rect_w - 1;
                if right_inner >= 0 && (right_inner as u32) < total_w {
                    set_pixel(&mut rgba, right_inner as u32, py, inner_color.0, inner_color.1, inner_color.2, 255);
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
    fn id(&self) -> super::ToolId {
        super::ToolId::Tool(icy_engine_edit::tools::Tool::Tag)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn cancel_capture(&mut self) {
        self.state.cancel_capture();
    }

    fn handle_terminal_message(&mut self, ctx: &mut ToolContext, msg: &TerminalMessage) -> ToolResult {
        if let Some(options) = ctx.options.as_ref() {
            let guard = options.read();
            self.state.selected_taglist = guard.selected_taglist.clone();
            self.state.taglists_dir = crate::Settings::taglists_dir();
        }

        match msg {
            TerminalMessage::Press(evt) => {
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };
                Self::handle_mouse_down(ctx.state, &mut self.state, pos, evt.modifiers.clone(), evt.button)
            }
            TerminalMessage::Move(evt) | TerminalMessage::Drag(evt) => {
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };
                let cursor = Some(Self::cursor_for_state(&self.state));
                let result = Self::handle_mouse_drag(ctx.state, &mut self.state, pos);
                ToolResult::SetCursorIcon(cursor).and(result)
            }
            TerminalMessage::Release(evt) => {
                let pos = evt.text_position.unwrap_or_default();
                Self::handle_mouse_up(ctx.state, &mut self.state, pos)
            }
            _ => ToolResult::None,
        }
    }

    fn handle_event(&mut self, ctx: &mut ToolContext, event: &iced::Event) -> ToolResult {
        match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                use iced::keyboard::key::Named;
                match key {
                    iced::keyboard::Key::Named(Named::Escape) => {
                        // Cancel add-tag mode (undo the newly created tag)
                        if self.state.add_new_index.is_some() {
                            self.state.add_new_index = None;
                            self.state.selection_drag_active = false;
                            self.state.end_drag();
                            let _ = ctx.state.undo();
                            return ToolResult::EndCapture.and(ToolResult::Redraw);
                        }

                        // Cancel selection-drag rectangle
                        if self.state.selection_drag_active {
                            self.state.cancel_selection_drag();
                            return ToolResult::EndCapture.and(ToolResult::Redraw);
                        }
                        ToolResult::None
                    }
                    iced::keyboard::Key::Named(Named::Delete | Named::Backspace) => TagTool::handle_key(ctx.state, &mut self.state, key),
                    _ => ToolResult::None,
                }
            }
            _ => ToolResult::None,
        }
    }

    fn handle_message(&mut self, ctx: &mut ToolContext, msg: &ToolMessage) -> ToolResult {
        let (selected_taglist, taglists_dir) = ctx
            .options
            .as_ref()
            .map(|o| {
                let guard = o.read();
                (guard.selected_taglist.clone(), crate::Settings::taglists_dir())
            })
            .unwrap_or_default();

        // Keep taglist in sync with settings
        self.state.selected_taglist = selected_taglist.clone();
        self.state.taglists_dir = taglists_dir;

        match *msg {
            ToolMessage::TagEdit(index) => {
                self.state.open_edit_dialog_for_tag(ctx.state, index, &selected_taglist);
                ToolResult::Redraw
            }
            ToolMessage::TagDelete(index) => self.state.delete_tag(ctx.state, index),
            ToolMessage::TagClone(index) => self.state.clone_tag(ctx.state, index),
            ToolMessage::TagContextMenuClose => {
                self.state.close_context_menu();
                ToolResult::Redraw
            }
            ToolMessage::TagOpenList => {
                self.state.open_list_dialog(ctx.state);
                ToolResult::Redraw
            }
            ToolMessage::TagStartAdd => {
                if self.state.add_new_index.is_some() {
                    self.state.cancel_add_mode(ctx.state)
                } else {
                    self.state.start_add_mode(ctx.state)
                }
            }
            ToolMessage::TagEditSelected => {
                if self.state.selection.len() == 1 {
                    let idx = self.state.selection[0];
                    self.state.open_edit_dialog_for_tag(ctx.state, idx, &selected_taglist);
                    ToolResult::Redraw
                } else {
                    ToolResult::None
                }
            }
            ToolMessage::TagDeleteSelected => self.state.delete_selected_tags(ctx.state),
            _ => ToolResult::None,
        }
    }

    fn view_toolbar(&self, ctx: &super::ToolViewContext) -> Element<'_, ToolMessage> {
        let add_button = if ctx.tag_add_mode {
            button(text(fl!("tag-toolbar-add")).size(TEXT_SIZE_SMALL))
                .on_press(ToolMessage::TagStartAdd)
                .style(button::primary)
        } else {
            button(text(fl!("tag-toolbar-add")).size(TEXT_SIZE_SMALL))
                .on_press(ToolMessage::TagStartAdd)
                .style(button::secondary)
        };

        let tags_button = button(text(fl!("tag-toolbar-tags")).size(TEXT_SIZE_SMALL))
            .on_press(ToolMessage::TagOpenList)
            .style(button::secondary);

        let left_side = row![add_button, tags_button].spacing(SPACE_8).align_y(iced::Alignment::Center);

        let _has_selected_tags = ctx.tag_selection_count > 0;

        let middle: Element<'_, ToolMessage> = if ctx.tag_selection_count > 1 {
            text(fl!("tag-toolbar-selected-tags", count = ctx.tag_selection_count))
                .size(TEXT_SIZE_SMALL)
                .into()
        } else if let Some(tag_info) = ctx.selected_tag.clone() {
            let edit_button = button(text(fl!("tag-toolbar-edit")).size(TEXT_SIZE_SMALL))
                .on_press(ToolMessage::TagEditSelected)
                .style(button::secondary);

            let pos_text = text(format!("({}, {})", tag_info.position.x, tag_info.position.y)).size(TEXT_SIZE_SMALL);
            let replacement_text = if tag_info.replacement.is_empty() {
                text(fl!("tag-toolbar-no-replacement")).size(TEXT_SIZE_SMALL)
            } else {
                text(tag_info.replacement).size(TEXT_SIZE_SMALL)
            };

            row![edit_button, pos_text, replacement_text]
                .spacing(SPACE_8)
                .align_y(iced::Alignment::Center)
                .into()
        } else if ctx.tag_add_mode {
            text(fl!("tag-toolbar-add-hint")).size(TEXT_SIZE_SMALL).into()
        } else {
            text("").into()
        };

        row![
            Space::new().width(iced::Length::Fill),
            left_side,
            Space::new().width(iced::Length::Fixed(SPACE_16)),
            middle,
            Space::new().width(iced::Length::Fill),
        ]
        .spacing(SPACE_8)
        .align_y(iced::Alignment::Center)
        .into()
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
                // Check for double-click to open edit dialog
                /*if tag_state.double_click_detector.is_double_click(index) {
                    tag_state.pending_click = None;
                    // Open edit dialog for this tag
                    if let Some(tag) = state.get_buffer().tags.get(index).cloned() {
                        tag_state.dialog = Some(TagDialog::edit(&tag, index, &tag_state.selected_taglist, tag_state.taglists_dir.clone()));
                    }
                    return ToolResult::Redraw;
                }*/

                // Handle Ctrl+Click for multi-selection toggle
                if modifiers.ctrl || modifiers.meta {
                    if tag_state.selection.contains(&index) {
                        tag_state.selection.retain(|&i| i != index);
                    } else {
                        tag_state.selection.push(index);
                    }
                    return ToolResult::Redraw;
                }

                // Store pending click - don't start drag yet, wait for mouse move
                tag_state.pending_click = Some((index, tag_pos));
                tag_state.drag_start = pos;
                tag_state.drag_cur = pos;

                // Select the tag if not already selected
                if !tag_state.selection.contains(&index) {
                    tag_state.selection.clear();
                    tag_state.selection.push(index);
                    return ToolResult::StartCapture.and(ToolResult::Redraw);
                }
                return ToolResult::None;
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
        // Convert pending click to drag if mouse moved
        if let Some((index, tag_pos)) = tag_state.pending_click.take() {
            // Check if mouse moved enough to start drag (threshold of 1 cell)
            let delta = tag_state.drag_cur - tag_state.drag_start;
            if delta.x.abs() > 0 || delta.y.abs() > 0 {
                // Start actual drag operation
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
                    tag_state.drag_indices = vec![index];
                    tag_state.drag_start_positions = vec![tag_pos];
                }

                tag_state.drag_active = true;
                let desc = if tag_state.selection.len() > 1 {
                    format!("Move {} tags", tag_state.selection.len())
                } else {
                    "Move tag".to_string()
                };
                tag_state.drag_undo = Some(state.begin_atomic_undo(desc));
            } else {
                // Mouse didn't move enough, restore pending click
                tag_state.pending_click = Some((index, tag_pos));
                return ToolResult::None;
            }
        }

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

        // Handle pending click (mouse was pressed but not dragged)
        if let Some((_index, _tag_pos)) = tag_state.pending_click.take() {
            // Just a click, selection was already set in mouse_down
            return ToolResult::EndCapture.and(ToolResult::Redraw);
        }

        // Handle new tag placement completion
        if let Some(new_tag_index) = tag_state.add_new_index.take() {
            tag_state.end_drag();

            // Open edit dialog for the newly placed tag
            if let Some(tag) = state.get_buffer().tags.get(new_tag_index).cloned() {
                tag_state.dialog = Some(TagDialog::edit(
                    &tag,
                    new_tag_index,
                    &tag_state.selected_taglist,
                    tag_state.taglists_dir.clone(),
                ));
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
