use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};

use character::{Character, FIGChar};
use errors::FigError;
use header::Header;
use zip::ZipArchive;

use crate::{AttributedChar, EngineResult, Position};

use super::AnsiFont;

pub mod character;
mod errors;
pub mod header;
pub struct FIGFont {
    name: String,
    pub(crate) header: Header,
    pub(crate) chars: HashMap<char, Character>,
}

const ADDITIONAL_CHARS: usize = 7;
const ADDITIONAL_CHARS_MAP: [u8; ADDITIONAL_CHARS] = [196, 214, 220, 228, 246, 252, 223];

impl FIGFont {
    pub fn load(file_name: &Path) -> EngineResult<Self> {
        let mut res = if is_zip(file_name)? {
            let mut zip = ZipArchive::new(File::open(file_name)?)?;
            // should be the only file in ZIP archive according to FIGlet spec
            if zip.len() != 1 {
                return Err(FigError::InvalidZIP.into());
            }
            let f = zip.by_index(0)?;
            let mut reader = BufReader::new(f);
            FIGFont::read(&mut reader)?
        } else {
            let f = File::open(file_name)?;
            let mut reader = BufReader::new(f);
            FIGFont::read(&mut reader)?
        };

        if let Some(name) = file_name.file_name() {
            res.name = name.to_string_lossy().to_string();
        }
        Ok(res)
    }

    pub fn read<R: Read>(reader: &mut BufReader<R>) -> EngineResult<Self> {
        let header = Header::read(reader)?;
        let mut chars = HashMap::new();
        let mut char_number = b' ' as usize;
        loop {
            let Ok(char) = Character::read(reader, &header, char_number > 126 + ADDITIONAL_CHARS) else {
                break;
            };
            if char_number <= 126 {
                chars.insert(char_number as u8 as char, char);
            } else if char_number <= 126 + ADDITIONAL_CHARS {
                let number = ADDITIONAL_CHARS_MAP[char_number - 127];
                chars.insert(number as char, char);
                break;
            } else {
                if let Some(ch) = &char.ch {
                    chars.insert(*ch, char);
                }
            }
            char_number += 1;
        }

        Ok(FIGFont {
            name: String::new(),
            header,
            chars,
        })
    }

    pub fn set_name(&mut self, new_name: String) {
        self.name = new_name;
    }
}

fn is_zip(file_name: &Path) -> EngineResult<bool> {
    if let Ok(mut f) = File::open(file_name) {
        let mut h = [0; 2];
        f.read_exact(&mut h)?;
        Ok(h.eq(b"PK"))
    } else {
        Ok(false)
    }
}

impl AnsiFont for FIGFont {
    fn name(&self) -> &str {
        &self.name
    }

    fn has_char(&self, ch: char) -> bool {
        self.chars.contains_key(&ch)
    }

    fn render_next(&self, editor: &mut crate::editor::EditState, _prev_char: char, ch: char) -> crate::Position {
        if ch == '\n' {
            return Position::new(0, editor.get_caret().get_position().y + self.header.height() as i32);
        }
        let Some(ch) = self.chars.get(&ch) else {
            return editor.get_caret().get_position();
        };

        let caret_pos = editor.get_caret().get_position();

        match self.header.horiz_layout() {
            header::LayoutMode::Full => {
                // already at the right position
            }
            header::LayoutMode::Fitting => {
                // todo
            }
            header::LayoutMode::Smushing => {
                // todo
            }
        }
        let color = editor.get_caret().attribute;

        let mut y = caret_pos.y;
        for line in &ch.lines {
            let mut x: i32 = caret_pos.x;
            for ch in line {
                let ch: char = match *ch {
                    FIGChar::HardBlank => ' ',
                    FIGChar::Char(ch) => ch,
                };
                let attributed_char = AttributedChar::new(ch, color);
                editor.set_char(Position::new(x, y), attributed_char).unwrap();
                x += 1;
            }
            y += 1;
        }
        caret_pos + Position::new(ch.lines[0].len() as i32, 0)
    }

    fn font_type(&self) -> super::FontType {
        super::FontType::Figlet
    }

    fn as_bytes(&self) -> EngineResult<Vec<u8>> {
        let mut res = String::new();
        res.push_str(&self.header.generate_string());
        res.push('\n');
        for ch in ' '..='~' {
            if let Some(ch) = self.chars.get(&ch) {
                res.push_str(&ch.generate_string(&self.header));
            }
        }
        for ch in ADDITIONAL_CHARS_MAP.iter() {
            if let Some(ch) = self.chars.get(&(*ch as char)) {
                res.push_str(&ch.generate_string(&self.header));
            } else {
                break;
            }
        }

        for ch in self.chars.keys().filter(|ch| **ch > '~') {
            if let Some(ch) = self.chars.get(ch) {
                if ch.ch.is_some() {
                    res.push_str(&ch.generate_string(&self.header));
                }
            }
        }
        Ok(res.into_bytes())
    }
}

pub(crate) fn read_line<R: Read>(reader: &mut BufReader<R>) -> EngineResult<String> {
    let mut data = Vec::new();
    reader.read_until(b'\n', &mut data)?;
    if data.ends_with(b"\r\n") {
        data.pop();
        data.pop();
    } else if data.ends_with(b"\n") {
        data.pop();
    }
    Ok(String::from_utf8(data)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zipped() {
        let font1 = FIGFont::load(Path::new("src/ansi_font/figlet/doom.flf")).unwrap();
        let font2 = FIGFont::load(Path::new("src/ansi_font/figlet/doom_zipped.flf")).unwrap();
        assert_eq!(font1.header, font2.header);
        assert_eq!(font1.chars, font2.chars);
    }
}
