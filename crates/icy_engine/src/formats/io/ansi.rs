//! ANSI format (.ans, .ice, .diz) I/O implementation.

use crate::{Result, TextBuffer, TextScreen};

use super::super::ansi_v2::save_ansi_v2;
use super::super::{AnsiSaveOptionsV2, LoadData};

use crate::screen::EditableScreen;
/// Load an ANSI file into a TextScreen.
pub(crate) fn load_ansi(data: &[u8], load_data_opt: Option<LoadData>) -> Result<TextScreen> {
    let load_data = load_data_opt.unwrap_or_default();
    let width = load_data.default_terminal_width.unwrap_or(80);
    let mut result = TextScreen::new((width, 25));
    result.terminal_state_mut().is_terminal_buffer = false;

    let mut min_height = -1;
    if let Some(sauce) = &load_data.sauce_opt {
        let lines = result.apply_sauce(sauce);
        if lines.1 > 0 {
            min_height = lines.1 as i32;
        }
    }

    let mut parser = icy_parser_core::AnsiParser::new();
    if let Some(music) = load_data.ansi_music {
        parser.set_music_option(music);
    }

    let (file_data, is_unicode) = crate::prepare_data_for_parsing(data);
    if is_unicode {
        result.buffer.buffer_type = crate::BufferType::Unicode;
    }
    crate::load_with_parser(&mut result, &mut parser, file_data, true, min_height)?;
    Ok(result)
}

/// Save a TextBuffer to ANSI format using ANSI exporter v2.
pub(crate) fn save_ansi(buf: &TextBuffer, options: &AnsiSaveOptionsV2) -> Result<Vec<u8>> {
    save_ansi_v2(buf, options)
}
