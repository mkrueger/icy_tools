use icy_engine::{AttributeColor, AttributedChar, Layer, Line, TextAttribute, TextPane};

#[test]
fn test_get_char() {
    let mut layer = Layer::new(String::new(), (20, 20));
    layer.properties.has_alpha_channel = false;
    let mut line = Line::new();
    line.set_char(10, AttributedChar::new('a', TextAttribute::default()));

    layer.insert_line(0, line);

    assert_eq!(AttributedChar::invisible(), layer.char_at((-1, -1).into()));
    assert_eq!(AttributedChar::invisible(), layer.char_at((1000, 1000).into()));
    assert_eq!('a', layer.char_at((10, 0).into()).ch);
    assert_eq!(AttributedChar::invisible(), layer.char_at((9, 0).into()));
    assert_eq!(AttributedChar::invisible(), layer.char_at((11, 0).into()));
}

#[test]
fn test_get_char_intransparent() {
    let mut layer = Layer::new(String::new(), (20, 20));
    layer.properties.has_alpha_channel = true;

    let mut line = Line::new();
    line.set_char(10, AttributedChar::new('a', TextAttribute::default()));

    layer.insert_line(0, line);

    assert_eq!(AttributedChar::invisible(), layer.char_at((-1, -1).into()));
    assert_eq!(AttributedChar::invisible(), layer.char_at((1000, 1000).into()));
    assert_eq!('a', layer.char_at((10, 0).into()).ch);
    assert_eq!(AttributedChar::invisible(), layer.char_at((9, 0).into()));
    assert_eq!(AttributedChar::invisible(), layer.char_at((11, 0).into()));
}

#[test]
fn test_insert_line() {
    let mut layer = Layer::new(String::new(), (80, 0));
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
    let data = state.clipboard_data().unwrap();

    let layer = state.from_clipboard_data(&data).unwrap();

    assert_eq!(layer.width(), 7);
    assert_eq!(layer.height(), 8);

    assert_eq!(layer.properties.offset.x, 5);
    assert_eq!(layer.properties.offset.y, 6);

    assert!(layer.char_at((0, 0).into()).ch == '5');
}*/

#[test]
fn test_line_count_counts_visible_blank_rows() {
    let mut layer = Layer::new(String::new(), (10, 8));
    layer.set_char((0, 0), AttributedChar::new('a', TextAttribute::default()));
    assert_eq!(1, layer.line_count());

    // Spaces on black are empty.
    layer.set_char((0, 1), AttributedChar::new(' ', TextAttribute::default()));
    assert_eq!(1, layer.line_count());

    // Spaces on a coloured background are a visible bar.
    layer.set_char((3, 2), AttributedChar::new(' ', TextAttribute::new(7, 1)));
    assert_eq!(3, layer.line_count());

    // Blink selects the bright background in iCE mode.
    let mut blink = TextAttribute::default();
    blink.set_is_blinking(true);
    layer.set_char((0, 3), AttributedChar::new(' ', blink));
    assert_eq!(4, layer.line_count());

    // A glyph drawn in the black background colour is invisible ...
    layer.set_char((0, 4), AttributedChar::new('x', TextAttribute::new(0, 0)));
    assert_eq!(4, layer.line_count());

    // ... but a glyph drawn in a non-black background colour is not.
    layer.set_char((0, 5), AttributedChar::new('\u{DB}', TextAttribute::new(1, 1)));
    assert_eq!(6, layer.line_count());

    // RGB colours are compared as colours, not as palette index 0.
    let rgb = TextAttribute::from_colors(AttributeColor::Rgb(255, 0, 0), AttributeColor::Palette(0));
    layer.set_char((0, 6), AttributedChar::new('x', rgb));
    assert_eq!(7, layer.line_count());
}
