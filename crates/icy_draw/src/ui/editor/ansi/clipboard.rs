//! Clipboard operations for `AnsiEditorCore`.
//!
//! Cut / copy / paste flows split out of `ansi_editor.rs` to keep the core
//! editor file focused on tool dispatch and view wiring.

use icy_engine_edit::EditState;
use icy_ui::Task;

use super::ansi_editor::AnsiEditorCore;
use super::SelectionDrag;

impl AnsiEditorCore {
    /// Check if cut operation is available (selection exists)
    #[allow(dead_code)]
    pub fn can_cut(&self) -> bool {
        self.with_edit_state_readonly(|state| state.selection().is_some())
    }

    /// Check if current layer is an Image layer
    pub fn is_on_image_layer(&self) -> bool {
        self.with_edit_state_readonly(|state| state.get_cur_layer().map(|l| matches!(l.role, icy_engine::Role::Image)).unwrap_or(false))
    }

    /// Cut selection to clipboard (for Image layers: cut the entire layer)
    /// Returns a Task that performs the clipboard write
    pub fn cut<Message: Clone + Send + 'static>(
        &mut self,
        on_complete: impl Fn(Result<(), icy_engine_gui::ClipboardError>) -> Message + Clone + Send + 'static,
    ) -> Task<Message> {
        // Image layer: copy layer to clipboard and delete it
        if self.is_on_image_layer() {
            let task = self.copy_layer_to_clipboard(on_complete);
            let layer_idx = self.with_edit_state_readonly(|state| state.get_current_layer().ok());
            if let Some(idx) = layer_idx {
                let mut screen = self.screen.lock();
                if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
                    if let Err(e) = edit_state.remove_layer(idx) {
                        log::error!("Cut layer remove_layer failed: {}", e);
                    }
                }
            }
            return task;
        }

        // Normal layer: standard cut behavior
        let task = self.copy_without_deselect(on_complete);
        {
            let mut screen = self.screen.lock();
            if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
                if let Err(e) = edit_state.erase_selection() {
                    log::error!("Cut erase_selection failed: {}", e);
                }
                if let Err(e) = edit_state.clear_selection() {
                    log::error!("Cut clear_selection failed: {}", e);
                }
            }
        }

        // Robustly reset transient selection/drag state and refresh shader markers.
        self.is_dragging = false;
        self.mouse_capture_tool = None;
        self.selection_drag = SelectionDrag::None;
        self.start_selection = None;
        self.refresh_selection_display();
        task
    }

    /// Copy the current layer to clipboard as an image
    fn copy_layer_to_clipboard<Message: Clone + Send + 'static>(
        &self,
        on_complete: impl Fn(Result<(), icy_engine_gui::ClipboardError>) -> Message + Clone + Send + 'static,
    ) -> Task<Message> {
        use icy_ui::clipboard::STANDARD;

        let image_data = self.with_edit_state_readonly(|state| {
            let layer = state.get_cur_layer()?;
            // Get sixel data if available
            if let Some(sixel) = layer.sixels.first() {
                let width = sixel.width() as u32;
                let height = sixel.height() as u32;
                let rgba = sixel.picture_data.clone();
                Some((width, height, rgba))
            } else {
                None
            }
        });

        let Some((width, height, rgba)) = image_data else {
            return Task::done(on_complete(Err(icy_engine_gui::ClipboardError::NoSelection)));
        };

        // Create an RgbaImage from the raw data and encode as PNG
        let Some(img) = image::RgbaImage::from_raw(width, height, rgba) else {
            return Task::done(on_complete(Err(icy_engine_gui::ClipboardError::ImageCreationFailed)));
        };

        let mut png_bytes = Vec::new();
        if img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png).is_err() {
            return Task::done(on_complete(Err(icy_engine_gui::ClipboardError::ImageCreationFailed)));
        }

        STANDARD.write_image(png_bytes).map(move |()| on_complete(Ok(())))
    }

    /// Check if copy operation is available (selection exists)
    #[allow(dead_code)]
    pub fn can_copy(&self) -> bool {
        self.with_edit_state_readonly(|state| state.selection().is_some())
    }

    /// Copy selection to clipboard in multiple formats (ICY, RTF, Text, Image)
    /// Returns a Task that performs the clipboard write
    pub fn copy<Message: Clone + Send + 'static>(
        &mut self,
        on_complete: impl Fn(Result<(), icy_engine_gui::ClipboardError>) -> Message + Clone + Send + 'static,
    ) -> Task<Message> {
        // Image layer: copy layer as image
        if self.is_on_image_layer() {
            return self.copy_layer_to_clipboard(on_complete);
        }

        let task = self.copy_without_deselect(on_complete.clone());

        // Clear selection after copy
        {
            let mut screen = self.screen.lock();
            if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
                if let Err(e) = edit_state.clear_selection() {
                    log::error!("Copy clear_selection failed: {}", e);
                }
            }
        }

        // Robustly reset transient selection/drag state and refresh shader markers.
        self.is_dragging = false;
        self.mouse_capture_tool = None;
        self.selection_drag = SelectionDrag::None;
        self.start_selection = None;
        self.refresh_selection_display();
        task
    }

    /// Copy selection to clipboard without clearing the selection
    /// Used internally by cut() which handles its own selection clearing
    fn copy_without_deselect<Message: Clone + Send + 'static>(
        &mut self,
        on_complete: impl Fn(Result<(), icy_engine_gui::ClipboardError>) -> Message + Clone + Send + 'static,
    ) -> Task<Message> {
        let mut screen = self.screen.lock();

        match icy_engine_gui::copy_selection(&mut **screen, on_complete) {
            Ok(task) => task,
            Err(e) => {
                log::error!("Copy failed: {}", e);
                Task::none()
            }
        }
    }

    /// Check if paste operation is available (clipboard has compatible content)
    #[allow(dead_code)]
    pub fn can_paste(&self) -> bool {
        self.paste_handler.can_paste()
    }

    /// Paste from clipboard (ICY format, image, or text)
    /// Creates a floating layer that can be positioned before anchoring
    /// Note: This is the old sync version - prefer paste_icy_data/paste_image/paste_text
    #[allow(dead_code)]
    pub fn paste(&mut self) -> Result<(), String> {
        // Don't paste if already in paste mode
        if self.is_paste_mode() {
            return Ok(());
        }

        // This method is deprecated - paste_icy_data/paste_image/paste_text should be used instead
        Err("Use paste_icy_data, paste_image, or paste_text instead".to_string())
    }

    /// Paste ICY binary format data
    pub fn paste_icy_data(&mut self, data: &[u8]) -> Result<(), String> {
        if self.is_paste_mode() {
            return Ok(());
        }

        let previous_tool = self.current_tool.id();

        let mut screen_guard = self.screen.lock();
        let state = screen_guard
            .as_any_mut()
            .downcast_mut::<EditState>()
            .ok_or_else(|| "Could not access edit state".to_string())?;

        // Begin atomic undo
        let undo_guard = state.begin_atomic_undo("Paste".to_string());

        state.paste_clipboard_data(data).map_err(|e| e.to_string())?;

        drop(screen_guard);

        self.paste_handler.set_active(previous_tool, undo_guard);
        Ok(())
    }

    /// Paste image data as a Sixel
    pub fn paste_image(&mut self, img: image::RgbaImage) -> Result<(), String> {
        use icy_engine::{Position, Sixel};

        if self.is_paste_mode() {
            return Ok(());
        }

        let previous_tool = self.current_tool.id();

        let mut screen_guard = self.screen.lock();
        let state = screen_guard
            .as_any_mut()
            .downcast_mut::<EditState>()
            .ok_or_else(|| "Could not access edit state".to_string())?;

        // Begin atomic undo
        let undo_guard = state.begin_atomic_undo("Paste".to_string());

        let w = img.width();
        let h = img.height();
        let mut sixel = Sixel::new(Position::default());
        sixel.picture_data = img.into_raw();
        sixel.set_width(w as i32);
        sixel.set_height(h as i32);

        state.paste_sixel(sixel).map_err(|e| e.to_string())?;

        drop(screen_guard);

        self.paste_handler.set_active(previous_tool, undo_guard);
        Ok(())
    }

    /// Paste plain text
    pub fn paste_text(&mut self, text: &str) -> Result<(), String> {
        if self.is_paste_mode() {
            return Ok(());
        }

        let previous_tool = self.current_tool.id();

        let mut screen_guard = self.screen.lock();
        let state = screen_guard
            .as_any_mut()
            .downcast_mut::<EditState>()
            .ok_or_else(|| "Could not access edit state".to_string())?;

        // Begin atomic undo
        let undo_guard = state.begin_atomic_undo("Paste".to_string());

        state.paste_text(text).map_err(|e| e.to_string())?;

        drop(screen_guard);

        self.paste_handler.set_active(previous_tool, undo_guard);
        Ok(())
    }

    /// Check if we are in paste mode (floating layer active for positioning)
    /// This is the primary check for paste mode UI and input handling
    pub fn is_paste_mode(&self) -> bool {
        self.paste_handler.is_active()
    }
}
