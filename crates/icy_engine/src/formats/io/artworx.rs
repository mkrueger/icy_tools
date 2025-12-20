use super::super::{AnsiSaveOptionsV2, LoadData};
use crate::{
    AttributedChar, BitFont, BufferType, Color, EGA_PALETTE, FontMode, IceMode, LoadingError, Palette, Position, Result, SavingError, TextAttribute,
    TextBuffer, TextPane, TextScreen, analyze_font_usage, guess_font_name,
};

// http://fileformats.archiveteam.org/wiki/ArtWorx_Data_Format

// u8                   Version
// 3 * 64 = 192 u8      Palette
// 256 * 16 = 4096 u8   Font Data (only 8x16 supported)
// [ch u8, attr u8]*    Screen data
//
// A very simple format with a weird palette storage. Only 16 colors got used but a full 64 color palette is stored.
// Maybe useful for DOS demos running in text mode.

const HEADER_LENGTH: usize = 1 + 3 * 64 + 4096;
const VERSION: u8 = 1;

pub(crate) fn save_artworx(buf: &TextBuffer, options: &AnsiSaveOptionsV2) -> Result<Vec<u8>> {
    if buf.ice_mode != IceMode::Ice {
        return Err(crate::EngineError::OnlyIceModeSupported);
    }
    if buf.width() != 80 {
        return Err(crate::EngineError::WidthNotSupported { width: 80 });
    }
    if buf.palette.len() != 16 {
        return Err(crate::EngineError::Only16ColorPalettesSupported);
    }

    let fonts = analyze_font_usage(buf);
    if fonts.len() > 1 {
        return Err(crate::EngineError::OnlySingleFontSupported);
    }

    let mut result = vec![1]; // version
    result.extend(to_ega_data(&buf.palette));
    if buf.font_dimensions().height != 16 {
        return Err(SavingError::Only8x16FontsSupported.into());
    }

    if let Some(font) = buf.font(fonts[0]) {
        result.extend(font.convert_to_u8_data());
    } else {
        return Err(SavingError::NoFontFound.into());
    }

    for y in 0..buf.height() {
        for x in 0..buf.width() {
            let ch = buf.char_at((x, y).into());
            let attr = ch.attribute.as_u8(IceMode::Ice);
            let ch = ch.ch as u32;
            if ch > 255 {
                return Err(SavingError::Only8BitCharactersSupported.into());
            }
            result.push(ch as u8);
            result.push(attr);
        }
    }
    if let Some(sauce) = &options.save_sauce {
        sauce.write(&mut result)?;
    }
    Ok(result)
}

/// Note: SAUCE is applied externally by FileFormat::from_bytes().
pub(crate) fn load_artworx(data: &[u8], load_data_opt: Option<&LoadData>) -> Result<TextScreen> {
    let mut screen = TextScreen::new((80, 25));
    screen.buffer.terminal_state.is_terminal_buffer = false;
    let max_height = load_data_opt.and_then(|ld| ld.max_height());

    screen.buffer.set_width(80);
    screen.buffer.buffer_type = BufferType::CP437;
    screen.buffer.ice_mode = IceMode::Ice;
    screen.buffer.font_mode = FontMode::Single;
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
    screen.buffer.palette = from_ega_data(&data[o..(o + palette_size)]);
    o += palette_size;

    let font_size = 4096;
    screen.buffer.clear_font_table();
    let mut font = BitFont::from_basic(8, 16, &data[o..(o + font_size)]);
    font.yaff_font.name = Some(guess_font_name(&font));
    screen.buffer.set_font(0, font);
    o += font_size;

    loop {
        // Check height limit before processing a new row
        if let Some(max_h) = max_height {
            if pos.y >= max_h {
                return Ok(screen);
            }
        }

        for _ in 0..screen.buffer.width() {
            if o + 2 > file_size {
                return Ok(screen);
            }
            screen.buffer.layers[0].set_height(pos.y + 1);
            screen.buffer.set_height(pos.y + 1);

            let attribute = TextAttribute::from_u8(data[o + 1], screen.buffer.ice_mode);
            screen.buffer.layers[0].set_char(pos, AttributedChar::new(data[o] as char, attribute));
            pos.x += 1;
            o += 2;
        }
        pos.x = 0;
        pos.y += 1;
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
        ega_colors[EGA_COLOR_OFFSETS[i]] = palette.color(i as u32);
    }
    let mut res = Vec::with_capacity(3 * 64);
    for col in ega_colors {
        let (r, g, b) = col.rgb();
        res.push(r >> 2);
        res.push(g >> 2);
        res.push(b >> 2);
    }
    res
}

pub fn _get_save_sauce_default_adf(buf: &TextBuffer) -> (bool, String) {
    if buf.width() != 80 {
        return (true, "width != 80".to_string());
    }

    (false, String::new())
}
