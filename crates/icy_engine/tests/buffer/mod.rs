use bstr::BString;
use icy_engine::{
    AttributedChar, Layer, Line, SaveOptions, Size, TextAttribute, TextBuffer, TextPane,
    formats::{FileFormat, LoadData},
};
use icy_sauce::MetaData as SauceMetaData;

mod layer;

// FIXME: buffer.rs tests need to be updated to match current API
// The tests reference deprecated APIs like caret.pos, caret.up(), get_char().unwrap()
// mod buffer;

#[test]
fn test_insert_char() {
    let mut line = Line::new();
    line.insert_char(100, AttributedChar::default());
    assert_eq!(101, line.chars.len());
    line.insert_char(1, AttributedChar::default());
    assert_eq!(102, line.chars.len());
}

#[test]
fn test_set_char() {
    let mut line = Line::new();
    line.set_char(100, AttributedChar::default());
    assert_eq!(101, line.chars.len());
    line.set_char(100, AttributedChar::default());
    assert_eq!(101, line.chars.len());
}

#[test]
fn test_respect_sauce_width() {
    let mut buf = TextBuffer::default();
    buf.set_width(10);
    for x in 0..buf.width() {
        buf.layers[0].set_char((x, 0), AttributedChar::new('1', TextAttribute::default()));
        buf.layers[0].set_char((x, 1), AttributedChar::new('2', TextAttribute::default()));
        buf.layers[0].set_char((x, 2), AttributedChar::new('3', TextAttribute::default()));
    }

    // Create SAUCE metadata with title
    let sauce = SauceMetaData {
        title: BString::from("Test"),
        author: BString::default(),
        group: BString::default(),
        comments: Vec::new(),
    };

    let mut opt = SaveOptions::new();
    opt.sauce = Some(sauce);
    let ansi_bytes = FileFormat::Ansi.to_bytes(&buf, &opt).unwrap();

    // Extract SAUCE from the bytes - SAUCE is now returned in LoadedDocument, not passed in
    let load_data = LoadData::new(None, None);
    let loaded_buf = FileFormat::Ansi.from_bytes(&ansi_bytes, Some(load_data)).unwrap().screen.buffer;
    assert_eq!(10, loaded_buf.width());
    assert_eq!(10, loaded_buf.layers[0].width());
}

#[test]
fn test_layer_offset() {
    let mut buf: TextBuffer = TextBuffer::default();

    let mut new_layer = Layer::new("1", Size::new(10, 10));
    new_layer.properties.has_alpha_channel = true;
    new_layer.set_offset((2, 2));
    new_layer.set_char((5, 5), AttributedChar::new('a', TextAttribute::default()));
    buf.layers.push(new_layer);

    assert_eq!('a', buf.char_at((7, 7).into()).ch);
}

#[test]
fn test_layer_negative_offset() {
    let mut buf: TextBuffer = TextBuffer::default();

    let mut new_layer = Layer::new("1", Size::new(10, 10));
    new_layer.properties.has_alpha_channel = true;
    new_layer.set_offset((-2, -2));
    new_layer.set_char((5, 5), AttributedChar::new('a', TextAttribute::default()));
    buf.layers.push(new_layer);

    let mut new_layer = Layer::new("2", Size::new(10, 10));
    new_layer.properties.has_alpha_channel = true;
    new_layer.set_offset((2, 2));
    new_layer.set_char((5, 5), AttributedChar::new('b', TextAttribute::default()));
    buf.layers.push(new_layer);

    assert_eq!('a', buf.char_at((3, 3).into()).ch);
    assert_eq!('b', buf.char_at((7, 7).into()).ch);
}
