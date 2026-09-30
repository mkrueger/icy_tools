//! VT52 text as the Atari ST shows it, for editing: characters with the resolution's colors.
//!
//! The ST's text screens:
//! - Low: 40 × 25 characters of 8 × 8 pixels, 16 colors.
//! - Medium: 80 × 25, 4 colors; its pixels are twice as high as wide, so the 8 × 8 font is shown
//!   with doubled rows.
//! - High: 80 × 25 characters of 8 × 16 pixels, black and white.

use super::super::{append_sauce, apply_sauce_to_buffer_without_resize, LoadData, SauceBuilder, SaveOptions};
use crate::{
    igs::{ATARI_ST_FONT_8x16, ATARI_ST_FONT_8x8},
    BitFont, EditableScreen, Position, Result, Size, TerminalResolution, TerminalResolutionExt, TextBuffer, TextPane, TextScreen,
};
use icy_sauce::CharacterFormat;

const ESC: u8 = 0x1B;
const ROWS: i32 = 25;

/// The columns of the ST's text screen in `resolution`.
pub fn atari_st_columns(resolution: TerminalResolution) -> i32 {
    match resolution {
        TerminalResolution::Low => 40,
        TerminalResolution::Medium | TerminalResolution::High => 80,
    }
}

/// The font of the ST's text screen in `resolution`.
pub fn atari_st_font(resolution: TerminalResolution) -> BitFont {
    match resolution {
        TerminalResolution::Low => ATARI_ST_FONT_8x8.clone(),
        TerminalResolution::Medium => {
            let font = &*ATARI_ST_FONT_8x8;
            let mut bytes = Vec::with_capacity(256 * 16);
            for code in 0..256u32 {
                let glyph = font.glyph(char::from_u32(code).unwrap_or(' '));
                for row in &glyph.data[..8] {
                    bytes.extend([*row, *row]);
                }
            }
            BitFont::create_8("Atari ST 8x8 medium", 8, 16, &bytes)
        }
        TerminalResolution::High => ATARI_ST_FONT_8x16.clone(),
    }
}

/// The ST's text screen in `resolution`, `height` lines high: its columns, font and colors.
pub fn atari_st_buffer(resolution: TerminalResolution, height: i32) -> TextBuffer {
    let mut buffer = TextBuffer::new(Size::new(atari_st_columns(resolution), height));
    set_atari_st_screen(&mut buffer, resolution);
    buffer
}

fn set_atari_st_screen(buffer: &mut TextBuffer, resolution: TerminalResolution) {
    buffer.clear_font_table();
    let font = atari_st_font(resolution);
    buffer.set_font_dimensions(font.size());
    buffer.set_font(0, font);
    buffer.palette = resolution.palette().clone();
    buffer.buffer_type = crate::BufferType::AtariSt;
    buffer.terminal_state.is_terminal_buffer = false;
}

/// The resolution of an ST text screen: its columns tell Low from the others, its colors
/// Medium from High.
pub fn atari_st_resolution(buffer: &TextBuffer) -> TerminalResolution {
    if buffer.width() <= atari_st_columns(TerminalResolution::Low) {
        TerminalResolution::Low
    } else if buffer.palette.len() <= 2 {
        TerminalResolution::High
    } else {
        TerminalResolution::Medium
    }
}

/// The text color the ST starts with: the last color, black on the desktop's palettes.
pub fn atari_st_text_color(resolution: TerminalResolution) -> u32 {
    resolution.palette().len() as u32 - 1
}

fn color_code(color: u32) -> u8 {
    0x20 + (color & 0x0F) as u8
}

pub(crate) fn save_vt52(buf: &TextBuffer, options: &SaveOptions) -> Result<Vec<u8>> {
    if buf.buffer_type != crate::BufferType::AtariSt {
        return Err(crate::EngineError::BufferTypeMismatch {
            expected: "Atari ST".to_string(),
        });
    }
    let colors = buf.palette.len().max(2) as u32;
    let text = colors - 1;
    let color = |value: u32| if value < colors { value } else { text };
    // Background color 0, clear the screen, and no wrapping, so full lines end with CR LF too.
    let mut result = vec![ESC, b'c', color_code(0), ESC, b'E', ESC, b'w'];
    let mut current = (u32::MAX, 0);
    let height = buf.height();
    for y in 0..height {
        let cells: Vec<(u8, u32, u32)> = (0..buf.width())
            .map(|x| {
                let ch = buf.char_at(Position::new(x, y));
                if !ch.is_visible() {
                    return (b' ', text, 0);
                }
                // Control codes are VT52 commands; the ST cannot print them from a file.
                let code = u8::try_from(ch.ch as u32).ok().filter(|code| *code >= 0x20).unwrap_or(b' ');
                (code, color(ch.attribute.foreground()), color(ch.attribute.background()))
            })
            .collect();
        let length = cells
            .iter()
            .rposition(|&(code, _, background)| code != b' ' || background != 0)
            .map_or(0, |last| last + 1);
        for &(code, foreground, background) in &cells[..length] {
            if current.0 != foreground {
                result.extend([ESC, b'b', color_code(foreground)]);
                current.0 = foreground;
            }
            if current.1 != background {
                result.extend([ESC, b'c', color_code(background)]);
                current.1 = background;
            }
            result.push(code);
        }
        if y + 1 < height {
            if current.1 != 0 {
                // The new line is cleared with the background color.
                result.extend([ESC, b'c', color_code(0)]);
                current.1 = 0;
            }
            result.extend(b"\r\n");
        }
    }
    result.extend([ESC, b'b', color_code(text), ESC, b'c', color_code(0), ESC, b'v']);

    if let Some(meta) = &options.sauce {
        let sauce = buf.build_character_sauce(meta, CharacterFormat::Ansi);
        append_sauce(&mut result, sauce)?;
    }
    Ok(result)
}

/// Loads VT52 text. The resolution comes from `default_terminal_width` (40 is Low) or SAUCE;
/// otherwise text with more than four colors that fits 40 columns is Low, the rest Medium.
pub(crate) fn load_vt52(data: &[u8], load_data_opt: Option<&LoadData>, sauce_opt: Option<&icy_sauce::SauceRecord>) -> Result<TextScreen> {
    let sauce_columns = sauce_opt.and_then(|sauce| match sauce.capabilities() {
        Some(icy_sauce::Capabilities::Character(capabilities)) => Some(capabilities.columns as usize),
        _ => None,
    });
    let columns = load_data_opt
        .and_then(LoadData::default_terminal_width)
        .or(sauce_columns.filter(|columns| *columns > 0));
    let resolution = if columns == Some(40) {
        TerminalResolution::Low
    } else {
        TerminalResolution::Medium
    };
    let mut result = load_in(data, resolution, sauce_opt)?;
    if columns.is_none() {
        let buffer = &result.buffer;
        let used_colors = (0..buffer.height())
            .flat_map(|y| (0..buffer.width()).map(move |x| buffer.char_at(Position::new(x, y))))
            .filter(crate::AttributedChar::is_visible)
            .flat_map(|ch| [ch.attribute.foreground(), ch.attribute.background()])
            .max()
            .unwrap_or(0);
        let fits = (0..buffer.height()).all(|y| buffer.line_length(y) <= atari_st_columns(TerminalResolution::Low));
        if used_colors > 3 && fits {
            result = load_in(data, TerminalResolution::Low, sauce_opt)?;
        }
    }
    Ok(result)
}

fn load_in(data: &[u8], resolution: TerminalResolution, sauce_opt: Option<&icy_sauce::SauceRecord>) -> Result<TextScreen> {
    let mut result = TextScreen::from_buffer(atari_st_buffer(resolution, ROWS));
    if let Some(sauce) = sauce_opt {
        apply_sauce_to_buffer_without_resize(&mut result.buffer, sauce);
        set_atari_st_screen(&mut result.buffer, resolution);
    }
    result.caret_mut().attribute.set_foreground(atari_st_text_color(resolution));
    result.caret_mut().attribute.set_background(0);
    let data = match sauce_opt {
        Some(_) => data.strip_suffix(&[0x1A]).unwrap_or(data),
        None => data,
    };
    let mut parser = icy_parser_core::Vt52Parser::new(icy_parser_core::VT52Mode::Mixed);
    crate::load_with_parser(&mut result, &mut parser, data, true, ROWS)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AttributedChar, FileFormat, TextAttribute};

    fn colored(ch: char, foreground: u32, background: u32) -> AttributedChar {
        let mut attribute = TextAttribute::default();
        attribute.set_foreground(foreground);
        attribute.set_background(background);
        AttributedChar::new(ch, attribute)
    }

    fn round_trip(buffer: &TextBuffer, columns: usize) -> TextBuffer {
        let mut options = SaveOptions::default();
        options.preprocess.optimize_colors = false;
        let bytes = FileFormat::Vt52.to_bytes(buffer, &options).unwrap();
        FileFormat::Vt52
            .from_bytes(&bytes, Some(LoadData::new(None, Some(columns))))
            .unwrap()
            .screen
            .buffer
    }

    #[test]
    fn characters_and_colors_survive_saving_and_loading() {
        for resolution in [TerminalResolution::Low, TerminalResolution::Medium, TerminalResolution::High] {
            let mut buffer = atari_st_buffer(resolution, 25);
            let colors = resolution.palette().len() as u32;
            let width = buffer.width();
            for x in 0..width {
                let code = char::from(0x20 + (x as u8 * 3) % 0xE0);
                buffer.layers[0].set_char((x, 0), colored(code, x as u32 % colors, (x as u32 + 1) % colors));
            }
            buffer.layers[0].set_char((5, 2), colored('ä', colors - 1, 0));
            let columns = if resolution == TerminalResolution::Low { 40 } else { 80 };
            let loaded = round_trip(&buffer, columns);
            assert_eq!(loaded.buffer_type, crate::BufferType::AtariSt);
            for x in 0..width {
                let (saved, read) = (buffer.char_at((x, 0).into()), loaded.char_at((x, 0).into()));
                assert_eq!(
                    (read.ch, read.attribute.foreground(), read.attribute.background()),
                    (saved.ch, saved.attribute.foreground(), saved.attribute.background()),
                    "{resolution:?} column {x}"
                );
            }
            assert_eq!(loaded.char_at((5, 2).into()).ch, 'ä');
            assert_eq!(
                loaded.char_at((0, 1).into()).attribute.background(),
                0,
                "the empty line has no background color"
            );
        }
    }

    #[test]
    fn the_resolution_is_recognized() {
        let low = atari_st_buffer(TerminalResolution::Low, 25);
        let medium = atari_st_buffer(TerminalResolution::Medium, 25);
        let high = atari_st_buffer(TerminalResolution::High, 25);
        assert_eq!(
            [&low, &medium, &high].map(atari_st_resolution),
            [TerminalResolution::Low, TerminalResolution::Medium, TerminalResolution::High]
        );
        assert_eq!(medium.font_dimensions(), Size::new(8, 16), "medium resolution pixels are twice as high");
        assert_eq!(low.font_dimensions(), Size::new(8, 8));

        // Sixteen colors in 40 columns are low resolution text.
        let text = b"\x1bE\x1bb\x2cHELLO";
        let loaded = FileFormat::Vt52.from_bytes(text, None).unwrap().screen.buffer;
        assert_eq!(atari_st_resolution(&loaded), TerminalResolution::Low);
        assert_eq!(loaded.char_at((0, 0).into()).attribute.foreground(), 12);
    }

    #[test]
    fn control_codes_are_not_written_as_commands() {
        let mut buffer = atari_st_buffer(TerminalResolution::Medium, 1);
        buffer.layers[0].set_char((0, 0), colored('\u{7}', 3, 0));
        buffer.layers[0].set_char((1, 0), colored('A', 3, 0));
        let bytes = FileFormat::Vt52.to_bytes(&buffer, &SaveOptions::default()).unwrap();
        assert!(!bytes.contains(&0x07));
        assert!(bytes.windows(2).any(|pair| pair == b" A"));
    }
}
