use crate::{AttributedChar, Position, Size, TextAttribute, editor::EditState};
use i18n_embed_fl::fl;

pub mod font;

#[derive(Copy, Clone, Debug)]
pub enum FontType {
    Outline,
    Block,
    Color,
    Figlet,
}

#[derive(Clone)]
pub struct FontGlyph {
    pub size: Size,
    pub data: Vec<u8>,
}

impl FontGlyph {
    fn render(&self, editor: &mut EditState, font_type: FontType) -> Position {
        let caret_pos = editor.get_caret().get_position();
        let outline_style = editor.get_outline_style();
        let color: TextAttribute = editor.get_caret().attribute;
        let _undo = editor.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-char_font_glyph"));

        let mut cur = caret_pos;
        let mut char_offset = 0;
        let mut leading_space = true;
        while char_offset < self.data.len() {
            let ch = self.data[char_offset];
            char_offset += 1;

            if ch == 13 {
                cur.x = caret_pos.x;
                cur.y += 1;
                leading_space = true;
            } else {
                let attributed_char = match font_type {
                    FontType::Outline => {
                        if ch == b'@' || ch == b' ' && leading_space {
                            cur.x += 1;
                            continue;
                        }
                        leading_space = false;
                        if ch == b'O' {
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
                        if ch == 0xF7 {
                            AttributedChar::new(' ', color)
                        } else {
                            AttributedChar::new(ch as char, color)
                        }
                    }
                    FontType::Color => {
                        let ch = ch as char;
                        let ch_attr = TextAttribute::from_u8(self.data[char_offset], crate::IceMode::Ice); // tdf fonts don't support ice mode by default
                        char_offset += 1;
                        let ch = AttributedChar::new(ch, ch_attr);
                        if ch.is_transparent() {
                            cur.x += 1;
                            continue;
                        }
                        ch
                    }
                    _ => {
                        panic!("Unsupported font type");
                    }
                };
                editor.set_char(cur, attributed_char).unwrap();
                cur.x += 1;
            }
        }
        Position::new(cur.x, caret_pos.y)
    }
}
