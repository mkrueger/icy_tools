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

#[test]
fn decodes_every_raster_format_by_extension() {
    let mut pixels = image::RgbaImage::new(3, 2);
    pixels.put_pixel(1, 0, image::Rgba([200, 10, 30, 255]));
    for (format, encoding) in [
        (ImageFormat::Png, image::ImageFormat::Png),
        (ImageFormat::Bmp, image::ImageFormat::Bmp),
        (ImageFormat::Tga, image::ImageFormat::Tga),
        (ImageFormat::Tiff, image::ImageFormat::Tiff),
        (ImageFormat::WebP, image::ImageFormat::WebP),
        (ImageFormat::Qoi, image::ImageFormat::Qoi),
        (ImageFormat::Ico, image::ImageFormat::Ico),
    ] {
        let mut data = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(pixels.clone()).write_to(&mut data, encoding).unwrap();
        assert_eq!(ImageFormat::from_extension(format.extension()), Some(format));
        let decoded = format.decode_rgba(data.get_ref()).unwrap_or_else(|error| panic!("{format:?}: {error}"));
        assert_eq!(decoded.dimensions(), (3, 2), "{format:?}");
        assert_eq!(decoded.get_pixel(1, 0), &image::Rgba([200, 10, 30, 255]), "{format:?}");
    }
    assert_eq!(ImageFormat::from_extension("TIFF"), Some(ImageFormat::Tiff));
}

#[test]
fn mislabelled_images_fall_back_to_content_detection() {
    let mut data = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::new(4, 4)).write_to(&mut data, image::ImageFormat::Png).unwrap();
    assert_eq!(ImageFormat::Jpeg.decode_rgba(data.get_ref()).unwrap().dimensions(), (4, 4));
    assert!(ImageFormat::Tga.decode_rgba(b"not an image").is_err());
}
