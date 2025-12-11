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

use crate::{
    AttributedChar, BitFont, Caret, EditableScreen, HyperLink, IceMode, Layer, Line, MouseField, Palette, Position, Rectangle, RenderOptions, Result,
    SauceMetaData, SaveOptions, SavedCaretState, Screen, Selection, SelectionMask, Sixel, Size, TerminalState, TextBuffer, TextPane, TextScreen, clipboard,
    overlay_mask::OverlayMask,
};
use icy_parser_core::{IgsCommand, RipCommand, SkypixCommand};
use parking_lot::Mutex as ParkingMutex;

pub struct EditState {
    screen: TextScreen,
    tool_overlay_mask: OverlayMask,

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
        let screen = TextScreen::default();
        let mut tool_overlay_mask = OverlayMask::default();
        tool_overlay_mask.set_size(screen.buffer.get_size());

        Self {
            screen,
            undo_stack: Arc::new(Mutex::new(Vec::new())),
            redo_stack: Vec::new(),
            current_tag: 0,
            outline_style: 0,
            mirror_mode: false,
            tool_overlay_mask,
            is_palette_dirty: false,
            is_buffer_dirty: false,
            sauce_meta: SauceMetaData::default(),
        }
    }
}

impl EditState {
    pub fn from_buffer(buffer: TextBuffer) -> Self {
        let screen = TextScreen::from_buffer(buffer);
        let mut tool_overlay_mask = OverlayMask::default();
        tool_overlay_mask.set_size(screen.buffer.get_size());

        Self {
            screen,
            tool_overlay_mask,
            ..Default::default()
        }
    }

    pub fn set_buffer(&mut self, buffer: TextBuffer) {
        self.screen.buffer = buffer;
        self.set_mask_size();
    }

    pub fn set_mask_size(&mut self) {
        self.screen.selection_mask.set_size(self.screen.buffer.get_size());
        self.tool_overlay_mask.set_size(self.screen.buffer.get_size());
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

    pub fn get_caret_mut(&mut self) -> &mut Caret {
        &mut self.screen.caret
    }

    pub fn get_buffer_and_caret_mut(&mut self) -> (&mut TextBuffer, &mut Caret) {
        (&mut self.screen.buffer, &mut self.screen.caret)
    }

    pub fn get_copy_text(&self) -> Option<String> {
        let Some(selection) = &self.screen.selection_opt else {
            return None;
        };
        clipboard::get_text(&self.screen.buffer, self.screen.buffer.buffer_type, selection)
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
        self.redo_stack.clear();
        AtomicUndoGuard::new(description.into(), self.undo_stack.clone(), operation_type)
    }

    fn clamp_current_layer(&mut self) {
        self.screen.current_layer = self.screen.current_layer.clamp(0, self.screen.buffer.layers.len().saturating_sub(1));
    }

    fn push_undo_action(&mut self, mut op: Box<dyn UndoOperation>) -> Result<()> {
        op.redo(self)?;
        self.push_plain_undo(op)
    }

    fn push_plain_undo(&mut self, op: Box<dyn UndoOperation>) -> Result<()> {
        if op.changes_data() {
            self.set_is_buffer_dirty();
        }
        let Ok(mut stack) = self.undo_stack.lock() else {
            return Err(crate::EngineError::Generic("Failed to lock undo stack".to_string()));
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
        for layer in &self.screen.buffer.layers {
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

    fn undo(&mut self) -> Result<()> {
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

    fn redo(&mut self) -> Result<()> {
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

// Delegate TextPane to inner screen
impl TextPane for EditState {
    fn get_char(&self, pos: Position) -> AttributedChar {
        self.screen.get_char(pos)
    }

    fn get_line_count(&self) -> i32 {
        self.screen.get_line_count()
    }

    fn get_width(&self) -> i32 {
        self.screen.get_width()
    }

    fn get_height(&self) -> i32 {
        self.screen.get_height()
    }

    fn get_line_length(&self, line: i32) -> i32 {
        self.screen.get_line_length(line)
    }

    fn get_rectangle(&self) -> Rectangle {
        self.screen.get_rectangle()
    }

    fn get_size(&self) -> Size {
        self.screen.get_size()
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

    fn get_font(&self, font_number: usize) -> Option<&BitFont> {
        self.screen.get_font(font_number)
    }

    fn font_count(&self) -> usize {
        self.screen.font_count()
    }

    fn get_font_dimensions(&self) -> Size {
        self.screen.get_font_dimensions()
    }

    fn get_selection(&self) -> Option<Selection> {
        self.screen.get_selection()
    }

    fn selection_mask(&self) -> &SelectionMask {
        self.screen.selection_mask()
    }

    fn hyperlinks(&self) -> &Vec<HyperLink> {
        self.screen.hyperlinks()
    }

    fn to_bytes(&mut self, extension: &str, options: &SaveOptions) -> Result<Vec<u8>> {
        self.screen.to_bytes(extension, options)
    }

    fn get_copy_text(&self) -> Option<String> {
        self.screen.get_copy_text()
    }

    fn get_copy_rich_text(&self) -> Option<String> {
        self.screen.get_copy_rich_text()
    }

    fn get_clipboard_data(&self) -> Option<Vec<u8>> {
        self.screen.get_clipboard_data()
    }

    fn mouse_fields(&self) -> &Vec<MouseField> {
        self.screen.mouse_fields()
    }

    fn get_version(&self) -> u64 {
        self.screen.get_version()
    }

    fn default_foreground_color(&self) -> u32 {
        self.screen.default_foreground_color()
    }

    fn max_base_colors(&self) -> u32 {
        self.screen.max_base_colors()
    }

    fn get_resolution(&self) -> Size {
        self.screen.get_resolution()
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
        self.screen.set_selection(sel)
    }

    fn clear_selection(&mut self) -> Result<()> {
        self.screen.clear_selection()
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

    fn get_first_visible_line(&self) -> i32 {
        self.screen.get_first_visible_line()
    }

    fn get_last_visible_line(&self) -> i32 {
        self.screen.get_last_visible_line()
    }

    fn get_first_editable_line(&self) -> i32 {
        self.screen.get_first_editable_line()
    }

    fn get_last_editable_line(&self) -> i32 {
        self.screen.get_last_editable_line()
    }

    fn get_first_editable_column(&self) -> i32 {
        self.screen.get_first_editable_column()
    }

    fn get_last_editable_column(&self) -> i32 {
        self.screen.get_last_editable_column()
    }

    fn get_line(&self, line: usize) -> Option<&Line> {
        self.screen.get_line(line)
    }

    fn line_count(&self) -> usize {
        self.screen.line_count()
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
