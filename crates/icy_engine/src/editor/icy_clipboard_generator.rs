use i18n_embed_fl::fl;

use crate::{AttributedChar, BufferType, Layer, Position, Role, TextAttribute, TextPane, editor::EditState};

pub const ICY_CLIPBOARD_TYPE: &str = "com.icy-tools.clipboard";

impl EditState {
    pub fn get_clipboard_data(&self) -> Option<Vec<u8>> {
        if !self.is_something_selected() {
            return None;
        };
        let Some(layer) = self.get_cur_display_layer() else {
            return None;
        };

        let selection = self.get_selected_rectangle();

        let mut data = Vec::new();
        data.push(0);
        data.extend(i32::to_le_bytes(selection.start.x));
        data.extend(i32::to_le_bytes(selection.start.y));

        data.extend(u32::to_le_bytes(selection.get_size().width as u32));
        data.extend(u32::to_le_bytes(selection.get_size().height as u32));
        let need_convert_to_unicode = self.buffer.buffer_type != crate::BufferType::Unicode;
        for y in selection.y_range() {
            for x in selection.x_range() {
                let pos = Position::new(x, y);
                let ch = if self.get_is_selected((x, y)) {
                    layer.get_char(pos - layer.get_offset())
                } else {
                    AttributedChar::invisible()
                };
                let c = if need_convert_to_unicode {
                    self.unicode_converter.convert_to_unicode(ch)
                } else {
                    ch.ch
                };
                data.extend(u32::to_le_bytes(c as u32));
                data.extend(u16::to_le_bytes(ch.attribute.attr));
                data.extend(u16::to_le_bytes(ch.attribute.font_page as u16));
                data.extend(u32::to_le_bytes(ch.attribute.background_color));
                data.extend(u32::to_le_bytes(ch.attribute.foreground_color));
            }
        }
        Some(data)
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    pub fn from_clipboard_data(&self, data: &[u8]) -> Option<Layer> {
        if data[0] != 0 {
            return None;
        }
        let x = i32::from_le_bytes(data[1..5].try_into().unwrap());
        let y = i32::from_le_bytes(data[5..9].try_into().unwrap());
        let width = u32::from_le_bytes(data[9..13].try_into().unwrap()) as usize;
        let height = u32::from_le_bytes(data[13..17].try_into().unwrap()) as usize;
        let mut data = &data[17..];

        let mut layer = Layer::new(fl!(crate::LANGUAGE_LOADER, "layer-pasted-name"), (width, height));
        layer.properties.has_alpha_channel = true;
        layer.role = Role::PastePreview;
        layer.set_offset((x, y));
        let need_convert_to_unicode = self.buffer.buffer_type != BufferType::Unicode;
        for y in 0..height {
            for x in 0..width {
                let mut ch = unsafe { char::from_u32_unchecked(u32::from_le_bytes(data[0..4].try_into().unwrap())) };
                if need_convert_to_unicode {
                    let font_page = self.caret.get_font_page();
                    ch = self.unicode_converter.convert_from_unicode(ch, font_page);
                }
                let attr_ch = AttributedChar {
                    ch,
                    attribute: TextAttribute {
                        attr: u16::from_le_bytes(data[4..6].try_into().unwrap()),
                        font_page: u16::from_le_bytes(data[6..8].try_into().unwrap()) as usize,
                        background_color: u32::from_le_bytes(data[8..12].try_into().unwrap()),
                        foreground_color: u32::from_le_bytes(data[12..16].try_into().unwrap()),
                    },
                };
                layer.set_char((x as i32, y as i32), attr_ch);
                data = &data[16..];
            }
        }
        Some(layer)
    }
}
