use crate::{BufferType, IceMode, Position, Selection, Shape, TextAttribute, TextBuffer, TextPane};

pub fn get_rich_text(buffer: &TextBuffer, selection: &Selection) -> Option<String> {
    let mut rtf = String::new();

    // Collect colors (fg + bg) used in the selection
    use std::collections::HashMap;
    let mut color_map: HashMap<(u8, u8, u8), usize> = HashMap::new();
    let mut colors: Vec<(u8, u8, u8)> = Vec::new();

    let mut add_color = |rgb: (u8, u8, u8)| -> usize {
        if let Some(&idx) = color_map.get(&rgb) {
            idx
        } else {
            let idx = colors.len(); // RTF color table is 0-based
            color_map.insert(rgb, idx);
            colors.push(rgb);
            idx
        }
    };

    // Helper to extract RGB from a TextAttribute's color indices.
    let palette = &buffer.palette;

    // Enumerate selection to gather colors first
    if matches!(selection.shape, Shape::Rectangle) {
        let start = selection.min();
        let end = selection.max();
        for y in start.y..=end.y {
            for x in start.x..=end.x {
                let ch = buffer.get_char((x, y).into());
                let mut fg = ch.attribute.get_foreground();
                if buffer.buffer_type == BufferType::CP437 {
                    if ch.attribute.is_bold() && fg < 8 {
                        fg += 8; // bright variant
                    }
                }
                add_color(palette.get_rgb(fg));

                let mut bg = ch.attribute.get_background();
                if buffer.ice_mode == IceMode::Ice {
                    if ch.attribute.is_blinking() && bg < 8 {
                        bg += 8; // bright variant
                    }
                }
                if bg & (1 << 31) == 0 {
                    // skip transparent sentinel
                    add_color(palette.get_rgb(bg));
                }
            }
        }
    } else {
        let (start, end) = if selection.anchor < selection.lead {
            (selection.anchor, selection.lead)
        } else {
            (selection.lead, selection.anchor)
        };
        if start.y == end.y {
            for x in start.x..=end.x {
                let ch = buffer.get_char(Position::new(x, start.y));
                add_color(palette.get_rgb(ch.attribute.get_foreground()));
                let bg = ch.attribute.get_background();
                if bg & (1 << 31) == 0 {
                    add_color(palette.get_rgb(bg));
                }
            }
        } else {
            for x in start.x..(buffer.get_line_length(start.y)) {
                let ch = buffer.get_char(Position::new(x, start.y));
                add_color(palette.get_rgb(ch.attribute.get_foreground()));
                let bg = ch.attribute.get_background();
                if bg & (1 << 31) == 0 {
                    add_color(palette.get_rgb(bg));
                }
            }
            for y in start.y + 1..end.y {
                for x in 0..(buffer.get_line_length(y)) {
                    let ch = buffer.get_char(Position::new(x, y));
                    add_color(palette.get_rgb(ch.attribute.get_foreground()));
                    let bg = ch.attribute.get_background();
                    if bg & (1 << 31) == 0 {
                        add_color(palette.get_rgb(bg));
                    }
                }
            }
            for x in 0..=end.x {
                let ch = buffer.get_char(Position::new(x, end.y));
                add_color(palette.get_rgb(ch.attribute.get_foreground()));
                let bg = ch.attribute.get_background();
                if bg & (1 << 31) == 0 {
                    add_color(palette.get_rgb(bg));
                }
            }
        }
    }

    // Begin RTF header
    rtf.push_str("{\\rtf1\\ansi\\ansicpg1252\\deff0");

    // Font table (using Courier New as default monospace font)
    rtf.push_str("{\\fonttbl{\\f0\\fmodern\\fprq1\\fcharset0 Courier New;}}");

    // Color table
    rtf.push_str("{\\colortbl;"); // First entry is always empty
    for (r, g, b) in &colors {
        use std::fmt::Write;
        let _ = write!(rtf, "\\red{}\\green{}\\blue{};", r, g, b);
    }
    rtf.push_str("}");

    // Set default font
    rtf.push_str("\\f0 ");

    // State tracking
    let mut last_attr: Option<TextAttribute> = None;
    let mut last_fg_idx: Option<usize> = None;
    let mut last_bg_idx: Option<usize> = None;

    // Emit attribute differences
    let emit_attr = |rtf: &mut String,
                     prev: Option<TextAttribute>,
                     cur: TextAttribute,
                     fg_idx: usize,
                     bg_idx: Option<usize>,
                     last_fg_idx: &mut Option<usize>,
                     last_bg_idx: &mut Option<usize>| {
        use std::fmt::Write;

        // Foreground color change (add 1 because color table index 0 is reserved)
        if last_fg_idx.map(|i| i != fg_idx).unwrap_or(true) {
            let _ = write!(rtf, "\\cf{} ", fg_idx + 1);
            *last_fg_idx = Some(fg_idx);
        }

        // Background highlight
        if let Some(bi) = bg_idx {
            if last_bg_idx.map(|i| i != bi).unwrap_or(true) {
                let _ = write!(rtf, "\\highlight{} ", bi + 1);
                *last_bg_idx = Some(bi);
            }
        } else if last_bg_idx.is_some() {
            rtf.push_str("\\highlight0 ");
            *last_bg_idx = None;
        }

        // Attribute toggles
        let p = prev.unwrap_or_default();

        // Bold
        if p.is_bold() != cur.is_bold() {
            rtf.push_str(if cur.is_bold() { "\\b " } else { "\\b0 " });
        }

        // Italic
        if p.is_italic() != cur.is_italic() {
            rtf.push_str(if cur.is_italic() { "\\i " } else { "\\i0 " });
        }

        // Underline (single vs double)
        let prev_ul = (p.is_underlined(), p.is_double_underlined());
        let cur_ul = (cur.is_underlined(), cur.is_double_underlined());
        if prev_ul != cur_ul {
            if cur.is_double_underlined() {
                rtf.push_str("\\uldb ");
            } else if cur.is_underlined() {
                rtf.push_str("\\ul ");
            } else {
                rtf.push_str("\\ulnone ");
            }
        }

        // Strike through
        if p.is_crossed_out() != cur.is_crossed_out() {
            rtf.push_str(if cur.is_crossed_out() { "\\strike " } else { "\\strike0 " });
        }

        // Hidden text (concealed)
        if p.is_concealed() != cur.is_concealed() {
            rtf.push_str(if cur.is_concealed() { "\\v " } else { "\\v0 " });
        }
    };

    // Escape function - fixed Unicode handling
    fn rtf_escape(ch: char) -> String {
        match ch {
            '\\' => "\\\\".to_string(),
            '{' => "\\{".to_string(),
            '}' => "\\}".to_string(),
            '\n' => "\\line ".to_string(),
            '\r' => String::new(), // ignore CR
            '\t' => "\\tab ".to_string(),
            c if c as u32 <= 127 => c.to_string(),
            c => {
                // RTF uses signed 16-bit for Unicode
                let code = c as u32;
                if code <= 32767 {
                    format!("\\u{}?", code as i16)
                } else {
                    // For codes > 32767, use negative representation
                    format!("\\u{}?", (code as i32 - 65536) as i16)
                }
            }
        }
    }

    // Process content based on selection shape
    if matches!(selection.shape, Shape::Rectangle) {
        let start = selection.min();
        let end = selection.max();
        for y in start.y..=end.y {
            for x in start.x..=end.x {
                let ch = buffer.get_char((x, y).into());
                let unicode_ch = buffer.buffer_type.convert_to_unicode(ch.ch);
                let fg_rgb = palette.get_rgb(ch.attribute.get_foreground());
                let fg_idx = *color_map.get(&fg_rgb).unwrap();

                let bg_raw = ch.attribute.get_background();
                let bg_idx = if bg_raw & (1 << 31) != 0 {
                    None
                } else {
                    let bg_rgb = palette.get_rgb(bg_raw);
                    color_map.get(&bg_rgb).copied()
                };

                if last_attr.map(|la| la != ch.attribute).unwrap_or(true) || last_fg_idx != Some(fg_idx) || last_bg_idx != bg_idx {
                    emit_attr(&mut rtf, last_attr, ch.attribute, fg_idx, bg_idx, &mut last_fg_idx, &mut last_bg_idx);
                    last_attr = Some(ch.attribute);
                }

                rtf.push_str(&rtf_escape(unicode_ch));
            }
            rtf.push_str("\\par\n");
        }
    } else {
        let (start, end) = if selection.anchor < selection.lead {
            (selection.anchor, selection.lead)
        } else {
            (selection.lead, selection.anchor)
        };

        if start.y == end.y {
            // Single line selection
            for x in start.x..=end.x {
                let ch = buffer.get_char(Position::new(x, start.y));
                let unicode_ch = buffer.buffer_type.convert_to_unicode(ch.ch);
                let fg_rgb = palette.get_rgb(ch.attribute.get_foreground());
                let fg_idx = *color_map.get(&fg_rgb).unwrap();

                let bg_raw = ch.attribute.get_background();
                let bg_idx = if bg_raw & (1 << 31) != 0 {
                    None
                } else {
                    let bg_rgb = palette.get_rgb(bg_raw);
                    color_map.get(&bg_rgb).copied()
                };

                if last_attr.map(|la| la != ch.attribute).unwrap_or(true) || last_fg_idx != Some(fg_idx) || last_bg_idx != bg_idx {
                    emit_attr(&mut rtf, last_attr, ch.attribute, fg_idx, bg_idx, &mut last_fg_idx, &mut last_bg_idx);
                    last_attr = Some(ch.attribute);
                }

                rtf.push_str(&rtf_escape(unicode_ch));
            }
            rtf.push_str("\\par\n");
        } else {
            // Multi-line selection
            // First line (partial)
            for x in start.x..(buffer.get_line_length(start.y)) {
                let ch = buffer.get_char(Position::new(x, start.y));
                let unicode_ch = buffer.buffer_type.convert_to_unicode(ch.ch);
                let fg_rgb = palette.get_rgb(ch.attribute.get_foreground());
                let fg_idx = *color_map.get(&fg_rgb).unwrap();

                let bg_raw = ch.attribute.get_background();
                let bg_idx = if bg_raw & (1 << 31) != 0 {
                    None
                } else {
                    let bg_rgb = palette.get_rgb(bg_raw);
                    color_map.get(&bg_rgb).copied()
                };

                if last_attr.map(|la| la != ch.attribute).unwrap_or(true) || last_fg_idx != Some(fg_idx) || last_bg_idx != bg_idx {
                    emit_attr(&mut rtf, last_attr, ch.attribute, fg_idx, bg_idx, &mut last_fg_idx, &mut last_bg_idx);
                    last_attr = Some(ch.attribute);
                }
                rtf.push_str(&rtf_escape(unicode_ch));
            }
            rtf.push_str("\\par\n");

            // Middle lines (full)
            for y in start.y + 1..end.y {
                for x in 0..(buffer.get_line_length(y)) {
                    let ch = buffer.get_char(Position::new(x, y));
                    let unicode_ch = buffer.buffer_type.convert_to_unicode(ch.ch);
                    let fg_rgb = palette.get_rgb(ch.attribute.get_foreground());
                    let fg_idx = *color_map.get(&fg_rgb).unwrap();

                    let bg_raw = ch.attribute.get_background();
                    let bg_idx = if bg_raw & (1 << 31) != 0 {
                        None
                    } else {
                        let bg_rgb = palette.get_rgb(bg_raw);
                        color_map.get(&bg_rgb).copied()
                    };

                    if last_attr.map(|la| la != ch.attribute).unwrap_or(true) || last_fg_idx != Some(fg_idx) || last_bg_idx != bg_idx {
                        emit_attr(&mut rtf, last_attr, ch.attribute, fg_idx, bg_idx, &mut last_fg_idx, &mut last_bg_idx);
                        last_attr = Some(ch.attribute);
                    }
                    rtf.push_str(&rtf_escape(unicode_ch));
                }
                rtf.push_str("\\par\n");
            }

            // Last line (partial)
            for x in 0..=end.x {
                let ch = buffer.get_char(Position::new(x, end.y));
                let unicode_ch = buffer.buffer_type.convert_to_unicode(ch.ch);
                let fg_rgb = palette.get_rgb(ch.attribute.get_foreground());
                let fg_idx = *color_map.get(&fg_rgb).unwrap();

                let bg_raw = ch.attribute.get_background();
                let bg_idx = if bg_raw & (1 << 31) != 0 {
                    None
                } else {
                    let bg_rgb = palette.get_rgb(bg_raw);
                    color_map.get(&bg_rgb).copied()
                };

                if last_attr.map(|la| la != ch.attribute).unwrap_or(true) || last_fg_idx != Some(fg_idx) || last_bg_idx != bg_idx {
                    emit_attr(&mut rtf, last_attr, ch.attribute, fg_idx, bg_idx, &mut last_fg_idx, &mut last_bg_idx);
                    last_attr = Some(ch.attribute);
                }
                rtf.push_str(&rtf_escape(unicode_ch));
            }
            rtf.push_str("\\par\n");
        }
    }

    // Close RTF document
    rtf.push('}');

    Some(rtf)
}
