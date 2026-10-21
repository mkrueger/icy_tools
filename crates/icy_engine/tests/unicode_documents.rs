use icy_engine::{
    AnsiCompatibilityLevel, BufferType, EditableScreen, FileFormat, FormatOptions, IcyDrawFormatOptions, Position, SaveOptions, TextPane, TextScreen,
};
use icy_parser_core::{AnsiParser, CommandParser};

fn document(text: &str) -> TextScreen {
    let mut screen = TextScreen::new((12, 3));
    screen.buffer.buffer_type = BufferType::Unicode;
    screen.buffer.terminal_state.is_terminal_buffer = false;
    screen.set_unicode_width(true);
    AnsiParser::default().parse(text.as_bytes(), &mut icy_engine::ScreenSink::new(&mut screen));
    screen
}

fn native_options(compress: bool) -> SaveOptions {
    SaveOptions {
        format: FormatOptions::IcyDraw(IcyDrawFormatOptions {
            skip_thumbnail: true,
            compress,
        }),
        ..SaveOptions::icy_draw()
    }
}

#[test]
fn native_unicode_documents_preserve_graphemes_and_width_mode() {
    for compress in [false, true] {
        for text in ["", "A\u{754c}e\u{301}\u{1f600}"] {
            let original = document(text);
            let bytes = FileFormat::IcyDraw.to_bytes(&original.buffer, &native_options(compress)).unwrap();
            let mut loaded = FileFormat::IcyDraw.from_bytes(&bytes, None).unwrap().screen;
            assert_eq!(loaded.buffer.buffer_type, BufferType::Unicode);
            assert!(loaded.unicode_width());
            for x in 0..12 {
                let position = Position::new(x, 0);
                assert_eq!(original.char_at(position), loaded.char_at(position));
                assert_eq!(original.grapheme_at(position), loaded.grapheme_at(position));
                assert_eq!(original.is_grapheme_continuation(position), loaded.is_grapheme_continuation(position));
            }
            loaded.set_caret_position(Position::new(0, 1));
            AnsiParser::default().parse("\u{754c}".as_bytes(), &mut icy_engine::ScreenSink::new(&mut loaded));
            assert_eq!(loaded.grapheme_at(Position::new(0, 1)), Some(("\u{754c}", 2)));
        }
    }
}

#[test]
fn native_legacy_documents_keep_legacy_cell_widths() {
    let screen = TextScreen::new((12, 3));
    let bytes = FileFormat::IcyDraw.to_bytes(&screen.buffer, &native_options(true)).unwrap();
    let loaded = FileFormat::IcyDraw.from_bytes(&bytes, None).unwrap().screen;
    assert!(!loaded.unicode_width());
    assert_eq!(loaded.buffer.buffer_type, screen.buffer.buffer_type);
}

#[test]
fn unicode_text_export_roundtrips_wide_and_combining_characters() {
    let screen = document("A\u{754c}e\u{301}\u{1f600}|");
    let bytes = FileFormat::Ascii.to_bytes(&screen.buffer, &SaveOptions::default()).unwrap();
    assert!(bytes.starts_with(b"\xEF\xBB\xBF"));
    assert!(String::from_utf8(bytes.clone()).unwrap().contains("A\u{754c}e\u{301}\u{1f600}|"));
    let loaded = FileFormat::Ascii.from_bytes(&bytes, None).unwrap().screen;
    assert_eq!(loaded.grapheme_at(Position::new(1, 0)), Some(("\u{754c}", 2)));
    assert_eq!(loaded.grapheme_at(Position::new(3, 0)), Some(("e\u{301}", 1)));
    assert_eq!(loaded.char_at(Position::new(6, 0)).ch, '|');
}

#[test]
fn unicode_ansi_export_roundtrips_without_extra_continuation_spaces() {
    let screen = document("A\u{754c}e\u{301}\u{1f600}|");
    for rgb in [false, true] {
        let mut options = SaveOptions::ansi(AnsiCompatibilityLevel::Utf8Terminal);
        if let FormatOptions::Ansi(ansi) = &mut options.format {
            ansi.always_use_rgb = rgb;
        }

        let bytes = FileFormat::Ansi.to_bytes(&screen.buffer, &options).unwrap();
        let loaded = FileFormat::Ansi.from_bytes(&bytes, None).unwrap().screen;
        assert_eq!(loaded.grapheme_at(Position::new(1, 0)), Some(("\u{754c}", 2)));
        assert_eq!(loaded.grapheme_at(Position::new(3, 0)), Some(("e\u{301}", 1)));
        assert_eq!(loaded.char_at(Position::new(6, 0)).ch, '|');
    }
}

#[test]
fn unicode_documents_reject_lossy_legacy_exports() {
    let screen = document("\u{754c}");
    assert!(FileFormat::Ansi.to_bytes(&screen.buffer, &SaveOptions::default()).is_err());
    assert!(FileFormat::XBin.to_bytes(&screen.buffer, &SaveOptions::default()).is_err());
    assert!(FileFormat::Bin.to_bytes(&screen.buffer, &SaveOptions::default()).is_err());
}

#[test]
fn explicit_utf8_input_accepts_bomless_text_and_rejects_invalid_bytes() {
    let options = || icy_engine::LoadData::default().with_convert_to_utf8(true);
    let screen = FileFormat::Ascii.from_bytes("A\u{754c}|".as_bytes(), Some(options())).unwrap().screen;
    assert_eq!(screen.buffer.buffer_type, BufferType::Unicode);
    assert_eq!(screen.grapheme_at(Position::new(1, 0)), Some(("\u{754c}", 2)));
    assert_eq!(screen.char_at(Position::new(3, 0)).ch, '|');
    assert!(FileFormat::Ascii.from_bytes(&[0xFF], Some(options())).is_err());
    let legacy = FileFormat::Ascii.from_bytes(&[0xFF], None).unwrap().screen;
    assert_eq!(legacy.buffer.buffer_type, BufferType::CP437);
}

#[test]
fn native_unicode_documents_accept_external_previews_and_validate_rgba_size() {
    let screen = document("\u{754c}");
    let pixels = vec![10, 20, 30, 255, 40, 50, 60, 255];
    let bytes = icy_engine::save_icy_draw_with_preview(&screen.buffer, &SaveOptions::icy_draw(), Some((icy_engine::Size::new(2, 1), pixels.clone()))).unwrap();
    let image = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (2, 1));
    assert_eq!(image.as_raw(), &pixels);
    let loaded = FileFormat::IcyDraw.from_bytes(&bytes, None).unwrap().screen;
    assert_eq!(loaded.grapheme_at(Position::new(0, 0)), Some(("\u{754c}", 2)));
    assert!(icy_engine::save_icy_draw_with_preview(&screen.buffer, &SaveOptions::icy_draw(), Some((icy_engine::Size::new(2, 1), vec![0; 7])),).is_err());
}
