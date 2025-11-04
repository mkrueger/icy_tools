use crate::{AttributedChar, Size, TextAttribute, UnicodeConverter, editor::EditState};
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
        let caret_pos = editor.get_caret().get_position();
        let outline_style = editor.get_outline_style();
        let color: TextAttribute = editor.get_caret().attribute;
        let _undo = editor.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-char_font_glyph"));

        let mut cur = caret_pos;
        let mut char_offset = 0;
        let mut leading_space = true;
        let converter = crate::ascii::CP437Converter::default();

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
                    attributed_char.ch = converter.convert_to_unicode(attributed_char);
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
}
