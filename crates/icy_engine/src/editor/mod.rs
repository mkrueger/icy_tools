pub mod undo_stack;
use std::sync::{Arc, Mutex};

use i18n_embed_fl::fl;
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
mod rtf_generator;
mod selection_operations;
mod tag_operations;

use crate::{
    AttributedChar, Buffer, BufferType, Caret, EngineResult, Layer, Position, Role, Selection, SelectionMask, Shape, TextAttribute, TextPane, UnicodeConverter,
    ascii, overlay_mask::OverlayMask,
};

pub struct EditState {
    buffer: Buffer,
    caret: Caret,
    selection_opt: Option<Selection>,
    selection_mask: SelectionMask,
    tool_overlay_mask: OverlayMask,
    unicode_converter: Box<dyn UnicodeConverter>,

    current_layer: usize,
    current_tag: usize,

    outline_style: usize,
    mirror_mode: bool,

    undo_stack: Arc<Mutex<Vec<Box<dyn UndoOperation>>>>,
    redo_stack: Vec<Box<dyn UndoOperation>>,

    pub is_palette_dirty: bool,
    is_buffer_dirty: bool,

    pub scrollback_buffer: Option<Buffer>,
    pub scrollback_offset: usize,
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
        let buffer = Buffer::default();
        let mut selection_mask = SelectionMask::default();
        selection_mask.set_size(buffer.get_size());
        let mut tool_overlay_mask = OverlayMask::default();
        tool_overlay_mask.set_size(buffer.get_size());

        Self {
            unicode_converter: Box::<ascii::CP437Converter>::default(),
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

            scrollback_buffer: None,
            scrollback_offset: 0,
        }
    }
}

impl EditState {
    pub fn get_max_scrollback_offset(&self) -> usize {
        self.buffer.scrollback_lines.len()
    }
    pub fn set_scroll_position(&mut self, line: usize) {
        self.scrollback_offset = line;
        if line == 0 {
            self.scrollback_buffer = None;
            return;
        }

        // Create a new buffer with same settings as current buffer
        let mut scroll_buffer = Buffer::new(self.buffer.get_size());

        // Copy buffer settings
        scroll_buffer.is_terminal_buffer = self.buffer.is_terminal_buffer;
        scroll_buffer.terminal_state = self.buffer.terminal_state.clone();
        scroll_buffer.buffer_type = self.buffer.buffer_type;
        scroll_buffer.palette = self.buffer.palette.clone();

        let viewport_height = self.buffer.terminal_state.get_height();
        let total_scrollback_lines = self.buffer.scrollback_lines.len();

        // Calculate which lines to show
        // line = 0 means bottom (no scroll)
        // line = total_scrollback_lines means top of scrollback
        let start_line = total_scrollback_lines.saturating_sub(line);
        let end_line = (start_line + viewport_height as usize).min(total_scrollback_lines + self.buffer.layers[0].lines.len());

        // Clear the default layer and resize if needed
        scroll_buffer.layers.clear();
        scroll_buffer.layers.push(Layer::new("scrollback_view".to_string(), self.buffer.get_size()));

        let mut y = 0;

        // Copy lines from scrollback buffer
        for i in start_line..total_scrollback_lines.min(end_line) {
            if let Some(scrollback_line) = self.buffer.scrollback_lines.get(i) {
                if y < viewport_height {
                    // Copy the line to the scroll buffer
                    if y >= scroll_buffer.layers[0].lines.len() as i32 {
                        scroll_buffer.layers[0].lines.push(scrollback_line.clone());
                    } else {
                        scroll_buffer.layers[0].lines[y as usize] = scrollback_line.clone();
                    }
                    y += 1;
                }
            }
        }

        // If we haven't filled the viewport, add lines from the current buffer
        let remaining_lines = viewport_height - y;
        if remaining_lines > 0 && start_line < total_scrollback_lines {
            // We're showing a mix of scrollback and current buffer
            for i in 0..remaining_lines.min(self.buffer.layers[0].lines.len() as i32) {
                if y < viewport_height {
                    if let Some(current_line) = self.buffer.layers[0].lines.get(i as usize) {
                        if y >= scroll_buffer.layers[0].lines.len() as i32 {
                            scroll_buffer.layers[0].lines.push(current_line.clone());
                        } else {
                            scroll_buffer.layers[0].lines[y as usize] = current_line.clone();
                        }
                        y += 1;
                    }
                }
            }
        } else if start_line >= total_scrollback_lines {
            // We're only showing current buffer (scrolled to bottom area)
            let current_start = line.saturating_sub(total_scrollback_lines);
            for i in current_start..(current_start + viewport_height as usize).min(self.buffer.layers[0].lines.len()) {
                if let Some(current_line) = self.buffer.layers[0].lines.get(i) {
                    if y >= scroll_buffer.layers[0].lines.len() as i32 {
                        scroll_buffer.layers[0].lines.push(current_line.clone());
                    } else {
                        scroll_buffer.layers[0].lines[y as usize] = current_line.clone();
                    }
                    y += 1;
                }
            }
        }

        scroll_buffer.update_hyperlinks();
        self.scrollback_buffer = Some(scroll_buffer);
    }

    pub fn clear_scrollback_buffer(&mut self) {
        self.buffer.scrollback_lines.clear();
        self.set_scroll_position(0);
    }

    pub fn from_buffer(buffer: Buffer) -> Self {
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

    pub fn set_unicode_converter(&mut self, parser: Box<dyn UnicodeConverter>) {
        self.unicode_converter = parser;
    }

    pub fn get_unicode_converter(&self) -> &dyn UnicodeConverter {
        &*self.unicode_converter
    }

    pub fn set_buffer(&mut self, buffer: Buffer) {
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

    pub fn get_display_buffer(&self) -> &Buffer {
        if let Some(ref scrollback) = self.scrollback_buffer {
            scrollback
        } else {
            &self.buffer
        }
    }

    pub fn get_buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn get_buffer_mut(&mut self) -> &mut Buffer {
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

    pub fn get_cur_layer(&self) -> Option<&Layer> {
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

    pub fn get_buffer_and_caret_mut(&mut self) -> (&mut Buffer, &mut Caret, &mut Box<dyn UnicodeConverter>) {
        (&mut self.buffer, &mut self.caret, &mut self.unicode_converter)
    }

    pub fn get_copy_text(&self) -> Option<String> {
        let Some(selection) = &self.selection_opt else {
            return None;
        };

        let mut res = String::new();
        if matches!(selection.shape, Shape::Rectangle) {
            let start = selection.min();
            let end = selection.max();
            for y in start.y..=end.y {
                for x in start.x..=end.x {
                    let ch = self.buffer.get_char((x, y));
                    res.push(self.unicode_converter.convert_to_unicode(ch));
                }
                res.push('\n');
            }
        } else {
            let (start, end) = if selection.anchor < selection.lead {
                (selection.anchor, selection.lead)
            } else {
                (selection.lead, selection.anchor)
            };
            if start.y == end.y {
                for x in start.x..=end.x {
                    let ch = self.buffer.get_char(Position::new(x, start.y));
                    res.push(self.unicode_converter.convert_to_unicode(ch));
                }
            } else {
                for x in start.x..(self.buffer.get_line_length(start.y)) {
                    let ch = self.buffer.get_char(Position::new(x, start.y));
                    res.push(self.unicode_converter.convert_to_unicode(ch));
                }
                res.push('\n');
                for y in start.y + 1..end.y {
                    for x in 0..(self.buffer.get_line_length(y)) {
                        let ch = self.buffer.get_char(Position::new(x, y));
                        res.push(self.unicode_converter.convert_to_unicode(ch));
                    }
                    res.push('\n');
                }
                for x in 0..=end.x {
                    let ch = self.buffer.get_char(Position::new(x, end.y));
                    res.push(self.unicode_converter.convert_to_unicode(ch));
                }
            }
        }
        Some(res)
    }

    pub fn get_clipboard_data(&self) -> Option<Vec<u8>> {
        if !self.is_something_selected() {
            return None;
        };
        let Some(layer) = self.get_cur_layer() else {
            return None;
        };

        let selection = self.get_selected_rectangle();

        let mut data = Vec::new();
        data.push(0);
        data.extend(i32::to_le_bytes(selection.start.x));
        data.extend(i32::to_le_bytes(selection.start.y));

        data.extend(u32::to_le_bytes(selection.get_size().width as u32));
        data.extend(u32::to_le_bytes(selection.get_size().height as u32));
        let need_convert_to_unicode = self.buffer.buffer_type != BufferType::Unicode;
        for y in selection.y_range() {
            for x in selection.x_range() {
                let pos = Position::new(x, y);
                let ch = if self.get_is_selected((x, y)) {
                    layer.get_char(pos - layer.get_offset())
                } else {
                    AttributedChar::invisible()
                };
                let c = if need_convert_to_unicode {
                    self.unicode_converter.convert_to_unicode(ch)
                } else {
                    ch.ch
                };
                data.extend(u32::to_le_bytes(c as u32));
                data.extend(u16::to_le_bytes(ch.attribute.attr));
                data.extend(u16::to_le_bytes(ch.attribute.font_page as u16));
                data.extend(u32::to_le_bytes(ch.attribute.background_color));
                data.extend(u32::to_le_bytes(ch.attribute.foreground_color));
            }
        }
        Some(data)
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    pub fn from_clipboard_data(&self, data: &[u8]) -> Option<Layer> {
        if data[0] != 0 {
            return None;
        }
        let x = i32::from_le_bytes(data[1..5].try_into().unwrap());
        let y = i32::from_le_bytes(data[5..9].try_into().unwrap());
        let width = u32::from_le_bytes(data[9..13].try_into().unwrap()) as usize;
        let height = u32::from_le_bytes(data[13..17].try_into().unwrap()) as usize;
        let mut data = &data[17..];

        let mut layer = Layer::new(fl!(crate::LANGUAGE_LOADER, "layer-pasted-name"), (width, height));
        layer.properties.has_alpha_channel = true;
        layer.role = Role::PastePreview;
        layer.set_offset((x, y));
        let need_convert_to_unicode = self.buffer.buffer_type != BufferType::Unicode;
        for y in 0..height {
            for x in 0..width {
                let mut ch = unsafe { char::from_u32_unchecked(u32::from_le_bytes(data[0..4].try_into().unwrap())) };
                if need_convert_to_unicode {
                    let font_page = self.caret.get_font_page();
                    ch = self.unicode_converter.convert_from_unicode(ch, font_page);
                }
                let attr_ch = AttributedChar {
                    ch,
                    attribute: TextAttribute {
                        attr: u16::from_le_bytes(data[4..6].try_into().unwrap()),
                        font_page: u16::from_le_bytes(data[6..8].try_into().unwrap()) as usize,
                        background_color: u32::from_le_bytes(data[8..12].try_into().unwrap()),
                        foreground_color: u32::from_le_bytes(data[12..16].try_into().unwrap()),
                    },
                };
                layer.set_char((x as i32, y as i32), attr_ch);
                data = &data[16..];
            }
        }
        Some(layer)
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
