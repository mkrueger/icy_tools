pub mod undo_stack;
use std::sync::{Arc, Mutex};

pub use undo_stack::*;

mod undo_operations;

mod editor_error;
pub use editor_error::*;

mod layer_operations;
pub use layer_operations::*;
mod area_operations;
mod edit_operations;
pub use area_operations::*;
mod font_operations;
mod selection_operations;
mod tag_operations;

use crate::{Caret, EngineResult, Layer, SauceMetaData, Selection, SelectionMask, TextBuffer, TextPane, clipboard, overlay_mask::OverlayMask};

pub struct EditState {
    buffer: TextBuffer,
    caret: Caret,
    selection_opt: Option<Selection>,
    selection_mask: SelectionMask,
    tool_overlay_mask: OverlayMask,

    current_layer: usize,
    current_tag: usize,

    outline_style: usize,
    mirror_mode: bool,

    undo_stack: Arc<Mutex<Vec<Box<dyn UndoOperation>>>>,
    redo_stack: Vec<Box<dyn UndoOperation>>,

    pub is_palette_dirty: bool,
    is_buffer_dirty: bool,

    /// SAUCE metadata for the file (title, author, group, comments)
    sauce_meta: SauceMetaData,
}

pub struct AtomicUndoGuard {
    base_count: usize,
    description: String,
    operation_type: OperationType,

    undo_stack: Arc<Mutex<Vec<Box<dyn UndoOperation>>>>,
}

impl AtomicUndoGuard {
    fn new(description: String, undo_stack: Arc<Mutex<Vec<Box<dyn UndoOperation>>>>, operation_type: OperationType) -> Self {
        let base_count = undo_stack.lock().unwrap().len();
        Self {
            base_count,
            description,
            operation_type,
            undo_stack,
        }
    }

    pub fn end(&mut self) {
        self.end_action();
    }

    fn end_action(&mut self) {
        let stack = self.undo_stack.lock().unwrap().drain(self.base_count..).collect();
        let stack = Arc::new(Mutex::new(stack));
        self.undo_stack
            .lock()
            .unwrap()
            .push(Box::new(undo_operations::AtomicUndo::new(self.description.clone(), stack, self.operation_type)));
        self.base_count = usize::MAX;
    }
}

impl Drop for AtomicUndoGuard {
    fn drop(&mut self) {
        let count = self.undo_stack.lock().unwrap().len();
        if self.base_count >= count {
            return;
        }
        self.end_action();
    }
}

impl Default for EditState {
    fn default() -> Self {
        let buffer = TextBuffer::default();
        let mut selection_mask = SelectionMask::default();
        selection_mask.set_size(buffer.get_size());
        let mut tool_overlay_mask = OverlayMask::default();
        tool_overlay_mask.set_size(buffer.get_size());

        Self {
            buffer,
            caret: Caret::default(),
            selection_opt: None,
            undo_stack: Arc::new(Mutex::new(Vec::new())),
            redo_stack: Vec::new(),
            current_layer: 0,
            current_tag: 0,
            outline_style: 0,
            mirror_mode: false,
            selection_mask,
            tool_overlay_mask,
            is_palette_dirty: false,
            is_buffer_dirty: false,
            sauce_meta: SauceMetaData::default(),
        }
    }
}

impl EditState {
    pub fn from_buffer(buffer: TextBuffer) -> Self {
        let mut selection_mask = SelectionMask::default();
        selection_mask.set_size(buffer.get_size());
        let mut tool_overlay_mask = OverlayMask::default();
        tool_overlay_mask.set_size(buffer.get_size());

        Self {
            buffer,
            selection_mask,
            tool_overlay_mask,
            ..Default::default()
        }
    }

    pub fn set_buffer(&mut self, buffer: TextBuffer) {
        self.buffer = buffer;
        self.set_mask_size();
    }

    pub fn set_mask_size(&mut self) {
        self.selection_mask.set_size(self.buffer.get_size());
        self.tool_overlay_mask.set_size(self.buffer.get_size());
    }

    pub fn get_tool_overlay_mask(&self) -> &OverlayMask {
        &self.tool_overlay_mask
    }

    pub fn get_tool_overlay_mask_mut(&mut self) -> &mut OverlayMask {
        &mut self.tool_overlay_mask
    }

    pub fn get_buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    pub fn get_buffer_mut(&mut self) -> &mut TextBuffer {
        &mut self.buffer
    }

    pub fn is_buffer_dirty(&self) -> bool {
        self.is_buffer_dirty
    }

    pub fn set_is_buffer_dirty(&mut self) {
        self.is_buffer_dirty = true;
    }

    pub fn set_buffer_clean(&mut self) {
        self.is_buffer_dirty = false;
    }

    /// Get a reference to the SAUCE metadata
    pub fn get_sauce_meta(&self) -> &SauceMetaData {
        &self.sauce_meta
    }

    /// Get a mutable reference to the SAUCE metadata
    pub fn get_sauce_meta_mut(&mut self) -> &mut SauceMetaData {
        &mut self.sauce_meta
    }

    /// Set the SAUCE metadata
    pub fn set_sauce_meta(&mut self, sauce_meta: SauceMetaData) {
        self.sauce_meta = sauce_meta;
    }

    pub fn get_cur_layer(&self) -> Option<&Layer> {
        if let Ok(layer) = self.get_current_layer() {
            self.buffer.layers.get(layer)
        } else {
            None
        }
    }

    pub fn get_cur_display_layer(&self) -> Option<&Layer> {
        if let Ok(layer) = self.get_current_layer() {
            self.buffer.layers.get(layer)
        } else {
            None
        }
    }

    pub fn get_cur_layer_mut(&mut self) -> Option<&mut Layer> {
        if let Ok(layer) = self.get_current_layer() {
            self.buffer.layers.get_mut(layer)
        } else {
            None
        }
    }

    pub fn get_caret(&self) -> &Caret {
        &self.caret
    }

    pub fn get_caret_mut(&mut self) -> &mut Caret {
        &mut self.caret
    }

    pub fn get_buffer_and_caret_mut(&mut self) -> (&mut TextBuffer, &mut Caret) {
        (&mut self.buffer, &mut self.caret)
    }

    pub fn get_copy_text(&self) -> Option<String> {
        let Some(selection) = &self.selection_opt else {
            return None;
        };
        clipboard::get_text(&self.buffer, self.buffer.buffer_type, selection)
    }

    pub fn get_overlay_layer(&mut self, cur_layer: usize) -> &mut Layer {
        self.buffer.get_overlay_layer(cur_layer)
    }

    /// Returns the get current layer of this [`EditState`].
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn get_current_layer(&self) -> EngineResult<usize> {
        let len = self.buffer.layers.len();
        if len > 0 {
            Ok(self.current_layer.clamp(0, len - 1))
        } else {
            Err(anyhow::anyhow!("No layers"))
        }
    }

    pub fn set_current_layer(&mut self, layer: usize) {
        self.current_layer = layer.clamp(0, self.buffer.layers.len().saturating_sub(1));
    }

    pub fn get_current_tag(&self) -> EngineResult<usize> {
        let len = self.buffer.tags.len();
        if len > 0 { Ok(self.current_tag.clamp(0, len - 1)) } else { Ok(0) }
    }

    pub fn set_current_tag(&mut self, tag: usize) {
        let len = self.buffer.tags.len();
        self.current_tag = tag.clamp(0, len.saturating_sub(1));
        self.caret.attribute = self.buffer.tags[self.current_tag].attribute;
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
        self.redo_stack.clear();
        AtomicUndoGuard::new(description.into(), self.undo_stack.clone(), operation_type)
    }

    fn clamp_current_layer(&mut self) {
        self.current_layer = self.current_layer.clamp(0, self.buffer.layers.len().saturating_sub(1));
    }

    fn push_undo_action(&mut self, mut op: Box<dyn UndoOperation>) -> EngineResult<()> {
        op.redo(self)?;
        self.push_plain_undo(op)
    }

    fn push_plain_undo(&mut self, op: Box<dyn UndoOperation>) -> EngineResult<()> {
        if op.changes_data() {
            self.set_is_buffer_dirty();
        }
        let Ok(mut stack) = self.undo_stack.lock() else {
            return Err(anyhow::anyhow!("Failed to lock undo stack"));
        };
        stack.push(op);
        self.redo_stack.clear();
        Ok(())
    }

    /// Returns the undo stack len of this [`EditState`].
    ///
    /// # Panics
    ///
    /// Panics if .
    pub fn undo_stack_len(&self) -> usize {
        self.undo_stack.lock().unwrap().len()
    }

    pub fn get_undo_stack(&self) -> Arc<Mutex<Vec<Box<dyn UndoOperation>>>> {
        self.undo_stack.clone()
    }

    pub fn has_floating_layer(&self) -> bool {
        for layer in &self.buffer.layers {
            if layer.role.is_paste() {
                return true;
            }
        }
        false
    }

    pub fn get_mirror_mode(&self) -> bool {
        self.mirror_mode
    }

    pub fn set_mirror_mode(&mut self, mirror_mode: bool) {
        self.mirror_mode = mirror_mode;
    }
}

impl UndoState for EditState {
    fn undo_description(&self) -> Option<String> {
        self.undo_stack.lock().unwrap().last().map(|op| op.get_description())
    }

    fn can_undo(&self) -> bool {
        !self.undo_stack.lock().unwrap().is_empty()
    }

    fn undo(&mut self) -> EngineResult<()> {
        let Some(mut op) = self.undo_stack.lock().unwrap().pop() else {
            return Ok(());
        };
        if op.changes_data() {
            self.set_is_buffer_dirty();
        }

        let res = op.undo(self);
        self.redo_stack.push(op);
        res
    }

    fn redo_description(&self) -> Option<String> {
        self.redo_stack.last().map(|op| op.get_description())
    }

    fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    fn redo(&mut self) -> EngineResult<()> {
        if let Some(mut op) = self.redo_stack.pop() {
            if op.changes_data() {
                self.set_is_buffer_dirty();
            }
            let res = op.redo(self);
            self.undo_stack.lock().unwrap().push(op);
            return res;
        }
        Ok(())
    }
}
