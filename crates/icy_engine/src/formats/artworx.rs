use std::path::Path;

use super::{LoadData, Position, SaveOptions, TextAttribute};
use crate::{
    AttributedChar, BitFont, BufferFeatures, BufferType, Color, EGA_PALETTE, EngineResult, FontMode, IceMode, LoadingError, OutputFormat, Palette, SavingError,
    TextBuffer, TextPane, analyze_font_usage, guess_font_name,
};

// http://fileformats.archiveteam.org/wiki/ArtWorx_Data_Format

// u8                   Version
// 3 * 64 = 192 u8      Palette
// 256 * 16 = 4096 u8   Font Data (only 8x16 supported)
// [ch u8, attr u8]*    Screen data
//
// A very simple format with a weird palette storage. Only 16 colors got used but a full 64 color palette is stored.
// Maybe useful for DOS demos running in text mode.

#[derive(Default)]
pub(crate) struct Artworx {}

const HEADER_LENGTH: usize = 1 + 3 * 64 + 4096;
const VERSION: u8 = 1;

impl OutputFormat for Artworx {
    fn get_file_extension(&self) -> &str {
        "adf"
    }

    fn get_name(&self) -> &str {
        "Artworx"
    }

    fn analyze_features(&self, _features: &BufferFeatures) -> String {
        String::new()
    }

    fn to_bytes(&self, buf: &mut crate::TextBuffer, options: &SaveOptions) -> EngineResult<Vec<u8>> {
        if buf.ice_mode != IceMode::Ice {
            return Err(anyhow::anyhow!("Only ice mode files are supported by this format."));
        }
        if buf.get_width() != 80 {
            return Err(anyhow::anyhow!("Only width==80 files are supported by this format."));
        }
        if buf.palette.len() != 16 {
            return Err(anyhow::anyhow!("Only 16 color palettes are supported by this format."));
        }

        let fonts = analyze_font_usage(buf);
        if fonts.len() > 1 {
            return Err(anyhow::anyhow!("Only single font files are supported by this format."));
        }

        let mut result = vec![1]; // version
        result.extend(to_ega_data(&buf.palette));
        if buf.get_font_dimensions().height != 16 {
            return Err(SavingError::Only8x16FontsSupported.into());
        }

        if let Some(font) = buf.get_font(fonts[0]) {
            result.extend(font.convert_to_u8_data());
        } else {
            return Err(SavingError::NoFontFound.into());
        }

        for y in 0..buf.get_height() {
            for x in 0..buf.get_width() {
                let ch = buf.get_char((x, y).into());
                let attr = ch.attribute.as_u8(IceMode::Ice);
                let ch = ch.ch as u32;
                if ch > 255 {
                    return Err(SavingError::Only8BitCharactersSupported.into());
                }
                result.push(ch as u8);
                result.push(attr);
            }
        }
        if options.save_sauce {
            buf.write_sauce_info(icy_sauce::SauceDataType::Character, icy_sauce::CharacterFormat::Ansi, &mut result)?;
        }
        Ok(result)
    }

    fn load_buffer(&self, file_name: &Path, data: &[u8], load_data_opt: Option<LoadData>) -> EngineResult<crate::TextBuffer> {
        let mut result = TextBuffer::new((80, 25));
        result.terminal_state.is_terminal_buffer = false;
        result.file_name = Some(file_name.into());
        let load_data = load_data_opt.unwrap_or_default();
        let max_height = load_data.max_height();
        if let Some(sauce) = load_data.sauce_opt {
            result.load_sauce(sauce);
        }
        result.set_width(80);
        result.buffer_type = BufferType::CP437;
        result.palette_mode = crate::PaletteMode::Free16;
        result.ice_mode = IceMode::Ice;
        result.font_mode = FontMode::Single;
        let file_size = data.len();
        let mut o = 0;
        let mut pos = Position::default();
        if file_size < HEADER_LENGTH {
            return Err(LoadingError::FileTooShort.into());
        }

        let version = data[o];
        if version != VERSION {
            return Err(LoadingError::UnsupportedADFVersion(version).into());
        }
        o += 1;

        // convert EGA -> VGA colors.
        let palette_size = 3 * 64;
        result.palette = from_ega_data(&data[o..(o + palette_size)]);
        o += palette_size;

        let font_size = 4096;
        result.clear_font_table();
        let mut font = BitFont::from_basic(8, 16, &data[o..(o + font_size)]);
        font.yaff_font.name = Some(guess_font_name(&font));
        result.set_font(0, font);
        o += font_size;

        loop {
            // Check height limit before processing a new row
            if let Some(max_h) = max_height {
                if pos.y >= max_h {
                    return Ok(result);
                }
            }

            for _ in 0..result.get_width() {
                if o + 2 > file_size {
                    return Ok(result);
                }
                result.layers[0].set_height(pos.y + 1);
                result.set_height(pos.y + 1);

                let attribute = TextAttribute::from_u8(data[o + 1], result.ice_mode);
                result.layers[0].set_char(pos, AttributedChar::new(data[o] as char, attribute));
                pos.x += 1;
                o += 2;
            }
            pos.x = 0;
            pos.y += 1;
        }
    }
}

static EGA_COLOR_OFFSETS: [usize; 16] = [0, 1, 2, 3, 4, 5, 20, 7, 56, 57, 58, 59, 60, 61, 62, 63];

pub fn from_ega_data(pal: &[u8]) -> Palette {
    let mut colors = Vec::new();
    for i in EGA_COLOR_OFFSETS {
        let o = 3 * i;

        let r = pal[o];
        let g = pal[o + 1];
        let b = pal[o + 2];
        colors.push(Color::new(r << 2 | r >> 4, g << 2 | g >> 4, b << 2 | b >> 4));
    }

    Palette::from_slice(&colors)
}

pub fn to_ega_data(palette: &Palette) -> Vec<u8> {
    // just store the first 16 colors to the standard EGA palette
    let mut ega_colors = EGA_PALETTE.to_vec();
    for i in 0..16 {
        if i >= palette.len() {
            break;
        }
        ega_colors[EGA_COLOR_OFFSETS[i]] = palette.get_color(i as u32);
    }
    let mut res = Vec::with_capacity(3 * 64);
    for col in ega_colors {
        let (r, g, b) = col.get_rgb();
        res.push(r >> 2);
        res.push(g >> 2);
        res.push(b >> 2);
    }
    res
}

pub fn get_save_sauce_default_adf(buf: &TextBuffer) -> (bool, String) {
    if buf.get_width() != 80 {
        return (true, "width != 80".to_string());
    }

    if buf.has_sauce() {
        return (true, String::new());
    }

    (false, String::new())
}
