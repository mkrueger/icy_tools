use icy_engine::{MouseButton, Position};
use icy_engine_edit::tools::layer_drag_delta;
use icy_engine_gui::TerminalMessage;
use icy_ui::keyboard::{key::Named, Key};

use super::{ToolContext, ToolHandler, ToolId, ToolResult};

#[derive(Default)]
pub struct MoveLayerTool {
    drag: Option<(Position, Position)>,
    target: Position,
    /// The undo stack length and layer after the last arrow-key nudge, so consecutive nudges
    /// share one undo step.
    nudge: Option<(usize, usize)>,
}

impl MoveLayerTool {
    fn cursor_at(&self, ctx: &ToolContext<'_>, position: Option<Position>) -> icy_ui::mouse::Interaction {
        use icy_ui::mouse::Interaction;
        if self.drag.is_some() {
            Interaction::Grabbing
        } else if !Self::over_layer(ctx, position) {
            Interaction::Idle
        } else if Self::movable(ctx) {
            Interaction::Grab
        } else {
            Interaction::NotAllowed
        }
    }

    fn over_layer(ctx: &ToolContext<'_>, position: Option<Position>) -> bool {
        position.is_some_and(|position| {
            ctx.state
                .get_current_layer()
                .ok()
                .and_then(|index| ctx.state.layer_move_bounds(index))
                .is_some_and(|bounds| bounds.contains_pt(position))
        })
    }

    fn movable(ctx: &ToolContext<'_>) -> bool {
        ctx.state.get_current_layer().is_ok_and(|index| ctx.state.can_move_layer(index))
    }

    fn cancel(&mut self, ctx: &mut ToolContext<'_>) -> ToolResult {
        ctx.state.set_layer_preview_offset(None);
        self.drag = None;
        ToolResult::EndCapture.and(ToolResult::Redraw).and(ToolResult::SetCursorIcon(None))
    }
}

impl ToolHandler for MoveLayerTool {
    fn id(&self) -> ToolId {
        ToolId::MoveLayer
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn cancel_capture(&mut self) {
        self.drag = None;
    }

    fn cursor(&self) -> icy_ui::mouse::Interaction {
        if self.drag.is_some() {
            icy_ui::mouse::Interaction::Grabbing
        } else {
            icy_ui::mouse::Interaction::Idle
        }
    }

    fn show_caret(&self) -> bool {
        false
    }

    fn handle_terminal_message(&mut self, ctx: &mut ToolContext<'_>, msg: &TerminalMessage) -> ToolResult {
        match msg {
            TerminalMessage::Move(evt) => {
                return ToolResult::SetCursorIcon(Some(self.cursor_at(ctx, evt.text_position)));
            }
            TerminalMessage::Press(evt) if evt.button == MouseButton::Left => {
                if !Self::over_layer(ctx, evt.text_position) {
                    return ToolResult::None;
                }
                if !Self::movable(ctx) {
                    let message = crate::fl!("tool-move-layer-locked");
                    log::warn!("{message}");
                    return ToolResult::Status(message);
                }
                if let (Some(position), Some(layer)) = (evt.text_position, ctx.state.get_cur_layer()) {
                    self.target = layer.offset();
                    self.drag = Some((position, self.target));
                    return ToolResult::StartCapture
                        .and(ToolResult::Redraw)
                        .and(ToolResult::SetCursorIcon(Some(icy_ui::mouse::Interaction::Grabbing)));
                }
            }
            TerminalMessage::Drag(evt) => {
                if let (Some((start, offset)), Some(position)) = (self.drag, evt.text_position) {
                    self.target = offset + layer_drag_delta(start, position, evt.modifiers.shift);
                    ctx.state.set_layer_preview_offset(Some(self.target));
                    return ToolResult::Redraw.and(ToolResult::SetCursorIcon(Some(icy_ui::mouse::Interaction::Grabbing)));
                }
            }
            TerminalMessage::Release(evt) => {
                if let Some((start, offset)) = self.drag.take() {
                    if let Some(position) = evt.text_position {
                        self.target = offset + layer_drag_delta(start, position, evt.modifiers.shift);
                    }
                    ctx.state.set_layer_preview_offset(None);
                    if self.target != offset {
                        if let Err(error) = ctx.state.move_layer(self.target) {
                            log::error!("Could not move layer: {error}");
                            return ToolResult::EndCapture.and(ToolResult::Status(error.to_string()));
                        }
                    }
                    return ToolResult::EndCapture
                        .and(ToolResult::Redraw)
                        .and(ToolResult::SetCursorIcon(Some(self.cursor_at(ctx, evt.text_position))));
                }
            }
            _ => {}
        }
        ToolResult::None
    }

    fn handle_event(&mut self, ctx: &mut ToolContext<'_>, event: &icy_ui::Event) -> ToolResult {
        let icy_ui::Event::Keyboard(icy_ui::keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
            return ToolResult::None;
        };
        if *key == Key::Named(Named::Escape) {
            return self.cancel(ctx);
        }
        if self.drag.is_some() || modifiers.alt() || modifiers.control() || modifiers.logo() {
            return ToolResult::None;
        }
        let step = if modifiers.shift() { 10 } else { 1 };
        let delta = match key {
            Key::Named(Named::ArrowLeft) => Position::new(-step, 0),
            Key::Named(Named::ArrowRight) => Position::new(step, 0),
            Key::Named(Named::ArrowUp) => Position::new(0, -step),
            Key::Named(Named::ArrowDown) => Position::new(0, step),
            _ => return ToolResult::None,
        };
        if !Self::movable(ctx) {
            let message = crate::fl!("tool-move-layer-locked");
            log::warn!("{message}");
            return ToolResult::Status(message);
        }
        let previous = self.nudge.take();
        let Ok(index) = ctx.state.get_current_layer() else {
            return ToolResult::None;
        };
        let merge = previous == Some((ctx.state.undo_stack_len(), index));
        if let Err(error) = ctx.state.nudge_layer(delta, merge) {
            log::error!("Could not move layer: {error}");
            return ToolResult::Status(error.to_string());
        }
        self.nudge = Some((ctx.state.undo_stack_len(), index));
        ToolResult::Redraw
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::{KeyModifiers, Size, TextBuffer};
    use icy_engine_edit::{EditState, UndoState};
    use icy_engine_gui::terminal::TerminalMouseEvent;

    #[test]
    fn move_layer_tool_previews_cancels_and_commits_in_legacy_editor() {
        let mut state = EditState::from_buffer(TextBuffer::new(Size::new(20, 10)));
        let mut undo = None;
        let mut ctx = ToolContext {
            state: &mut state,
            options: None,
            undo_guard: &mut undo,
            half_block_mapper: None,
        };
        let mut tool = MoveLayerTool::default();
        assert_eq!(tool.cursor_at(&ctx, Some(Position::new(0, 0))), icy_ui::mouse::Interaction::Grab);
        assert_eq!(tool.cursor_at(&ctx, Some(Position::new(20, 0))), icy_ui::mouse::Interaction::Idle);
        assert_eq!(tool.cursor_at(&ctx, None), icy_ui::mouse::Interaction::Idle);
        let mouse = |position| TerminalMouseEvent::new((0.0, 0.0), Some(position), MouseButton::Left, KeyModifiers::default());
        tool.handle_terminal_message(&mut ctx, &TerminalMessage::Press(mouse(Position::new(25, 5))));
        assert!(tool.drag.is_none(), "presses outside the layer do not start a drag");
        tool.handle_terminal_message(&mut ctx, &TerminalMessage::Press(mouse(Position::new(5, 5))));
        tool.handle_terminal_message(&mut ctx, &TerminalMessage::Drag(mouse(Position::new(1, 2))));
        assert_eq!(tool.cursor_at(&ctx, None), icy_ui::mouse::Interaction::Grabbing);
        assert_eq!(ctx.state.get_cur_layer().unwrap().offset(), Position::new(-4, -3));
        assert_eq!(ctx.state.get_cur_layer().unwrap().base_offset(), Position::default());
        tool.cancel(&mut ctx);
        assert_eq!(ctx.state.get_cur_layer().unwrap().offset(), Position::default());
        assert_eq!(ctx.state.undo_stack_len(), 0);
        tool.handle_terminal_message(&mut ctx, &TerminalMessage::Press(mouse(Position::new(5, 5))));
        tool.handle_terminal_message(&mut ctx, &TerminalMessage::Release(mouse(Position::new(1, 2))));
        assert_eq!(ctx.state.get_cur_layer().unwrap().offset(), Position::new(-4, -3));
        assert_eq!(ctx.state.undo_stack_len(), 1);
        ctx.state.undo().unwrap();
        assert_eq!(ctx.state.get_cur_layer().unwrap().offset(), Position::default());
        ctx.state.get_cur_layer_mut().unwrap().properties.is_position_locked = true;
        assert_eq!(tool.cursor_at(&ctx, Some(Position::new(0, 0))), icy_ui::mouse::Interaction::NotAllowed);
        assert_eq!(tool.cursor_at(&ctx, Some(Position::new(20, 0))), icy_ui::mouse::Interaction::Idle);
        tool.handle_terminal_message(&mut ctx, &TerminalMessage::Press(mouse(Position::new(5, 5))));
        assert!(tool.drag.is_none());
    }

    #[test]
    fn move_layer_tool_merges_consecutive_nudges_in_legacy_editor() {
        use icy_ui::keyboard::Modifiers;
        let mut state = EditState::from_buffer(TextBuffer::new(Size::new(20, 10)));
        let mut undo = None;
        let mut ctx = ToolContext {
            state: &mut state,
            options: None,
            undo_guard: &mut undo,
            half_block_mapper: None,
        };
        let mut tool = MoveLayerTool::default();
        let key = |named, modifiers| {
            icy_ui::Event::Keyboard(icy_ui::keyboard::Event::KeyPressed {
                key: Key::Named(named),
                modified_key: Key::Named(named),
                physical_key: icy_ui::keyboard::key::Physical::Unidentified(icy_ui::keyboard::key::NativeCode::Unidentified),
                location: icy_ui::keyboard::Location::Standard,
                modifiers,
                text: None,
                repeat: false,
            })
        };
        for _ in 0..3 {
            tool.handle_event(&mut ctx, &key(Named::ArrowRight, Modifiers::empty()));
        }
        tool.handle_event(&mut ctx, &key(Named::ArrowDown, Modifiers::SHIFT));
        assert_eq!(ctx.state.get_cur_layer().unwrap().offset(), Position::new(3, 10));
        assert_eq!(ctx.state.undo_stack_len(), 1);
        ctx.state.undo().unwrap();
        assert_eq!(ctx.state.get_cur_layer().unwrap().offset(), Position::default());
    }
}
