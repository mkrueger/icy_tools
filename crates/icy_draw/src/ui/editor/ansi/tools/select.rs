//! Select Tool
//!
//! Rectangle selection with move/resize support.
//! Supports add (Shift), remove (Ctrl), and replace modes.

use iced::Element;
use iced::widget::{Space, row, text};
use icy_engine::{AddType, Position, Rectangle, Selection, TextPane};
use icy_engine_gui::TerminalMessage;
use icy_engine_gui::terminal::crt_state::{is_command_pressed, is_ctrl_pressed, is_shift_pressed};

use super::{ToolContext, ToolHandler, ToolId, ToolMessage, ToolResult, ToolViewContext};
use crate::ui::editor::ansi::selection_drag::{DragParameters, SelectionDrag, compute_dragged_selection, hit_test_selection};
use crate::ui::editor::ansi::widget::segmented_control::gpu::{Segment, SegmentedControlMessage, ShaderSegmentedControl};
use crate::ui::editor::ansi::widget::toolbar::top::{SelectionMode, SelectionModifier};
use icy_engine_edit::AtomicUndoGuard;
use icy_engine_edit::tools::Tool;

/// Select tool state
pub struct SelectTool {
    selection_mode: SelectionMode,
    selection_mode_control: ShaderSegmentedControl,
    /// Current drag mode
    drag_mode: SelectionDrag,
    hover_drag: SelectionDrag,
    /// Start position of drag
    start_pos: Option<Position>,
    /// Current position during drag
    current_pos: Option<Position>,
    /// Selection at start of resize operation
    start_selection: Option<Rectangle>,
    /// Whether currently dragging
    is_dragging: bool,
    /// Current modifier (from keyboard)
    modifier: SelectionModifier,
    /// Atomic undo guard for selection drag operations
    selection_undo: Option<AtomicUndoGuard>,
}

impl Default for SelectTool {
    fn default() -> Self {
        Self {
            selection_mode: SelectionMode::default(),
            selection_mode_control: ShaderSegmentedControl::new(),
            drag_mode: SelectionDrag::default(),
            hover_drag: SelectionDrag::default(),
            start_pos: None,
            current_pos: None,
            start_selection: None,
            is_dragging: false,
            modifier: SelectionModifier::default(),
            selection_undo: None,
        }
    }
}

impl SelectTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        self.selection_mode = mode;
    }

    fn cancel_drag(&mut self) {
        self.drag_mode = SelectionDrag::None;
        self.hover_drag = SelectionDrag::None;
        self.start_pos = None;
        self.current_pos = None;
        self.start_selection = None;
        self.is_dragging = false;
        self.selection_undo = None;
    }

    fn selection_add_type(&self) -> AddType {
        match self.modifier {
            SelectionModifier::Replace => AddType::Default,
            SelectionModifier::Add => AddType::Add,
            SelectionModifier::Remove => AddType::Subtract,
        }
    }

    fn get_char_at(ctx: &ToolContext, pos: Position) -> icy_engine::AttributedChar {
        if let Some(layer) = ctx.state.get_cur_layer() {
            layer.char_at(pos - layer.offset())
        } else {
            icy_engine::AttributedChar::invisible()
        }
    }
}

impl ToolHandler for SelectTool {
    fn id(&self) -> ToolId {
        ToolId::Tool(Tool::Select)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn cancel_capture(&mut self) {
        self.cancel_drag();
    }

    fn handle_message(&mut self, ctx: &mut ToolContext<'_>, msg: &ToolMessage) -> ToolResult {
        match *msg {
            ToolMessage::SelectSetMode(mode) => {
                self.selection_mode = mode;
                ToolResult::None
            }
            ToolMessage::SelectAll => {
                let buf = ctx.state.get_buffer();
                let rect = Rectangle::from(0, 0, buf.width(), buf.height());
                let _ = ctx.state.set_selection(rect);
                ToolResult::Redraw
            }
            ToolMessage::SelectNone => {
                let _ = ctx.state.clear_selection();
                ToolResult::Redraw
            }
            ToolMessage::SelectInvert => {
                let _ = ctx.state.inverse_selection();
                ToolResult::Redraw
            }
            _ => ToolResult::None,
        }
    }
    fn handle_terminal_message(&mut self, ctx: &mut ToolContext, msg: &TerminalMessage) -> ToolResult {
        match msg {
            TerminalMessage::Move(evt) => {
                if self.is_dragging {
                    return ToolResult::None;
                }
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };
                self.hover_drag = hit_test_selection(ctx.state.selection(), pos);
                ToolResult::None
            }

            TerminalMessage::Press(evt) => {
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };

                // Determine modifier from *global* keyboard state (event modifiers can be stale).
                self.modifier = if is_shift_pressed() {
                    SelectionModifier::Add
                } else if is_ctrl_pressed() || is_command_pressed() {
                    SelectionModifier::Remove
                } else {
                    SelectionModifier::Replace
                };

                // Non-rect selection modes: apply immediately on click.
                if !matches!(self.selection_mode, SelectionMode::Normal) {
                    let cur_ch = Self::get_char_at(ctx, pos);
                    match self.selection_mode {
                        SelectionMode::Character => {
                            ctx.state.enumerate_selections(|_, ch, _| self.modifier.get_response(ch.ch == cur_ch.ch));
                        }
                        SelectionMode::Attribute => {
                            ctx.state
                                .enumerate_selections(|_, ch, _| self.modifier.get_response(ch.attribute == cur_ch.attribute));
                        }
                        SelectionMode::Foreground => {
                            ctx.state
                                .enumerate_selections(|_, ch, _| self.modifier.get_response(ch.attribute.foreground() == cur_ch.attribute.foreground()));
                        }
                        SelectionMode::Background => {
                            ctx.state
                                .enumerate_selections(|_, ch, _| self.modifier.get_response(ch.attribute.background() == cur_ch.attribute.background()));
                        }
                        SelectionMode::Normal => {}
                    }
                    return ToolResult::Redraw;
                }

                // Get current selection
                let current_selection = ctx.state.selection();
                let current_rect = current_selection.map(|s| s.as_rectangle());

                // In Add/Remove mode, commit current selection to mask and start fresh
                if self.modifier != SelectionModifier::Replace {
                    let _ = ctx.state.add_selection_to_mask();
                    let _ = ctx.state.deselect();
                    self.drag_mode = SelectionDrag::Create;
                    self.start_selection = None;
                } else {
                    // Replace mode - check for move/resize of existing selection
                    let _ = ctx.state.clear_selection_mask();
                    let hit = hit_test_selection(current_selection, pos);
                    if hit != SelectionDrag::None {
                        self.drag_mode = hit;
                        self.start_selection = current_rect;
                    } else {
                        self.drag_mode = SelectionDrag::Create;
                        self.start_selection = None;
                        let _ = ctx.state.clear_selection();
                    }
                }

                self.start_pos = Some(pos);
                self.current_pos = Some(pos);
                self.is_dragging = true;
                self.hover_drag = SelectionDrag::None;
                self.selection_undo = Some(ctx.state.begin_atomic_undo("Selection"));

                ToolResult::StartCapture.and(ToolResult::Redraw)
            }

            TerminalMessage::Drag(evt) => {
                if let Some(pos) = evt.text_position {
                    if self.is_dragging {
                        self.current_pos = Some(pos);

                        let add_type = self.selection_add_type();

                        if self.drag_mode == SelectionDrag::Create {
                            if let Some(start) = self.start_pos {
                                let selection = Selection {
                                    anchor: start,
                                    lead: pos,
                                    locked: false,
                                    shape: icy_engine::Shape::Rectangle,
                                    add_type,
                                };
                                let _ = ctx.state.set_selection(selection);
                            }
                            return ToolResult::Redraw;
                        }

                        if let (Some(start_rect), Some(start_pos)) = (self.start_selection, self.start_pos) {
                            let params = DragParameters {
                                start_rect,
                                start_pos,
                                cur_pos: pos,
                            };
                            if let Some(new_rect) = compute_dragged_selection(self.drag_mode, params) {
                                let mut selection = Selection::from(new_rect);
                                selection.add_type = add_type;
                                let _ = ctx.state.set_selection(selection);
                            }
                        }
                        return ToolResult::Redraw;
                    }
                }
                ToolResult::None
            }

            TerminalMessage::Release(evt) => {
                if self.is_dragging {
                    if let Some(pos) = evt.text_position {
                        self.current_pos = Some(pos);
                    }
                    self.is_dragging = false;

                    let end_pos = evt.text_position;
                    let add_type = self.selection_add_type();

                    // If it's just a click (start == end) in Create mode, keep mask intact but clear active selection.
                    if self.drag_mode == SelectionDrag::Create {
                        if let (Some(start), Some(end)) = (self.start_pos, end_pos) {
                            if start == end {
                                let _ = ctx.state.deselect();
                            }
                        }
                    }

                    // Only commit to mask for Add/Subtract modes.
                    if matches!(add_type, AddType::Add | AddType::Subtract) {
                        let _ = ctx.state.add_selection_to_mask();
                        let _ = ctx.state.deselect();
                    }

                    // Reset state - dropping the guard groups all operations into one undo entry
                    self.drag_mode = SelectionDrag::None;
                    self.start_pos = None;
                    self.current_pos = None;
                    self.start_selection = None;
                    self.hover_drag = SelectionDrag::None;
                    self.selection_undo = None;

                    ToolResult::EndCapture.and(ToolResult::Redraw)
                } else {
                    ToolResult::None
                }
            }

            _ => ToolResult::None,
        }
    }

    fn handle_event(&mut self, ctx: &mut ToolContext, event: &iced::Event) -> ToolResult {
        match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
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
            _ => ToolResult::None,
        }
    }

    fn view_toolbar(&self, ctx: &ToolViewContext) -> Element<'_, ToolMessage> {
        let mode = self.selection_mode;

        let segments = vec![
            Segment::text("Rect", SelectionMode::Normal),
            Segment::text("Char", SelectionMode::Character),
            Segment::text("Attr", SelectionMode::Attribute),
            Segment::text("Fg", SelectionMode::Foreground),
            Segment::text("Bg", SelectionMode::Background),
        ];

        let segmented_control = self
            .selection_mode_control
            .view(segments, mode, ctx.font.clone(), &ctx.theme)
            .map(|msg| match msg {
                SegmentedControlMessage::Selected(m) | SegmentedControlMessage::Toggled(m) | SegmentedControlMessage::CharClicked(m) => {
                    ToolMessage::SelectSetMode(m)
                }
            });

        row![
            Space::new().width(iced::Length::Fill),
            segmented_control,
            Space::new().width(iced::Length::Fixed(16.0)),
            text("⇧: add   ⌃/Ctrl: remove").size(14).style(|theme: &iced::Theme| text::Style {
                color: Some(theme.extended_palette().secondary.base.color),
            }),
            Space::new().width(iced::Length::Fill),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .into()
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        if self.is_dragging {
            self.drag_mode.to_cursor_interaction().unwrap_or(iced::mouse::Interaction::Crosshair)
        } else {
            self.hover_drag.to_cursor_interaction().unwrap_or(iced::mouse::Interaction::Crosshair)
        }
    }

    fn show_caret(&self) -> bool {
        false
    }

    fn show_selection(&self) -> bool {
        true
    }
}
