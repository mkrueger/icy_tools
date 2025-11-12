
#[cfg(test)]
mod tests {
    use i18n_embed_fl::fl;

    use crate::{AttributedChar, Layer, Line, TextAttribute, TextPane};

    #[test]
    fn test_get_char() {
        let mut layer = Layer::new(fl!(crate::LANGUAGE_LOADER, "layer-background-name"), (20, 20));
        layer.properties.has_alpha_channel = false;
        let mut line = Line::new();
        line.set_char(10, AttributedChar::new('a', TextAttribute::default()));

        layer.insert_line(0, line);

        assert_eq!(AttributedChar::invisible(), layer.get_char((-1, -1).into()));
        assert_eq!(AttributedChar::invisible(), layer.get_char((1000, 1000).into()));
        assert_eq!('a', layer.get_char((10, 0).into()).ch);
        assert_eq!(AttributedChar::invisible(), layer.get_char((9, 0).into()));
        assert_eq!(AttributedChar::invisible(), layer.get_char((11, 0).into()));
    }

    #[test]
    fn test_get_char_intransparent() {
        let mut layer = Layer::new(fl!(crate::LANGUAGE_LOADER, "layer-background-name"), (20, 20));
        layer.properties.has_alpha_channel = true;

        let mut line = Line::new();
        line.set_char(10, AttributedChar::new('a', TextAttribute::default()));

        layer.insert_line(0, line);

        assert_eq!(AttributedChar::invisible(), layer.get_char((-1, -1).into()));
        assert_eq!(AttributedChar::invisible(), layer.get_char((1000, 1000).into()));
        assert_eq!('a', layer.get_char((10, 0).into()).ch);
        assert_eq!(AttributedChar::invisible(), layer.get_char((9, 0).into()));
        assert_eq!(AttributedChar::invisible(), layer.get_char((11, 0).into()));
    }

    #[test]
    fn test_insert_line() {
        let mut layer = Layer::new(fl!(crate::LANGUAGE_LOADER, "layer-background-name"), (80, 0));
        let mut line = Line::new();
        line.chars.push(AttributedChar::new('a', TextAttribute::default()));
        layer.insert_line(10, line);

        assert_eq!('a', layer.lines[10].chars[0].ch);
        assert_eq!(11, layer.lines.len());

        layer.insert_line(11, Line::new());
        assert_eq!(12, layer.lines.len());
    }
    /*
    #[test]
    fn test_clipboard() {
        let mut state = EditState::default();

        for i in 0..25 {
            for x in 0..80 {
                state
                    .set_char(
                        (x, i),
                        AttributedChar {
                            ch: unsafe { char::from_u32_unchecked((b'0' + (x % 10)) as u32) },
                            attribute: TextAttribute::default(),
                        },
                    )
                    .unwrap();
            }
        }

        state.set_selection(Rectangle::from_min_size((5, 6), (7, 8))).unwrap();
        let data = state.get_clipboard_data().unwrap();

        let layer = state.from_clipboard_data(&data).unwrap();

        assert_eq!(layer.get_width(), 7);
        assert_eq!(layer.get_height(), 8);

        assert_eq!(layer.properties.offset.x, 5);
        assert_eq!(layer.properties.offset.y, 6);

        assert!(layer.get_char((0, 0).into()).ch == '5');
    }*/
}
