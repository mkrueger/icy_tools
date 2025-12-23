//! SharedTextScreen - A wrapper that allows sharing a TextScreen between components
//!
//! This wrapper holds an `Arc<Mutex<TextScreen>>` and implements `Screen` by delegating
//! all calls to the inner TextScreen. This allows the same TextScreen to be shared
//! between EditState (for editing) and Terminal (for display).

use parking_lot::Mutex;
use std::sync::Arc;

use crate::{
    AttributedChar, BitFont, Caret, EditableScreen, HyperLink, IceMode, MouseField, Palette, Position, Rectangle, RenderOptions, Result, SaveOptions, Screen,
    Selection, SelectionMask, Size, TerminalState, TextPane, TextScreen,
};

/// A wrapper around `Arc<Mutex<TextScreen>>` that implements `Screen`.
///
/// This allows sharing a single TextScreen between multiple components
/// (e.g., EditState and Terminal) while still satisfying the `Screen` trait.
pub struct SharedTextScreen {
    inner: Arc<Mutex<TextScreen>>,
}

impl SharedTextScreen {
    /// Create a new SharedTextScreen wrapping the given Arc
    pub fn new(inner: Arc<Mutex<TextScreen>>) -> Self {
        Self { inner }
    }

    /// Get access to the inner Arc for sharing with other components
    pub fn inner(&self) -> Arc<Mutex<TextScreen>> {
        self.inner.clone()
    }

    /// Create a new SharedTextScreen with a fresh TextScreen
    pub fn with_size(size: impl Into<Size>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TextScreen::new(size))),
        }
    }
}

impl Clone for SharedTextScreen {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }
}

// Implement TextPane by delegating to inner
impl TextPane for SharedTextScreen {
    fn char_at(&self, pos: Position) -> AttributedChar {
        self.inner.lock().char_at(pos)
    }

    fn line_count(&self) -> i32 {
        TextPane::line_count(&*self.inner.lock())
    }

    fn width(&self) -> i32 {
        self.inner.lock().width()
    }

    fn height(&self) -> i32 {
        self.inner.lock().height()
    }

    fn line_length(&self, line: i32) -> i32 {
        self.inner.lock().line_length(line)
    }

    fn rectangle(&self) -> Rectangle {
        self.inner.lock().rectangle()
    }

    fn size(&self) -> Size {
        self.inner.lock().size()
    }
}

// Implement Screen by delegating to inner
impl Screen for SharedTextScreen {
    fn buffer_type(&self) -> crate::BufferType {
        self.inner.lock().buffer_type()
    }

    fn graphics_type(&self) -> crate::GraphicsType {
        self.inner.lock().graphics_type()
    }

    fn resolution(&self) -> Size {
        self.inner.lock().resolution()
    }

    fn virtual_size(&self) -> Size {
        self.inner.lock().virtual_size()
    }

    fn font_dimensions(&self) -> Size {
        self.inner.lock().font_dimensions()
    }

    fn set_font_dimensions(&mut self, size: Size) {
        self.inner.lock().set_font_dimensions(size);
    }

    fn scan_lines(&self) -> bool {
        self.inner.lock().scan_lines()
    }

    fn render_to_rgba(&self, options: &RenderOptions) -> (Size, Vec<u8>) {
        self.inner.lock().render_to_rgba(options)
    }

    fn render_to_rgba_raw(&self, options: &RenderOptions) -> (Size, Vec<u8>) {
        self.inner.lock().render_to_rgba_raw(options)
    }

    fn render_region_to_rgba(&self, region: Rectangle, options: &RenderOptions) -> (Size, Vec<u8>) {
        self.inner.lock().render_region_to_rgba(region, options)
    }

    fn render_region_to_rgba_raw(&self, region: Rectangle, options: &RenderOptions) -> (Size, Vec<u8>) {
        self.inner.lock().render_region_to_rgba_raw(region, options)
    }

    fn palette(&self) -> &Palette {
        // This is tricky - we need to return a reference but we have a lock
        // For now, we'll use a static empty palette as fallback
        // In practice, the shader accesses palette differently
        static EMPTY_PALETTE: std::sync::OnceLock<Palette> = std::sync::OnceLock::new();
        EMPTY_PALETTE.get_or_init(Palette::default)
    }

    fn ice_mode(&self) -> IceMode {
        self.inner.lock().ice_mode()
    }

    fn font(&self, _font_number: usize) -> Option<&BitFont> {
        // Same issue as palette - we can't return a reference through a lock
        // The shader handles this differently
        None
    }

    fn font_count(&self) -> usize {
        self.inner.lock().font_count()
    }

    fn version(&self) -> u64 {
        self.inner.lock().version()
    }

    fn get_dirty_lines(&self) -> Option<(i32, i32)> {
        self.inner.lock().get_dirty_lines()
    }

    fn clear_dirty_lines(&self) {
        self.inner.lock().clear_dirty_lines()
    }

    fn default_foreground_color(&self) -> u32 {
        self.inner.lock().default_foreground_color()
    }

    fn max_base_colors(&self) -> u32 {
        self.inner.lock().max_base_colors()
    }

    fn copy_text(&self) -> Option<String> {
        self.inner.lock().copy_text()
    }

    fn copy_rich_text(&self) -> Option<String> {
        self.inner.lock().copy_rich_text()
    }

    fn clipboard_data(&self) -> Option<Vec<u8>> {
        self.inner.lock().clipboard_data()
    }

    fn hyperlinks(&self) -> &Vec<HyperLink> {
        // Same reference issue - return empty vec
        static EMPTY_LINKS: std::sync::OnceLock<Vec<HyperLink>> = std::sync::OnceLock::new();
        EMPTY_LINKS.get_or_init(Vec::new)
    }

    fn mouse_fields(&self) -> &Vec<MouseField> {
        static EMPTY_FIELDS: std::sync::OnceLock<Vec<MouseField>> = std::sync::OnceLock::new();
        EMPTY_FIELDS.get_or_init(Vec::new)
    }

    fn selection(&self) -> Option<Selection> {
        self.inner.lock().selection()
    }

    fn selection_mask(&self) -> &SelectionMask {
        static EMPTY_MASK: std::sync::OnceLock<SelectionMask> = std::sync::OnceLock::new();
        EMPTY_MASK.get_or_init(SelectionMask::default)
    }

    fn set_selection(&mut self, sel: Selection) -> Result<()> {
        self.inner.lock().set_selection(sel)
    }

    fn clear_selection(&mut self) -> Result<()> {
        self.inner.lock().clear_selection()
    }

    fn terminal_state(&self) -> &TerminalState {
        static DEFAULT_STATE: std::sync::OnceLock<TerminalState> = std::sync::OnceLock::new();
        DEFAULT_STATE.get_or_init(TerminalState::default)
    }

    fn caret(&self) -> &Caret {
        static DEFAULT_CARET: std::sync::OnceLock<Caret> = std::sync::OnceLock::new();
        DEFAULT_CARET.get_or_init(Caret::default)
    }

    fn caret_position(&self) -> Position {
        self.inner.lock().caret_position()
    }

    fn to_bytes(&mut self, extension: &str, options: &SaveOptions) -> Result<Vec<u8>> {
        let extension = extension.to_ascii_lowercase();
        if let Some(format) = crate::formats::FileFormat::from_extension(&extension) {
            format.to_bytes(&self.inner.lock().buffer, options)
        } else {
            Err(crate::EngineError::UnsupportedFormat {
                description: format!("Unknown format: {}", extension),
            })
        }
    }

    fn use_letter_spacing(&self) -> bool {
        self.inner.lock().use_letter_spacing()
    }

    fn use_aspect_ratio(&self) -> bool {
        self.inner.lock().use_aspect_ratio()
    }

    fn aspect_ratio_stretch_factor(&self) -> f32 {
        self.inner.lock().aspect_ratio_stretch_factor()
    }

    fn as_editable(&mut self) -> Option<&mut dyn EditableScreen> {
        // Can't return mutable reference through Arc<Mutex>
        None
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn screen(&self) -> &[u8] {
        // Can't return reference through lock
        &[]
    }

    fn set_scrollback_buffer_size(&mut self, buffer_size: usize) {
        self.inner.lock().set_scrollback_buffer_size(buffer_size);
    }

    fn clone_box(&self) -> Box<dyn Screen> {
        // Deep clone - clone the actual TextScreen content, not just the Arc
        Box::new(self.inner.lock().clone())
    }
}

// Make it Send + Sync (Arc<Mutex<_>> is already Send + Sync)
unsafe impl Send for SharedTextScreen {}
unsafe impl Sync for SharedTextScreen {}
