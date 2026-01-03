//! Click Tool (Text Cursor / Keyboard Input)
//!
//! The primary text editing tool. Handles:
//! - Caret positioning and movement
//! - Character input and insertion
//! - Line operations (insert, delete)
//! - Cursor navigation (arrows, home, end, etc.)
//! - Layer dragging (Ctrl+Click+Drag, or always for Image layers)

use icy_engine::{BufferType, Role};
use icy_engine::{Position, TextPane};
use icy_engine_edit::AtomicUndoGuard;
use icy_engine_gui::terminal::crt_state::{is_command_pressed, is_ctrl_pressed};
use icy_engine_gui::TerminalMessage;
use icy_ui::keyboard::key::Physical;
use icy_ui::Element;

use super::{handle_navigation_key, SelectionMouseState, ToolContext, ToolHandler, ToolId, ToolMessage, ToolResult, ToolViewContext, UiAction};
use crate::ui::editor::ansi::{FKeyToolbarMessage, ShaderFKeyToolbar};
use crate::ui::FKeySets;
use crate::Settings;

/// Click tool state
#[derive(Default)]
pub struct ClickTool {
    /// F-key toolbar (GPU shader version)
    pub fkey_toolbar: ShaderFKeyToolbar,

    /// Currently selected F-key set index (mirrors `Options.fkeys.current_set`)
    current_fkey_set: usize,

    /// Whether layer drag is active (Ctrl+Click+Drag, or always for Image layers)
    layer_drag_active: bool,
    /// Layer offset at start of drag
    layer_drag_start_offset: Position,
    /// Start position of drag
    drag_start: Option<Position>,
    /// Current position during drag
    drag_current: Option<Position>,

    /// Atomic undo guard for layer drag operations
    layer_drag_undo: Option<AtomicUndoGuard>,

    // === Selection Mouse State (shared with FontTool) ===
    selection_mouse: SelectionMouseState,

    /// Whether the current layer is an Image layer (Role::Image)
    /// Updated on layer change to control cursor/caret display
    is_on_image_layer: bool,
}

impl ClickTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_fkey_cache(&mut self) {
        self.fkey_toolbar.clear_cache();
    }

    pub fn type_fkey_slot(&mut self, ctx: &mut ToolContext, set_idx: usize, slot: usize) -> ToolResult {
        let Some(options) = ctx.options else {
            return ToolResult::None;
        };

        let code = {
            let opts = options.read();
            opts.fkeys.code_at(set_idx, slot)
        };

        let buffer_type = ctx.state.get_buffer().buffer_type;
        let raw = char::from_u32(code as u32).unwrap_or(' ');
        let unicode_cp437 = BufferType::CP437.convert_to_unicode(raw);
        let target = buffer_type.convert_from_unicode(unicode_cp437);

        if let Err(e) = ctx.state.type_key(target) {
            log::warn!("Failed to type fkey (set {}, slot {}): {}", set_idx, slot, e);
            return ToolResult::None;
        }

        self.clear_fkey_cache();
        ToolResult::Commit("Type fkey".to_string())
    }

    pub fn set_current_fkey_set(&mut self, options: &std::sync::Arc<parking_lot::RwLock<Settings>>, set_idx: usize) {
        let fkeys_to_save = {
            let mut opts = options.write();
            opts.fkeys.clamp_current_set();

            let count = opts.fkeys.set_count();
            let clamped = if count == 0 { 0 } else { set_idx % count };

            opts.fkeys.current_set = clamped;
            opts.fkeys.clone()
        };

        self.current_fkey_set = {
            let opts = options.read();
            opts.fkeys.current_set
        };

        std::thread::spawn(move || {
            let _ = fkeys_to_save.save();
        });

        self.clear_fkey_cache();
    }

    pub fn sync_fkey_set_from_options(&mut self, options: &std::sync::Arc<parking_lot::RwLock<Settings>>) {
        self.current_fkey_set = {
            let opts = options.read();
            opts.fkeys.current_set
        };
    }

    pub fn current_fkey_set(&self) -> usize {
        self.current_fkey_set
    }

    pub fn handle_fkey_toolbar_message(&mut self, ctx: &mut ToolContext, msg: FKeyToolbarMessage) -> ToolResult {
        match msg {
            FKeyToolbarMessage::TypeFKey(slot) => self.type_fkey_slot(ctx, self.current_fkey_set, slot),
            FKeyToolbarMessage::OpenCharSelector(slot) => ToolResult::Ui(UiAction::OpenCharSelectorForFKey(slot)),
            FKeyToolbarMessage::NextSet => {
                let Some(options) = ctx.options else {
                    return ToolResult::None;
                };
                let next = self.current_fkey_set.saturating_add(1);
                self.set_current_fkey_set(options, next);
                ToolResult::Redraw
            }
            FKeyToolbarMessage::PrevSet => {
                let Some(options) = ctx.options else {
                    return ToolResult::None;
                };
                let cur = self.current_fkey_set;
                let prev = {
                    let opts = options.read();
                    let count = opts.fkeys.set_count();
                    if count == 0 {
                        0
                    } else {
                        (cur + count - 1) % count
                    }
                };
                self.set_current_fkey_set(options, prev);
                ToolResult::Redraw
            }
        }
    }

    /// Update the cached image layer status from the current layer
    pub fn update_image_layer_status(&mut self, ctx: &ToolContext) {
        self.is_on_image_layer = ctx.state.get_cur_layer().map(|l| matches!(l.role, Role::Image)).unwrap_or(false);
    }

    /// Check if the current layer is an Image layer
    pub fn is_on_image_layer(&self) -> bool {
        self.is_on_image_layer
    }
}

impl ToolHandler for ClickTool {
    fn id(&self) -> ToolId {
        ToolId::Tool(icy_engine_edit::tools::Tool::Click)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn cancel_capture(&mut self) {
        // Reset layer drag state
        self.layer_drag_active = false;
        self.drag_start = None;
        self.drag_current = None;
        self.layer_drag_undo = None;

        // Reset selection drag state
        self.selection_mouse.cancel();
    }

    fn view_toolbar(&self, ctx: &ToolViewContext) -> Element<'_, ToolMessage> {
        self.fkey_toolbar
            .view(ctx.fkeys.clone(), ctx.font.clone(), ctx.palette.clone(), ctx.caret_fg, ctx.caret_bg, &ctx.theme)
            .map(ToolMessage::ClickFKeyToolbar)
    }

    fn handle_message(&mut self, ctx: &mut ToolContext, msg: &ToolMessage) -> ToolResult {
        match msg {
            ToolMessage::ClickFKeyToolbar(m) => self.handle_fkey_toolbar_message(ctx, m.clone()),
            _ => ToolResult::None,
        }
    }

    fn handle_terminal_message(&mut self, ctx: &mut ToolContext, msg: &TerminalMessage) -> ToolResult {
        // Update image layer status on every message
        self.update_image_layer_status(ctx);

        match msg {
            TerminalMessage::Move(evt) => {
                if self.layer_drag_active {
                    return ToolResult::None;
                }

                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };

                // Hover: update cursor interaction for selection resize handles.
                self.selection_mouse.handle_move(ctx.state.selection(), pos);
                ToolResult::None
            }

            TerminalMessage::Press(evt) => {
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };

                // Image layers: always start layer drag on click (no Ctrl needed)
                // Normal layers: require Ctrl/Cmd for layer drag
                let start_layer_drag = evt.button == icy_engine::MouseButton::Left && (self.is_on_image_layer || is_ctrl_pressed() || is_command_pressed());

                if start_layer_drag {
                    // Start layer drag
                    self.layer_drag_active = true;
                    self.drag_start = Some(pos);
                    self.drag_current = Some(pos);

                    // Get current layer offset
                    if let Some(layer) = ctx.state.get_cur_layer() {
                        self.layer_drag_start_offset = layer.offset();
                    }

                    if self.layer_drag_undo.is_none() {
                        self.layer_drag_undo = Some(ctx.state.begin_atomic_undo("Move layer".to_string()));
                    }

                    ToolResult::StartCapture.and(ToolResult::Redraw)
                } else if evt.button == icy_engine::MouseButton::Left {
                    // Selection drag handling (shared with FontTool)
                    self.selection_mouse.handle_press(ctx, pos);
                    ToolResult::StartCapture.and(ToolResult::Redraw)
                } else {
                    ToolResult::None
                }
            }

            TerminalMessage::Drag(evt) => {
                if self.layer_drag_active {
                    if let Some(pos) = evt.text_position {
                        self.drag_current = Some(pos);

                        // Calculate delta and update layer preview
                        if let Some(start) = self.drag_start {
                            let delta = pos - start;
                            let new_offset = self.layer_drag_start_offset + delta;

                            ctx.state.set_layer_preview_offset(Some(new_offset));
                        }

                        return ToolResult::Redraw;
                    }
                }

                // Selection drag update
                if let Some(pos) = evt.text_position {
                    if self.selection_mouse.handle_drag(ctx, pos) {
                        // Use lightweight rect-only redraw during drag for performance
                        return ToolResult::RedrawSelectionRect;
                    }
                }

                ToolResult::None
            }

            TerminalMessage::Release(evt) => {
                if self.layer_drag_active {
                    self.layer_drag_active = false;

                    // Apply final layer offset
                    if let (Some(start), Some(pos)) = (self.drag_start, evt.text_position) {
                        let delta = pos - start;
                        let new_offset = self.layer_drag_start_offset + delta;

                        ctx.state.set_layer_preview_offset(None);
                        let _ = ctx.state.move_layer(new_offset);
                    }

                    self.drag_start = None;
                    self.drag_current = None;

                    // Dropping the guard groups everything into one undo entry.
                    self.layer_drag_undo = None;

                    ToolResult::EndCapture.and(ToolResult::Commit("Move layer".to_string()))
                } else if self.selection_mouse.handle_release(ctx, evt.text_position) {
                    ToolResult::EndCapture.and(ToolResult::Redraw)
                } else {
                    ToolResult::None
                }
            }

            _ => ToolResult::None,
        }
    }

    fn handle_event(&mut self, ctx: &mut ToolContext, event: &icy_ui::Event) -> ToolResult {
        match event {
            icy_ui::Event::Keyboard(icy_ui::keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                physical_key,
                ..
            }) => {
                use icy_ui::keyboard::key::Named;
                use icy_ui::keyboard::Key;

                // - Ctrl+,  => previous set
                // - Ctrl+.  => next set
                // - Ctrl+/  => default set
                if modifiers.control() && !modifiers.alt() {
                    let Some(options) = ctx.options else {
                        return ToolResult::None;
                    };
                    match physical_key {
                        Physical::Code(icy_ui::keyboard::key::Code::Comma) => {
                            let cur = self.current_fkey_set;
                            let prev = {
                                let opts = options.read();
                                let count = opts.fkeys.set_count();
                                if count == 0 {
                                    0
                                } else {
                                    (cur + count - 1) % count
                                }
                            };
                            self.set_current_fkey_set(options, prev);
                            return ToolResult::Redraw;
                        }
                        Physical::Code(icy_ui::keyboard::key::Code::Period) => {
                            let next = self.current_fkey_set.saturating_add(1);
                            self.set_current_fkey_set(options, next);
                            return ToolResult::Redraw;
                        }
                        Physical::Code(icy_ui::keyboard::key::Code::Slash) => {
                            let default_set = FKeySets::default().current_set;
                            self.set_current_fkey_set(options, default_set);
                            return ToolResult::Redraw;
                        }
                        _ => {}
                    }
                }

                // Image layers don't support text input
                if self.is_on_image_layer {
                    // Fall through to handle Delete/Backspace for layer deletion
                }

                // Shift+Space inserts 0xFF (hard blank) - works for all font types (not for Image layers)
                if !self.is_on_image_layer && modifiers.shift() {
                    if let icy_ui::keyboard::Key::Named(Named::Space) = key {
                        if let Err(e) = ctx.state.type_key('\u{00FF}') {
                            log::warn!("Failed to type hard blank: {}", e);
                            return ToolResult::None;
                        }
                        return ToolResult::Commit("Type hard blank".to_string());
                    }
                }

                // Character input using the translated text (respects keyboard layout) - not for Image layers
                if !self.is_on_image_layer && !modifiers.control() && !modifiers.alt() {
                    if let Some(input_text) = text {
                        if let Some(ch) = input_text.chars().next() {
                            // Skip control characters (0x00-0x1F) and DEL (0x7F) - these should be handled
                            // by Named key handlers (Backspace, Tab, Enter, Delete, etc.)
                            if ch < ' ' || ch == '\x7F' {
                                // Fall through to Named key handling below
                            } else {
                                // Convert Unicode -> buffer encoding (CP437 etc.)
                                let buffer_type = ctx.state.get_buffer().buffer_type;
                                let encoded = buffer_type.convert_from_unicode(ch);
                                if let Err(e) = ctx.state.type_key(encoded) {
                                    log::warn!("Failed to type character: {}", e);
                                    return ToolResult::None;
                                }
                                return ToolResult::Commit("Type character".to_string());
                            }
                        }
                    }
                }

                // Handle Space key (text field may not contain it) - not for Image layers
                if !self.is_on_image_layer {
                    if let icy_ui::keyboard::Key::Named(Named::Space) = key {
                        if !modifiers.shift() && !modifiers.control() {
                            let buffer_type = ctx.state.get_buffer().buffer_type;
                            let encoded = buffer_type.convert_from_unicode(' ');
                            if let Err(e) = ctx.state.type_key(encoded) {
                                log::warn!("Failed to type space: {}", e);
                                return ToolResult::None;
                            }
                            return ToolResult::Commit("Type character".to_string());
                        }
                    }
                }

                if let icy_ui::keyboard::Key::Named(named) = key {
                    match named {
                        // F-keys - not for Image layers (except Alt+F for set switching)
                        Named::F1
                        | Named::F2
                        | Named::F3
                        | Named::F4
                        | Named::F5
                        | Named::F6
                        | Named::F7
                        | Named::F8
                        | Named::F9
                        | Named::F10
                        | Named::F11
                        | Named::F12 => {
                            let slot = match named {
                                Named::F1 => 0,
                                Named::F2 => 1,
                                Named::F3 => 2,
                                Named::F4 => 3,
                                Named::F5 => 4,
                                Named::F6 => 5,
                                Named::F7 => 6,
                                Named::F8 => 7,
                                Named::F9 => 8,
                                Named::F10 => 9,
                                Named::F11 => 10,
                                Named::F12 => 11,
                                _ => 0,
                            };

                            if modifiers.alt() && slot < 10 {
                                let Some(options) = ctx.options else {
                                    return ToolResult::None;
                                };
                                let base = if modifiers.shift() { 10 } else { 0 };
                                self.set_current_fkey_set(options, base + slot);
                                return ToolResult::Redraw;
                            }

                            // Image layers don't support F-key typing
                            if self.is_on_image_layer {
                                return ToolResult::None;
                            }

                            return self.type_fkey_slot(ctx, self.current_fkey_set, slot);
                        }

                        // Click-specific: Backspace and Enter
                        Named::Backspace => {
                            // Image layer: delete the entire layer
                            if self.is_on_image_layer {
                                let _ = ctx.state.remove_layer(ctx.state.get_current_layer().unwrap_or(0));
                                return ToolResult::Commit("Delete layer".to_string());
                            }
                            // Normal layer: erase selection or backspace
                            let _ = if ctx.state.is_something_selected() {
                                ctx.state.erase_selection()
                            } else {
                                ctx.state.backspace()
                            };
                            return ToolResult::Commit("Backspace".to_string());
                        }
                        Named::Enter => {
                            // Image layers don't support Enter
                            if self.is_on_image_layer {
                                return ToolResult::None;
                            }
                            let _ = ctx.state.new_line();
                            return ToolResult::Commit("New line".to_string());
                        }
                        _ => {}
                    }
                }

                // Image layers: handle Delete specially (delete the layer)
                if self.is_on_image_layer {
                    if let icy_ui::keyboard::Key::Named(icy_ui::keyboard::key::Named::Delete) = key {
                        let _ = ctx.state.remove_layer(ctx.state.get_current_layer().unwrap_or(0));
                        return ToolResult::Commit("Delete layer".to_string());
                    }
                    // Image layers don't support other navigation keys
                    return ToolResult::None;
                }

                // Common navigation keys (arrows, home, end, page up/down, delete, tab, insert)
                let nav_result = handle_navigation_key(ctx, key, modifiers);
                if nav_result.is_handled() {
                    return nav_result.to_tool_result();
                }

                ToolResult::None
            }
            _ => ToolResult::None,
        }
    }

    fn cursor(&self) -> icy_ui::mouse::Interaction {
        if self.layer_drag_active {
            icy_ui::mouse::Interaction::Grabbing
        } else if self.is_on_image_layer {
            // Image layers always show grab cursor
            icy_ui::mouse::Interaction::Grab
        } else if let Some(cursor) = self.selection_mouse.cursor() {
            cursor
        } else {
            icy_ui::mouse::Interaction::Text
        }
    }

    fn show_caret(&self) -> bool {
        // Don't show caret on Image layers
        !self.is_on_image_layer
    }

    fn show_selection(&self) -> bool {
        // Don't show selection on Image layers
        !self.is_on_image_layer
    }
}
