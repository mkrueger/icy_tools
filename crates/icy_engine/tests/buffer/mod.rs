use bstr::BString;
use icy_engine::{
    AttributedChar, Layer, Line, SaveOptions, Size, TextAttribute, TextBuffer, TextPane,
    formats::{FileFormat, LoadData},
};
use icy_sauce::{AspectRatio, Capabilities, CharacterCapabilities, CharacterFormat, LetterSpacing, SauceRecord, SauceRecordBuilder};

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
    for x in 0..buf.get_width() {
        buf.layers[0].set_char((x, 0), AttributedChar::new('1', TextAttribute::default()));
        buf.layers[0].set_char((x, 1), AttributedChar::new('2', TextAttribute::default()));
        buf.layers[0].set_char((x, 2), AttributedChar::new('3', TextAttribute::default()));
    }

    // Create a SAUCE record with the correct width
    let char_caps = CharacterCapabilities::with_font(
        CharacterFormat::Ansi,
        10, // columns
        3,  // lines
        false,
        LetterSpacing::EightPixel,
        AspectRatio::Square,
        Some(BString::from("IBM VGA")),
    )
    .unwrap();

    let sauce = SauceRecordBuilder::default()
        .title(BString::from("Test"))
        .unwrap()
        .capabilities(Capabilities::Character(char_caps))
        .unwrap()
        .build();

    let mut opt = SaveOptions::new();
    opt.save_sauce = Some(sauce);
    let ansi_bytes = FileFormat::Ansi.to_bytes(&buf, &opt).unwrap();

    // Extract SAUCE from the bytes
    let sauce_opt = SauceRecord::from_bytes(&ansi_bytes).ok().flatten();
    let load_data = LoadData::new(sauce_opt, None, None);
    let loaded_buf = FileFormat::Ansi
        .from_bytes(&std::path::PathBuf::from("test.ans"), &ansi_bytes, Some(load_data))
        .unwrap()
        .buffer;
    assert_eq!(10, loaded_buf.get_width());
    assert_eq!(10, loaded_buf.layers[0].get_width());
}

#[test]
fn test_layer_offset() {
    let mut buf: TextBuffer = TextBuffer::default();

    let mut new_layer = Layer::new("1", Size::new(10, 10));
    new_layer.properties.has_alpha_channel = true;
    new_layer.set_offset((2, 2));
    new_layer.set_char((5, 5), AttributedChar::new('a', TextAttribute::default()));
    buf.layers.push(new_layer);

    assert_eq!('a', buf.get_char((7, 7).into()).ch);
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

    assert_eq!('a', buf.get_char((3, 3).into()).ch);
    assert_eq!('b', buf.get_char((7, 7).into()).ch);
}
