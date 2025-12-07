use crate::{
    AttributedChar, BitFont, BufferFeatures, Result, FontMode, IceMode, LoadingError, OutputFormat, Palette, PaletteMode, Position, SavingError,
    TextBuffer, TextPane, analyze_font_usage, attribute, guess_font_name,
};
use std::path::Path;

use super::{LoadData, SaveOptions, TextAttribute};

const XBIN_HEADER_SIZE: usize = 11;
const XBIN_PALETTE_LENGTH: usize = 3 * 16;

const FLAG_PALETTE: u8 = 0b_0000_0001;
const FLAG_FONT: u8 = 0b_0000_0010;
const FLAG_COMPRESS: u8 = 0b_0000_0100;
const FLAG_NON_BLINK_MODE: u8 = 0b_0000_1000;
const FLAG_512CHAR_MODE: u8 = 0b_0001_0000;

lazy_static::lazy_static! {
    /// ICE mode, no extended font (single font)
    static ref ATTR_TABLE_ICE: [TextAttribute; 256] = {
        let mut table: [TextAttribute; 256] = [TextAttribute::default(); 256];
        for i in 0u8..=255 {
            let bg = (i >> 4) as u32;
            let fg = (i & 0b1111) as u32;
            table[i as usize] = TextAttribute {
                font_page: 0,
                foreground_color: fg,
                background_color: bg,
                attr: attribute::NONE,
            };
        }
        table
    };

    /// ICE mode, extended font (512 char mode)
    static ref ATTR_TABLE_ICE_EXT: [TextAttribute; 256] = {
        let mut table: [TextAttribute; 256] = [TextAttribute::default(); 256];
        for i in 0u8..=255 {
            let bg = (i >> 4) as u32;
            let fg = (i & 0b1111) as u32;
            let (font_page, actual_fg) = if fg > 7 { (1, fg - 8) } else { (0, fg) };
            table[i as usize] = TextAttribute {
                font_page,
                foreground_color: actual_fg,
                background_color: bg,
                attr: attribute::NONE,
            };
        }
        table
    };

    /// Blink mode, no extended font (single font)
    static ref ATTR_TABLE_BLINK: [TextAttribute; 256] = {
        let mut table: [TextAttribute; 256] = [TextAttribute::default(); 256];
        for i in 0u8..=255 {
            let blink = i & 0b1000_0000 != 0;
            let bg = ((i >> 4) & 0b0111) as u32;
            let fg = (i & 0b1111) as u32;
            table[i as usize] = TextAttribute {
                font_page: 0,
                foreground_color: fg,
                background_color: bg,
                attr: if blink { attribute::BLINK } else { attribute::NONE },
            };
        }
        table
    };

    /// Blink mode, extended font (512 char mode)
    static ref ATTR_TABLE_BLINK_EXT: [TextAttribute; 256] = {
        let mut table: [TextAttribute; 256] = [TextAttribute::default(); 256];
        for i in 0u8..=255 {
            let blink = i & 0b1000_0000 != 0;
            let bg = ((i >> 4) & 0b0111) as u32;
            let fg = (i & 0b1111) as u32;
            let (font_page, actual_fg) = if fg > 7 { (1, fg - 8) } else { (0, fg) };
            table[i as usize] = TextAttribute {
                font_page,
                foreground_color: actual_fg,
                background_color: bg,
                attr: if blink { attribute::BLINK } else { attribute::NONE },
            };
        }
        table
    };
}

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
enum Compression {
    Off = 0b0000_0000,
    Char = 0b0100_0000,
    Attr = 0b1000_0000,
    Full = 0b1100_0000,
}

#[derive(Default)]
pub struct XBin {}

impl OutputFormat for XBin {
    fn get_file_extension(&self) -> &str {
        "xb"
    }

    fn get_name(&self) -> &str {
        "XBin"
    }

    fn analyze_features(&self, _features: &BufferFeatures) -> String {
        String::new()
    }

    fn to_bytes(&self, buf: &mut crate::TextBuffer, options: &SaveOptions) -> Result<Vec<u8>> {
        let mut result = Vec::new();

        result.extend_from_slice(b"XBIN");
        result.push(0x1A); // CP/M EOF char (^Z) - used by DOS as well

        result.push(buf.get_width() as u8);
        result.push((buf.get_width() >> 8) as u8);
        result.push(buf.get_height() as u8);
        result.push((buf.get_height() >> 8) as u8);

        let mut flags = 0;
        let fonts = analyze_font_usage(buf);
        let Some(font) = buf.get_font(fonts[0]) else {
            return Err(SavingError::NoFontFound.into());
        };
        if font.length() != 256 {
            return Err(crate::EngineError::InvalidXBin { message: "1st font must be 256 chars long".to_string() });
        }

        if fonts.len() > 2 {
            return Err(crate::EngineError::InvalidXBin { message: "Only up to 2 fonts are supported".to_string() });
        }

        if font.size().width != 8 || font.size().height < 1 || font.size().height > 32 {
            return Err(SavingError::InvalidXBinFont.into());
        }

        result.push(font.size().height as u8);
        if !font.is_default() || !buf.has_fonts() || fonts.len() > 1 {
            flags |= FLAG_FONT;
        }

        if !buf.palette.is_default() {
            flags |= FLAG_PALETTE;
        }

        if options.compress {
            flags |= FLAG_COMPRESS;
        }

        if matches!(buf.ice_mode, IceMode::Ice) {
            flags |= FLAG_NON_BLINK_MODE;
        }

        if fonts.len() == 2 {
            flags |= FLAG_512CHAR_MODE;
        }

        result.push(flags);

        if (flags & FLAG_PALETTE) == FLAG_PALETTE {
            let mut pal = buf.palette.clone();
            pal.fill_to_16();
            let palette_data = pal.as_vec_63();
            if palette_data.len() != XBIN_PALETTE_LENGTH {
                return Err(crate::EngineError::InvalidXBin {
                    message: format!("Invalid palette data length was {} should be {}", palette_data.len(), XBIN_PALETTE_LENGTH),
                });
            }
            result.extend(palette_data);
        }
        if flags & FLAG_FONT == FLAG_FONT {
            let font_data = font.convert_to_u8_data();
            let font_len = font_data.len();
            if font_len != 256 * font.size().height as usize {
                return Err(crate::EngineError::InvalidXBin { message: "Invalid font length".to_string() });
            }
            result.extend(font_data);
            if flags & FLAG_512CHAR_MODE == FLAG_512CHAR_MODE {
                if fonts.len() != 2 {
                    return Err(crate::EngineError::InvalidXBin { message: "File needs 2 fonts for 512 char mode".to_string() });
                }
                if let Some(ext_font) = buf.get_font(fonts[1]) {
                    if ext_font.length() != 256 {
                        return Err(crate::EngineError::InvalidXBin { message: "2nd font must be 256 chars long".to_string() });
                    }

                    let ext_font_data = ext_font.convert_to_u8_data();
                    if ext_font_data.len() != font_len {
                        return Err(crate::EngineError::InvalidXBin { message: "2nd font must be same height as 1st font".to_string() });
                    }
                    result.extend(ext_font_data);
                } else {
                    return Err(crate::EngineError::InvalidXBin { message: "Can't get second font".to_string() });
                }
            }
        }
        if options.compress {
            compress_backtrack(&mut result, buf, &fonts)?;
        } else {
            for y in 0..buf.get_height() {
                for x in 0..buf.get_width() {
                    let ch = buf.get_char((x, y).into());
                    let attr = encode_attr(buf, ch, &fonts);
                    let ch = ch.ch as u32;
                    if ch > 255 {
                        return Err(SavingError::Only8BitCharactersSupported.into());
                    }
                    result.push(ch as u8);
                    result.push(attr);
                }
            }
        }

        if let Some(sauce) = &options.save_sauce {
            sauce.write(&mut result)?;
        }
        Ok(result)
    }

    fn load_buffer(&self, file_name: &Path, data: &[u8], load_data_opt: Option<LoadData>) -> Result<crate::TextBuffer> {
        let mut result = TextBuffer::new((80, 25));
        result.terminal_state.is_terminal_buffer = false;
        result.file_name = Some(file_name.into());
        let load_data = load_data_opt.unwrap_or_default();
        let max_height = load_data.max_height();
        if let Some(sauce) = &load_data.sauce_opt {
            super::apply_sauce_to_buffer(&mut result, sauce);
        }

        if data.len() < XBIN_HEADER_SIZE {
            return Err(LoadingError::FileTooShort.into());
        }
        if b"XBIN" != &data[0..4] {
            return Err(LoadingError::IDMismatch.into());
        }

        let mut o = 4;

        // let eof_char = bytes[o];
        o += 1;
        let width = data[o] as i32 + ((data[o + 1] as i32) << 8);
        if !(1..=4096).contains(&width) {
            return Err(crate::EngineError::InvalidXBin { message: format!("Width out of range: {} (1-4096)", width) });
        }
        result.set_width(width);
        o += 2;
        let mut height = data[o] as i32 + ((data[o + 1] as i32) << 8);
        // Apply height limit if specified
        if let Some(max_h) = max_height {
            height = height.min(max_h);
        }
        result.set_height(height);
        // Pre-allocate lines for the known size - this is the key optimization
        result.layers[0].preallocate_lines(width, height);
        o += 2;
        let mut font_size = data[o];
        if font_size == 0 {
            font_size = 16;
        }
        if font_size > 32 {
            return Err(crate::EngineError::InvalidXBin { message: format!("Font height too large: {} (32 max)", font_size) });
        }
        o += 1;
        let flags = data[o];
        o += 1;

        let has_custom_palette = (flags & FLAG_PALETTE) == FLAG_PALETTE;
        let has_custom_font = (flags & FLAG_FONT) == FLAG_FONT;
        let is_compressed = (flags & FLAG_COMPRESS) == FLAG_COMPRESS;
        let use_ice = (flags & FLAG_NON_BLINK_MODE) == FLAG_NON_BLINK_MODE;
        let extended_char_mode = (flags & FLAG_512CHAR_MODE) == FLAG_512CHAR_MODE;

        result.font_mode = if extended_char_mode { FontMode::FixedSize } else { FontMode::Single };
        result.palette_mode = if extended_char_mode { PaletteMode::Free8 } else { PaletteMode::Free16 };
        result.ice_mode = if use_ice { IceMode::Ice } else { IceMode::Blink };

        if has_custom_palette {
            result.palette = Palette::from_63(&data[o..(o + XBIN_PALETTE_LENGTH)]);
            o += XBIN_PALETTE_LENGTH;
        }
        if has_custom_font {
            let font_length = font_size as usize * 256;
            result.clear_font_table();
            let mut font = BitFont::create_8("", 8, font_size, &data[o..(o + font_length)]);
            font.yaff_font.name = Some(guess_font_name(&font));
            result.set_font(0, font);
            o += font_length;
            if extended_char_mode {
                let mut font = BitFont::create_8("", 8, font_size, &data[o..(o + font_length)]);
                font.yaff_font.name = Some(guess_font_name(&font));
                result.set_font(1, font);
                o += font_length;
            }
        }
        if is_compressed {
            read_data_compressed(&mut result, &data[o..])?;
        } else {
            read_data_uncompressed(&mut result, &data[o..])?;
        }
        Ok(result)
    }
}

/// Advance position - fast version without TextBuffer reference
#[inline(always)]
fn advance_pos_fast(width: i32, height: i32, pos: &mut Position) -> bool {
    pos.x += 1;
    if pos.x >= width {
        pos.x = 0;
        pos.y += 1;
        if pos.y >= height {
            return false;
        }
    }
    true
}

fn select_attr_table(result: &TextBuffer) -> &'static [TextAttribute; 256] {
    match (result.ice_mode, matches!(result.font_mode, FontMode::FixedSize)) {
        (IceMode::Ice | IceMode::Unlimited, false) => &*ATTR_TABLE_ICE,
        (IceMode::Ice | IceMode::Unlimited, true) => &*ATTR_TABLE_ICE_EXT,
        (IceMode::Blink, false) => &*ATTR_TABLE_BLINK,
        (IceMode::Blink, true) => &*ATTR_TABLE_BLINK_EXT,
    }
}

fn read_data_compressed(result: &mut TextBuffer, bytes: &[u8]) -> Result<bool> {
    let mut pos = Position::default();
    let width = result.get_width();
    let height = result.get_height();
    let attr_table = select_attr_table(result);
    let mut o = 0;
    let len = bytes.len();

    while o < len {
        // SAFETY: o < len checked above
        let xbin_compression = unsafe { *bytes.get_unchecked(o) };

        o += 1;
        let compression = unsafe { std::mem::transmute(xbin_compression & 0b_1100_0000) };
        let repeat_counter = (xbin_compression & 0b_0011_1111) + 1;

        match compression {
            Compression::Off => {
                for _ in 0..repeat_counter {
                    if o + 2 > len {
                        log::error!("Invalid XBin. Read char block beyond EOF.");
                        break;
                    }
                    // SAFETY: o + 2 <= len checked above
                    let char_code = unsafe { *bytes.get_unchecked(o) };
                    let attribute = unsafe { *bytes.get_unchecked(o + 1) };
                    o += 2;
                    let attributed_char = decode_char(attr_table, char_code, attribute);
                    result.layers[0].set_char_unchecked(pos, attributed_char);

                    if !advance_pos_fast(width, height, &mut pos) {
                        return Ok(true);
                    }
                }
            }
            Compression::Char => {
                if o >= len {
                    log::error!("Invalid XBin. Read char compression block beyond EOF.");
                    break;
                }
                // SAFETY: o < len checked above
                let char_code = unsafe { *bytes.get_unchecked(o) };
                o += 1;
                for _ in 0..repeat_counter {
                    if o >= len {
                        log::error!("Invalid XBin. Read char compression block beyond EOF.");
                        break;
                    }
                    // SAFETY: o < len checked above
                    let attributed_char = decode_char(attr_table, char_code, unsafe { *bytes.get_unchecked(o) });
                    result.layers[0].set_char_unchecked(pos, attributed_char);
                    o += 1;
                    if !advance_pos_fast(width, height, &mut pos) {
                        return Ok(true);
                    }
                }
            }
            Compression::Attr => {
                if o >= len {
                    log::error!("Invalid XBin. Read attribute compression block beyond EOF.");
                    break;
                }
                // SAFETY: o < len checked above
                let attribute = unsafe { *bytes.get_unchecked(o) };
                o += 1;
                for _ in 0..repeat_counter {
                    if o >= len {
                        log::error!("Invalid XBin. Read attribute compression block beyond EOF.");
                        break;
                    }
                    // SAFETY: o < len checked above
                    let attributed_char = decode_char(attr_table, unsafe { *bytes.get_unchecked(o) }, attribute);
                    result.layers[0].set_char_unchecked(pos, attributed_char);
                    o += 1;
                    if !advance_pos_fast(width, height, &mut pos) {
                        return Ok(true);
                    }
                }
            }
            Compression::Full => {
                if o + 2 > len {
                    log::error!("Invalid XBin. Read compression block beyond EOF.");
                    break;
                }
                // SAFETY: o + 2 <= len checked above
                let char_code = unsafe { *bytes.get_unchecked(o) };
                let attr = unsafe { *bytes.get_unchecked(o + 1) };
                o += 2;
                let rep_ch = decode_char(attr_table, char_code, attr);

                for _ in 0..repeat_counter {
                    result.layers[0].set_char_unchecked(pos, rep_ch);
                    if !advance_pos_fast(width, height, &mut pos) {
                        return Ok(true);
                    }
                }
            }
        }
    }

    Ok(true)
}

#[inline(always)]
fn decode_char(attr_table: &[TextAttribute; 256], char_code: u8, attr: u8) -> AttributedChar {
    AttributedChar::new(char_code as char, attr_table[attr as usize])
}

fn encode_attr(buf: &TextBuffer, ch: AttributedChar, fonts: &[usize]) -> u8 {
    if fonts.len() == 2 {
        (ch.attribute.as_u8(buf.ice_mode) & 0b_1111_0111) | if ch.attribute.font_page as usize == fonts[1] { 0b1000 } else { 0 }
    } else {
        ch.attribute.as_u8(buf.ice_mode)
    }
}

fn read_data_uncompressed(result: &mut TextBuffer, bytes: &[u8]) -> Result<bool> {
    let width = result.get_width();
    let height = result.get_height();
    let attr_table = select_attr_table(result);
    let mut pos = Position::default();
    let mut o = 0;
    let len = bytes.len();

    while o + 1 < len {
        // SAFETY: o + 1 < len checked above
        let char_code = unsafe { *bytes.get_unchecked(o) };
        let attr = unsafe { *bytes.get_unchecked(o + 1) };
        let attributed_char = decode_char(attr_table, char_code, attr);
        result.layers[0].set_char_unchecked(pos, attributed_char);
        o += 2;
        if !advance_pos_fast(width, height, &mut pos) {
            return Ok(true);
        }
    }

    if o < len {
        // last byte is not important enough to throw an error
        // there seem to be some invalid files out there.
        log::error!("Invalid XBin. Read char block beyond EOF.");
    }

    Ok(true)
}

fn count_length(
    mut run_mode: Compression,
    mut run_ch: AttributedChar,
    mut end_run: Option<bool>,
    mut run_count: u8,
    buffer: &TextBuffer,
    y: i32,
    mut x: i32,
) -> usize {
    let mut count = 0;
    while x < buffer.get_width() {
        let cur = buffer.get_char((x, y).into());
        let next = buffer.get_char((x + 1, y).into());

        if run_count > 0 {
            if end_run.is_none() {
                if run_count >= 64 {
                    end_run = Some(true);
                } else if run_count > 0 {
                    match run_mode {
                        Compression::Off => {
                            if x + 2 < buffer.get_width() && cur == next {
                                end_run = Some(true);
                            } else if x + 2 < buffer.get_width() {
                                let next2 = buffer.get_char((x + 2, y).into());
                                end_run = Some(cur.ch == next.ch && cur.ch == next2.ch || cur.attribute == next.attribute && cur.attribute == next2.attribute);
                            }
                        }
                        Compression::Char => {
                            if cur.ch != run_ch.ch {
                                end_run = Some(true);
                            } else if x + 3 < buffer.get_width() {
                                let next2 = buffer.get_char((x + 2, y).into());
                                let next3 = buffer.get_char((x + 3, y).into());
                                end_run = Some(cur == next && cur == next2 && cur == next3);
                            }
                        }
                        Compression::Attr => {
                            if cur.attribute != run_ch.attribute {
                                end_run = Some(true);
                            } else if x + 3 < buffer.get_width() {
                                let next2 = buffer.get_char((x + 2, y).into());
                                let next3 = buffer.get_char((x + 3, y).into());
                                end_run = Some(cur == next && cur == next2 && cur == next3);
                            }
                        }
                        Compression::Full => {
                            end_run = Some(cur != run_ch);
                        }
                    }
                }
            }

            if let Some(true) = end_run {
                count += 1;
                run_count = 0;
            }
        }
        end_run = None;

        if run_count > 0 {
            match run_mode {
                Compression::Off => {
                    count += 2;
                }
                Compression::Char | Compression::Attr => {
                    count += 1;
                }
                Compression::Full => {
                    // nothing
                }
            }
        } else {
            if x + 1 < buffer.get_width() {
                if cur == next {
                    run_mode = Compression::Full;
                } else if cur.ch == next.ch {
                    run_mode = Compression::Char;
                } else if cur.attribute == next.attribute {
                    run_mode = Compression::Attr;
                } else {
                    run_mode = Compression::Off;
                }
            } else {
                run_mode = Compression::Off;
            }
            count += 2;
            run_ch = cur;
            end_run = None;
        }
        run_count += 1;
        x += 1;
    }
    count
}

fn compress_backtrack(outputdata: &mut Vec<u8>, buffer: &TextBuffer, fonts: &[usize]) -> Result<()> {
    for y in 0..buffer.get_height() {
        let mut run_buf = Vec::new();
        let mut run_mode = Compression::Off;
        let mut run_count = 0;
        let mut run_ch = AttributedChar::default();

        for x in 0..buffer.get_width() {
            let cur = buffer.get_char((x, y).into());

            let next = if x + 1 < buffer.get_width() {
                buffer.get_char((x + 1, y).into())
            } else {
                AttributedChar::default()
            };

            if run_count > 0 {
                let mut end_run = false;
                if run_count >= 64 {
                    end_run = true;
                } else if run_count > 0 {
                    match run_mode {
                        Compression::Off => {
                            if x + 2 < buffer.get_width() && (cur.ch == next.ch || cur.attribute == next.attribute) {
                                let l1 = count_length(run_mode, run_ch, Some(true), run_count, buffer, y, x);
                                let l2 = count_length(run_mode, run_ch, Some(false), run_count, buffer, y, x);
                                end_run = l1 < l2;
                            }
                        }
                        Compression::Char => {
                            if cur.ch != run_ch.ch || cur.get_font_page() != run_ch.get_font_page() {
                                end_run = true;
                            } else if x + 4 < buffer.get_width() {
                                let next2 = buffer.get_char((x + 2, y).into());
                                if cur.attribute == next.attribute && cur.attribute == next2.attribute {
                                    let l1 = count_length(run_mode, run_ch, Some(true), run_count, buffer, y, x);
                                    let l2 = count_length(run_mode, run_ch, Some(false), run_count, buffer, y, x);
                                    end_run = l1 < l2;
                                }
                            }
                        }
                        Compression::Attr => {
                            if cur.attribute != run_ch.attribute || cur.get_font_page() != run_ch.get_font_page() {
                                end_run = true;
                            } else if x + 3 < buffer.get_width() {
                                let next2 = buffer.get_char((x + 2, y).into());
                                if cur.ch == next.ch && cur.ch == next2.ch {
                                    let l1 = count_length(run_mode, run_ch, Some(true), run_count, buffer, y, x);
                                    let l2 = count_length(run_mode, run_ch, Some(false), run_count, buffer, y, x);
                                    end_run = l1 < l2;
                                }
                            }
                        }
                        Compression::Full => {
                            end_run = cur != run_ch;
                        }
                    }
                }

                if end_run {
                    outputdata.push((run_mode as u8) | (run_count - 1));
                    outputdata.extend(&run_buf);
                    run_count = 0;
                }
            }

            let ch_code = cur.ch as u32;
            if ch_code > 255 {
                return Err(SavingError::Only8BitCharactersSupported.into());
            }
            if run_count > 0 {
                match run_mode {
                    Compression::Off => {
                        run_buf.push(ch_code as u8);
                        run_buf.push(encode_attr(buffer, cur, fonts));
                    }
                    Compression::Char => {
                        run_buf.push(encode_attr(buffer, cur, fonts));
                    }
                    Compression::Attr => {
                        run_buf.push(ch_code as u8);
                    }
                    Compression::Full => {
                        // nothing
                    }
                }
            } else {
                run_buf.clear();
                if x + 1 < buffer.get_width() {
                    if cur == next {
                        run_mode = Compression::Full;
                    } else if cur.ch == next.ch {
                        run_mode = Compression::Char;
                    } else if cur.attribute == next.attribute {
                        run_mode = Compression::Attr;
                    } else {
                        run_mode = Compression::Off;
                    }
                } else {
                    run_mode = Compression::Off;
                }
                if let Compression::Attr = run_mode {
                    run_buf.push(encode_attr(buffer, cur, fonts));
                    run_buf.push(ch_code as u8);
                } else {
                    run_buf.push(ch_code as u8);
                    run_buf.push(encode_attr(buffer, cur, fonts));
                }

                run_ch = cur;
            }
            run_count += 1;
        }

        if run_count > 0 {
            outputdata.push((run_mode as u8) | (run_count - 1));
            outputdata.extend(run_buf);
        }
    }
    Ok(())
}

pub fn get_save_sauce_default_xb(_buf: &TextBuffer) -> (bool, String) {
    (false, String::new())
}
