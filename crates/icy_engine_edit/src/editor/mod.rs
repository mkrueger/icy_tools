pub mod undo_stack;
use std::sync::{Arc, Mutex};

pub use undo_stack::*;

pub mod undo_operation;
pub use undo_operation::EditorUndoOp;

pub mod session_state;
pub use session_state::AnsiEditorSessionState;

mod editor_error;
pub use editor_error::*;

mod terminal_input;

mod layer_operations;
pub use layer_operations::*;
mod area_operations;
pub(crate) use area_operations::{flip_layer_x, flip_layer_y, generate_flipx_table, generate_flipy_table};
mod edit_operations;
mod font_operations;
mod selection_operations;
mod tag_operations;

mod tdf_renderer;
pub use tdf_renderer::TdfEditStateRenderer;

// ============================================================================
// Format Mode
// ============================================================================

/// Document format mode - determines available features
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FormatMode {
    /// Legacy DOS: 16 fixed colors, single font, no palette editing
    LegacyDos,
    /// XBin: 16 colors from selectable palette, single font
    XBin,
    /// XBin Extended: 8 colors, custom palette (first 8), dual fonts
    XBinExtended,
    /// Unrestricted: Full RGB, unlimited fonts
    #[default]
    Unrestricted,
}

impl FormatMode {
    /// All available format modes
    pub const ALL: [FormatMode; 4] = [FormatMode::LegacyDos, FormatMode::XBin, FormatMode::XBinExtended, FormatMode::Unrestricted];

    /// Get the description for this format mode
    pub fn description(&self) -> &'static str {
        match self {
            FormatMode::LegacyDos => "16 fixed colors, single font, no palette editing",
            FormatMode::XBin => "16 colors from selectable palette, single font",
            FormatMode::XBinExtended => "8 fg colors, dual fonts, custom palette",
            FormatMode::Unrestricted => "Full RGB colors, unlimited fonts",
        }
    }

    /// Check if palette editing is allowed
    pub fn allows_palette_editing(&self) -> bool {
        matches!(self, FormatMode::XBin | FormatMode::XBinExtended | FormatMode::Unrestricted)
    }

    /// Check if font selection is allowed
    pub fn allows_font_selection(&self) -> bool {
        matches!(self, FormatMode::XBinExtended | FormatMode::Unrestricted)
    }

    /// Get maximum number of fonts
    pub fn max_fonts(&self) -> usize {
        match self {
            FormatMode::LegacyDos | FormatMode::XBin => 1,
            FormatMode::XBinExtended => 2,
            FormatMode::Unrestricted => usize::MAX,
        }
    }

    /// Get number of available colors
    pub fn color_count(&self) -> usize {
        match self {
            FormatMode::LegacyDos | FormatMode::XBin => 16,
            FormatMode::XBinExtended => 8,
            FormatMode::Unrestricted => 16777216, // 24-bit RGB
        }
    }

    /// Derive FormatMode from buffer's palette_mode and font_mode.
    /// Default is Unrestricted, falls back to LegacyDos if no match.
    pub fn from_buffer(buffer: &TextBuffer) -> Self {
        use crate::FontMode;

        match buffer.font_mode {
            FontMode::Sauce => FormatMode::LegacyDos,
            FontMode::Single => FormatMode::XBin,
            FontMode::FixedSize => FormatMode::XBinExtended,
            FontMode::Unlimited => FormatMode::Unrestricted,
        }
    }

    /// Apply this format mode to a buffer (sets palette_mode and font_mode)
    pub fn apply_to_buffer(&self, buffer: &mut TextBuffer) {
        use crate::FontMode;

        match self {
            FormatMode::LegacyDos => {
                buffer.font_mode = FontMode::Sauce;
            }
            FormatMode::XBin => {
                buffer.font_mode = FontMode::Single;
            }
            FormatMode::XBinExtended => {
                buffer.font_mode = FontMode::FixedSize;
            }
            FormatMode::Unrestricted => {
                buffer.font_mode = FontMode::Unlimited;
            }
        }
    }
}

impl std::fmt::Display for FormatMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatMode::LegacyDos => write!(f, "Legacy DOS"),
            FormatMode::XBin => write!(f, "XBin"),
            FormatMode::XBinExtended => write!(f, "XBin Extended"),
            FormatMode::Unrestricted => write!(f, "Unrestricted"),
        }
    }
}

use crate::{
    AnsiSaveOptionsV2, AttributedChar, BitFont, Caret, EditableScreen, HyperLink, IceMode, Layer, Line, MouseField, Palette, Position, Rectangle,
    RenderOptions, Result, SauceMetaData, SavedCaretState, Screen, Selection, SelectionMask, Sixel, Size, TerminalState, TextBuffer, TextPane, TextScreen,
    clipboard, overlay_mask::OverlayMask,
};
use icy_parser_core::{IgsCommand, RipCommand, SkypixCommand};
use parking_lot::Mutex as ParkingMutex;

pub struct EditState {
    screen: TextScreen,
    tool_overlay_mask: OverlayMask,

    /// Selection state
    pub(crate) selection_opt: Option<Selection>,
    pub(crate) selection_mask: SelectionMask,

    current_tag: usize,

    outline_style: usize,
    mirror_mode: bool,

    /// Serializable undo stack (wrapped in Arc<Mutex> for atomic operations)
    undo_stack: Arc<Mutex<EditorUndoStack>>,

    pub is_palette_dirty: bool,

    /// SAUCE metadata for the file (title, author, group, comments)
    sauce_meta: SauceMetaData,
}

/// Guard for atomic undo operations
/// When dropped, collects all operations pushed since creation into an Atomic operation
pub struct AtomicUndoGuard {
    base_count: usize,
    description: String,
    operation_type: OperationType,
    undo_stack: Arc<Mutex<EditorUndoStack>>,
    ended: bool,
}

impl AtomicUndoGuard {
    fn new(description: String, undo_stack: Arc<Mutex<EditorUndoStack>>, operation_type: OperationType) -> Self {
        let base_count = undo_stack.lock().unwrap().undo_len();
        Self {
            base_count,
            description,
            operation_type,
            undo_stack,
            ended: false,
        }
    }

    pub fn end(&mut self) {
        if self.ended {
            return;
        }
        self.end_action();
    }

    /// Discard all operations in this atomic group without committing them.
    /// The operations are removed from the undo stack and the guard is marked as ended.
    ///
    /// NOTE: This only removes operations from the stack, it does NOT undo the actual
    /// buffer changes. For paste operations, use `discard_and_undo()` instead.
    pub fn discard(&mut self) {
        if self.ended {
            return;
        }
        let mut stack = self.undo_stack.lock().unwrap();
        // Remove all operations pushed since base_count
        while stack.undo_len() > self.base_count {
            stack.pop_undo();
        }
        self.ended = true;
    }

    /// Discard all operations in this atomic group AND undo them.
    /// This properly reverts all buffer changes made since the guard was created.
    ///
    /// Use this for operations like paste cancel where you need to undo the actual
    /// changes (e.g., remove the pasted layer) not just clear the undo stack.
    pub fn discard_and_undo(&mut self, edit_state: &mut super::EditState) {
        if self.ended {
            return;
        }

        let mut stack = self.undo_stack.lock().unwrap();
        // Pop and undo all operations pushed since base_count (in reverse order)
        while stack.undo_len() > self.base_count {
            if let Some(mut op) = stack.pop_undo() {
                // Need to drop the lock before calling undo to avoid deadlock
                drop(stack);
                if let Err(e) = op.undo(edit_state) {
                    log::warn!("Failed to undo operation during discard: {}", e);
                }
                stack = self.undo_stack.lock().unwrap();
            }
        }
        edit_state.mark_dirty();
        self.ended = true;
    }

    fn end_action(&mut self) {
        if self.ended {
            return;
        }
        let mut stack = self.undo_stack.lock().unwrap();
        let current_len = stack.undo_len();
        if current_len <= self.base_count {
            self.ended = true;
            return;
        }

        // Collect all operations pushed since base_count
        let mut operations = Vec::new();
        while stack.undo_len() > self.base_count {
            if let Some(op) = stack.pop_undo() {
                operations.push(op);
            }
        }
        operations.reverse(); // Restore original order

        // Small optimizations to reduce undo stack size and serialization cost.
        operations = coalesce_atomic_operations(operations);
        if operations.is_empty() {
            self.ended = true;
            return;
        }

        // If optimization collapsed to a single operation, avoid an Atomic wrapper.
        // Keep Atomic when we need to preserve a non-default operation_type.
        if operations.len() == 1 && self.operation_type == OperationType::Unknown {
            stack.push_undo(operations.pop().unwrap());
            self.ended = true;
            return;
        }

        // Push as a single Atomic operation
        stack.push_undo(EditorUndoOp::Atomic {
            description: self.description.clone(),
            operations,
            operation_type: self.operation_type,
        });
        self.ended = true;
    }
}

fn coalesce_atomic_operations(operations: Vec<EditorUndoOp>) -> Vec<EditorUndoOp> {
    let mut out: Vec<EditorUndoOp> = Vec::with_capacity(operations.len());
    let mut iter = operations.into_iter().peekable();

    while let Some(op) = iter.next() {
        match op {
            EditorUndoOp::SetSelection { old, new } => {
                // Collapse consecutive SetSelection ops (e.g. drag updates) into one:
                // keep the first 'old' and the last 'new'.
                let mut last_new = new;
                while let Some(EditorUndoOp::SetSelection { new, .. }) = iter.peek() {
                    last_new = *new;
                    iter.next();
                }
                out.push(EditorUndoOp::SetSelection { old, new: last_new });
            }
            EditorUndoOp::SetSelectionMask { description, old, new } => {
                // Collapse consecutive SetSelectionMask ops: keep first old, last new.
                let mut last_description = description;
                let mut last_new = new;
                while let Some(EditorUndoOp::SetSelectionMask { description, new, .. }) = iter.peek() {
                    last_description = description.clone();
                    last_new = new.clone();
                    iter.next();
                }
                out.push(EditorUndoOp::SetSelectionMask {
                    description: last_description,
                    old,
                    new: last_new,
                });
            }
            EditorUndoOp::MoveLayer { index, from, to } => {
                // Collapse consecutive MoveLayer ops for the same layer (drag updates):
                // keep the first 'from' and the last 'to'.
                let mut last_to = to;
                while let Some(EditorUndoOp::MoveLayer {
                    index: next_index,
                    to: next_to,
                    ..
                }) = iter.peek()
                {
                    if *next_index != index {
                        break;
                    }
                    last_to = *next_to;
                    iter.next();
                }
                out.push(EditorUndoOp::MoveLayer { index, from, to: last_to });
            }
            EditorUndoOp::MoveTag { tag, old_pos, new_pos } => {
                // Collapse consecutive MoveTag ops for the same tag (drag updates):
                // keep the first 'old_pos' and the last 'new_pos'.
                let mut last_new = new_pos;
                while let Some(EditorUndoOp::MoveTag {
                    tag: next_tag,
                    new_pos: next_new,
                    ..
                }) = iter.peek()
                {
                    if *next_tag != tag {
                        break;
                    }
                    last_new = *next_new;
                    iter.next();
                }
                out.push(EditorUndoOp::MoveTag {
                    tag,
                    old_pos,
                    new_pos: last_new,
                });
            }
            other => out.push(other),
        }
    }

    out
}

impl Drop for AtomicUndoGuard {
    fn drop(&mut self) {
        self.end_action();
    }
}

impl Default for EditState {
    fn default() -> Self {
        let screen = TextScreen::default();
        let mut tool_overlay_mask = OverlayMask::default();
        tool_overlay_mask.set_size(screen.buffer.size());

        let mut selection_mask = SelectionMask::default();
        selection_mask.set_size(screen.buffer.size());

        Self {
            screen,
            selection_opt: None,
            selection_mask,
            undo_stack: Arc::new(Mutex::new(EditorUndoStack::new())),
            current_tag: 0,
            outline_style: 0,
            mirror_mode: false,
            tool_overlay_mask,
            is_palette_dirty: false,
            sauce_meta: SauceMetaData::default(),
        }
    }
}

impl EditState {
    pub fn from_buffer(buffer: TextBuffer) -> Self {
        let screen = TextScreen::from_buffer(buffer);
        let mut tool_overlay_mask = OverlayMask::default();
        tool_overlay_mask.set_size(screen.buffer.size());

        let mut edit_state = Self {
            screen,
            tool_overlay_mask,
            ..Default::default()
        };
        edit_state.set_mask_size();
        edit_state
    }

    pub fn set_buffer(&mut self, buffer: TextBuffer) {
        self.screen.buffer = buffer;
        self.set_mask_size();
    }

    pub fn set_mask_size(&mut self) {
        self.selection_mask.set_size(self.screen.buffer.size());
        self.tool_overlay_mask.set_size(self.screen.buffer.size());
    }

    pub fn get_tool_overlay_mask(&self) -> &OverlayMask {
        &self.tool_overlay_mask
    }

    pub fn get_tool_overlay_mask_mut(&mut self) -> &mut OverlayMask {
        &mut self.tool_overlay_mask
    }

    pub fn get_buffer(&self) -> &TextBuffer {
        &self.screen.buffer
    }

    pub fn get_buffer_mut(&mut self) -> &mut TextBuffer {
        &mut self.screen.buffer
    }

    /// Get the current format mode, derived from buffer's palette_mode and font_mode
    pub fn get_format_mode(&self) -> FormatMode {
        FormatMode::from_buffer(&self.screen.buffer)
    }

    /// Set the format mode (applies palette_mode and font_mode to the buffer)
    pub fn set_format_mode(&mut self, mode: FormatMode) {
        mode.apply_to_buffer(&mut self.screen.buffer);
    }

    /// Get a reference to the SAUCE metadata
    pub fn get_sauce_meta(&self) -> &SauceMetaData {
        &self.sauce_meta
    }

    /// Set the SAUCE metadata
    pub fn set_sauce_meta(&mut self, sauce_meta: SauceMetaData) {
        self.sauce_meta = sauce_meta;
    }

    pub fn get_cur_layer(&self) -> Option<&Layer> {
        if let Ok(layer) = self.get_current_layer() {
            self.screen.buffer.layers.get(layer)
        } else {
            None
        }
    }

    pub fn get_cur_display_layer(&self) -> Option<&Layer> {
        if let Ok(layer) = self.get_current_layer() {
            self.screen.buffer.layers.get(layer)
        } else {
            None
        }
    }

    pub fn get_cur_layer_mut(&mut self) -> Option<&mut Layer> {
        if let Ok(layer) = self.get_current_layer() {
            self.screen.buffer.layers.get_mut(layer)
        } else {
            None
        }
    }

    pub fn get_caret(&self) -> &Caret {
        &self.screen.caret
    }

    pub(crate) fn get_caret_mut(&mut self) -> &mut Caret {
        &mut self.screen.caret
    }

    // =========================================================================
    // Public caret manipulation methods (safe API without exposing &mut Caret)
    // =========================================================================

    /// Convert a document position (absolute) to a layer-relative position.
    ///
    /// When clicking on the canvas, the position is in document coordinates.
    /// But the caret position should be relative to the current layer's offset.
    ///
    /// # Example
    /// If the current layer has offset (10, 5) and the user clicks at document
    /// position (15, 8), the layer-relative position is (5, 3).
    pub fn document_to_layer_position(&self, doc_pos: Position) -> Position {
        let layer_offset = self.get_cur_layer().map(|l| l.offset()).unwrap_or_default();
        doc_pos - layer_offset
    }

    /// Convert a layer-relative position to a document position (absolute).
    ///
    /// This is the inverse of `document_to_layer_position`.
    pub fn layer_to_document_position(&self, layer_pos: Position) -> Position {
        let layer_offset = self.get_cur_layer().map(|l| l.offset()).unwrap_or_default();
        layer_pos + layer_offset
    }

    /// Set the caret position from a document (absolute) position.
    ///
    /// This converts the document position to layer-relative coordinates
    /// before setting the caret. Use this when handling mouse clicks on the canvas.
    pub fn set_caret_from_document_position(&mut self, doc_pos: Position) {
        let layer_pos = self.document_to_layer_position(doc_pos);
        self.set_caret_position(layer_pos);
    }

    /// Set the caret position
    pub fn set_caret_position(&mut self, pos: Position) {
        self.screen.caret.set_position(pos);
    }

    /// Set the caret X coordinate
    pub fn set_caret_x(&mut self, x: i32) {
        self.screen.caret.x = x;
    }

    /// Set the caret Y coordinate
    pub fn set_caret_y(&mut self, y: i32) {
        self.screen.caret.y = y;
    }

    /// Constrain a foreground color based on the current font mode.
    /// In XBinExtended mode (FontMode::FixedSize), FG colors are limited to 0-7.
    /// Other modes allow all 16 colors.
    pub fn constrain_foreground_color(&self, color: u32) -> u32 {
        if !self.screen.buffer.font_mode.has_high_fg_colors() && color >= 8 && color < 16 {
            color % 8
        } else {
            color
        }
    }

    /// Constrain a background color based on the current ice mode.
    /// In Blink mode (no high bg colors), BG colors are limited to 0-7.
    /// In Ice mode, all 16 colors are allowed.
    pub fn constrain_background_color(&self, color: u32) -> u32 {
        if !self.screen.buffer.ice_mode.has_high_bg_colors() && color >= 8 && color < 16 {
            color % 8
        } else {
            color
        }
    }

    /// Set the caret foreground color (applies constraint based on font mode)
    pub fn set_caret_foreground(&mut self, color: u32) {
        let constrained = self.constrain_foreground_color(color);
        self.screen.caret.attribute.set_foreground(constrained);
    }

    /// Set the caret background color (applies constraint based on ice mode)
    pub fn set_caret_background(&mut self, color: u32) {
        let constrained = self.constrain_background_color(color);
        self.screen.caret.attribute.set_background(constrained);
    }

    /// Swap foreground and background colors, returns (new_fg, new_bg)
    /// Applies constraints: if swapped BG->FG would be 8-15 in FixedSize mode, it's reduced to 0-7
    pub fn swap_caret_colors(&mut self) -> (u32, u32) {
        let fg = self.screen.caret.attribute.foreground();
        let bg = self.screen.caret.attribute.background();
        // Apply constraints when swapping
        let new_fg = self.constrain_foreground_color(bg);
        let new_bg = self.constrain_background_color(fg);
        self.screen.caret.attribute.set_foreground(new_fg);
        self.screen.caret.attribute.set_background(new_bg);
        (new_fg, new_bg)
    }

    /// Reset caret colors to default (fg=7, bg=0)
    pub fn reset_caret_colors(&mut self) {
        self.screen.caret.attribute.set_foreground(7);
        self.screen.caret.attribute.set_background(0);
    }

    /// Set the caret's font page
    pub fn set_caret_font_page(&mut self, page: u8) {
        self.screen.caret.set_font_page(page);
    }

    /// Set the caret's text attribute
    pub fn set_caret_attribute(&mut self, attr: icy_engine::TextAttribute) {
        self.screen.caret.attribute = attr;
    }

    /// Move caret up by given amount (clamped to 0)
    pub fn move_caret_up(&mut self, amount: i32) {
        self.screen.caret.y = (self.screen.caret.y - amount).max(0);
    }

    /// Move caret down by given amount
    pub fn move_caret_down(&mut self, amount: i32) {
        self.screen.caret.y += amount;
    }

    /// Move caret left by given amount (clamped to 0)
    pub fn move_caret_left(&mut self, amount: i32) {
        self.screen.caret.x = (self.screen.caret.x - amount).max(0);
    }

    /// Move caret right by given amount
    pub fn move_caret_right(&mut self, amount: i32) {
        self.screen.caret.x += amount;
    }

    /// Set caret visibility
    pub fn set_caret_visible(&mut self, visible: bool) {
        self.screen.caret.visible = visible;
    }

    // =========================================================================
    // Public buffer manipulation methods (safe API)
    // =========================================================================

    /// Mark the buffer as dirty
    pub fn mark_buffer_dirty(&mut self) {
        self.screen.buffer.mark_dirty();
    }

    /// Get mutable reference to buffer layers (for rendering into buffer)
    pub fn get_layers_mut(&mut self) -> &mut Vec<Layer> {
        &mut self.screen.buffer.layers
    }

    pub fn copy_text(&self) -> Option<String> {
        let Some(selection) = &self.selection_opt else {
            return None;
        };
        clipboard::text(&self.screen.buffer, self.screen.buffer.buffer_type, selection)
    }

    /// Returns the get current layer of this [`EditState`].
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn get_current_layer(&self) -> Result<usize> {
        let len = self.screen.buffer.layers.len();
        if len > 0 {
            Ok(self.screen.current_layer.clamp(0, len - 1))
        } else {
            Err(crate::EngineError::Generic("No layers".to_string()))
        }
    }

    pub fn set_current_layer(&mut self, layer: usize) {
        self.screen.current_layer = layer.clamp(0, self.screen.buffer.layers.len().saturating_sub(1));
    }

    pub fn get_current_tag(&self) -> Result<usize> {
        let len = self.screen.buffer.tags.len();
        if len > 0 { Ok(self.current_tag.clamp(0, len - 1)) } else { Ok(0) }
    }

    pub fn set_current_tag(&mut self, tag: usize) {
        let len = self.screen.buffer.tags.len();
        if len == 0 {
            return;
        }
        self.current_tag = tag.clamp(0, len.saturating_sub(1));
        self.screen.caret.attribute = self.screen.buffer.tags[self.current_tag].attribute;
    }

    pub fn get_outline_style(&self) -> usize {
        self.outline_style
    }

    pub fn set_outline_style(&mut self, outline_style: usize) {
        self.outline_style = outline_style;
    }

    #[must_use]
    pub fn begin_atomic_undo(&mut self, description: impl Into<String>) -> AtomicUndoGuard {
        self.begin_typed_atomic_undo(description, OperationType::Unknown)
    }

    #[must_use]
    pub fn begin_typed_atomic_undo(&mut self, description: impl Into<String>, operation_type: OperationType) -> AtomicUndoGuard {
        self.undo_stack.lock().unwrap().clear_redo();
        AtomicUndoGuard::new(description.into(), self.undo_stack.clone(), operation_type)
    }

    fn clamp_current_layer(&mut self) {
        self.screen.current_layer = self.screen.current_layer.clamp(0, self.screen.buffer.layers.len().saturating_sub(1));
    }

    /// Push and execute an undo operation
    pub(crate) fn push_undo_action(&mut self, mut op: EditorUndoOp) -> Result<()> {
        op.redo(self)?;
        self.push_plain_undo(op)
    }

    /// Push an undo operation without executing it
    pub(crate) fn push_plain_undo(&mut self, op: EditorUndoOp) -> Result<()> {
        if op.changes_data() {
            self.mark_dirty();
            self.screen.mark_dirty();
        }
        self.undo_stack.lock().unwrap().push(op);
        Ok(())
    }

    /// Returns the undo stack len of this [`EditState`].
    pub fn undo_stack_len(&self) -> usize {
        self.undo_stack.lock().unwrap().undo_len()
    }

    /// Get clone of the undo stack (for serialization)
    pub fn get_undo_stack(&self) -> Arc<Mutex<EditorUndoStack>> {
        self.undo_stack.clone()
    }

    /// Get the floating layer content as collaboration Blocks (for PasteAsSelection)
    /// Returns None if no floating layer exists
    #[cfg(feature = "collaboration")]
    pub fn get_floating_layer_blocks(&self) -> Option<crate::collaboration::Blocks> {
        use icy_engine::TextPane;

        let layer = self.get_cur_layer()?;

        let columns = layer.width() as u32;
        let rows = layer.height() as u32;
        let mut data = Vec::with_capacity((columns * rows) as usize);

        for y in 0..layer.height() {
            for x in 0..layer.width() {
                let ch = layer.char_at((x, y).into());
                data.push(crate::collaboration::Block {
                    code: ch.ch as u32,
                    fg: ch.attribute.foreground() as u8,
                    bg: ch.attribute.background() as u8,
                });
            }
        }

        Some(crate::collaboration::Blocks { columns, rows, data })
    }

    /// Get the floating layer position (offset)
    pub fn get_floating_layer_position(&self) -> Option<(i32, i32)> {
        let layer = self.get_cur_layer()?;
        let offset = layer.offset();
        Some((offset.x, offset.y))
    }

    pub fn get_mirror_mode(&self) -> bool {
        self.mirror_mode
    }

    pub fn set_mirror_mode(&mut self, mirror_mode: bool) {
        self.mirror_mode = mirror_mode;
    }

    /// Set the selection mask (without undo, just plain operation)
    #[inline(always)]
    pub(crate) fn set_selection_mask(&mut self, mask: SelectionMask) {
        #[cfg(debug_assertions)]
        eprintln!("[DEBUG] EditState::set_selection_mask - Setting selection mask");
        self.selection_mask = mask;
        self.screen.mark_dirty();
    }

    pub fn mark_dirty(&mut self) {
        self.screen.mark_dirty();
    }
}

impl UndoState for EditState {
    fn undo_description(&self) -> Option<String> {
        self.undo_stack.lock().unwrap().undo_description()
    }

    fn can_undo(&self) -> bool {
        self.undo_stack.lock().unwrap().can_undo()
    }

    fn undo(&mut self) -> Result<()> {
        let Some(mut op) = self.undo_stack.lock().unwrap().pop_undo() else {
            return Ok(());
        };
        if op.changes_data() {
            self.mark_dirty();
        }

        let res = op.undo(self);
        self.undo_stack.lock().unwrap().push_redo(op);
        res
    }

    fn redo_description(&self) -> Option<String> {
        self.undo_stack.lock().unwrap().redo_description()
    }

    fn can_redo(&self) -> bool {
        self.undo_stack.lock().unwrap().can_redo()
    }

    fn redo(&mut self) -> Result<()> {
        let Some(mut op) = self.undo_stack.lock().unwrap().pop_redo() else {
            return Ok(());
        };
        if op.changes_data() {
            self.mark_dirty();
        }

        let res = op.redo(self);
        self.undo_stack.lock().unwrap().push_undo(op);
        res
    }
}

// Delegate TextPane to inner screen
impl TextPane for EditState {
    fn char_at(&self, pos: Position) -> AttributedChar {
        self.screen.char_at(pos)
    }

    fn line_count(&self) -> i32 {
        TextPane::line_count(&self.screen)
    }

    fn width(&self) -> i32 {
        self.screen.width()
    }

    fn height(&self) -> i32 {
        self.screen.height()
    }

    fn line_length(&self, line: i32) -> i32 {
        self.screen.line_length(line)
    }

    fn rectangle(&self) -> Rectangle {
        self.screen.rectangle()
    }

    fn size(&self) -> Size {
        self.screen.size()
    }
}

// Delegate Screen to inner screen
impl Screen for EditState {
    fn buffer_type(&self) -> crate::BufferType {
        self.screen.buffer_type()
    }

    fn use_letter_spacing(&self) -> bool {
        self.screen.use_letter_spacing()
    }

    fn use_aspect_ratio(&self) -> bool {
        self.screen.use_aspect_ratio()
    }

    fn scan_lines(&self) -> bool {
        self.screen.scan_lines()
    }

    fn ice_mode(&self) -> IceMode {
        self.screen.ice_mode()
    }

    fn caret(&self) -> &Caret {
        self.screen.caret()
    }

    fn terminal_state(&self) -> &TerminalState {
        self.screen.terminal_state()
    }

    fn palette(&self) -> &Palette {
        self.screen.palette()
    }

    fn render_to_rgba(&self, options: &RenderOptions) -> (Size, Vec<u8>) {
        self.screen.render_to_rgba(options)
    }

    fn render_region_to_rgba(&self, px_region: Rectangle, options: &RenderOptions) -> (Size, Vec<u8>) {
        self.screen.render_region_to_rgba(px_region, options)
    }

    fn font(&self, font_number: usize) -> Option<&BitFont> {
        self.screen.font(font_number)
    }

    fn font_count(&self) -> usize {
        self.screen.font_count()
    }

    fn font_dimensions(&self) -> Size {
        self.screen.font_dimensions()
    }

    fn selection(&self) -> Option<Selection> {
        self.selection_opt
    }

    fn selection_mask(&self) -> &SelectionMask {
        &self.selection_mask
    }

    fn hyperlinks(&self) -> &Vec<HyperLink> {
        self.screen.hyperlinks()
    }

    fn to_bytes(&mut self, extension: &str, options: &AnsiSaveOptionsV2) -> Result<Vec<u8>> {
        self.screen.to_bytes(extension, options)
    }

    fn copy_text(&self) -> Option<String> {
        let selection = self.selection_opt.as_ref()?;
        clipboard::text(&self.screen.buffer, self.screen.buffer.buffer_type, selection)
    }

    fn copy_rich_text(&self) -> Option<String> {
        let selection = self.selection_opt.as_ref()?;
        clipboard::get_rich_text(&self.screen.buffer, selection)
    }

    fn clipboard_data(&self) -> Option<Vec<u8>> {
        // Use EditState's own selection_mask and selection_opt, not screen's
        clipboard::clipboard_data(&self.screen.buffer, self.screen.current_layer, &self.selection_mask, &self.selection_opt)
    }

    fn mouse_fields(&self) -> &Vec<MouseField> {
        self.screen.mouse_fields()
    }

    fn version(&self) -> u64 {
        self.screen.version()
    }

    fn default_foreground_color(&self) -> u32 {
        self.screen.default_foreground_color()
    }

    fn max_base_colors(&self) -> u32 {
        self.screen.max_base_colors()
    }

    fn resolution(&self) -> Size {
        self.screen.resolution()
    }

    fn virtual_size(&self) -> Size {
        self.screen.virtual_size()
    }

    fn screen(&self) -> &[u8] {
        self.screen.screen()
    }

    fn set_scrollback_buffer_size(&mut self, buffer_size: usize) {
        self.screen.set_scrollback_buffer_size(buffer_size)
    }

    fn set_selection(&mut self, sel: Selection) -> Result<()> {
        if self.selection_opt.as_ref() != Some(&sel) {
            self.selection_opt = Some(sel);
            self.mark_dirty();
        }
        Ok(())
    }

    fn clear_selection(&mut self) -> Result<()> {
        if self.selection_opt.is_some() {
            self.selection_opt = None;
            self.mark_dirty();
        }
        Ok(())
    }

    fn as_editable(&mut self) -> Option<&mut dyn EditableScreen> {
        Some(self)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Screen> {
        self.screen.clone_box()
    }
}

// Delegate EditableScreen to inner screen
impl EditableScreen for EditState {
    fn snapshot_scrollback(&mut self) -> Option<std::sync::Arc<ParkingMutex<Box<dyn Screen>>>> {
        self.screen.snapshot_scrollback()
    }

    fn first_visible_line(&self) -> i32 {
        self.screen.first_visible_line()
    }

    fn last_visible_line(&self) -> i32 {
        self.screen.last_visible_line()
    }

    fn first_editable_line(&self) -> i32 {
        self.screen.first_editable_line()
    }

    fn last_editable_line(&self) -> i32 {
        self.screen.last_editable_line()
    }

    fn first_editable_column(&self) -> i32 {
        self.screen.first_editable_column()
    }

    fn last_editable_column(&self) -> i32 {
        self.screen.last_editable_column()
    }

    fn get_line(&self, line: usize) -> Option<&Line> {
        self.screen.get_line(line)
    }

    fn physical_line_count(&self) -> usize {
        EditableScreen::physical_line_count(&self.screen)
    }

    fn set_resolution(&mut self, size: Size) {
        self.screen.set_resolution(size)
    }

    fn screen_mut(&mut self) -> &mut Vec<u8> {
        self.screen.screen_mut()
    }

    fn set_graphics_type(&mut self, graphics_type: crate::GraphicsType) {
        self.screen.set_graphics_type(graphics_type)
    }

    fn update_hyperlinks(&mut self) {
        self.screen.update_hyperlinks()
    }

    fn clear_line(&mut self) {
        self.screen.clear_line()
    }

    fn clear_line_end(&mut self) {
        self.screen.clear_line_end()
    }

    fn clear_line_start(&mut self) {
        self.screen.clear_line_start()
    }

    fn clear_mouse_fields(&mut self) {
        self.screen.clear_mouse_fields()
    }

    fn add_mouse_field(&mut self, mouse_field: MouseField) {
        self.screen.add_mouse_field(mouse_field)
    }

    fn ice_mode_mut(&mut self) -> &mut IceMode {
        self.screen.ice_mode_mut()
    }

    fn buffer_type_mut(&mut self) -> &mut crate::BufferType {
        self.screen.buffer_type_mut()
    }

    fn caret_mut(&mut self) -> &mut Caret {
        self.screen.caret_mut()
    }

    fn palette_mut(&mut self) -> &mut Palette {
        self.screen.palette_mut()
    }

    fn terminal_state_mut(&mut self) -> &mut TerminalState {
        self.screen.terminal_state_mut()
    }

    fn reset_terminal(&mut self) {
        self.screen.reset_terminal()
    }

    fn set_char(&mut self, pos: Position, ch: AttributedChar) {
        self.screen.set_char(pos, ch)
    }

    fn set_size(&mut self, size: Size) {
        self.screen.set_size(size)
    }

    fn scroll_up(&mut self) {
        self.screen.scroll_up()
    }

    fn scroll_down(&mut self) {
        self.screen.scroll_down()
    }

    fn scroll_left(&mut self) {
        self.screen.scroll_left()
    }

    fn scroll_right(&mut self) {
        self.screen.scroll_right()
    }

    fn add_sixel(&mut self, pos: Position, sixel: Sixel) {
        self.screen.add_sixel(pos, sixel)
    }

    fn insert_line(&mut self, line: usize, new_line: Line) {
        self.screen.insert_line(line, new_line)
    }

    fn set_width(&mut self, width: i32) {
        self.screen.set_width(width)
    }

    fn set_height(&mut self, height: i32) {
        self.screen.set_height(height)
    }

    fn add_hyperlink(&mut self, link: HyperLink) {
        self.screen.add_hyperlink(link)
    }

    fn set_font(&mut self, font_number: usize, font: BitFont) {
        self.screen.set_font(font_number, font)
    }

    fn remove_font(&mut self, font_number: usize) -> Option<BitFont> {
        self.screen.remove_font(font_number)
    }

    fn clear_font_table(&mut self) {
        self.screen.clear_font_table()
    }

    fn clear_scrollback(&mut self) {
        self.screen.clear_scrollback()
    }

    fn remove_terminal_line(&mut self, line: i32) {
        self.screen.remove_terminal_line(line)
    }

    fn insert_terminal_line(&mut self, line: i32) {
        self.screen.insert_terminal_line(line)
    }

    fn clear_screen(&mut self) {
        self.screen.clear_screen()
    }

    fn mark_dirty(&self) {
        self.screen.mark_dirty()
    }

    fn layer_count(&self) -> usize {
        self.screen.layer_count()
    }

    fn get_current_layer(&self) -> usize {
        self.screen.get_current_layer()
    }

    fn set_current_layer(&mut self, layer: usize) -> Result<()> {
        self.screen.set_current_layer(layer)
    }

    fn get_layer(&self, layer: usize) -> Option<&Layer> {
        self.screen.get_layer(layer)
    }

    fn get_layer_mut(&mut self, layer: usize) -> Option<&mut Layer> {
        self.screen.get_layer_mut(layer)
    }

    fn saved_caret_pos(&mut self) -> &mut Position {
        self.screen.saved_caret_pos()
    }

    fn saved_cursor_state(&mut self) -> &mut SavedCaretState {
        self.screen.saved_cursor_state()
    }

    fn handle_rip_command(&mut self, cmd: RipCommand) {
        self.screen.handle_rip_command(cmd)
    }

    fn handle_skypix_command(&mut self, cmd: SkypixCommand) {
        self.screen.handle_skypix_command(cmd)
    }

    fn handle_igs_command(&mut self, cmd: IgsCommand) {
        self.screen.handle_igs_command(cmd)
    }

    fn set_aspect_ratio(&mut self, enabled: bool) {
        self.screen.set_aspect_ratio(enabled)
    }

    fn set_letter_spacing(&mut self, enabled: bool) {
        self.screen.set_letter_spacing(enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::{Size, TextBuffer};

    fn create_test_edit_state() -> EditState {
        let buffer = TextBuffer::create(Size::new(80, 25));
        EditState::from_buffer(buffer)
    }

    fn create_test_edit_state_with_layer_offset(offset: Position) -> EditState {
        let mut buffer = TextBuffer::create(Size::new(80, 25));
        if let Some(layer) = buffer.layers.get_mut(0) {
            layer.set_offset(offset);
        }
        EditState::from_buffer(buffer)
    }

    #[test]
    fn test_document_to_layer_position_no_offset() {
        let state = create_test_edit_state();

        // With no offset, document and layer positions should be the same
        let doc_pos = Position::new(10, 5);
        let layer_pos = state.document_to_layer_position(doc_pos);

        assert_eq!(layer_pos, Position::new(10, 5));
    }

    #[test]
    fn test_document_to_layer_position_with_offset() {
        let state = create_test_edit_state_with_layer_offset(Position::new(10, 5));

        // Document position (15, 8) with layer offset (10, 5) should give layer position (5, 3)
        let doc_pos = Position::new(15, 8);
        let layer_pos = state.document_to_layer_position(doc_pos);

        assert_eq!(layer_pos, Position::new(5, 3));
    }

    #[test]
    fn test_layer_to_document_position_no_offset() {
        let state = create_test_edit_state();

        // With no offset, layer and document positions should be the same
        let layer_pos = Position::new(10, 5);
        let doc_pos = state.layer_to_document_position(layer_pos);

        assert_eq!(doc_pos, Position::new(10, 5));
    }

    #[test]
    fn test_layer_to_document_position_with_offset() {
        let state = create_test_edit_state_with_layer_offset(Position::new(10, 5));

        // Layer position (5, 3) with layer offset (10, 5) should give document position (15, 8)
        let layer_pos = Position::new(5, 3);
        let doc_pos = state.layer_to_document_position(layer_pos);

        assert_eq!(doc_pos, Position::new(15, 8));
    }

    #[test]
    fn test_document_layer_position_roundtrip() {
        let state = create_test_edit_state_with_layer_offset(Position::new(20, 10));

        // Converting doc->layer->doc should give the original position
        let original_doc_pos = Position::new(50, 30);
        let layer_pos = state.document_to_layer_position(original_doc_pos);
        let roundtrip_doc_pos = state.layer_to_document_position(layer_pos);

        assert_eq!(roundtrip_doc_pos, original_doc_pos);
    }

    #[test]
    fn test_set_caret_from_document_position() {
        let mut state = create_test_edit_state_with_layer_offset(Position::new(10, 5));

        // Clicking at document position (15, 8) with layer offset (10, 5)
        // should set caret to layer position (5, 3)
        state.set_caret_from_document_position(Position::new(15, 8));

        let caret_pos = state.get_caret().position();
        assert_eq!(caret_pos, Position::new(5, 3));
    }

    #[test]
    fn test_set_caret_from_document_position_negative_result() {
        let mut state = create_test_edit_state_with_layer_offset(Position::new(10, 5));

        // Clicking at document position (5, 2) with layer offset (10, 5)
        // should set caret to layer position (-5, -3) - this is valid for layers
        state.set_caret_from_document_position(Position::new(5, 2));

        let caret_pos = state.get_caret().position();
        assert_eq!(caret_pos, Position::new(-5, -3));
    }
}
