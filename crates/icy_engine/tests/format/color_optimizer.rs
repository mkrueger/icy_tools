use crate::{AttributedChar, ColorOptimizer, TextAttribute, TextBuffer, TextPane};

#[test]
pub fn test_foreground_optimization() {
    let mut buffer = TextBuffer::new((5, 1));
    let attr = TextAttribute::new(14, 0);
    buffer.layers[0].set_char((0, 0), AttributedChar::new('A', attr));

    let save_options = crate::SaveOptions::default();
    let opt = ColorOptimizer::new(&buffer, &save_options);

    let opt_buf = opt.optimize(&buffer);
    for x in 0..opt_buf.width() {
        assert_eq!(opt_buf.layers[0].char_at((x, 0).into()).attribute.foreground(), 14, "x={x}");
    }
}

#[test]
pub fn test_background_optimization() {
    let mut buffer = TextBuffer::new((5, 1));
    for x in 0..buffer.width() {
        let attr = TextAttribute::new(14, x as u32);
        buffer.layers[0].set_char((x, 0), AttributedChar::new(219 as char, attr));
    }
    let save_options = crate::SaveOptions::default();
    let opt = ColorOptimizer::new(&buffer, &save_options);

    let opt_buf = opt.optimize(&buffer);
    for x in 0..opt_buf.width() {
        assert_eq!(opt_buf.layers[0].char_at((x, 0).into()).attribute.background(), 0, "x={x}");
    }
}

#[test]
pub fn test_ws_normalization() {
    let mut buffer = TextBuffer::new((5, 1));
    for x in 0..buffer.width() {
        buffer.layers[0].set_char((x, 0), AttributedChar::new(0 as char, TextAttribute::default()));
    }
    buffer.layers[0].set_char((3, 0), AttributedChar::new(255 as char, TextAttribute::default()));

    let mut save_options = crate::SaveOptions::default();
    save_options.normalize_whitespaces = true;
    let opt = ColorOptimizer::new(&buffer, &save_options);

    let opt_buf = opt.optimize(&buffer);
    for x in 0..opt_buf.width() {
        assert_eq!(opt_buf.layers[0].char_at((x, 0).into()).ch, ' ', "x={x}");
    }
}
