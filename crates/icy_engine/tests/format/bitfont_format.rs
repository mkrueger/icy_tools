use icy_engine::formats::BitFontFormat;
use std::path::Path;

#[test]
fn bitfont_format_from_extension() {
    // YAFF
    assert_eq!(BitFontFormat::from_extension("yaff"), Some(BitFontFormat::Yaff));
    assert_eq!(BitFontFormat::from_extension("YAFF"), Some(BitFontFormat::Yaff));
    assert_eq!(BitFontFormat::from_extension(".yaff"), Some(BitFontFormat::Yaff));

    // PSF
    assert_eq!(BitFontFormat::from_extension("psf"), Some(BitFontFormat::Psf));
    assert_eq!(BitFontFormat::from_extension("PSF"), Some(BitFontFormat::Psf));

    // Raw formats
    assert_eq!(BitFontFormat::from_extension("f08"), Some(BitFontFormat::Raw(8)));
    assert_eq!(BitFontFormat::from_extension("f8"), Some(BitFontFormat::Raw(8)));
    assert_eq!(BitFontFormat::from_extension("f14"), Some(BitFontFormat::Raw(14)));
    assert_eq!(BitFontFormat::from_extension("f16"), Some(BitFontFormat::Raw(16)));
    assert_eq!(BitFontFormat::from_extension("F16"), Some(BitFontFormat::Raw(16)));
    assert_eq!(BitFontFormat::from_extension(".f19"), Some(BitFontFormat::Raw(19)));

    // Invalid
    assert_eq!(BitFontFormat::from_extension("txt"), None);
    assert_eq!(BitFontFormat::from_extension("f"), None);
    assert_eq!(BitFontFormat::from_extension(""), None);
}

#[test]
fn bitfont_format_from_path() {
    assert_eq!(BitFontFormat::from_path(Path::new("font.yaff")), Some(BitFontFormat::Yaff));
    assert_eq!(BitFontFormat::from_path(Path::new("/path/to/console.psf")), Some(BitFontFormat::Psf));
    assert_eq!(BitFontFormat::from_path(Path::new("dos.f16")), Some(BitFontFormat::Raw(16)));
    assert_eq!(BitFontFormat::from_path(Path::new("font.f08")), Some(BitFontFormat::Raw(8)));
    assert_eq!(BitFontFormat::from_path(Path::new("noext")), None);
}

#[test]
fn bitfont_format_extension() {
    assert_eq!(BitFontFormat::Yaff.extension(), "yaff");
    assert_eq!(BitFontFormat::Psf.extension(), "psf");
    assert_eq!(BitFontFormat::Raw(8).extension(), "f08");
    assert_eq!(BitFontFormat::Raw(14).extension(), "f14");
    assert_eq!(BitFontFormat::Raw(16).extension(), "f16");
}

#[test]
fn bitfont_format_is_bitfont_extension() {
    assert!(BitFontFormat::is_bitfont_extension("yaff"));
    assert!(BitFontFormat::is_bitfont_extension("psf"));
    assert!(BitFontFormat::is_bitfont_extension("f08"));
    assert!(BitFontFormat::is_bitfont_extension("f16"));
    assert!(!BitFontFormat::is_bitfont_extension("txt"));
    assert!(!BitFontFormat::is_bitfont_extension("ans"));
}
