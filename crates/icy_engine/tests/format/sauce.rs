use bstr::BString;
use icy_engine::{formats::FileFormat, AttributedChar, IceMode, SauceMetaData, SaveOptions, TextAttribute, TextBuffer};

fn sauce_meta() -> SauceMetaData {
    SauceMetaData {
        title: BString::from("Title"),
        author: BString::from("Author"),
        group: BString::from("Group"),
        comments: vec![BString::from("A comment line")],
    }
}

fn test_buffer() -> TextBuffer {
    let mut buf = TextBuffer::new((80, 25));
    // iCE Draw only supports iCE colors.
    buf.ice_mode = IceMode::Ice;
    for (y, line) in ["Hello", "SAUCE", "World"].iter().enumerate() {
        for (x, ch) in line.chars().enumerate() {
            buf.layers[0].set_char((x as i32, y as i32), AttributedChar::new(ch, TextAttribute::default()));
        }
    }
    buf
}

/// The SAUCE `FileSize` must be the length of the content before the EOF marker,
/// excluding the EOF marker, the comment block and the record itself.
fn check_file_size(format: FileFormat) {
    let buf = test_buffer();
    let mut opt = SaveOptions::new();
    opt.sauce = Some(sauce_meta());
    let bytes = format.to_bytes(&buf, &opt).unwrap();

    let sauce = icy_sauce::SauceRecord::from_bytes(&bytes).unwrap().expect("SAUCE record expected");
    let content_len = bytes.len() - 1 - sauce.record_len();
    assert_eq!(content_len as u32, sauce.file_size(), "{format:?}: FileSize must equal the content length");
    assert_eq!(0x1A, bytes[content_len], "{format:?}: EOF marker expected after the content");

    // Saving without SAUCE produces exactly the content.
    let plain = format.to_bytes(&buf, &SaveOptions::new()).unwrap();
    assert_eq!(plain.as_slice(), &bytes[..content_len], "{format:?}: content must not change");
}

#[test]
fn test_sauce_file_size_ansi() {
    check_file_size(FileFormat::Ansi);
}

#[test]
fn test_sauce_file_size_character_formats() {
    for format in [FileFormat::Ascii, FileFormat::Avatar, FileFormat::PCBoard] {
        check_file_size(format);
    }
}

#[test]
fn test_sauce_file_size_binary_formats() {
    for format in [
        FileFormat::Bin,
        FileFormat::XBin,
        FileFormat::TundraDraw,
        FileFormat::IceDraw,
        FileFormat::Artworx,
    ] {
        check_file_size(format);
    }
}
