use crate::{EngineResult, Position, Size, ansi_font::AnsiFont, editor::EditState};
use std::{error::Error, fs::File, io::Read, path::Path};

use super::{FontGlyph, FontType};

#[derive(Clone)]
pub struct TheDrawFont {
    pub name: String,
    pub font_type: FontType,
    pub spaces: i32,
    char_table: Vec<Option<FontGlyph>>,
}

static THE_DRAW_FONT_ID: &[u8; 18] = b"TheDraw FONTS file";
const THE_DRAW_FONT_HEADER_SIZE: usize = 233;

pub const MAX_WIDTH: usize = 30;
pub const MAX_HEIGHT: usize = 12;
pub const FONT_NAME_LEN: usize = 12;
pub const FONT_NAME_LEN_MAX: usize = 12 + 4; // There are 4 null bytes after the name so maximum would be 16 
pub const MAX_LETTER_SPACE: usize = 40;

pub const CHAR_TABLE_SIZE: usize = 94;

const CTRL_Z: u8 = 0x1A; // indicates end of file

const FONT_INDICATOR: u32 = 0xFF00_AA55;

impl TheDrawFont {
    pub fn new(name: impl Into<String>, font_type: FontType, spaces: i32) -> Self {
        Self {
            name: name.into(),
            font_type,
            spaces,
            char_table: vec![None; CHAR_TABLE_SIZE],
        }
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn load(file_name: &Path) -> EngineResult<Vec<TheDrawFont>> {
        let mut f = File::open(file_name)?;
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes)?;
        TheDrawFont::from_bytes(&bytes)
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn from_bytes(bytes: &[u8]) -> EngineResult<Vec<TheDrawFont>> {
        let mut result = Vec::new();

        if bytes.len() < THE_DRAW_FONT_HEADER_SIZE {
            return Err(TdfError::FileTooShort.into());
        }

        if bytes[0] as usize != THE_DRAW_FONT_ID.len() + 1 {
            return Err(TdfError::IdLengthMismatch(bytes[0]).into());
        }

        if THE_DRAW_FONT_ID != &bytes[1..19] {
            return Err(TdfError::IdMismatch.into());
        }
        let mut o = THE_DRAW_FONT_ID.len() + 1;

        let magic_byte = bytes[o];
        if magic_byte != CTRL_Z {
            return Err(TdfError::IdMismatch.into());
        }
        o += 1;

        while o < bytes.len() {
            if bytes[o] == 0 {
                break;
            }
            let indicator = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
            if indicator != FONT_INDICATOR {
                return Err(TdfError::FontIndicatorMismatch.into());
            }
            o += 4; // FONT_INDICATOR bytes!

            let orig_font_name_len = bytes[o] as usize;
            let mut font_name_len = orig_font_name_len.min(FONT_NAME_LEN_MAX);
            o += 1;

            // May be 0 terminated and the font name len is wrong. - it usually is.
            for i in 0..font_name_len {
                if bytes[o + i] == 0 {
                    font_name_len = i;
                    break;
                }
            }

            let name = String::from_utf8_lossy(&bytes[o..(o + font_name_len)]).to_string();
            if name.len() > FONT_NAME_LEN {
                log::error!("Font name too long, was {} for font: {}", name.len(), name);
            }

            o += FONT_NAME_LEN;

            o += 4; // 4 magic bytes!

            let font_type = match bytes[o] {
                0 => FontType::Outline,
                1 => FontType::Block,
                2 => FontType::Color,
                unsupported => {
                    return Err(TdfError::UnsupportedTtfType(unsupported).into());
                }
            };
            o += 1;

            let spaces: i32 = bytes[o] as i32;
            if spaces > MAX_LETTER_SPACE as i32 {
                return Err(TdfError::LetterSpaceTooMuch(spaces).into());
            }
            o += 1;

            let block_size = (bytes[o] as u16 | ((bytes[o + 1] as u16) << 8)) as usize;
            o += 2;

            let mut char_lookup_table = Vec::new();
            for i in 0..CHAR_TABLE_SIZE {
                let cur_char = bytes[o] as u16 | ((bytes[o + 1] as u16) << 8);
                println!("Char offset: {i}:{:04X}", cur_char);
                o += 2;
                char_lookup_table.push(cur_char);
            }

            let mut char_table = Vec::new();
            for char_offset in char_lookup_table {
                let mut char_offset = char_offset as usize;

                if char_offset == 0xFFFF {
                    char_table.push(None);
                    continue;
                }

                if char_offset >= block_size {
                    return Err(TdfError::GlyphOutsideFontDataSize(char_offset).into());
                }
                char_offset += o;

                if char_offset + 2 > bytes.len() {
                    log::error!("Cannot read glyph dimensions at offset {}", char_offset);
                    char_table.push(None);
                    continue;
                }

                let width = bytes[char_offset] as usize;
                char_offset += 1;
                let height = bytes[char_offset] as usize;
                char_offset += 1;

                let mut char_data = Vec::new();
                let mut valid_glyph = true;

                // Read glyph data according to font type
                loop {
                    if char_offset >= bytes.len() {
                        log::error!("Data overflow reading char at offset {char_offset}");
                        valid_glyph = false;
                        break;
                    }

                    let ch = bytes[char_offset];
                    char_offset += 1;

                    // Check for end of glyph data
                    if ch == 0 {
                        break;
                    }

                    char_data.push(ch);

                    // Handle font type specific data
                    match font_type {
                        FontType::Color => {
                            // For color fonts, we need to read the attribute byte
                            // EXCEPT for CR (13) and '&' (38)
                            if ch != 13 && ch != b'&' {
                                if char_offset >= bytes.len() {
                                    log::error!("Data overflow reading color byte at offset {char_offset}");
                                    valid_glyph = false;
                                    break;
                                }
                                let color_byte = bytes[char_offset];
                                char_offset += 1;
                                char_data.push(color_byte);
                            }
                        }
                        FontType::Outline | FontType::Block => {
                            // No additional bytes for outline/block fonts
                        }
                        _ => {
                            log::error!("Unsupported font type during read: {:?}", font_type);
                            valid_glyph = false;
                            break;
                        }
                    }
                }

                if valid_glyph && !char_data.is_empty() {
                    char_table.push(Some(FontGlyph {
                        size: Size::new(width as i32, height as i32),
                        data: char_data,
                    }));
                } else {
                    char_table.push(None);
                }
            }
            o += block_size;

            result.push(TheDrawFont {
                name,
                font_type,
                spaces,
                char_table,
            });
        }

        Ok(result)
    }

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn as_tdf_bytes(&self) -> EngineResult<Vec<u8>> {
        let mut result = Vec::new();
        result.push(THE_DRAW_FONT_ID.len() as u8 + 1);
        result.extend(THE_DRAW_FONT_ID);
        result.push(CTRL_Z);
        self.add_font_data(&mut result)?;
        Ok(result)
    }

    fn add_font_data(&self, result: &mut Vec<u8>) -> EngineResult<()> {
        result.extend(u32::to_le_bytes(FONT_INDICATOR));
        if self.name.len() > FONT_NAME_LEN {
            return Err(TdfError::NameTooLong(self.name.len()).into());
        }
        result.push(FONT_NAME_LEN as u8);
        result.extend(self.name.as_bytes());
        result.extend(vec![0; FONT_NAME_LEN - self.name.len()]);
        result.extend([0, 0, 0, 0]);
        let type_byte = match self.font_type {
            FontType::Outline => 0,
            FontType::Block => 1,
            FontType::Color => 2,
            FontType::Figlet => 3,
        };
        result.push(type_byte);
        if self.spaces > MAX_LETTER_SPACE as i32 {
            return Err(TdfError::LetterSpaceTooMuch(self.spaces).into());
        }
        result.push(self.spaces as u8);
        let mut char_lookup_table = Vec::new();
        let mut font_data = Vec::new();
        for glyph in &self.char_table {
            match glyph {
                Some(glyph) => {
                    char_lookup_table.extend(u16::to_le_bytes(font_data.len() as u16));
                    font_data.push(glyph.size.width as u8);
                    font_data.push(glyph.size.height as u8);
                    font_data.extend(&glyph.data);
                    font_data.push(0);
                }
                None => char_lookup_table.extend(u16::to_le_bytes(0xFFFF)),
            }
        }
        result.extend(u16::to_le_bytes(font_data.len() as u16));
        result.extend(char_lookup_table);
        result.extend(font_data);
        // font name length is always 11

        // unused bytes?
        Ok(())
    }

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn create_font_bundle(fonts: &[TheDrawFont]) -> EngineResult<Vec<u8>> {
        let mut result = Vec::new();
        result.push(THE_DRAW_FONT_ID.len() as u8 + 1);
        result.extend(THE_DRAW_FONT_ID);
        result.push(CTRL_Z);

        for font in fonts {
            font.add_font_data(&mut result)?;
        }
        result.push(0);

        Ok(result)
    }

    pub fn transform_outline(outline: usize, ch: u8) -> u8 {
        if ch > 64 && ch - 64 <= 17 {
            if outline >= TheDrawFont::OUTLINE_CHAR_SET.len() {
                ch
            } else {
                TheDrawFont::OUTLINE_CHAR_SET[outline][(ch - 65) as usize]
            }
        } else {
            b' '
        }
    }

    pub fn get_font_height(&self) -> i32 {
        let f = self.char_table.iter().flatten().next();
        if let Some(glyph) = f { glyph.size.height } else { 0 }
    }

    pub fn has_char(&self, char_code: u8) -> bool {
        let char_offset = (char_code as i32) - b' ' as i32 - 1;
        if let Some(glyph) = self.char_table.get(char_offset as usize) {
            glyph.is_some()
        } else {
            false
        }
    }

    pub fn set_glyph(&mut self, ch: char, glyph: FontGlyph) {
        let char_offset = (ch as i32) - b' ' as i32 - 1;
        if char_offset > 0 || char_offset < self.char_table.len() as i32 {
            self.char_table[char_offset as usize] = Some(glyph);
        }
    }

    pub fn clear_glyph(&mut self, ch: char) {
        let char_offset = (ch as i32) - b' ' as i32 - 1;
        if char_offset > 0 || char_offset < self.char_table.len() as i32 {
            self.char_table[char_offset as usize] = None;
        }
    }

    pub fn render(&self, editor: &mut EditState, char_code: u8, edit_mode: bool) -> Option<Size> {
        let char_index = (char_code as i32) - b' ' as i32 - 1;
        if char_index < 0 || char_index > self.char_table.len() as i32 {
            return None;
        }
        let table_entry = &self.char_table[char_index as usize];
        let Some(glyph) = table_entry else {
            return None;
        };
        let pos = editor.get_caret().get_position();
        let end_pos = glyph.render(editor, self.font_type, edit_mode);
        Some(Size::new(glyph.size.width, end_pos.y - pos.y + 1))
    }

    pub const OUTLINE_STYLES: usize = 19;
    const OUTLINE_CHAR_SET: [[u8; 17]; TheDrawFont::OUTLINE_STYLES] = [
        [
            0xC4, 0xC4, 0xB3, 0xB3, 0xDA, 0xBF, 0xDA, 0xBF, 0xC0, 0xD9, 0xC0, 0xD9, 0xB4, 0xC3, 0x20, 0x20, 0x20,
        ],
        [
            0xCD, 0xC4, 0xB3, 0xB3, 0xD5, 0xB8, 0xDA, 0xBF, 0xD4, 0xBE, 0xC0, 0xD9, 0xB5, 0xC3, 0x20, 0x20, 0x20,
        ],
        [
            0xC4, 0xCD, 0xB3, 0xB3, 0xDA, 0xBF, 0xD5, 0xB8, 0xC0, 0xD9, 0xD4, 0xBE, 0xB4, 0xC6, 0x20, 0x20, 0x20,
        ],
        [
            0xCD, 0xCD, 0xB3, 0xB3, 0xD5, 0xB8, 0xD5, 0xB8, 0xD4, 0xBE, 0xD4, 0xBE, 0xB5, 0xC6, 0x20, 0x20, 0x20,
        ],
        [
            0xC4, 0xC4, 0xBA, 0xB3, 0xD6, 0xBF, 0xDA, 0xB7, 0xC0, 0xBD, 0xD3, 0xD9, 0xB6, 0xC3, 0x20, 0x20, 0x20,
        ],
        [
            0xCD, 0xC4, 0xBA, 0xB3, 0xC9, 0xB8, 0xDA, 0xB7, 0xD4, 0xBC, 0xD3, 0xD9, 0xB9, 0xC3, 0x20, 0x20, 0x20,
        ],
        [
            0xC4, 0xCD, 0xBA, 0xB3, 0xD6, 0xBF, 0xD5, 0xBB, 0xC0, 0xBD, 0xC8, 0xBE, 0xB6, 0xC6, 0x20, 0x20, 0x20,
        ],
        [
            0xCD, 0xCD, 0xBA, 0xB3, 0xC9, 0xB8, 0xD5, 0xBB, 0xD4, 0xBC, 0xC8, 0xBE, 0xB9, 0xC6, 0x20, 0x20, 0x20,
        ],
        [
            0xC4, 0xC4, 0xB3, 0xBA, 0xDA, 0xB7, 0xD6, 0xBF, 0xD3, 0xD9, 0xC0, 0xBD, 0xB4, 0xC7, 0x20, 0x20, 0x20,
        ],
        [
            0xCD, 0xC4, 0xB3, 0xBA, 0xD5, 0xBB, 0xD6, 0xBF, 0xC8, 0xBE, 0xC0, 0xBD, 0xB5, 0xC7, 0x20, 0x20, 0x20,
        ],
        [
            0xC4, 0xCD, 0xB3, 0xBA, 0xDA, 0xB7, 0xC9, 0xB8, 0xD3, 0xD9, 0xD4, 0xBC, 0xB4, 0xCC, 0x20, 0x20, 0x20,
        ],
        [
            0xCD, 0xCD, 0xB3, 0xBA, 0xD5, 0xBB, 0xC9, 0xB8, 0xC8, 0xBE, 0xD4, 0xBC, 0xB5, 0xCC, 0x20, 0x20, 0x20,
        ],
        [
            0xC4, 0xC4, 0xBA, 0xBA, 0xD6, 0xB7, 0xD6, 0xB7, 0xD3, 0xBD, 0xD3, 0xBD, 0xB6, 0xC7, 0x20, 0x20, 0x20,
        ],
        [
            0xCD, 0xC4, 0xBA, 0xBA, 0xC9, 0xBB, 0xD6, 0xB7, 0xC8, 0xBC, 0xD3, 0xBD, 0xB9, 0xC7, 0x20, 0x20, 0x20,
        ],
        [
            0xC4, 0xCD, 0xBA, 0xBA, 0xD6, 0xB7, 0xC9, 0xBB, 0xD3, 0xBD, 0xC8, 0xBC, 0xB6, 0xCC, 0x20, 0x20, 0x20,
        ],
        [
            0xCD, 0xCD, 0xBA, 0xBA, 0xC9, 0xBB, 0xC9, 0xBB, 0xC8, 0xBC, 0xC8, 0xBC, 0xB9, 0xCC, 0x20, 0x20, 0x20,
        ],
        [
            0xDC, 0xDC, 0xDB, 0xDB, 0xDC, 0xDC, 0xDC, 0xDC, 0xDB, 0xDB, 0xDB, 0xDB, 0xDB, 0xDB, 0x20, 0x20, 0x20,
        ],
        [
            0xDF, 0xDF, 0xDB, 0xDB, 0xDB, 0xDB, 0xDB, 0xDB, 0xDF, 0xDF, 0xDF, 0xDF, 0xDB, 0xDB, 0x20, 0x20, 0x20,
        ],
        [
            0xDF, 0xDC, 0xDE, 0xDD, 0xDE, 0xDD, 0xDC, 0xDC, 0xDF, 0xDF, 0xDE, 0xDD, 0xDB, 0xDB, 0x20, 0x20, 0x20,
        ],
    ];
}

#[derive(Debug, Clone)]
pub enum TdfError {
    FileTooShort,
    IdMismatch,
    NameTooLong(usize),
    UnsupportedTtfType(u8),
    GlyphOutsideFontDataSize(usize),
    LetterSpaceTooMuch(i32),
    IdLengthMismatch(u8),
    FontIndicatorMismatch,
}

impl std::fmt::Display for TdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TdfError::FileTooShort => write!(f, "file too short."),
            TdfError::IdMismatch => write!(f, "id mismatch."),
            TdfError::NameTooLong(len) => {
                write!(f, "name too long was {len} max is {FONT_NAME_LEN}")
            }
            TdfError::UnsupportedTtfType(t) => {
                write!(f, "unsupported ttf type {t}, only 0, 1, 2 are valid.")
            }
            TdfError::GlyphOutsideFontDataSize(i) => write!(f, "glyph {i} outside font data size"),
            TdfError::LetterSpaceTooMuch(spaces) => {
                write!(f, "letter space is max {MAX_LETTER_SPACE} was {spaces}")
            }
            TdfError::IdLengthMismatch(len) => write!(f, "id length mismatch {len} should be 19."),
            TdfError::FontIndicatorMismatch => {
                write!(f, "font indicator mismatch should be 0x55AA00FF.")
            }
        }
    }
}

impl Error for TdfError {
    fn description(&self) -> &str {
        "use std::display"
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

impl AnsiFont for TheDrawFont {
    fn name(&self) -> &str {
        &self.name
    }

    fn has_char(&self, ch: char) -> bool {
        self.has_char(ch as u8)
    }

    fn render_next(&self, editor: &mut EditState, _prev_char: char, ch: char, edit_mode: bool) -> Position {
        if ch == '\n' {
            return Position::new(0, editor.get_caret().get_position().y + self.get_font_height());
        }
        let caret_pos = editor.get_caret().get_position();
        let char_offset = (ch as i32) - b' ' as i32 - 1;
        if char_offset < 0 || char_offset > self.char_table.len() as i32 {
            return caret_pos;
        }
        if let Some(Some(glyph)) = &self.char_table.get(char_offset as usize) {
            return glyph.render(editor, self.font_type, edit_mode);
        }
        return caret_pos;
    }

    fn font_type(&self) -> FontType {
        self.font_type
    }

    fn as_bytes(&self) -> EngineResult<Vec<u8>> {
        self.as_tdf_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::{FontType, TheDrawFont};
    const TEST_FONT: &[u8] = include_bytes!("CODERX.TDF");

    #[test]
    fn test_load() {
        let result = TheDrawFont::from_bytes(TEST_FONT).unwrap();
        for r in &result {
            assert!(matches!(r.font_type, FontType::Color));
        }
        assert_eq!(6, result.len());
        assert_eq!("Coder Blue", result[0].name);
        assert_eq!("Coder Green", result[1].name);
        assert_eq!("Coder Margen", result[2].name);
        assert_eq!("Coder Purple", result[3].name);
        assert_eq!("Coder Red", result[4].name);
        assert_eq!("Coder Silver", result[5].name);
    }

    #[test]
    fn test_load_save_multi() {
        let result = TheDrawFont::from_bytes(TEST_FONT).unwrap();
        let bundle = TheDrawFont::create_font_bundle(&result).unwrap();
        let result = TheDrawFont::from_bytes(&bundle).unwrap();
        for r in &result {
            assert!(matches!(r.font_type, FontType::Color));
        }
        assert_eq!(6, result.len());
        assert_eq!("Coder Blue", result[0].name);
        assert_eq!("Coder Green", result[1].name);
        assert_eq!("Coder Margen", result[2].name);
        assert_eq!("Coder Purple", result[3].name);
        assert_eq!("Coder Red", result[4].name);
        assert_eq!("Coder Silver", result[5].name);
    }
}
