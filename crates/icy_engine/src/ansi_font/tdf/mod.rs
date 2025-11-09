use crate::{AttributedChar, BufferType, Size, TextAttribute, TextBuffer, TextPane, editor::EditState};
use i18n_embed_fl::fl;

pub mod font;

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum FontType {
    Outline,
    Block,
    #[default]
    Color,
    Figlet,
}

#[derive(Clone)]
pub struct FontGlyph {
    pub size: Size,
    pub data: Vec<u8>,
}

impl FontGlyph {
    fn render(&self, editor: &mut EditState, font_type: FontType, edit_mode: bool) -> Size {
        let caret_pos = editor.get_caret().position();
        let outline_style = editor.get_outline_style();
        let color: TextAttribute = editor.get_caret().attribute;
        let _undo = editor.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-char_font_glyph"));

        let mut cur = caret_pos;
        let mut char_offset = 0;
        let mut leading_space = true;

        // Track the actual rendered dimensions
        let mut max_width = 0i32;

        while char_offset < self.data.len() {
            let ch = self.data[char_offset];
            char_offset += 1;
            let is_eol_marker = ch == b'&';
            if is_eol_marker {
                if !edit_mode {
                    continue;
                }
            }

            if ch == 13 {
                // Update max width before moving to next line
                max_width = max_width.max(cur.x - caret_pos.x);
                cur.x = caret_pos.x;
                cur.y += 1;
                leading_space = true;
            } else {
                let mut attributed_char = if is_eol_marker {
                    // non edit mode handled above
                    AttributedChar::new('&', TextAttribute::default())
                } else {
                    match font_type {
                        FontType::Outline => {
                            if ch == b'@' && !edit_mode || ch == b' ' && leading_space {
                                cur.x += 1;
                                continue;
                            }
                            leading_space = false;
                            if ch == b'O' && !edit_mode {
                                AttributedChar::new(' ', color)
                            } else {
                                AttributedChar::new(font::TheDrawFont::transform_outline(outline_style, ch) as char, color)
                            }
                        }
                        FontType::Block => {
                            if ch == b' ' {
                                cur.x += 1;
                                continue;
                            }

                            if ch == 0xFF && !edit_mode {
                                AttributedChar::new(' ', color)
                            } else {
                                AttributedChar::new(ch as char, color)
                            }
                        }
                        FontType::Color => {
                            let ch_attr = if is_eol_marker {
                                TextAttribute::default()
                            } else {
                                if let Some(next_byte) = self.data.get(char_offset) {
                                    char_offset += 1;
                                    TextAttribute::from_u8(*next_byte, crate::IceMode::Ice)
                                } else {
                                    TextAttribute::default()
                                }
                            };

                            if ch == 0xFF && !edit_mode {
                                AttributedChar::new(' ', ch_attr)
                            } else {
                                let result = AttributedChar::new(ch as char, ch_attr);
                                if result.is_transparent() && result.attribute.get_background() == 0 {
                                    cur.x += 1;
                                    continue;
                                }
                                result
                            }
                        }
                        _ => {
                            panic!("Unsupported font type");
                        }
                    }
                };
                if editor.get_buffer().buffer_type == crate::BufferType::Unicode {
                    attributed_char.ch = BufferType::CP437.convert_to_unicode(attributed_char.ch);
                }
                editor.set_char(cur, attributed_char).unwrap();

                cur.x += 1;
                // Update max width after placing character
                max_width = max_width.max(cur.x - caret_pos.x);
            }
        }

        // Calculate final height (number of lines rendered)
        let max_height = cur.y - caret_pos.y + 1;

        // Update final max width in case last line was longest
        max_width = max_width.max(cur.x - caret_pos.x);

        Size::new(max_width, max_height)
    }

    pub fn from_buffer(buffer: &TextBuffer, font_type: FontType) -> Self {
        let mut data = Vec::new();
        let mut max_width = 0;

        // Helper function to get the actual line length (excluding trailing spaces)
        fn get_actual_line_length(buffer: &TextBuffer, y: i32) -> i32 {
            let mut len = 0;
            for x in 0..buffer.get_width() {
                let ch = buffer.get_char((x, y).into());
                if !ch.is_transparent() || ch.attribute.get_background() != TextAttribute::TRANSPARENT_COLOR && ch.attribute.get_background() > 0 {
                    len = x + 1;
                }
            }
            len
        }

        // Determine the actual bounds of the content
        let mut actual_height = 0;
        for y in 0..buffer.get_height() {
            let line_length = get_actual_line_length(buffer, y);
            if line_length > 0 {
                actual_height = y + 1;
                // Check for & terminator to get actual width
                let mut actual_line_length = 0;
                for x in 0..line_length {
                    let ch = buffer.get_char((x, y).into());
                    actual_line_length = x + 1;
                    if ch.ch == '&' {
                        break; // Stop at &
                    }
                }
                max_width = max_width.max(actual_line_length);
            }
        }

        if actual_height == 0 {
            return FontGlyph {
                size: Size::new(0, 0),
                data: Vec::new(),
            };
        }

        match font_type {
            FontType::Outline => {
                const VALID_OUTLINE_CHARS: &str = " @OABCDEFGHIJKLMNPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!\"#$%'()*+,-./:;<=>?[\\]^_`{|}~";

                for y in 0..actual_height {
                    if y > 0 {
                        data.push(13); // CR for new line
                    }

                    let line_length = get_actual_line_length(buffer, y);
                    let mut line_data = Vec::new();

                    for x in 0..line_length {
                        let ch = buffer.get_char((x, y).into());

                        // Check for end-of-line marker
                        if ch.ch == '&' {
                            line_data.push(b'&');
                            break; // Stop processing this line
                        }

                        // Only include valid outline characters
                        if VALID_OUTLINE_CHARS.contains(ch.ch) {
                            line_data.push(ch.ch as u8);
                        }
                    }

                    data.extend(line_data);
                }
            }

            FontType::Block => {
                for y in 0..actual_height {
                    if y > 0 {
                        data.push(13); // CR for new line
                    }

                    let line_length = get_actual_line_length(buffer, y);

                    for x in 0..line_length {
                        let ch = buffer.get_char((x, y).into());

                        // Include character
                        data.push(ch.ch as u8);

                        // Check for end-of-line marker
                        if ch.ch == '&' {
                            break; // Stop processing this line
                        }
                    }
                }
            }

            FontType::Color => {
                for y in 0..actual_height {
                    if y > 0 {
                        data.push(13); // CR for new line (no attribute byte after CR)
                    }

                    let line_length = get_actual_line_length(buffer, y);

                    for x in 0..line_length {
                        let ch = buffer.get_char((x, y).into());

                        // Check for end-of-line marker
                        if ch.ch == '&' {
                            // & doesn't have an attribute byte in color mode
                            data.push(b'&');
                            break; // Stop processing this line
                        }

                        // Add character and its attribute
                        data.push(ch.ch as u8);
                        data.push(ch.attribute.as_u8(crate::IceMode::Ice));
                    }
                }
            }

            FontType::Figlet => {
                // Figlet fonts are text-based and have different structure
                // For now, just store the raw characters without attributes
                for y in 0..actual_height {
                    if y > 0 {
                        data.push(b'\n'); // Use newline for Figlet
                    }

                    let line_length = get_actual_line_length(buffer, y);
                    for x in 0..line_length {
                        let ch = buffer.get_char((x, y).into());
                        data.push(ch.ch as u8);
                    }
                }
            }
        }

        FontGlyph {
            size: Size::new(max_width, actual_height),
            data,
        }
    }
}
