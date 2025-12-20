use icy_engine::{AttributedChar, EditableScreen, Size, TextAttribute, TextScreen, formats::FileFormat};

use super::BitFontEditState;

/// Embedded preview template file (debug_preview.icy)
const PREVIEW_TEMPLATE: &[u8] = include_bytes!("debug_preview.icy");

impl BitFontEditState {
    /// Build preview screen for the given glyph using the current font
    ///
    /// Loads the preview template from debug_preview.icy and adds:
    /// - The tiled character display
    /// - The current character number (#NNN)
    pub fn build_preview_content_for(&self, tile_char: char, fg_color: u8, bg_color: u8) -> TextScreen {
        // Load the preview template from embedded .icy file
        let mut screen = match FileFormat::IcyDraw.from_bytes(PREVIEW_TEMPLATE, None) {
            Ok(loaded_doc) => loaded_doc.screen,
            Err(_) => {
                // Fallback to blank screen if template fails to load
                TextScreen::new(Size::new(80, 25))
            }
        };
        // Set font and options
        screen.buffer.buffer_type = icy_engine::BufferType::CP437;
        screen.buffer.set_font(0, self.build_font());
        screen.buffer.set_use_letter_spacing(self.use_letter_spacing());
        screen.buffer.terminal_state.is_terminal_buffer = true;
        screen.caret.attribute = TextAttribute::from_color(fg_color, bg_color);
        screen.set_caret_position((21, 19).into());
        screen.print_str(format!("{:03}", tile_char as u8).as_str());

        for row in 0..3 {
            for col in 0..13 {
                screen.set_caret_position((27 + col, 18 + row).into());
                screen.print_char(AttributedChar::from_char(tile_char));
            }
        }
        screen
    }
}
