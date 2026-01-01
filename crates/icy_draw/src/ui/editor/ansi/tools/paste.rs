//! Paste Tool Handler
//!
//! Handles the paste mode where a floating layer can be positioned before anchoring.
//! This tool is automatically activated when content is pasted and handles:
//! - Mouse drag to move the floating layer
//! - Keyboard shortcuts for layer manipulation (rotate, flip, stamp, anchor, cancel)

use super::{ToolContext, ToolHandler, ToolMessage, ToolResult};
use crate::fl;
use icy_ui::widget::{button, row, text, tooltip, Space};
use icy_ui::{Element, Length, Theme};
use icy_engine::{MouseButton, Position, Sixel};
use icy_engine_edit::tools::Tool;
use icy_engine_edit::AtomicUndoGuard;
use icy_engine_edit::EditState;
use icy_engine_gui::TerminalMessage;

/// State for the paste/floating layer tool
#[derive(Default)]
pub struct PasteTool {
    /// Whether paste mode is active (floating layer exists)
    active: bool,
    /// Tool that was active before paste mode started
    previous_tool: Option<super::ToolId>,

    /// Whether a drag is currently active
    drag_active: bool,
    /// Layer offset when drag started
    drag_start_offset: Position,
    /// Mouse position when drag started
    drag_start_pos: Position,
    /// Current mouse position during drag
    drag_cur_pos: Position,

    /// Atomic undo guard for moving pasted layer during drag
    move_undo: Option<AtomicUndoGuard>,

    /// Atomic undo guard for the entire paste operation (paste + all manipulations).
    /// This groups paste, move, rotate, flip, etc. into a single undo action.
    paste_undo: Option<AtomicUndoGuard>,
}

impl PasteTool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if paste is potentially available
    /// Note: With async clipboard API, this is always true since we can't
    /// synchronously check clipboard contents. The actual availability
    /// is determined when the clipboard is read.
    pub fn can_paste(&self) -> bool {
        // Always return true - the async paste operation will handle
        // cases where no compatible content is available
        true
    }

    /// Set paste mode active (used when paste data is received asynchronously)
    pub fn set_active(&mut self, previous_tool: super::ToolId) {
        self.active = true;
        self.previous_tool = Some(previous_tool);
        self.abort();
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    fn finish_paste(&mut self) -> super::ToolId {
        self.active = false;
        self.abort();
        // End the atomic undo group - all paste operations are now a single undo action
        self.paste_undo = None;
        self.previous_tool.take().unwrap_or(super::ToolId::Tool(Tool::Click))
    }

    /// Check if a drag is currently active
    pub fn is_drag_active(&self) -> bool {
        self.drag_active
    }

    pub fn perform_action(&mut self, state: &mut EditState, action: PasteAction) -> ToolResult {
        if !self.active {
            return ToolResult::None;
        }

        match action {
            PasteAction::None => ToolResult::None,

            PasteAction::Move(dx, dy) => {
                let current_offset = state.get_cur_layer().map(|l| l.offset()).unwrap_or_default();
                let new_pos = Position::new(current_offset.x + dx, current_offset.y + dy);
                if let Err(e) = state.move_layer(new_pos) {
                    log::warn!("Failed to move layer: {}", e);
                }
                ToolResult::UpdateLayerBounds
                    .and(ToolResult::CollabOperation(new_pos.x, new_pos.y))
                    .and(ToolResult::Redraw)
            }

            PasteAction::Stamp => {
                if let Err(e) = state.stamp_layer_down() {
                    log::warn!("Failed to stamp layer: {}", e);
                }
                ToolResult::Commit("Stamp floating layer".to_string()).and(ToolResult::Redraw)
            }

            PasteAction::Rotate => {
                if let Err(e) = state.paste_rotate() {
                    log::warn!("Failed to rotate layer: {}", e);
                }
                ToolResult::UpdateLayerBounds
                    .and(ToolResult::Commit("Rotate floating layer".to_string()))
                    .and(ToolResult::Redraw)
            }

            PasteAction::FlipX => {
                if let Err(e) = state.paste_flip_x() {
                    log::warn!("Failed to flip layer X: {}", e);
                }
                ToolResult::UpdateLayerBounds
                    .and(ToolResult::Commit("Flip floating layer X".to_string()))
                    .and(ToolResult::Redraw)
            }

            PasteAction::FlipY => {
                if let Err(e) = state.paste_flip_y() {
                    log::warn!("Failed to flip layer Y: {}", e);
                }
                ToolResult::UpdateLayerBounds
                    .and(ToolResult::Commit("Flip floating layer Y".to_string()))
                    .and(ToolResult::Redraw)
            }

            PasteAction::ToggleTransparent => {
                if let Err(e) = state.make_layer_transparent() {
                    log::warn!("Failed to make layer transparent: {}", e);
                }
                ToolResult::Commit("Toggle floating layer transparent".to_string()).and(ToolResult::Redraw)
            }

            PasteAction::Anchor => {
                // If user anchors while a paste drag is active, first commit the drag position.
                if let Some(new_offset) = self.finish_pending_move() {
                    state.set_layer_preview_offset(None);
                    let _ = state.move_layer(new_offset);
                }

                // Use paste_anchor which handles both local anchor AND collaboration sync
                if let Err(e) = state.paste_anchor() {
                    log::error!("Failed to anchor layer: {}", e);
                }

                let prev = self.finish_paste();
                let results = vec![
                    ToolResult::EndCapture,
                    ToolResult::SetCursorIcon(None),
                    ToolResult::UpdateLayerBounds,
                    ToolResult::Commit("Anchor floating layer".to_string()),
                    ToolResult::SwitchTool(prev),
                    ToolResult::Redraw,
                ];

                ToolResult::Multi(results)
            }

            PasteAction::KeepAsLayer => {
                // Keep the paste as a separate layer, just exit paste mode
                // If there's a pending move, commit it first
                if let Some(new_offset) = self.finish_pending_move() {
                    state.set_layer_preview_offset(None);
                    let _ = state.move_layer(new_offset);
                }

                // Convert the floating layer to a normal layer by pushing the AddFloatingLayer undo operation
                // This changes the role from PasteImage/PastePreview to Image/Normal
                if let Err(e) = state.add_floating_layer() {
                    log::warn!("Failed to finalize floating layer: {}", e);
                }

                let prev = self.finish_paste();
                ToolResult::Multi(vec![
                    ToolResult::EndCapture,
                    ToolResult::SetCursorIcon(None),
                    ToolResult::UpdateLayerBounds,
                    ToolResult::Commit("Keep as layer".to_string()),
                    ToolResult::SwitchTool(prev),
                    ToolResult::Redraw,
                ])
            }

            PasteAction::Discard => {
                // Cancel any active paste drag.
                self.abort();
                state.set_layer_preview_offset(None);

                // Discard AND undo all operations in the atomic group.
                // This properly reverts all paste operations (layer insertion, moves, rotations, etc.)
                // by executing their undo functions, not just removing them from the stack.
                if let Some(ref mut guard) = self.paste_undo {
                    guard.discard_and_undo(state);
                }

                self.active = false;
                self.paste_undo = None;
                let prev = self.previous_tool.take().unwrap_or(super::ToolId::Tool(Tool::Click));

                ToolResult::Multi(vec![
                    ToolResult::EndCapture,
                    ToolResult::SetCursorIcon(None),
                    ToolResult::UpdateLayerBounds,
                    ToolResult::SwitchTool(prev),
                    ToolResult::Redraw,
                ])
            }
        }
    }

    /// Get the current drag offset (delta from start)
    pub fn drag_offset(&self) -> Position {
        if self.drag_active {
            Position::new(self.drag_cur_pos.x - self.drag_start_pos.x, self.drag_cur_pos.y - self.drag_start_pos.y)
        } else {
            Position::default()
        }
    }

    /// Get the target layer offset based on current drag state
    pub fn target_layer_offset(&self) -> Position {
        if self.drag_active {
            let delta = self.drag_offset();
            Position::new(self.drag_start_offset.x + delta.x, self.drag_start_offset.y + delta.y)
        } else {
            self.drag_start_offset
        }
    }

    /// Start a drag operation
    pub fn start_drag(&mut self, start_pos: Position, layer_offset: Position) {
        self.drag_active = true;
        self.drag_start_pos = start_pos;
        self.drag_cur_pos = start_pos;
        self.drag_start_offset = layer_offset;
    }

    /// Update the current drag position
    pub fn update_drag(&mut self, pos: Position) {
        if self.drag_active {
            self.drag_cur_pos = pos;
        }
    }

    /// End the drag operation and return the final offset
    pub fn end_drag(&mut self) -> Position {
        let offset = self.target_layer_offset();
        self.drag_active = false;
        offset
    }

    /// Cancel the drag operation (returns to original position)
    pub fn cancel_drag(&mut self) {
        self.drag_active = false;
    }

    /// Finish an in-progress drag (if any) and return the final target offset.
    /// Also clears the atomic undo guard used for the move.
    pub fn finish_pending_move(&mut self) -> Option<Position> {
        if self.drag_active {
            let offset = self.end_drag();
            self.move_undo = None;
            Some(offset)
        } else {
            None
        }
    }

    /// Abort any in-progress drag/move without committing.
    pub fn abort(&mut self) {
        self.drag_active = false;
        self.move_undo = None;
    }
}

/// Result of paste tool keyboard handling
#[derive(Clone, Debug, PartialEq)]
pub enum PasteAction {
    /// No action taken
    None,
    /// Anchor the floating layer (merge with layer below)
    Anchor,
    /// Keep as separate layer and exit paste mode
    KeepAsLayer,
    /// Discard the floating layer (cancel paste)
    Discard,
    /// Move layer by delta
    Move(i32, i32),
    /// Stamp layer down (copy to layer below without anchoring)
    Stamp,
    /// Rotate layer 90° clockwise
    Rotate,
    /// Flip layer horizontally
    FlipX,
    /// Flip layer vertically
    FlipY,
    /// Toggle transparent mode
    ToggleTransparent,
}

impl PasteTool {
    /// Handle a keyboard event in paste mode
    /// Returns the action to perform
    pub fn handle_key(&self, key: &icy_ui::keyboard::Key) -> PasteAction {
        use icy_ui::keyboard::key::Named;
        use icy_ui::keyboard::Key;

        match key {
            // Escape - cancel paste
            Key::Named(Named::Escape) => PasteAction::Discard,
            // Enter - anchor layer
            Key::Named(Named::Enter) => PasteAction::Anchor,
            // Arrow keys - move floating layer
            Key::Named(Named::ArrowUp) => PasteAction::Move(0, -1),
            Key::Named(Named::ArrowDown) => PasteAction::Move(0, 1),
            Key::Named(Named::ArrowLeft) => PasteAction::Move(-1, 0),
            Key::Named(Named::ArrowRight) => PasteAction::Move(1, 0),
            // S - stamp
            Key::Character(c) if c.eq_ignore_ascii_case("s") => PasteAction::Stamp,
            // R - rotate
            Key::Character(c) if c.eq_ignore_ascii_case("r") => PasteAction::Rotate,
            // X - flip horizontal
            Key::Character(c) if c.eq_ignore_ascii_case("x") => PasteAction::FlipX,
            // Y - flip vertical
            Key::Character(c) if c.eq_ignore_ascii_case("y") => PasteAction::FlipY,
            // T - toggle transparent
            Key::Character(c) if c.eq_ignore_ascii_case("t") => PasteAction::ToggleTransparent,
            _ => PasteAction::None,
        }
    }
}

impl ToolHandler for PasteTool {
    fn id(&self) -> super::ToolId {
        super::ToolId::Paste
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn cancel_capture(&mut self) {
        self.abort();
    }

    fn handle_message(&mut self, ctx: &mut ToolContext<'_>, msg: &ToolMessage) -> ToolResult {
        if !self.active {
            return ToolResult::None;
        }

        let action = match *msg {
            ToolMessage::PasteStamp => PasteAction::Stamp,
            ToolMessage::PasteRotate => PasteAction::Rotate,
            ToolMessage::PasteFlipX => PasteAction::FlipX,
            ToolMessage::PasteFlipY => PasteAction::FlipY,
            ToolMessage::PasteToggleTransparent => PasteAction::ToggleTransparent,
            ToolMessage::PasteAnchor => PasteAction::Anchor,
            ToolMessage::PasteCancel => PasteAction::Discard,
            _ => PasteAction::None,
        };

        if matches!(action, PasteAction::None) {
            return ToolResult::None;
        }

        self.perform_action(ctx.state, action)
    }

    fn view_toolbar(&self, _ctx: &super::ToolViewContext) -> Element<'static, ToolMessage> {
        if !self.active {
            return row![].into();
        }

        // Icon button style: transparent background, secondary color on normal, base color on hover
        fn icon_btn_style(theme: &Theme, status: button::Status) -> button::Style {
            button::Style {
                background: None,
                text_color: match status {
                    button::Status::Hovered | button::Status::Pressed => theme.background.on,
                    _ => theme.button.on,
                },
                border: icy_ui::Border::default(),
                shadow: icy_ui::Shadow::default(),
                snap: true,
                ..Default::default()
            }
        }

        fn paste_icon_btn(icon: &'static str, tooltip_text: String, msg: ToolMessage) -> Element<'static, ToolMessage> {
            tooltip(
                button(text(icon).size(16)).padding([4, 8]).on_press(msg).style(icon_btn_style),
                text(tooltip_text),
                tooltip::Position::Bottom,
            )
            .into()
        }

        let hint_text = fl!("paste-tool-hint");
        let content = row![
            paste_icon_btn("⌗", fl!("paste-tool-stamp"), ToolMessage::PasteStamp),
            paste_icon_btn("↻", fl!("paste-tool-rotate"), ToolMessage::PasteRotate),
            paste_icon_btn("⇆", fl!("paste-tool-flip-x"), ToolMessage::PasteFlipX),
            paste_icon_btn("⇅", fl!("paste-tool-flip-y"), ToolMessage::PasteFlipY),
            paste_icon_btn("◐", fl!("paste-tool-transparent"), ToolMessage::PasteToggleTransparent),
            Space::new().width(Length::Fill),
            text(hint_text).size(12).style(|theme: &Theme| text::Style { color: Some(theme.button.on) }),
        ]
        .spacing(2)
        .height(Length::Fill)
        .align_y(icy_ui::Alignment::Center);

        // Center vertically; AnsiEditor wraps this into the rounded container.
        row![Space::new().width(Length::Fill), content, Space::new().width(Length::Fill)]
            .align_y(icy_ui::Alignment::Center)
            .into()
    }

    fn handle_terminal_message(&mut self, ctx: &mut ToolContext, msg: &TerminalMessage) -> ToolResult {
        if !self.active {
            return ToolResult::None;
        }

        match msg {
            TerminalMessage::Press(evt) => {
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };
                if evt.button == MouseButton::Left {
                    // Get current layer offset
                    let layer_offset = ctx.state.get_cur_layer().map(|l| l.offset()).unwrap_or_default();
                    self.start_drag(pos, layer_offset);

                    if self.move_undo.is_none() {
                        self.move_undo = Some(ctx.state.begin_atomic_undo("Move pasted layer".to_string()));
                    }
                    ToolResult::StartCapture.and(ToolResult::SetCursorIcon(Some(icy_ui::mouse::Interaction::Grabbing)))
                } else {
                    ToolResult::None
                }
            }
            TerminalMessage::Drag(evt) => {
                if let Some(pos) = evt.text_position {
                    if self.drag_active {
                        self.update_drag(pos);
                        // Set preview offset for visual feedback
                        let new_offset = self.target_layer_offset();
                        ctx.state.set_layer_preview_offset(Some(new_offset));
                        return ToolResult::SetCursorIcon(Some(icy_ui::mouse::Interaction::Grabbing))
                            .and(ToolResult::UpdateLayerBounds)
                            .and(ToolResult::CollabOperation(new_offset.x, new_offset.y))
                            .and(ToolResult::Redraw);
                    }
                }
                ToolResult::None
            }
            TerminalMessage::Move(_evt) => {
                if self.drag_active {
                    ToolResult::SetCursorIcon(Some(icy_ui::mouse::Interaction::Grabbing))
                } else {
                    ToolResult::SetCursorIcon(Some(icy_ui::mouse::Interaction::Grab))
                }
            }
            TerminalMessage::Release(_evt) => {
                if self.drag_active {
                    let final_offset = self.end_drag();
                    // Clear preview and commit the actual move
                    ctx.state.set_layer_preview_offset(None);
                    let _ = ctx.state.move_layer(final_offset);

                    // Dropping the guard groups everything into one undo entry.
                    self.move_undo = None;
                    ToolResult::Multi(vec![
                        ToolResult::EndCapture,
                        ToolResult::SetCursorIcon(Some(icy_ui::mouse::Interaction::Grab)),
                        ToolResult::UpdateLayerBounds,
                        ToolResult::CollabOperation(final_offset.x, final_offset.y),
                        ToolResult::Commit("Move pasted layer".to_string()),
                    ])
                } else {
                    ToolResult::None
                }
            }
            _ => ToolResult::None,
        }
    }

    fn handle_event(&mut self, ctx: &mut ToolContext, event: &icy_ui::Event) -> ToolResult {
        if !self.active {
            return ToolResult::None;
        }

        match event {
            icy_ui::Event::Keyboard(icy_ui::keyboard::Event::KeyPressed { key, .. }) => {
                let action: PasteAction = self.handle_key(key);
                self.perform_action(ctx.state, action)
            }
            _ => ToolResult::None,
        }
    }
}
