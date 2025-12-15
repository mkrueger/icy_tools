use icy_engine::formats::ImageFormat;
use std::path::Path;

#[test]
fn image_format_extension_detection() {
    assert_eq!(ImageFormat::from_extension("png"), Some(ImageFormat::Png));
    assert_eq!(ImageFormat::from_extension("PNG"), Some(ImageFormat::Png));
    assert_eq!(ImageFormat::from_extension("gif"), Some(ImageFormat::Gif));
    assert_eq!(ImageFormat::from_extension("GIF"), Some(ImageFormat::Gif));
    assert_eq!(ImageFormat::from_extension("jpg"), Some(ImageFormat::Jpeg));
    assert_eq!(ImageFormat::from_extension("jpeg"), Some(ImageFormat::Jpeg));
    assert_eq!(ImageFormat::from_extension("bmp"), Some(ImageFormat::Bmp));
    assert_eq!(ImageFormat::from_extension("six"), Some(ImageFormat::Sixel));
    assert_eq!(ImageFormat::from_extension("sixel"), Some(ImageFormat::Sixel));
    assert_eq!(ImageFormat::from_extension("xyz"), None);
}

#[test]
fn image_format_path_detection() {
    assert_eq!(ImageFormat::from_path(Path::new("test.png")), Some(ImageFormat::Png));
    assert_eq!(ImageFormat::from_path(Path::new("/path/to/file.gif")), Some(ImageFormat::Gif));
    assert_eq!(ImageFormat::from_path(Path::new("noext")), None);
}

#[test]
fn image_format_animation_support() {
    assert!(!ImageFormat::Png.supports_animation());
    assert!(ImageFormat::Gif.supports_animation());
}
