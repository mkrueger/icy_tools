use byteorder::{LittleEndian, ReadBytesExt};
use std::io;
use std::io::Cursor;
use std::io::prelude::*;

use crate::{EditableScreen, EngineResult, Size};

use super::Bgi;
use super::Direction;
use super::character::Character;
use super::character::SCALE_DOWN;
use super::character::SCALE_UP;

pub struct Font {
    pub name: String,
    pub size: u16,

    characters: Vec<Option<Character>>,

    /// Height from origin to top of capitol
    pub org_to_cap: i8,
    /// Height from origin to baseline
    pub org_to_base: i8,
    /// Height from origin to bot of decender
    pub org_to_dec: i8,

    pub capital_height: i32,
    pub base_height: i32,
    pub descender_height: i32,
    pub lower_case_height: i32,
}

impl Font {
    pub fn get_height(&self) -> i32 {
        self.org_to_cap.abs() as i32 + self.org_to_dec.abs() as i32
    }

    pub fn load(buf: &[u8]) -> EngineResult<Self> {
        let mut br = Cursor::new(buf);

        // skip header
        while br.read_u8()? != 0x1A {}

        // fheader
        let header_size = br.read_u16::<LittleEndian>()?;
        let name: [u8; 4] = [br.read_u8()?, br.read_u8()?, br.read_u8()?, br.read_u8()?];
        let font_name = String::from_utf8_lossy(&name).to_string();
        let font_size = br.read_u16::<LittleEndian>()?;
        let _font_major = br.read_u8()?;
        let _font_minor = br.read_u8()?;
        let _min_major = br.read_u8()?;
        let _min_minor = br.read_u8()?;

        br.set_position(header_size as u64);

        // header
        let _sig = br.read_u8()?;
        let character_count = br.read_u16::<LittleEndian>()?;
        br.read_u8()?; // unused byte
        let first = br.read_u8()?;
        let _character_offset = br.read_u16::<LittleEndian>()?;
        let _scan_flag = br.read_u8()?;
        let org_to_cap = br.read_i8()?;
        let org_to_base = br.read_i8()?;
        let org_to_dec = br.read_i8()?;
        let name: [u8; 4] = [br.read_u8()?, br.read_u8()?, br.read_u8()?, br.read_u8()?];
        let _short_font_name = String::from_utf8_lossy(&name).to_string();
        br.read_u8()?; // unused byte

        // read offset table
        let mut font_offsets = Vec::new();
        for _ in 0..character_count {
            font_offsets.push(br.read_u16::<LittleEndian>()?);
        }

        // read character width table
        let mut char_widths = Vec::new();
        for _ in 0..character_count {
            char_widths.push(br.read_u8()?);
        }

        let mut characters = Vec::new();
        for _ in 0..first {
            characters.push(None);
        }

        let start = br.position();
        for i in 0..character_count as usize {
            br.seek(io::SeekFrom::Start(start + font_offsets[i] as u64))?;
            characters.push(Some(Character::load(&mut br, char_widths[i] as i32)?));
        }

        let mut capital_height = 40;
        let mut base_height = 0;
        let mut descender_height = -7;

        if (b'E' as usize) < characters.len() {
            if let Some(bc) = &characters[b'E' as usize] {
                let mut is_first = true;
                let mut min = 0;
                let mut max = 0;
                for s in &bc.strokes {
                    if is_first || max < s.y {
                        max = s.y;
                    }
                    if is_first || min > s.y {
                        min = s.y;
                    }
                    is_first = false;
                }

                capital_height = max.abs();
                base_height = min.abs();
            }
        }

        let mut lower_case_height = capital_height / 2;
        if (b'q' as usize) < characters.len() {
            if let Some(bc) = &characters[b'q' as usize] {
                let mut is_first = true;
                let mut min = 0;
                let mut max = 0;
                for s in &bc.strokes {
                    if is_first || max < s.y {
                        max = s.y;
                    }
                    if is_first || min > s.y {
                        min = s.y;
                    }
                    is_first = false;
                }
                descender_height = min.abs();
            }
        }

        if (b'x' as usize) < characters.len() {
            if let Some(bc) = &characters[b'x' as usize] {
                let mut is_first = true;
                let mut min = 0;
                let mut max = 0;
                for s in &bc.strokes {
                    if is_first || max < s.y {
                        max = s.y;
                    }
                    if is_first || min > s.y {
                        min = s.y;
                    }
                    is_first = false;
                }
                lower_case_height = max.abs();
            }
        }

        Ok(Self {
            name: font_name,
            size: font_size,
            org_to_cap,
            org_to_base,
            org_to_dec,

            characters,

            capital_height,
            base_height,
            descender_height,
            lower_case_height,
        })
    }

    pub fn from_file(file: &str) -> EngineResult<Self> {
        let mut file = std::fs::File::open(file)?;
        let mut buf: Vec<u8> = Vec::new();
        file.read_to_end(&mut buf)?;
        Self::load(&buf)
    }

    pub fn draw_character(&self, bgi: &mut Bgi, buf: &mut dyn EditableScreen, x: i32, y: i32, dir: Direction, size: i32, character: u8) -> f32 {
        if character as usize >= self.characters.len() {
            return 0.0;
        }

        if let Some(ch) = &self.characters[character as usize] {
            ch.draw(bgi, buf, self, x, y, dir, size);
            ch.get_width(size)
        } else {
            0.0
        }
    }

    pub fn get_real_text_size(&self, str: &str, dir: Direction, size: i32) -> Size {
        let mut width = 0.0;
        for c in str.bytes() {
            if c as usize >= self.characters.len() {
                continue;
            }
            if let Some(ch) = &self.characters[c as usize] {
                width += ch.get_width(size);
            }
        }
        match dir {
            Direction::Horizontal => Size::new(
                width as i32,
                (self.get_height() + self.org_to_dec.abs() as i32 + 1) * SCALE_UP[size as usize] / SCALE_DOWN[size as usize],
            ),
            Direction::Vertical => Size::new(
                (self.get_height() + self.org_to_dec.abs() as i32 + 1) * SCALE_UP[size as usize] / SCALE_DOWN[size as usize],
                width as i32,
            ),
        }
    }

    pub fn get_text_size(&self, str: &str, dir: Direction, size: i32) -> Size {
        let mut width = 0.0;
        for c in str.bytes() {
            if c as usize >= self.characters.len() {
                continue;
            }
            if let Some(ch) = &self.characters[c as usize] {
                width += ch.width as f64;
            }
        }
        match dir {
            Direction::Horizontal => Size::new(
                width as i32,
                (self.get_height() + self.org_to_dec as i32 + 1) * SCALE_UP[size as usize] / SCALE_DOWN[size as usize],
            ),
            Direction::Vertical => Size::new(
                (self.get_height() + self.org_to_dec as i32 + 1) * SCALE_UP[size as usize] / SCALE_DOWN[size as usize],
                width as i32,
            ),
        }
    }

    pub fn get_max_character_size(&self, dir: Direction, size: i32) -> Size {
        let mut width = 0.0f32;
        for ch in self.characters.iter().flatten() {
            width = width.max(ch.get_width(size));
        }
        match dir {
            Direction::Horizontal => Size::new(
                width.round() as i32,
                (self.get_height() + self.org_to_dec as i32 + 1) * SCALE_UP[size as usize] / SCALE_DOWN[size as usize],
            ),
            Direction::Vertical => Size::new(
                (self.get_height() + self.org_to_dec as i32 + 1) * SCALE_UP[size as usize] / SCALE_DOWN[size as usize],
                width.round() as i32,
            ),
        }
    }
}
