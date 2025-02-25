use std::path::Path;

use super::{LoadData, SaveOptions, TextAttribute};
use crate::{
    AttributedChar, BitFont, Buffer, EngineResult, IceMode, LoadingError, OutputFormat, Palette, Position, SavingError, Size, TextPane, analyze_font_usage,
    guess_font_name,
};

// http://fileformats.archiveteam.org/wiki/ICEDraw

const HEADER_SIZE: usize = 4 + 4 * 2;

const IDF_V1_3_HEADER: &[u8] = b"\x041.3";
// TODO: Find source for 1.4 - 1.3 is the only one I could find now, has been too long since I wrote this code.
//       and I cant see differences from 1.3 in my implementation.
const IDF_V1_4_HEADER: &[u8] = b"\x041.4";

const FONT_SIZE: usize = 4096;
const PALETTE_SIZE: usize = 3 * 16;

#[derive(Default)]
pub(super) struct IceDraw {}

impl OutputFormat for IceDraw {
    fn get_file_extension(&self) -> &str {
        "idf"
    }

    fn get_name(&self) -> &str {
        "IceDraw"
    }

    fn to_bytes(&self, buf: &mut crate::Buffer, options: &SaveOptions) -> EngineResult<Vec<u8>> {
        if buf.ice_mode != IceMode::Ice {
            return Err(anyhow::anyhow!("Only ice mode files are supported by this format."));
        }

        if buf.get_height() > 200 {
            return Err(anyhow::anyhow!("Only up do 200 lines are supported by this format."));
        }
        let fonts = analyze_font_usage(buf);
        if fonts.len() > 1 {
            return Err(anyhow::anyhow!("Only single font files are supported by this format."));
        }
        if buf.palette.len() != 16 {
            return Err(anyhow::anyhow!("Only 16 color palettes are supported by this format."));
        }

        let mut result = IDF_V1_4_HEADER.to_vec();
        // x1
        result.push(0);
        result.push(0);

        // y1
        result.push(0);
        result.push(0);

        let w = buf.get_width().saturating_sub(1);
        result.push(w as u8);
        result.push((w >> 8) as u8);

        let h = buf.get_height().saturating_sub(1);
        result.push(h as u8);
        result.push((h >> 8) as u8);

        for y in 0..buf.get_height() {
            let mut x = 0;
            while x < buf.get_width() {
                let ch = buf.get_char((x, y));
                let mut rle_count = 1;
                if options.compress {
                    while x + rle_count < buf.get_width() && rle_count < (u16::MAX) as i32 {
                        if ch != buf.get_char((x + rle_count, y)) {
                            break;
                        }
                        rle_count += 1;
                    }
                    if rle_count > 3 || ch.ch == '\x01' {
                        result.push(1);
                        result.push(0);

                        result.push(rle_count as u8);
                        result.push((rle_count >> 8) as u8);
                    } else {
                        rle_count = 1;
                    }
                }
                let attr = ch.attribute.as_u8(buf.ice_mode);
                let ch = ch.ch as u32;
                if ch > 255 {
                    return Err(SavingError::Only8BitCharactersSupported.into());
                }

                // fake repeat
                if ch == 1 && attr == 0 && rle_count == 1 {
                    result.extend([1, 0, 1, 0]);
                }
                result.push(ch as u8);
                result.push(attr);

                x += rle_count;
            }
        }

        // font
        if buf.get_font_dimensions() != Size::new(8, 16) {
            return Err(SavingError::Only8x16FontsSupported.into());
        }
        if let Some(font) = buf.get_font(fonts[0]) {
            result.extend(font.convert_to_u8_data());
        } else {
            return Err(SavingError::NoFontFound.into());
        }

        // palette
        result.extend(buf.palette.as_vec_63());
        if options.save_sauce {
            buf.write_sauce_info(icy_sauce::SauceDataType::BinaryText, icy_sauce::char_caps::ContentType::Unknown(0), &mut result)?;
        }
        Ok(result)
    }

    fn load_buffer(&self, file_name: &Path, data: &[u8], _load_data_opt: Option<LoadData>) -> EngineResult<crate::Buffer> {
        let mut result = Buffer::new((80, 25));
        result.ice_mode = IceMode::Ice;
        result.is_terminal_buffer = false;
        result.file_name = Some(file_name.into());

        if data.len() < HEADER_SIZE + FONT_SIZE + PALETTE_SIZE {
            return Err(LoadingError::FileTooShort.into());
        }
        let version = &data[0..4];

        if version != IDF_V1_3_HEADER && version != IDF_V1_4_HEADER {
            return Err(LoadingError::IDMismatch.into());
        }

        let mut o = 4;
        let x1 = (data[o] as u16 + ((data[o + 1] as u16) << 8)) as i32;
        o += 2;
        let y1 = (data[o] as u16 + ((data[o + 1] as u16) << 8)) as i32;
        o += 2;
        let x2 = (data[o] as u16 + ((data[o + 1] as u16) << 8)) as i32;
        o += 2;
        // skip y2
        o += 2;

        if x2 < x1 {
            return Err(anyhow::anyhow!("invalid bounds for idf width needs to be >=0."));
        }

        result.set_width(x2 - x1 + 1);
        let data_size = data.len() - FONT_SIZE - PALETTE_SIZE;
        let mut pos = Position::new(x1, y1);

        while o + 1 < data_size {
            let mut rle_count = 1;
            let mut char_code = data[o];
            o += 1;
            let mut attr = data[o];
            o += 1;

            if char_code == 1 && attr == 0 {
                rle_count = data[o] as i32 + ((data[o + 1] as i32) << 8);

                if o + 3 >= data_size {
                    break;
                }
                o += 2;
                char_code = data[o];
                o += 1;
                attr = data[o];
                o += 1;
            }
            while rle_count > 0 {
                result.layers[0].set_height(pos.y + 1);
                result.set_height(pos.y + 1);
                let attribute = TextAttribute::from_u8(attr, result.ice_mode);
                result.layers[0].set_char(pos, AttributedChar::new(char_code as char, attribute));
                advance_pos(x1, x2, &mut pos);
                rle_count -= 1;
            }
        }
        let mut font = BitFont::from_basic(8, 16, &data[o..(o + FONT_SIZE)]);
        font.name = guess_font_name(&font);
        result.set_font(0, font);
        o += FONT_SIZE;

        result.palette = Palette::from_63(&data[o..(o + PALETTE_SIZE)]);
        Ok(result)
    }
}

pub fn get_save_sauce_default_idf(buf: &Buffer) -> (bool, String) {
    if buf.get_width() != 80 {
        return (true, "width != 80".to_string());
    }

    if buf.has_sauce() {
        return (true, String::new());
    }

    (false, String::new())
}

fn advance_pos(x1: i32, x2: i32, pos: &mut Position) -> bool {
    pos.x += 1;
    if pos.x > x2 {
        pos.x = x1;
        pos.y += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use crate::{AttributedChar, BitFont, Buffer, Color, OutputFormat, TextAttribute, TextPane, compare_buffers};

    #[test]
    pub fn test_ice() {
        let mut buffer = create_buffer();
        buffer.ice_mode = crate::IceMode::Ice;
        buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::from_u8(0b0000_1000, crate::IceMode::Ice)));
        buffer.layers[0].set_char((1, 0), AttributedChar::new('B', TextAttribute::from_u8(0b1100_1111, crate::IceMode::Ice)));
        test_ice_draw(&mut buffer);
    }

    #[test]
    pub fn test_repeat_char() {
        let mut buffer = create_buffer();
        buffer.ice_mode = crate::IceMode::Ice;
        buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::from_u8(0b0000_1000, crate::IceMode::Ice)));
        buffer.layers[0].set_char((1, 0), AttributedChar::new('\x01', TextAttribute::from_u8(0, crate::IceMode::Ice)));
        test_ice_draw(&mut buffer);
    }

    #[test]
    pub fn test_custom_palette() {
        let mut buffer = create_buffer();
        buffer.ice_mode = crate::IceMode::Ice;

        for i in 0..4 {
            buffer.palette.set_color(i, Color::new(8 + i as u8 * 8, 0, 0));
        }
        for i in 0..4 {
            buffer.palette.set_color(4 + i, Color::new(0, 8 + i as u8 * 8, 0));
        }
        for i in 0..4 {
            buffer.palette.set_color(8 + i, Color::new(0, 0, 8 + i as u8 * 8));
        }
        for i in 0..3 {
            buffer.palette.set_color(12 + i, Color::new(i as u8 * 16, i as u8 * 8, 8 + i as u8 * 8));
        }

        buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::from_u8(0b0000_1000, crate::IceMode::Ice)));
        buffer.layers[0].set_char((1, 0), AttributedChar::new('B', TextAttribute::from_u8(0b1100_1111, crate::IceMode::Ice)));
        test_ice_draw(&mut buffer);
    }

    #[test]
    pub fn test_custom_font() {
        let mut buffer = create_buffer();
        buffer.set_font(0, BitFont::from_ansi_font_page(42).unwrap());
        buffer.ice_mode = crate::IceMode::Ice;
        buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::from_u8(0b0000_1000, crate::IceMode::Blink)));
        test_ice_draw(&mut buffer);
    }

    fn create_buffer() -> Buffer {
        let mut buffer = Buffer::new((80, 25));
        for y in 0..buffer.get_height() {
            for x in 0..buffer.get_width() {
                buffer.layers[0].set_char((x, y), AttributedChar::new(' ', TextAttribute::default()));
            }
        }
        buffer
    }

    fn test_ice_draw(buffer: &mut Buffer) -> Buffer {
        let xb = super::IceDraw::default();
        let mut opt = crate::SaveOptions::default();
        opt.compress = false;
        let bytes = xb.to_bytes(buffer, &opt).unwrap();
        let buffer2 = xb.load_buffer(std::path::Path::new("test.idf"), &bytes, None).unwrap();
        compare_buffers(buffer, &buffer2, crate::CompareOptions::ALL);
        buffer2
    }
}
