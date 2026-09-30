//! Every DOS attribute must survive saving and loading in each format that can store it,
//! both in the first cell (where writer and loader start from their default attribute)
//! and after a cell with another attribute.

use icy_engine::{AttributeColor, AttributedChar, FileFormat, IceMode, Palette, Position, SaveOptions, TextAttribute, TextBuffer, TextPane};

/// Foreground and background as RGB plus blinking, so palette and RGB loaders compare equal.
fn resolved(attribute: TextAttribute, palette: &Palette) -> ((u8, u8, u8), (u8, u8, u8), bool) {
    let rgb = |color: AttributeColor| match color {
        AttributeColor::Palette(index) | AttributeColor::ExtendedPalette(index) => palette.rgb(index as u32),
        AttributeColor::Rgb(r, g, b) => (r, g, b),
        AttributeColor::Transparent => (0, 0, 0),
    };
    (rgb(attribute.foreground_color()), rgb(attribute.background_color()), attribute.is_blinking())
}

/// Saves `byte` in the first cell and after a differently colored cell, and returns the
/// positions whose attribute did not survive.
fn failures(format: FileFormat, ice_mode: IceMode, byte: u8) -> Vec<&'static str> {
    let attribute = TextAttribute::from_u8(byte, ice_mode);
    let mut buffer = TextBuffer::new((80, 25));
    buffer.ice_mode = ice_mode;
    buffer.layers[0].set_char((0, 0), AttributedChar::new('A', attribute));
    buffer.layers[0].set_char((2, 0), AttributedChar::new('B', TextAttribute::from_u8(0x4E, ice_mode)));
    buffer.layers[0].set_char((3, 0), AttributedChar::new('C', attribute));

    let mut options = SaveOptions::default();
    options.preprocess.optimize_colors = false;
    let bytes = format.to_bytes(&buffer, &options).unwrap();
    let loaded = format.from_bytes(&bytes, None).unwrap().screen.buffer;

    let expected = resolved(attribute, &buffer.palette);
    [("first cell", Position::new(0, 0)), ("after another color", Position::new(3, 0))]
        .into_iter()
        .filter(|(_, position)| resolved(loaded.char_at(*position).attribute, &loaded.palette) != expected)
        .map(|(name, _)| name)
        .collect()
}

/// `full` formats store bit 7 (blink or iCE background) in this mode; the others only the
/// lower seven bits, because without SAUCE bit 7 cannot be told apart when loading.
fn check(format: FileFormat, ice_mode: IceMode, full: bool) {
    let last = if full { 0xFF } else { 0x7F };
    let broken: Vec<String> = (0..=last)
        .filter_map(|byte| {
            let failed = failures(format, ice_mode, byte);
            (!failed.is_empty()).then(|| format!("{byte:02X} ({})", failed.join(", ")))
        })
        .collect();
    assert!(broken.is_empty(), "{format:?} {ice_mode:?} loses attributes: {}", broken.join(" "));
}

#[test]
fn ansi_keeps_every_attribute() {
    check(FileFormat::Ansi, IceMode::Blink, true);
    check(FileFormat::Ansi, IceMode::Ice, true);
}

#[test]
fn avatar_keeps_every_attribute() {
    check(FileFormat::Avatar, IceMode::Blink, true);
    check(FileFormat::Avatar, IceMode::Ice, false);
}

#[test]
fn pcboard_keeps_every_attribute() {
    check(FileFormat::PCBoard, IceMode::Blink, false);
    check(FileFormat::PCBoard, IceMode::Ice, true);
}

#[test]
fn ctrla_keeps_every_attribute() {
    check(FileFormat::CtrlA, IceMode::Blink, true);
    check(FileFormat::CtrlA, IceMode::Ice, true);
}

#[test]
fn renegade_keeps_every_attribute() {
    check(FileFormat::Renegade, IceMode::Blink, false);
    check(FileFormat::Renegade, IceMode::Ice, true);
}

#[test]
fn binary_formats_keep_every_attribute() {
    check(FileFormat::Bin, IceMode::Blink, true);
    check(FileFormat::Bin, IceMode::Ice, false);
    check(FileFormat::XBin, IceMode::Blink, true);
    check(FileFormat::XBin, IceMode::Ice, true);
    check(FileFormat::IceDraw, IceMode::Ice, true);
    check(FileFormat::Artworx, IceMode::Ice, true);
    check(FileFormat::IcyDraw, IceMode::Blink, true);
    check(FileFormat::IcyDraw, IceMode::Ice, true);
}

#[test]
fn tundra_keeps_every_attribute() {
    // TundraDraw stores RGB colors but no blinking.
    check(FileFormat::TundraDraw, IceMode::Blink, false);
    check(FileFormat::TundraDraw, IceMode::Ice, true);
}
