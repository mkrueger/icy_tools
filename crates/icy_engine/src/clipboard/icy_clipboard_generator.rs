use crate::{AttributedChar, BufferType, Layer, Position, Role, Selection, SelectionMask, TextAttribute, TextBuffer, TextPane};
pub const ICY_CLIPBOARD_TYPE: &str = "com.icy-tools.clipboard";

pub fn clipboard_data(buffer: &TextBuffer, layer: usize, selection_mask: &SelectionMask, selection: &Option<Selection>) -> Option<Vec<u8>> {
    let selection_rect = selection_mask.selected_rectangle(selection);

    let mut data = Vec::new();
    data.push(0);
    data.extend(i32::to_le_bytes(selection_rect.start.x));
    data.extend(i32::to_le_bytes(selection_rect.start.y));

    data.extend(u32::to_le_bytes(selection_rect.size().width as u32));
    data.extend(u32::to_le_bytes(selection_rect.size().height as u32));
    let layer = &buffer.layers[layer];
    for y in selection_rect.y_range() {
        for x in selection_rect.x_range() {
            let pos = Position::new(x, y);
            let ch = if selection_mask.selected_in_selection((x, y), selection) {
                layer.char_at(pos - layer.offset())
            } else {
                AttributedChar::invisible()
            };
            let c = buffer.buffer_type.convert_to_unicode(ch.ch);
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
pub fn from_clipboard_data(buffer_type: BufferType, data: &[u8]) -> Option<Layer> {
    if data[0] != 0 {
        return None;
    }
    let x = i32::from_le_bytes(data[1..5].try_into().unwrap());
    let y = i32::from_le_bytes(data[5..9].try_into().unwrap());
    let width = u32::from_le_bytes(data[9..13].try_into().unwrap()) as usize;
    let height = u32::from_le_bytes(data[13..17].try_into().unwrap()) as usize;
    let mut data = &data[17..];

    let mut layer = Layer::new(format!("New Layer"), (width, height));
    layer.properties.has_alpha_channel = true;
    layer.role = Role::PastePreview;
    layer.set_offset((x, y));
    for y in 0..height {
        for x in 0..width {
            let ch = unsafe { char::from_u32_unchecked(u32::from_le_bytes(data[0..4].try_into().unwrap())) };
            let ch = buffer_type.convert_from_unicode(ch);
            let attr_ch = AttributedChar {
                ch,
                attribute: TextAttribute {
                    attr: u16::from_le_bytes(data[4..6].try_into().unwrap()),
                    font_page: u16::from_le_bytes(data[6..8].try_into().unwrap()) as u8,
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
