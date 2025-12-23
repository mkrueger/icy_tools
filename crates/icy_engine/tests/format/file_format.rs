use icy_engine::{BufferType, formats::FileFormat};
use std::path::Path;

#[test]
fn file_format_from_extension() {
    assert_eq!(FileFormat::from_extension("ans"), Some(FileFormat::Ansi));
    assert_eq!(FileFormat::from_extension("ANS"), Some(FileFormat::Ansi));
    assert_eq!(FileFormat::from_extension("diz"), Some(FileFormat::Ansi));
    assert_eq!(FileFormat::from_extension("xb"), Some(FileFormat::XBin));
    assert_eq!(FileFormat::from_extension("unknown"), None);
}

#[test]
fn file_format_from_path() {
    assert_eq!(FileFormat::from_path(Path::new("test.ans")), Some(FileFormat::Ansi));
    assert_eq!(FileFormat::from_path(Path::new("/path/to/file.xb")), Some(FileFormat::XBin));
    assert_eq!(FileFormat::from_path(Path::new("noext")), None);
}

#[test]
fn file_format_uses_parser() {
    assert!(FileFormat::Ansi.uses_parser());
    assert!(FileFormat::Avatar.uses_parser());
    assert!(!FileFormat::XBin.uses_parser());
    assert!(!FileFormat::IcyDraw.uses_parser());
}

#[test]
fn file_format_supports_save() {
    assert!(FileFormat::Ansi.supports_save());
    assert!(FileFormat::XBin.supports_save());
}

#[test]
fn file_format_is_animated() {
    assert!(FileFormat::IcyAnim.is_animated());
    assert!(!FileFormat::Ansi.is_animated());
}

#[test]
fn file_format_is_supported() {
    // Parser-based formats
    assert!(FileFormat::Ansi.is_supported());
    assert!(FileFormat::Avatar.is_supported());

    // Saveable formats
    assert!(FileFormat::XBin.is_supported());
    assert!(FileFormat::IcyDraw.is_supported());

    // Animation formats
    assert!(FileFormat::IcyAnim.is_supported());

    // Image formats
    assert!(FileFormat::Image(icy_engine::formats::ImageFormat::Png).is_supported());
    assert!(FileFormat::Image(icy_engine::formats::ImageFormat::Gif).is_supported());

    // Font formats
    assert!(FileFormat::CharacterFont(icy_engine::formats::CharacterFontFormat::Tdf).is_supported());
    assert!(FileFormat::CharacterFont(icy_engine::formats::CharacterFontFormat::Figlet).is_supported());
    assert!(FileFormat::BitFont(icy_engine::formats::BitFontFormat::Psf).is_supported());

    // Archives are NOT supported (need to be extracted first)
    assert!(!FileFormat::Archive(unarc_rs::unified::ArchiveFormat::Zip).is_supported());
}

#[test]
fn file_format_all_extensions_contain_primary() {
    for format in FileFormat::ALL {
        let exts = format.all_extensions();
        let primary = format.primary_extension();
        assert!(
            exts.contains(&primary),
            "Format {:?} primary extension '{}' not in all_extensions {:?}",
            format,
            primary,
            exts
        );
    }
}

#[test]
fn file_format_buffer_type_compatibility() {
    // CP437 formats
    assert!(FileFormat::Ansi.is_compatible_with(BufferType::CP437));
    assert!(FileFormat::XBin.is_compatible_with(BufferType::CP437));
    assert!(!FileFormat::Petscii.is_compatible_with(BufferType::CP437));
    assert!(!FileFormat::ViewData.is_compatible_with(BufferType::CP437));

    // PETSCII format
    assert!(FileFormat::Petscii.is_compatible_with(BufferType::Petscii));
    assert!(!FileFormat::Ansi.is_compatible_with(BufferType::Petscii));

    // Viewdata format
    assert!(FileFormat::ViewData.is_compatible_with(BufferType::Viewdata));
    assert!(FileFormat::Mode7.is_compatible_with(BufferType::Viewdata));
    assert!(!FileFormat::Ansi.is_compatible_with(BufferType::Viewdata));

    // IcyDraw supports everything
    assert!(FileFormat::IcyDraw.is_compatible_with(BufferType::CP437));
    assert!(FileFormat::IcyDraw.is_compatible_with(BufferType::Petscii));
    assert!(FileFormat::IcyDraw.is_compatible_with(BufferType::Viewdata));
    assert!(FileFormat::IcyDraw.is_compatible_with(BufferType::Atascii));
}

#[test]
fn file_format_save_formats_for_buffer_type() {
    let cp437_formats = FileFormat::save_formats_for_buffer_type(BufferType::CP437);
    assert!(cp437_formats.contains(&FileFormat::Ansi));
    assert!(cp437_formats.contains(&FileFormat::XBin));
    assert!(!cp437_formats.contains(&FileFormat::Petscii));

    let viewdata_formats = FileFormat::save_formats_for_buffer_type(BufferType::Viewdata);
    assert!(viewdata_formats.contains(&FileFormat::IcyDraw));
    assert!(!viewdata_formats.contains(&FileFormat::Ansi));
}
