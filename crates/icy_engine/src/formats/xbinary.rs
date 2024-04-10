use std::path::Path;

use icy_sauce::SauceInformation;

use crate::{
    analyze_font_usage, guess_font_name, AttributedChar, BitFont, Buffer, BufferFeatures, EngineResult, FontMode, IceMode, LoadingError, OutputFormat, Palette,
    PaletteMode, Position, SavingError, TextPane,
};

use super::{SaveOptions, TextAttribute};

const XBIN_HEADER_SIZE: usize = 11;
const XBIN_PALETTE_LENGTH: usize = 3 * 16;

const FLAG_PALETTE: u8 = 0b_0000_0001;
const FLAG_FONT: u8 = 0b_0000_0010;
const FLAG_COMPRESS: u8 = 0b_0000_0100;
const FLAG_NON_BLINK_MODE: u8 = 0b_0000_1000;
const FLAG_512CHAR_MODE: u8 = 0b_0001_0000;

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
enum Compression {
    Off = 0b0000_0000,
    Char = 0b0100_0000,
    Attr = 0b1000_0000,
    Full = 0b1100_0000,
}

#[derive(Default)]
pub(super) struct XBin {}

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

    fn to_bytes(&self, buf: &crate::Buffer, options: &SaveOptions) -> EngineResult<Vec<u8>> {
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
        if font.length != 256 {
            return Err(anyhow::anyhow!("File needs 1st font to be 256 chars long."));
        }

        if fonts.len() > 2 {
            return Err(anyhow::anyhow!("Only up to 2 fonts are supported by this format."));
        }

        if font.size.width != 8 || font.size.height < 1 || font.size.height > 32 {
            return Err(SavingError::InvalidXBinFont.into());
        }

        result.push(font.size.height as u8);
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
                return Err(anyhow::anyhow!(
                    "Invalid palette data length was {} should be {}.",
                    palette_data.len(),
                    XBIN_PALETTE_LENGTH
                ));
            }
            result.extend(palette_data);
        }
        if flags & FLAG_FONT == FLAG_FONT {
            let font_data = font.convert_to_u8_data();
            let font_len = font_data.len();
            if font_len != 256 * font.size.height as usize {
                return Err(anyhow::anyhow!("Invalid font len."));
            }
            result.extend(font_data);
            if flags & FLAG_512CHAR_MODE == FLAG_512CHAR_MODE {
                if fonts.len() != 2 {
                    return Err(anyhow::anyhow!("File needs 2 fonts for this save mode."));
                }
                if let Some(ext_font) = buf.get_font(fonts[1]) {
                    if ext_font.length != 256 {
                        return Err(anyhow::anyhow!("File needs 2nd font to be 256 chars long."));
                    }

                    let ext_font_data = ext_font.convert_to_u8_data();
                    if ext_font_data.len() != font_len {
                        return Err(anyhow::anyhow!("File needs 2nd font to be same height as 1st font."));
                    }
                    result.extend(ext_font_data);
                } else {
                    return Err(anyhow::anyhow!("Can't get second font."));
                }
            }
        }
        if options.compress {
            compress_backtrack(&mut result, buf, &fonts)?;
        } else {
            for y in 0..buf.get_height() {
                for x in 0..buf.get_width() {
                    let ch = buf.get_char((x, y));
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

        if options.save_sauce {
            buf.write_sauce_info(icy_sauce::SauceDataType::XBin, icy_sauce::char_caps::ContentType::Unknown(0), &mut result)?;
        }
        Ok(result)
    }

    fn load_buffer(&self, file_name: &Path, data: &[u8], sauce_opt: Option<SauceInformation>) -> EngineResult<crate::Buffer> {
        let mut result = Buffer::new((80, 25));
        result.is_terminal_buffer = false;
        result.file_name = Some(file_name.into());
        if let Some(sauce) = sauce_opt {
            result.load_sauce(sauce);
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
            return Err(anyhow::anyhow!("Invalid XBin. Width out of range: {width} (1-4096)."));
        }
        result.set_width(width);
        o += 2;
        let height = data[o] as i32 + ((data[o + 1] as i32) << 8);
        result.set_height(height);
        result.layers[0].set_size((width, height));
        o += 2;
        let mut font_size = data[o];
        if font_size == 0 {
            font_size = 16;
        }
        if font_size > 32 {
            return Err(anyhow::anyhow!("Invalid XBin. Font height too large: {font_size} (32 max)."));
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
            font.name = guess_font_name(&font);
            result.set_font(0, font);
            o += font_length;
            if extended_char_mode {
                let mut font = BitFont::create_8("", 8, font_size, &data[o..(o + font_length)]);
                font.name = guess_font_name(&font);
                result.set_font(1, font);
                o += font_length;
            }
        }
        if is_compressed {
            read_data_compressed(&mut result, &data[o..])?;
        } else {
            read_data_uncompressed(&mut result, &data[o..])?;
        }
        crate::crop_loaded_file(&mut result);

        Ok(result)
    }
}

fn advance_pos(result: &Buffer, pos: &mut Position) -> bool {
    pos.x += 1;
    if pos.x >= result.get_width() {
        pos.x = 0;
        pos.y += 1;
    }
    true
}

fn read_data_compressed(result: &mut Buffer, bytes: &[u8]) -> EngineResult<bool> {
    let mut pos = Position::default();
    let mut o = 0;
    while o < bytes.len() {
        let xbin_compression = bytes[o];

        o += 1;
        let compression = unsafe { std::mem::transmute(xbin_compression & 0b_1100_0000) };
        let repeat_counter = (xbin_compression & 0b_0011_1111) + 1;

        match compression {
            Compression::Off => {
                for _ in 0..repeat_counter {
                    if o + 2 > bytes.len() {
                        log::error!("Invalid XBin. Read char block beyond EOF.");
                        break;
                    }
                    let char_code = bytes[o];
                    let attribute = bytes[o + 1];
                    o += 2;
                    let attributed_char = decode_char(result, char_code, attribute);
                    result.layers[0].set_char(pos, attributed_char);

                    if !advance_pos(result, &mut pos) {
                        return Err(LoadingError::OutOfBounds.into());
                    }
                }
            }
            Compression::Char => {
                let char_code = bytes[o];
                o += 1;
                for _ in 0..repeat_counter {
                    if o + 1 > bytes.len() {
                        log::error!("Invalid XBin. Read char compression block beyond EOF.");
                        break;
                    }

                    let attributed_char = decode_char(result, char_code, bytes[o]);
                    result.layers[0].set_char(pos, attributed_char);
                    o += 1;
                    if !advance_pos(result, &mut pos) {
                        return Err(LoadingError::OutOfBounds.into());
                    }
                }
            }
            Compression::Attr => {
                let attribute = bytes[o];
                o += 1;
                for _ in 0..repeat_counter {
                    if o + 1 > bytes.len() {
                        log::error!("Invalid XBin. Read attribute compression block beyond EOF.");
                        break;
                    }
                    let attributed_char = decode_char(result, bytes[o], attribute);
                    result.layers[0].set_char(pos, attributed_char);
                    o += 1;
                    if !advance_pos(result, &mut pos) {
                        return Err(LoadingError::OutOfBounds.into());
                    }
                }
            }
            Compression::Full => {
                let char_code = bytes[o];
                o += 1;
                if o + 1 > bytes.len() {
                    log::error!("Invalid XBin. nRead compression block beyond EOF.");
                    break;
                }
                let attr = bytes[o];
                o += 1;
                let rep_ch = decode_char(result, char_code, attr);

                for _ in 0..repeat_counter {
                    result.layers[0].set_char(pos, rep_ch);
                    if !advance_pos(result, &mut pos) {
                        return Err(LoadingError::OutOfBounds.into());
                    }
                }
            }
        }
    }

    Ok(true)
}

fn decode_char(result: &Buffer, char_code: u8, attr: u8) -> AttributedChar {
    let mut attribute = TextAttribute::from_u8(attr, result.ice_mode);
    if attribute.get_foreground() > 7 && matches!(result.font_mode, FontMode::FixedSize) {
        attribute.set_font_page(1);
        attribute.set_foreground(attribute.foreground_color - 8);
    }
    AttributedChar::new(char_code as char, attribute)
}

fn encode_attr(buf: &Buffer, ch: AttributedChar, fonts: &[usize]) -> u8 {
    if fonts.len() == 2 {
        (ch.attribute.as_u8(buf.ice_mode) & 0b_1111_0111) | if ch.attribute.font_page == fonts[1] { 0b1000 } else { 0 }
    } else {
        ch.attribute.as_u8(buf.ice_mode)
    }
}

fn read_data_uncompressed(result: &mut Buffer, bytes: &[u8]) -> EngineResult<bool> {
    let mut pos = Position::default();
    let mut o = 0;
    while o < bytes.len() {
        if o + 1 >= bytes.len() {
            // last byte is not important enough to throw an error
            // there seem to be some invalid files out there.
            log::error!("Invalid XBin. Read char block beyond EOF.");
            return Ok(true);
        }
        let attributed_char = decode_char(result, bytes[o], bytes[o + 1]);
        result.layers[0].set_char(pos, attributed_char);
        o += 2;
        if !advance_pos(result, &mut pos) {
            return Err(LoadingError::OutOfBounds.into());
        }
    }

    Ok(true)
}

fn count_length(
    mut run_mode: Compression,
    mut run_ch: AttributedChar,
    mut end_run: Option<bool>,
    mut run_count: u8,
    buffer: &Buffer,
    y: i32,
    mut x: i32,
) -> usize {
    let mut count = 0;
    while x < buffer.get_width() {
        let cur = buffer.get_char((x, y));
        let next = buffer.get_char((x + 1, y));

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
                                let next2 = buffer.get_char((x + 2, y));
                                end_run = Some(cur.ch == next.ch && cur.ch == next2.ch || cur.attribute == next.attribute && cur.attribute == next2.attribute);
                            }
                        }
                        Compression::Char => {
                            if cur.ch != run_ch.ch {
                                end_run = Some(true);
                            } else if x + 3 < buffer.get_width() {
                                let next2 = buffer.get_char((x + 2, y));
                                let next3 = buffer.get_char((x + 3, y));
                                end_run = Some(cur == next && cur == next2 && cur == next3);
                            }
                        }
                        Compression::Attr => {
                            if cur.attribute != run_ch.attribute {
                                end_run = Some(true);
                            } else if x + 3 < buffer.get_width() {
                                let next2 = buffer.get_char((x + 2, y));
                                let next3 = buffer.get_char((x + 3, y));
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

fn compress_backtrack(outputdata: &mut Vec<u8>, buffer: &Buffer, fonts: &[usize]) -> EngineResult<()> {
    for y in 0..buffer.get_height() {
        let mut run_buf = Vec::new();
        let mut run_mode = Compression::Off;
        let mut run_count = 0;
        let mut run_ch = AttributedChar::default();

        for x in 0..buffer.get_width() {
            let cur = buffer.get_char((x, y));

            let next = if x + 1 < buffer.get_width() {
                buffer.get_char((x + 1, y))
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
                                let next2 = buffer.get_char((x + 2, y));
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
                                let next2 = buffer.get_char((x + 2, y));
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

pub fn get_save_sauce_default_xb(buf: &Buffer) -> (bool, String) {
    (buf.has_sauce(), String::new())
}

#[cfg(test)]
mod tests {
    use crate::{compare_buffers, AttributedChar, BitFont, Buffer, Color, OutputFormat, TextAttribute, TextPane};

    #[test]
    pub fn test_blink() {
        let mut buffer = create_xb_buffer();
        buffer.ice_mode = crate::IceMode::Blink;
        buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::from_u8(0b0000_1000, crate::IceMode::Blink)));
        buffer.layers[0].set_char((1, 0), AttributedChar::new('B', TextAttribute::from_u8(0b1000_1000, crate::IceMode::Blink)));
        let res = test_xbin(&buffer);
        let ch = res.layers[0].get_char((1, 0));

        assert_eq!(ch.attribute.get_foreground(), 0b1000);
        assert_eq!(ch.attribute.get_background(), 0b0000);
        assert!(ch.attribute.is_blinking());
    }

    #[test]
    pub fn test_ice() {
        let mut buffer = create_xb_buffer();
        buffer.ice_mode = crate::IceMode::Ice;
        buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::from_u8(0b0000_1000, crate::IceMode::Ice)));
        buffer.layers[0].set_char((1, 0), AttributedChar::new('B', TextAttribute::from_u8(0b1100_1111, crate::IceMode::Ice)));
        let res = test_xbin(&buffer);
        let ch = res.layers[0].get_char((1, 0));

        assert_eq!(ch.attribute.get_foreground(), 0b1111);
        assert_eq!(ch.attribute.get_background(), 0b1100);
    }

    #[test]
    pub fn test_custom_palette() {
        let mut buffer = create_xb_buffer();
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
        let res = test_xbin(&buffer);
        let ch = res.layers[0].get_char((1, 0));

        assert_eq!(ch.attribute.get_foreground(), 0b1111);
        assert_eq!(ch.attribute.get_background(), 0b1100);
    }

    #[test]
    pub fn test_custom_font() {
        let mut buffer = create_xb_buffer();
        buffer.set_font(0, BitFont::from_ansi_font_page(42).unwrap());
        buffer.ice_mode = crate::IceMode::Blink;
        buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::from_u8(0b0000_1000, crate::IceMode::Blink)));
        test_xbin(&buffer);
    }

    #[test]
    pub fn test_extended_font_ice() {
        let mut buffer = create_xb_buffer();
        buffer.ice_mode = crate::IceMode::Ice;
        buffer.set_font(1, BitFont::from_ansi_font_page(42).unwrap());
        let mut attr = TextAttribute::from_u8(0b1111_0111, crate::IceMode::Ice);
        attr.set_font_page(0);
        buffer.layers[0].set_char((0, 0), AttributedChar::new('A', attr));
        attr.set_font_page(1);
        buffer.layers[0].set_char((1, 0), AttributedChar::new('B', attr));
        let res = test_xbin(&buffer);

        let ch = res.layers[0].get_char((1, 0));

        assert_eq!(ch.attribute.get_foreground(), 0b0111);
        assert_eq!(ch.attribute.get_background(), 0b1111);
        assert_eq!(ch.attribute.get_font_page(), 1);
    }

    #[test]
    pub fn test_extended_font_blink() {
        let mut buffer = create_xb_buffer();
        buffer.ice_mode = crate::IceMode::Blink;
        buffer.set_font(1, BitFont::from_ansi_font_page(42).unwrap());
        let mut attr: TextAttribute = TextAttribute::from_u8(0b1111_0111, crate::IceMode::Blink);
        attr.set_font_page(0);
        buffer.layers[0].set_char((0, 0), AttributedChar::new('A', attr));
        attr.set_font_page(1);
        buffer.layers[0].set_char((1, 0), AttributedChar::new('B', attr));

        let mut opt = crate::SaveOptions::default();
        opt.compress = false;

        let res = test_xbin(&buffer);

        let ch = res.layers[0].get_char((1, 0));

        assert_eq!(ch.attribute.get_foreground(), 0b0111);
        assert_eq!(ch.attribute.get_background(), 0b0111);
        assert_eq!(ch.attribute.get_font_page(), 1);
        assert!(ch.attribute.is_blinking());
    }

    fn create_xb_buffer() -> Buffer {
        let mut buffer: Buffer = Buffer::new((80, 25));
        for y in 0..buffer.get_height() {
            for x in 0..buffer.get_width() {
                buffer.layers[0].set_char((x, y), AttributedChar::new(' ', TextAttribute::default()));
            }
        }
        buffer
    }

    fn test_xbin(buffer: &Buffer) -> Buffer {
        let xb = super::XBin::default();
        let mut opt = crate::SaveOptions::default();
        opt.compress = false;
        let bytes = xb.to_bytes(buffer, &opt).unwrap();
        let buffer2 = xb.load_buffer(std::path::Path::new("test.xb"), &bytes, None).unwrap();
        compare_buffers(buffer, &buffer2, crate::CompareOptions::ALL);

        opt.compress = true;
        let bytes = xb.to_bytes(buffer, &opt).unwrap();
        let buffer2 = xb.load_buffer(std::path::Path::new("test.xb"), &bytes, None).unwrap();
        compare_buffers(buffer, &buffer2, crate::CompareOptions::ALL);

        buffer2
    }
}
