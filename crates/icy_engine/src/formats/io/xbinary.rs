use crate::{
    AttributeColor, AttributedChar, BitFont, FontMode, IceMode, LoadingError, Palette, Result, SavingError, TextBuffer, TextPane, TextScreen,
    analyze_font_usage, attribute, guess_font_name,
};

use rayon::prelude::*;

use super::super::{AnsiSaveOptionsV2, LoadData, TextAttribute};

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
            let bg = i >> 4;
            let fg = i & 0b1111;
            let mut text_attr = TextAttribute::default();
            text_attr.set_font_page(0);
            text_attr.set_foreground_color(AttributeColor::Palette(fg));
            text_attr.set_background_color(AttributeColor::Palette(bg));
            table[i as usize] = text_attr;
        }
        table
    };

    /// ICE mode, extended font (512 char mode)
    static ref ATTR_TABLE_ICE_EXT: [TextAttribute; 256] = {
        let mut table: [TextAttribute; 256] = [TextAttribute::default(); 256];
        for i in 0u8..=255 {
            let bg = i >> 4;
            let fg = i & 0b1111;
            let (font_page, actual_fg) = if fg > 7 { (1, fg - 8) } else { (0, fg) };
            let mut text_attr = TextAttribute::default();
            text_attr.set_font_page(font_page);
            text_attr.set_foreground_color(AttributeColor::Palette(actual_fg));
            text_attr.set_background_color(AttributeColor::Palette(bg));
            table[i as usize] = text_attr;
        }
        table
    };

    /// Blink mode, no extended font (single font)
    static ref ATTR_TABLE_BLINK: [TextAttribute; 256] = {
        let mut table: [TextAttribute; 256] = [TextAttribute::default(); 256];
        for i in 0u8..=255 {
            let blink = i & 0b1000_0000 != 0;
            let bg = (i >> 4) & 0b0111;
            let fg = i & 0b1111;
            let mut text_attr = TextAttribute::default();
            text_attr.attr = if blink { attribute::BLINK } else { attribute::NONE };
            text_attr.set_font_page(0);
            text_attr.set_foreground_color(AttributeColor::Palette(fg));
            text_attr.set_background_color(AttributeColor::Palette(bg));
            table[i as usize] = text_attr;
        }
        table
    };

    /// Blink mode, extended font (512 char mode)
    static ref ATTR_TABLE_BLINK_EXT: [TextAttribute; 256] = {
        let mut table: [TextAttribute; 256] = [TextAttribute::default(); 256];
        for i in 0u8..=255 {
            let blink = i & 0b1000_0000 != 0;
            let bg = (i >> 4) & 0b0111;
            let fg = i & 0b1111;
            let (font_page, actual_fg) = if fg > 7 { (1, fg - 8) } else { (0, fg) };
            let mut text_attr = TextAttribute::default();
            text_attr.attr = if blink { attribute::BLINK } else { attribute::NONE };
            text_attr.set_font_page(font_page);
            text_attr.set_foreground_color(AttributeColor::Palette(actual_fg));
            text_attr.set_background_color(AttributeColor::Palette(bg));
            table[i as usize] = text_attr;
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

pub(crate) fn save_xbin(buf: &TextBuffer, options: &AnsiSaveOptionsV2) -> Result<Vec<u8>> {
    // Reserve a reasonable upper bound to avoid repeated reallocations.
    // For compressed output we reserve less to avoid a large upfront allocation.
    let pixel_bytes = (buf.width() as usize)
        .checked_mul(buf.height() as usize)
        .and_then(|v| v.checked_mul(2))
        .unwrap_or(0);
    let reserve_pixels = if options.compress { pixel_bytes / 2 } else { pixel_bytes };
    let mut result = Vec::with_capacity(XBIN_HEADER_SIZE + XBIN_PALETTE_LENGTH + reserve_pixels);

    result.extend_from_slice(b"XBIN");
    result.push(0x1A); // CP/M EOF char (^Z) - used by DOS as well

    result.push(buf.width() as u8);
    result.push((buf.width() >> 8) as u8);
    result.push(buf.height() as u8);
    result.push((buf.height() >> 8) as u8);

    let mut flags = 0;

    // FontSize is always part of the header (11 bytes total). Default VGA is 16.
    let mut fonts: Vec<u8> = Vec::new();
    let mut font_size: u8 = 16;
    let mut write_font_data = false;

    if buf.has_fonts() {
        // Fast path: if only 1 font slot, skip expensive analyze_font_usage (~21% hash overhead)
        let font_count = buf.font_count();
        fonts = if font_count <= 1 { vec![0u8] } else { analyze_font_usage(buf) };
        let primary_slot = *fonts.first().unwrap_or(&0) as u8;
        let Some(font) = buf.font(primary_slot) else {
            return Err(SavingError::NoFontFound.into());
        };
        if font.length() != 256 {
            return Err(crate::EngineError::InvalidXBin {
                message: "1st font must be 256 chars long".to_string(),
            });
        }

        if fonts.len() > 2 {
            return Err(crate::EngineError::InvalidXBin {
                message: "Only up to 2 fonts are supported".to_string(),
            });
        }

        if font.size().width != 8 || font.size().height < 1 || font.size().height > 32 {
            return Err(SavingError::InvalidXBinFont.into());
        }

        font_size = font.size().height as u8;

        // Spec requirements:
        // - Font bit indicates font data present.
        // - 512Chars requires Font bit to be set.
        if fonts.len() == 2 {
            flags |= FLAG_FONT;
            flags |= FLAG_512CHAR_MODE;
            write_font_data = true;
        } else if font_size != 16 || !font.is_default() {
            flags |= FLAG_FONT;
            write_font_data = true;
        }
    }

    // Always write FontSize in header.
    result.push(font_size);

    if !buf.palette.is_default() {
        flags |= FLAG_PALETTE;
    }

    if options.compress {
        flags |= FLAG_COMPRESS;
    }

    if matches!(buf.ice_mode, IceMode::Ice) {
        flags |= FLAG_NON_BLINK_MODE;
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

    if write_font_data {
        let primary_slot = *fonts.first().unwrap_or(&0) as u8;
        let Some(font) = buf.font(primary_slot) else {
            return Err(SavingError::NoFontFound.into());
        };
        let font_data = font.convert_to_u8_data();
        let font_len = font_data.len();
        if font_len != 256 * font.size().height as usize {
            return Err(crate::EngineError::InvalidXBin {
                message: "Invalid font length".to_string(),
            });
        }
        result.extend(font_data);
        if (flags & FLAG_512CHAR_MODE) == FLAG_512CHAR_MODE {
            let secondary_slot = *fonts.get(1).unwrap_or(&1) as u8;
            if let Some(ext_font) = buf.font(secondary_slot) {
                if ext_font.length() != 256 {
                    return Err(crate::EngineError::InvalidXBin {
                        message: "2nd font must be 256 chars long".to_string(),
                    });
                }

                let ext_font_data = ext_font.convert_to_u8_data();
                if ext_font_data.len() != font_len {
                    return Err(crate::EngineError::InvalidXBin {
                        message: "2nd font must be same height as 1st font".to_string(),
                    });
                }
                result.extend(ext_font_data);
            } else {
                return Err(crate::EngineError::InvalidXBin {
                    message: "Can't get second font".to_string(),
                });
            }
        }
    }

    if options.compress {
        compress_backtrack(&mut result, buf, &fonts)?;
    } else {
        for y in 0..buf.height() {
            for x in 0..buf.width() {
                let ch = buf.char_at((x, y).into());
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

/// Note: SAUCE is applied externally by FileFormat::from_bytes().
pub(crate) fn load_xbin(data: &[u8], load_data_opt: Option<&LoadData>) -> Result<TextScreen> {
    let mut screen = TextScreen::new((80, 25));
    screen.buffer.terminal_state.is_terminal_buffer = false;
    let max_height = load_data_opt.and_then(|ld| ld.max_height());

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
    if !(0..=4096).contains(&width) {
        return Err(crate::EngineError::InvalidXBin {
            message: format!("Width out of range: {} (0-4096)", width),
        });
    }
    screen.buffer.set_width(width);
    o += 2;
    let mut height = data[o] as i32 + ((data[o + 1] as i32) << 8);
    // Apply height limit if specified
    if let Some(max_h) = max_height {
        height = height.min(max_h);
    }
    screen.buffer.set_height(height);
    // Pre-allocate lines for the known size - this is the key optimization
    screen.buffer.layers[0].preallocate_lines(width, height);
    o += 2;
    let mut font_size = data[o];
    if font_size == 0 {
        font_size = 16;
    }
    if font_size > 32 {
        return Err(crate::EngineError::InvalidXBin {
            message: format!("Font height too large: {} (32 max)", font_size),
        });
    }
    o += 1;
    let flags = data[o];
    o += 1;

    let has_custom_palette = (flags & FLAG_PALETTE) == FLAG_PALETTE;
    let has_custom_font = (flags & FLAG_FONT) == FLAG_FONT;
    let is_compressed = (flags & FLAG_COMPRESS) == FLAG_COMPRESS;
    let use_ice = (flags & FLAG_NON_BLINK_MODE) == FLAG_NON_BLINK_MODE;
    let extended_char_mode = (flags & FLAG_512CHAR_MODE) == FLAG_512CHAR_MODE;

    // Spec: 512Chars requires Font bit to be set.
    if extended_char_mode && !has_custom_font {
        return Err(crate::EngineError::InvalidXBin {
            message: "512Chars flag set but Font flag is not set".to_string(),
        });
    }

    // Spec: If Font bit is not set, default font size should be VGA 16.
    if !has_custom_font && font_size != 16 {
        return Err(crate::EngineError::InvalidXBin {
            message: format!("FontSize {} requires Font flag to be set", font_size),
        });
    }

    screen.buffer.font_mode = if extended_char_mode { FontMode::FixedSize } else { FontMode::Single };
    screen.buffer.ice_mode = if use_ice { IceMode::Ice } else { IceMode::Blink };

    if has_custom_palette {
        if o + XBIN_PALETTE_LENGTH > data.len() {
            return Err(LoadingError::FileTooShort.into());
        }
        screen.buffer.palette = Palette::from_63(&data[o..(o + XBIN_PALETTE_LENGTH)]);
        o += XBIN_PALETTE_LENGTH;
    }
    if has_custom_font {
        let font_length = font_size as usize * 256;
        if o + font_length > data.len() {
            return Err(LoadingError::FileTooShort.into());
        }
        screen.buffer.clear_font_table();
        let mut font = BitFont::create_8("", 8, font_size, &data[o..(o + font_length)]);
        font.name = guess_font_name(&font);
        screen.buffer.set_font(0, font);
        o += font_length;
        if extended_char_mode {
            if o + font_length > data.len() {
                return Err(LoadingError::FileTooShort.into());
            }
            let mut font = BitFont::create_8("", 8, font_size, &data[o..(o + font_length)]);
            font.name = guess_font_name(&font);
            screen.buffer.set_font(1, font);
            o += font_length;
        }
    }

    // Image data is optional; allow width/height == 0 as palette/font-only containers.
    if width > 0 && height > 0 {
        if is_compressed {
            read_data_compressed(&mut screen.buffer, &data[o..])?;
        } else {
            read_data_uncompressed(&mut screen.buffer, &data[o..])?;
        }
    }
    Ok(screen)
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
    let width = result.width();
    let height = result.height();
    let width_u = width as usize;
    let height_u = height as usize;
    if width_u == 0 || height_u == 0 {
        return Ok(true);
    }

    // Fast writer into preallocated layer buffer
    let lines_ptr = result.layers[0].lines.as_mut_ptr();
    let mut x: usize = 0;
    let mut y: usize = 0;

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
                let mut rep = repeat_counter as usize;
                let available_pairs = (len.saturating_sub(o)) >> 1;
                if rep > available_pairs {
                    log::error!("Invalid XBin. Read char block beyond EOF.");
                    rep = available_pairs;
                }

                for _ in 0..rep {
                    // SAFETY: ensured by rep <= available_pairs
                    let char_code = unsafe { *bytes.get_unchecked(o) };
                    let attribute = unsafe { *bytes.get_unchecked(o + 1) };
                    o += 2;
                    let attributed_char = decode_char(attr_table, char_code, attribute);

                    // SAFETY: lines are preallocated to width/height
                    unsafe {
                        let line = &mut *lines_ptr.add(y);
                        *line.chars.get_unchecked_mut(x) = attributed_char;
                    }
                    x += 1;
                    if x >= width_u {
                        x = 0;
                        y += 1;
                        if y >= height_u {
                            return Ok(true);
                        }
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

                let mut rep = repeat_counter as usize;
                let available = len.saturating_sub(o);
                if rep > available {
                    log::error!("Invalid XBin. Read char compression block beyond EOF.");
                    rep = available;
                }

                for _ in 0..rep {
                    // SAFETY: ensured by rep <= available
                    let attributed_char = decode_char(attr_table, char_code, unsafe { *bytes.get_unchecked(o) });
                    o += 1;

                    unsafe {
                        let line = &mut *lines_ptr.add(y);
                        *line.chars.get_unchecked_mut(x) = attributed_char;
                    }
                    x += 1;
                    if x >= width_u {
                        x = 0;
                        y += 1;
                        if y >= height_u {
                            return Ok(true);
                        }
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

                let mut rep = repeat_counter as usize;
                let available = len.saturating_sub(o);
                if rep > available {
                    log::error!("Invalid XBin. Read attribute compression block beyond EOF.");
                    rep = available;
                }

                for _ in 0..rep {
                    let attributed_char = decode_char(attr_table, unsafe { *bytes.get_unchecked(o) }, attribute);
                    o += 1;

                    unsafe {
                        let line = &mut *lines_ptr.add(y);
                        *line.chars.get_unchecked_mut(x) = attributed_char;
                    }
                    x += 1;
                    if x >= width_u {
                        x = 0;
                        y += 1;
                        if y >= height_u {
                            return Ok(true);
                        }
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
                    unsafe {
                        let line = &mut *lines_ptr.add(y);
                        *line.chars.get_unchecked_mut(x) = rep_ch;
                    }
                    x += 1;
                    if x >= width_u {
                        x = 0;
                        y += 1;
                        if y >= height_u {
                            return Ok(true);
                        }
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

#[inline(always)]
fn encode_attr(buf: &TextBuffer, ch: AttributedChar, fonts: &[u8]) -> u8 {
    if fonts.len() == 2 {
        (ch.attribute.as_u8(buf.ice_mode) & 0b_1111_0111) | if ch.attribute.font_page() == fonts[1] { 0b1000 } else { 0 }
    } else {
        ch.attribute.as_u8(buf.ice_mode)
    }
}

fn read_data_uncompressed(result: &mut TextBuffer, bytes: &[u8]) -> Result<bool> {
    let width = result.width();
    let height = result.height();
    let width_u = width as usize;
    let height_u = height as usize;
    if width_u == 0 || height_u == 0 {
        return Ok(true);
    }
    let attr_table = select_attr_table(result);
    let lines_ptr = result.layers[0].lines.as_mut_ptr();
    let mut x: usize = 0;
    let mut y: usize = 0;
    let mut o = 0;
    let len = bytes.len();

    while o + 1 < len {
        // SAFETY: o + 1 < len checked above
        let char_code = unsafe { *bytes.get_unchecked(o) };
        let attr = unsafe { *bytes.get_unchecked(o + 1) };
        let attributed_char = decode_char(attr_table, char_code, attr);
        // SAFETY: lines are preallocated to width/height
        unsafe {
            let line = &mut *lines_ptr.add(y);
            *line.chars.get_unchecked_mut(x) = attributed_char;
        }
        o += 2;
        x += 1;
        if x >= width_u {
            x = 0;
            y += 1;
            if y >= height_u {
                return Ok(true);
            }
        }
    }

    if o < len {
        // last byte is not important enough to throw an error
        // there seem to be some invalid files out there.
        log::error!("Invalid XBin. Read char block beyond EOF.");
    }

    Ok(true)
}

fn compress_backtrack(outputdata: &mut Vec<u8>, buffer: &TextBuffer, fonts: &[u8]) -> Result<()> {
    // XBin compression is line-local. Encode each line independently (in parallel) and
    // append in scanline order to preserve identical output.
    let width = buffer.width() as usize;
    let height = buffer.height() as usize;
    if width == 0 || height == 0 {
        return Ok(());
    }

    let line_outputs: Result<Vec<Vec<u8>>> = (0..height)
        .into_par_iter()
        .map(|yy| {
            let y = yy as i32;
            let mut line_out: Vec<u8> = Vec::new();

            let mut ch_bytes: Vec<u8> = vec![0; width];
            let mut attr_bytes: Vec<u8> = vec![0; width];
            let mut dp_cost: Vec<usize> = vec![usize::MAX; width + 1];
            let mut dp_prev: Vec<usize> = vec![0; width + 1];
            let mut dp_mode: Vec<Compression> = vec![Compression::Off; width + 1];
            let mut runs: Vec<(usize, usize, Compression)> = Vec::with_capacity(width);

            compress_line_optimal(
                &mut line_out,
                buffer,
                fonts,
                y,
                &mut ch_bytes,
                &mut attr_bytes,
                &mut dp_cost,
                &mut dp_prev,
                &mut dp_mode,
                &mut runs,
            )?;

            Ok(line_out)
        })
        .collect();

    for mut line in line_outputs? {
        outputdata.append(&mut line);
    }
    Ok(())
}

/// Compress a single line using dynamic programming to find the optimal compression
fn compress_line_optimal(
    outputdata: &mut Vec<u8>,
    buffer: &TextBuffer,
    fonts: &[u8],
    y: i32,
    ch_bytes: &mut [u8],
    attr_bytes: &mut [u8],
    dp_cost: &mut [usize],
    dp_prev: &mut [usize],
    dp_mode: &mut [Compression],
    runs: &mut Vec<(usize, usize, Compression)>,
) -> Result<()> {
    let width = buffer.width() as usize;
    debug_assert_eq!(ch_bytes.len(), width);
    debug_assert_eq!(attr_bytes.len(), width);
    debug_assert_eq!(dp_cost.len(), width + 1);
    debug_assert_eq!(dp_prev.len(), width + 1);
    debug_assert_eq!(dp_mode.len(), width + 1);

    // Precompute the serialized bytes for this line once.
    let mut x = 0usize;
    while x < width {
        let cur = buffer.char_at((x as i32, y).into());
        let ch_code = cur.ch as u32;
        if ch_code > 255 {
            return Err(SavingError::Only8BitCharactersSupported.into());
        }
        // SAFETY: x < width, slices have length width
        unsafe {
            *ch_bytes.get_unchecked_mut(x) = ch_code as u8;
            *attr_bytes.get_unchecked_mut(x) = encode_attr(buffer, cur, fonts);
        }
        x += 1;
    }

    // DP init
    dp_cost.fill(usize::MAX);
    dp_cost[0] = 0;
    dp_prev[0] = 0;
    dp_mode[0] = Compression::Off;

    let ch_ptr = ch_bytes.as_ptr();
    let attr_ptr = attr_bytes.as_ptr();
    let dp_cost_ptr = dp_cost.as_mut_ptr();
    let dp_prev_ptr = dp_prev.as_mut_ptr();
    let dp_mode_ptr = dp_mode.as_mut_ptr();

    let mut i = 0usize;
    while i < width {
        // SAFETY: i < width, dp_cost length is width+1
        let current_cost = unsafe { *dp_cost_ptr.add(i) };
        if current_cost == usize::MAX {
            i += 1;
            continue;
        }

        // SAFETY: i < width
        let first_ch = unsafe { *ch_ptr.add(i) };
        let first_attr = unsafe { *attr_ptr.add(i) };

        let base_cost = current_cost + 3; // header + (ch,attr)

        let mut full_valid = true;
        let mut char_valid = true;
        let mut attr_valid = true;

        // Tight loop: keep bounds checks out of the inner loop.
        let max_run = 64.min(width - i);
        let mut run_len = 1;
        while run_len <= max_run {
            let end = i + run_len;
            let idx = end - 1;

            // SAFETY: idx < width
            let cur_ch = unsafe { *ch_ptr.add(idx) };
            let cur_attr = unsafe { *attr_ptr.add(idx) };

            if full_valid && (cur_ch != first_ch || cur_attr != first_attr) {
                full_valid = false;
            }
            if char_valid && cur_ch != first_ch {
                char_valid = false;
            }
            if attr_valid && cur_attr != first_attr {
                attr_valid = false;
            }

            if full_valid {
                // Full: header + (ch,attr)
                // SAFETY: end <= width
                unsafe {
                    let best = dp_cost_ptr.add(end);
                    if base_cost < *best {
                        *best = base_cost;
                        *dp_prev_ptr.add(end) = i;
                        *dp_mode_ptr.add(end) = Compression::Full;
                    }
                }
            }

            if char_valid {
                // Char: header + (ch,attr) + (run_len-1) attrs
                let cost = base_cost + (run_len - 1);
                unsafe {
                    let best = dp_cost_ptr.add(end);
                    if cost < *best {
                        *best = cost;
                        *dp_prev_ptr.add(end) = i;
                        *dp_mode_ptr.add(end) = Compression::Char;
                    }
                }
            }

            if attr_valid {
                // Attr: header + (attr,ch) + (run_len-1) chars
                let cost = base_cost + (run_len - 1);
                unsafe {
                    let best = dp_cost_ptr.add(end);
                    if cost < *best {
                        *best = cost;
                        *dp_prev_ptr.add(end) = i;
                        *dp_mode_ptr.add(end) = Compression::Attr;
                    }
                }
            }

            // Off: header + 2*run_len
            let cost = current_cost + 1 + (run_len << 1);
            unsafe {
                let best = dp_cost_ptr.add(end);
                if cost < *best {
                    *best = cost;
                    *dp_prev_ptr.add(end) = i;
                    *dp_mode_ptr.add(end) = Compression::Off;
                }
            }

            run_len += 1;
        }

        i += 1;
    }

    // Reconstruct runs
    runs.clear();
    let mut pos = width;
    while pos > 0 {
        let start = dp_prev[pos];
        let mode = dp_mode[pos];
        runs.push((start, pos, mode));
        pos = start;
    }
    runs.reverse();

    // Emit
    for (start, end, mode) in runs.iter().copied() {
        let run_len = end - start;
        outputdata.push((mode as u8) | ((run_len - 1) as u8));

        match mode {
            Compression::Full => {
                outputdata.push(ch_bytes[start]);
                outputdata.push(attr_bytes[start]);
            }
            Compression::Char => {
                outputdata.push(ch_bytes[start]);
                outputdata.push(attr_bytes[start]);
                outputdata.extend_from_slice(&attr_bytes[start + 1..end]);
            }
            Compression::Attr => {
                outputdata.push(attr_bytes[start]);
                outputdata.push(ch_bytes[start]);
                outputdata.extend_from_slice(&ch_bytes[start + 1..end]);
            }
            Compression::Off => {
                for i in start..end {
                    outputdata.push(ch_bytes[i]);
                    outputdata.push(attr_bytes[i]);
                }
            }
        }
    }

    Ok(())
}

pub fn _get_save_sauce_default_xb(_buf: &TextBuffer) -> (bool, String) {
    (false, String::new())
}
