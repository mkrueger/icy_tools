use crate::{BufferType, Position, Selection, Shape, TextPane};

pub fn get_text(buffer: &dyn TextPane, buffer_type: BufferType, selection: &Selection) -> Option<String> {
    let mut res = String::new();
    match selection.shape {
        Shape::Rectangle => {
            let start: crate::Position = selection.min();
            let end = selection.max();
            for y in start.y..=end.y {
                for x in start.x..=end.x {
                    let ch = buffer.get_char((x, y).into());
                    res.push(buffer_type.convert_to_unicode(ch.ch));
                }
                res.push('\n');
            }
        }
        Shape::Lines => {
            let (start, end) = if selection.anchor < selection.lead {
                (selection.anchor, selection.lead)
            } else {
                (selection.lead, selection.anchor)
            };
            if start.y == end.y {
                for x in start.x..=end.x {
                    let ch = buffer.get_char(Position::new(x, start.y));
                    res.push(buffer_type.convert_to_unicode(ch.ch));
                }
            } else {
                for x in start.x..(buffer.get_line_length(start.y)) {
                    let ch = buffer.get_char(Position::new(x, start.y));
                    res.push(buffer_type.convert_to_unicode(ch.ch));
                }
                res.push('\n');
                for y in start.y + 1..end.y {
                    for x in 0..(buffer.get_line_length(y)) {
                        let ch = buffer.get_char(Position::new(x, y));
                        res.push(buffer_type.convert_to_unicode(ch.ch));
                    }
                    res.push('\n');
                }
                for x in 0..=end.x {
                    let ch = buffer.get_char(Position::new(x, end.y));
                    res.push(buffer_type.convert_to_unicode(ch.ch));
                }
            }
            return Some(res);
        }
    }
    Some(res)
}
