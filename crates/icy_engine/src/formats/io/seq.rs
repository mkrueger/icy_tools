use super::super::{apply_sauce_to_buffer, LoadData, SaveOptions};
use crate::{
    AttributedChar, BitFont, EditableScreen, Palette, Position, Result, Size, TextBuffer, TextPane, TextScreen, C128_LOWER, C64_DEFAULT_PALETTE, C64_SHIFTED,
    C64_UNSHIFTED,
};

/// The Commodore machines whose 40 column text screen can be edited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PetsciiMachine {
    C64,
    C128,
}

/// The two character sets: upper case with graphics (the machine starts with it) or lower and
/// upper case. A screen shows one of them at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PetsciiCase {
    Upper,
    Lower,
}

/// The character set of `machine` in `case`, named so that [`petscii_charset`] recognizes it.
pub fn petscii_font(machine: PetsciiMachine, case: PetsciiCase) -> BitFont {
    let (mut font, name) = match (machine, case) {
        // The files are named after the shift key: shifted is upper case with graphics.
        (PetsciiMachine::C64, PetsciiCase::Upper) => (C64_SHIFTED.clone(), "C64 PETSCII upper"),
        (PetsciiMachine::C64, PetsciiCase::Lower) => (C64_UNSHIFTED.clone(), "C64 PETSCII lower"),
        (PetsciiMachine::C128, PetsciiCase::Upper) => (C64_SHIFTED.clone(), "C128 PETSCII upper"),
        (PetsciiMachine::C128, PetsciiCase::Lower) => (C128_LOWER.clone(), "C128 PETSCII lower"),
    };
    font.set_name(name);
    font
}

/// The machine and character set of a PETSCII screen, from the name of its font.
pub fn petscii_charset(buffer: &TextBuffer) -> (PetsciiMachine, PetsciiCase) {
    let name = buffer.font(0).map(|font| font.name().to_ascii_lowercase()).unwrap_or_default();
    let machine = if name.starts_with("c128") {
        PetsciiMachine::C128
    } else {
        PetsciiMachine::C64
    };
    let case = if name.contains("unshifted") || name.contains("lower") {
        PetsciiCase::Lower
    } else {
        PetsciiCase::Upper
    };
    (machine, case)
}

/// The screen color: every character has it as its background.
pub fn petscii_background(buffer: &TextBuffer) -> u32 {
    buffer
        .layers
        .first()
        .map_or(0, |layer| layer.char_at(Position::default()).attribute.background())
}

/// A PETSCII screen `width` × `height` of spaces in `foreground` on `background`, with the
/// machine's character set and the C64 palette.
pub fn petscii_buffer(machine: PetsciiMachine, case: PetsciiCase, size: Size, foreground: u32, background: u32) -> TextBuffer {
    let mut buffer = TextBuffer::new(size);
    buffer.clear_font_table();
    buffer.set_font(0, petscii_font(machine, case));
    buffer.set_font_dimensions(Size::new(8, 8));
    buffer.palette = Palette::from_slice(&C64_DEFAULT_PALETTE);
    buffer.buffer_type = crate::BufferType::Petscii;
    buffer.terminal_state.is_terminal_buffer = false;
    let mut blank = AttributedChar::new(' ', crate::TextAttribute::default());
    blank.attribute.set_foreground(foreground);
    blank.attribute.set_background(background);
    for y in 0..size.height {
        for x in 0..size.width {
            buffer.layers[0].set_char((x, y), blank);
        }
    }
    buffer
}

/// The screen code typed for `ch` in the character set `case`: letters are upper case in the
/// graphics set, and both cases in the other.
pub fn petscii_screen_code(ch: char, case: PetsciiCase) -> Option<u8> {
    match ch {
        '@' => Some(0),
        'a'..='z' => Some(ch as u8 - b'a' + 1),
        'A'..='Z' if case == PetsciiCase::Lower => Some(ch as u8),
        'A'..='Z' => Some(ch as u8 - b'A' + 1),
        '[' => Some(0x1B),
        '£' => Some(0x1C),
        ']' => Some(0x1D),
        '↑' | '^' => Some(0x1E),
        '←' | '_' => Some(0x1F),
        ' '..='?' => Some(ch as u8),
        'π' if case == PetsciiCase::Upper => Some(0x5E),
        _ => None,
    }
}

/// SEQ bytes that set the text color, by C64 color index.
const COLOR_CODES: [u8; 16] = [0x90, 0x05, 0x1C, 0x9F, 0x9C, 0x1E, 0x1F, 0x9E, 0x81, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0x9B];
const REVERSE_ON: u8 = 0x12;
const REVERSE_OFF: u8 = 0x92;
const RETURN: u8 = 0x0D;
const CURSOR_LEFT: u8 = 0x9D;
const INSERT: u8 = 0x94;
const COLUMNS: i32 = 40;

/// The text color and reverse mode the SEQ so far leaves the C64 in.
#[derive(Default)]
struct SeqWriter {
    color: Option<usize>,
    reverse: bool,
}

impl SeqWriter {
    /// Prints the screen code `code` in the color `foreground`, switching color and reverse mode.
    fn print(&mut self, result: &mut Vec<u8>, (code, foreground): (u8, usize)) {
        if self.color != Some(foreground) {
            result.push(COLOR_CODES[foreground]);
            self.color = Some(foreground);
        }
        if (code >= 0x80) != self.reverse {
            self.reverse = code >= 0x80;
            result.push(if self.reverse { REVERSE_ON } else { REVERSE_OFF });
        }
        result.push(print_code(code));
    }
}

/// The PETSCII byte that prints the screen code `code` (without its reverse bit).
fn print_code(code: u8) -> u8 {
    match code & 0x7F {
        code @ 0x00..=0x1F => code + 0x40,
        code @ 0x40..=0x5D => code + 0x80,
        0x5E => 0xFF,
        0x5F => 0xDF,
        code @ 0x60..=0x7F => code + 0x40,
        code => code,
    }
}

/// Writes the screen as SEQ: clear screen, the character set, then the rows with color and
/// reverse codes. Rows shorter than 40 columns end with RETURN; full rows wrap by themselves.
pub(crate) fn save_seq(buf: &TextBuffer, _options: &SaveOptions) -> Result<Vec<u8>> {
    if buf.buffer_type != crate::BufferType::Petscii {
        return Err(crate::EngineError::BufferTypeMismatch {
            expected: "Petscii".to_string(),
        });
    }
    if buf.width() > COLUMNS {
        return Err(crate::EngineError::Generic(format!(
            "SEQ screens are {COLUMNS} columns wide, not {}",
            buf.width()
        )));
    }
    let (_, case) = petscii_charset(buf);
    let mut result = vec![0x93, if case == PetsciiCase::Lower { 0x0E } else { 0x8E }];
    let cell = |x: i32, y: i32| {
        let ch = buf.char_at(Position::new(x, y));
        let code = if ch.is_visible() { u8::try_from(ch.ch as u32).unwrap_or(b' ') } else { b' ' };
        (code, (ch.attribute.foreground() & 0x0F) as usize)
    };
    let row_length = |y: i32| (0..buf.width()).rposition(|x| cell(x, y).0 != b' ').map_or(0, |last| last as i32 + 1);
    let height = (0..buf.height()).rposition(|y| row_length(y) > 0).map_or(0, |last| last as i32 + 1);
    let mut writer = SeqWriter::default();
    for y in 0..height {
        let length = row_length(y);
        let full = buf.width() == COLUMNS && length == COLUMNS;
        if full && y == 24 {
            // Printing into the lower right corner scrolls the screen: print the last character
            // one column early, then insert a space in front of it for the one before.
            for x in 0..COLUMNS - 2 {
                writer.print(&mut result, cell(x, y));
            }
            writer.print(&mut result, cell(COLUMNS - 1, y));
            result.extend([CURSOR_LEFT, INSERT]);
            writer.print(&mut result, cell(COLUMNS - 2, y));
            break;
        }
        for x in 0..length {
            writer.print(&mut result, cell(x, y));
        }
        if !full && y + 1 < height {
            result.push(RETURN);
            // RETURN ends reverse mode.
            writer.reverse = false;
        }
    }
    Ok(result)
}

/// Shows the whole screen in one character set, as the machine does: the one most characters
/// were printed in.
fn single_charset(result: &mut TextScreen) {
    let buffer = &mut result.buffer;
    let mut counts = [0usize; 2];
    for layer in &buffer.layers {
        for line in &layer.lines {
            for ch in line.chars.iter().filter(|ch| ch.is_visible() && ch.ch != ' ') {
                counts[usize::from(ch.attribute.font_page() == 1)] += 1;
            }
        }
    }
    let case = if counts[0] > counts[1] { PetsciiCase::Lower } else { PetsciiCase::Upper };
    buffer.clear_font_table();
    buffer.set_font(0, petscii_font(PetsciiMachine::C64, case));
    for layer in &mut buffer.layers {
        for line in &mut layer.lines {
            for ch in &mut line.chars {
                ch.attribute.set_font_page(0);
            }
        }
    }
    result.caret_mut().set_font_page(0);
}

pub(crate) fn load_seq(data: &[u8], _load_data_opt: Option<&LoadData>, sauce_opt: Option<&icy_sauce::SauceRecord>) -> Result<TextScreen> {
    let mut result = TextScreen::new((40, 25));

    result.buffer.clear_font_table();
    result.buffer.set_font(0, C64_UNSHIFTED.clone());
    result.buffer.set_font(1, C64_SHIFTED.clone());
    result.buffer.set_font_dimensions(Size::new(8, 8)); // C64 uses 8x8 fonts

    result.buffer.palette = Palette::from_slice(&C64_DEFAULT_PALETTE);
    result.buffer.buffer_type = crate::BufferType::Petscii;
    result.buffer.terminal_state.is_terminal_buffer = false;

    // Apply SAUCE settings early
    if let Some(sauce) = sauce_opt {
        apply_sauce_to_buffer(&mut result.buffer, sauce);
    }

    seq_prepare(&mut result);
    crate::load_with_parser(&mut result, &mut icy_parser_core::PetsciiParser::default(), data, true, 25)?;
    single_charset(&mut result);
    Ok(result)
}

pub fn seq_prepare(result: &mut dyn EditableScreen) {
    for y in 0..result.height() {
        for x in 0..result.width() {
            let mut ch = AttributedChar::default();
            ch.attribute.set_foreground(7);
            ch.attribute.set_background(0);
            result.set_char((x, y).into(), ch);
        }
    }
    result.caret_mut().set_foreground(7);
    result.caret_mut().set_background(0);
    result.caret_mut().set_font_page(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileFormat;

    fn round_trip(buffer: &TextBuffer) -> TextBuffer {
        let mut options = SaveOptions::default();
        options.preprocess.optimize_colors = false;
        let bytes = FileFormat::Petscii.to_bytes(buffer, &options).unwrap();
        FileFormat::Petscii.from_bytes(&bytes, None).unwrap().screen.buffer
    }

    #[test]
    fn every_screen_code_and_color_survives_saving_and_loading() {
        for case in [PetsciiCase::Upper, PetsciiCase::Lower] {
            let mut buffer = petscii_buffer(PetsciiMachine::C64, case, Size::new(40, 8), 14, 0);
            for code in 0..256u32 {
                let mut ch = AttributedChar::new(char::from_u32(code).unwrap(), crate::TextAttribute::default());
                ch.attribute.set_foreground(code % 16);
                buffer.layers[0].set_char((code as i32 % 40, code as i32 / 40), ch);
            }
            let loaded = round_trip(&buffer);
            assert_eq!(petscii_charset(&loaded), (PetsciiMachine::C64, case), "the character set is kept");
            for code in 0..256u32 {
                let position = Position::new(code as i32 % 40, code as i32 / 40);
                let ch = loaded.char_at(position);
                assert_eq!((ch.ch as u32, ch.attribute.foreground()), (code, code % 16), "{case:?} code {code:#04X}");
            }
        }
    }

    #[test]
    fn short_rows_end_with_return_and_the_lower_right_corner_does_not_scroll() {
        let mut buffer = petscii_buffer(PetsciiMachine::C64, PetsciiCase::Upper, Size::new(40, 25), 1, 0);
        buffer.layers[0].set_char((0, 0), AttributedChar::new('\u{1}', crate::TextAttribute::default()));
        for x in 0..40 {
            let mut ch = AttributedChar::new(char::from(0x81 + (x % 26) as u8), crate::TextAttribute::default());
            ch.attribute.set_foreground(x as u32 % 16);
            buffer.layers[0].set_char((x, 24), ch);
        }
        let bytes = FileFormat::Petscii.to_bytes(&buffer, &SaveOptions::default()).unwrap();
        assert!(
            bytes.ends_with(&[CURSOR_LEFT, INSERT, COLOR_CODES[38 % 16], print_code(0x81 + 38 % 26)]),
            "the last row ends with the insert trick: {bytes:02X?}"
        );
        let loaded = round_trip(&buffer);
        assert_eq!(loaded.char_at((0, 0).into()).ch as u32, 1, "nothing scrolled away");
        for x in 0..40 {
            let (saved, read) = (buffer.char_at((x, 24).into()), loaded.char_at((x, 24).into()));
            assert_eq!((read.ch, read.attribute.foreground()), (saved.ch, saved.attribute.foreground()), "column {x}");
        }
    }

    #[test]
    fn typing_follows_the_character_set() {
        assert_eq!(petscii_screen_code('A', PetsciiCase::Upper), Some(1));
        assert_eq!(petscii_screen_code('a', PetsciiCase::Upper), Some(1));
        assert_eq!(petscii_screen_code('A', PetsciiCase::Lower), Some(65));
        assert_eq!(petscii_screen_code('a', PetsciiCase::Lower), Some(1));
        assert_eq!(petscii_screen_code('£', PetsciiCase::Lower), Some(0x1C));
        assert_eq!(petscii_screen_code('7', PetsciiCase::Upper), Some(b'7'));
        assert_eq!(petscii_screen_code('€', PetsciiCase::Upper), None);
    }

    #[test]
    fn a_loaded_screen_shows_one_character_set() {
        let loaded = FileFormat::Petscii.from_bytes(b"\x0eHELLO\x8eX", None).unwrap().screen.buffer;
        assert_eq!(petscii_charset(&loaded).1, PetsciiCase::Lower, "most characters were printed in lower case");
        assert_eq!(loaded.font_count(), 1);
    }
}
