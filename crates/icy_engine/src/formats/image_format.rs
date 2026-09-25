//! Image format registry for exporting ANSI art as images.
//!
//! This module provides image export functionality for PNG and GIF formats.
//! GIF export supports blink animation rendering two frames.
//!
//! # Example
//!
//! ```no_run
//! use icy_engine::formats::ImageFormat;
//! use icy_engine::{TextBuffer, Rectangle};
//! use std::path::Path;
//!
//! // Export as PNG
//! let buffer = TextBuffer::default();
//! ImageFormat::Png.save_buffer(&buffer, Path::new("output.png")).unwrap();
//!
//! // Export as animated GIF (with blink)
//! ImageFormat::Gif.save_buffer(&buffer, Path::new("output.gif")).unwrap();
//! ```

use std::path::Path;

use crate::{Rectangle, RenderOptions, Result, Screen, TextBuffer, TextPane};

/// Image export formats for ANSI art.
///
/// These are separate from text-based `FileFormat` since they represent
/// rendered image output rather than text format conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    /// PNG image format - single static frame
    Png,
    /// GIF image format - supports blink animation (2 frames)
    Gif,
    /// JPEG image format (recognition only, no save/load yet)
    Jpeg,
    /// BMP image format (recognition only, no save/load yet)
    Bmp,
    /// Sixel graphics format (.six, .sixel)
    Sixel,
    /// Truevision TGA (recognition and decoding only)
    Tga,
    /// TIFF (recognition and decoding only)
    Tiff,
    /// WebP (recognition and decoding only)
    WebP,
    /// Quite OK Image format (recognition and decoding only)
    Qoi,
    /// Windows icon (recognition and decoding only)
    Ico,
}

impl ImageFormat {
    /// All available image formats
    pub const ALL: &'static [ImageFormat] = &[
        ImageFormat::Png,
        ImageFormat::Gif,
        ImageFormat::Jpeg,
        ImageFormat::Bmp,
        ImageFormat::Sixel,
        ImageFormat::Tga,
        ImageFormat::Tiff,
        ImageFormat::WebP,
        ImageFormat::Qoi,
        ImageFormat::Ico,
    ];

    /// Get the file extension for this image format.
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Gif => "gif",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Bmp => "bmp",
            ImageFormat::Sixel => "six",
            ImageFormat::Tga => "tga",
            ImageFormat::Tiff => "tif",
            ImageFormat::WebP => "webp",
            ImageFormat::Qoi => "qoi",
            ImageFormat::Ico => "ico",
        }
    }

    /// Get a human-readable name for this format.
    pub fn name(&self) -> &'static str {
        match self {
            ImageFormat::Png => "PNG Image",
            ImageFormat::Gif => "GIF Animation",
            ImageFormat::Jpeg => "JPEG Image",
            ImageFormat::Bmp => "BMP Image",
            ImageFormat::Sixel => "Sixel Graphics",
            ImageFormat::Tga => "TGA Image",
            ImageFormat::Tiff => "TIFF Image",
            ImageFormat::WebP => "WebP Image",
            ImageFormat::Qoi => "QOI Image",
            ImageFormat::Ico => "Windows Icon",
        }
    }

    /// Get a description of this format's capabilities.
    pub fn description(&self) -> &'static str {
        match self {
            ImageFormat::Png => "Static PNG image",
            ImageFormat::Gif => "Animated GIF with blink support",
            ImageFormat::Jpeg => "JPEG image (recognition only)",
            ImageFormat::Bmp => "BMP image (recognition only)",
            ImageFormat::Sixel => "Sixel terminal graphics",
            ImageFormat::Tga => "Truevision TGA image (recognition only)",
            ImageFormat::Tiff => "TIFF image (recognition only)",
            ImageFormat::WebP => "WebP image (recognition only)",
            ImageFormat::Qoi => "QOI image (recognition only)",
            ImageFormat::Ico => "Windows icon (recognition only)",
        }
    }

    /// Whether this format supports animation.
    pub fn supports_animation(&self) -> bool {
        matches!(self, ImageFormat::Gif)
    }

    /// Whether this format supports saving.
    pub fn supports_save(&self) -> bool {
        matches!(self, ImageFormat::Png | ImageFormat::Gif)
    }

    /// Detect image format from file extension.
    pub fn from_extension(ext: &str) -> Option<ImageFormat> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(ImageFormat::Png),
            "gif" => Some(ImageFormat::Gif),
            "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
            "bmp" => Some(ImageFormat::Bmp),
            "six" | "sixel" => Some(ImageFormat::Sixel),
            "tga" => Some(ImageFormat::Tga),
            "tif" | "tiff" => Some(ImageFormat::Tiff),
            "webp" => Some(ImageFormat::WebP),
            "qoi" => Some(ImageFormat::Qoi),
            "ico" => Some(ImageFormat::Ico),
            _ => None,
        }
    }

    /// Detect image format from file path.
    pub fn from_path(path: &Path) -> Option<ImageFormat> {
        path.extension().and_then(|ext| ext.to_str()).and_then(ImageFormat::from_extension)
    }

    fn image_crate_format(&self) -> Option<image::ImageFormat> {
        match self {
            ImageFormat::Png => Some(image::ImageFormat::Png),
            ImageFormat::Gif => Some(image::ImageFormat::Gif),
            ImageFormat::Jpeg => Some(image::ImageFormat::Jpeg),
            ImageFormat::Bmp => Some(image::ImageFormat::Bmp),
            ImageFormat::Tga => Some(image::ImageFormat::Tga),
            ImageFormat::Tiff => Some(image::ImageFormat::Tiff),
            ImageFormat::WebP => Some(image::ImageFormat::WebP),
            ImageFormat::Qoi => Some(image::ImageFormat::Qoi),
            ImageFormat::Ico => Some(image::ImageFormat::Ico),
            ImageFormat::Sixel => None,
        }
    }

    /// Decodes an image file of this format into RGBA pixels.
    ///
    /// The extension picks the decoder (TGA has no signature); files whose content does not
    /// match fall back to detection from the data.
    pub fn decode_rgba(&self, data: &[u8]) -> std::result::Result<image::RgbaImage, String> {
        let Some(format) = self.image_crate_format() else {
            let image = icy_sixel::SixelImage::decode(data).map_err(|error| error.to_string())?;
            return image::RgbaImage::from_raw(image.width as u32, image.height as u32, image.pixels).ok_or_else(|| "Invalid Sixel dimensions".to_string());
        };
        image::load_from_memory_with_format(data, format)
            .or_else(|error| image::load_from_memory(data).map_err(|_| error))
            .map(|image| image.into_rgba8())
            .map_err(|error| error.to_string())
    }

    /// Save a Screen to an image file.
    ///
    /// For PNG: Renders a single static frame.
    /// For GIF: Renders an animated GIF with blink effect (2 frames at ~560ms interval).
    ///
    /// # Arguments
    /// * `screen` - The screen to render (implements Screen trait)
    /// * `path` - Output file path
    ///
    /// # Returns
    /// `Ok(())` on success, or an error if rendering/saving fails.
    pub fn save_screen(&self, screen: &dyn Screen, path: &Path) -> Result<()> {
        let size = screen.size();
        let rect = Rectangle::from(0, 0, size.width, size.height);

        match self {
            ImageFormat::Png => self.save_screen_png(screen, path, rect),
            ImageFormat::Gif => self.save_screen_gif(screen, path, rect),
            _ => Err(crate::EngineError::FormatNotSupported {
                name: self.name().to_string(),
                operation: "saving".to_string(),
            }),
        }
    }

    /// Save a region of a Screen to an image file.
    pub fn save_screen_region(&self, screen: &dyn Screen, path: &Path, region: Rectangle) -> Result<()> {
        match self {
            ImageFormat::Png => self.save_screen_png(screen, path, region),
            ImageFormat::Gif => self.save_screen_gif(screen, path, region),
            _ => Err(crate::EngineError::FormatNotSupported {
                name: self.name().to_string(),
                operation: "saving".to_string(),
            }),
        }
    }

    fn save_screen_png(&self, screen: &dyn Screen, path: &Path, region: Rectangle) -> Result<()> {
        let options = RenderOptions {
            rect: region.into(),
            blink_on: true,
            ..Default::default()
        };

        let (size, pixels) = screen.render_to_rgba(&options);

        let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            image::RgbaImage::from_raw(size.width as u32, size.height as u32, pixels).ok_or(crate::EngineError::ImageBufferCreationFailed)?;

        img.save(path).map_err(|e| crate::EngineError::ImageSaveFailed { message: e.to_string() })?;

        Ok(())
    }

    fn save_screen_gif(&self, screen: &dyn Screen, path: &Path, region: Rectangle) -> Result<()> {
        use crate::gif_encoder::GifEncoder;

        let size = screen.size();
        let dim = screen.font_dimensions();
        let width = (region.width().min(size.width) * dim.width) as u16;
        let height = (region.height().min(size.height) * dim.height) as u16;

        // Get blink rate from the screen's buffer type (in milliseconds)
        let blink_rate_ms = screen.buffer_type().blink_rate();

        // Frame 1: blink_on = true (visible)
        let options1 = RenderOptions {
            rect: region.into(),
            blink_on: true,
            ..Default::default()
        };
        let (_frame1_size, frame1_data) = screen.render_to_rgba(&options1);

        // Frame 2: blink_on = false (hidden) - use screen's blink rate
        let options2 = RenderOptions {
            rect: region.into(),
            blink_on: false,
            ..Default::default()
        };
        let (_frame2_size, frame2_data) = screen.render_to_rgba(&options2);

        // Use new GIF encoder
        let encoder = GifEncoder::new(width, height);
        encoder.encode_blink_animation(path, frame1_data, frame2_data, blink_rate_ms as u32)
    }

    /// Save a `TextBuffer` to an image file.
    ///
    /// For PNG: Renders a single static frame.
    /// For GIF: Renders an animated GIF with blink effect (2 frames at ~560ms interval).
    ///
    /// # Arguments
    /// * `buffer` - The text buffer to render
    /// * `path` - Output file path
    ///
    /// # Returns
    /// `Ok(())` on success, or an error if rendering/saving fails.
    pub fn save_buffer(&self, buffer: &TextBuffer, path: &Path) -> Result<()> {
        let rect = Rectangle::from(0, 0, buffer.width(), buffer.height());

        match self {
            ImageFormat::Png => self.save_png(buffer, path, rect),
            ImageFormat::Gif => self.save_gif(buffer, path, rect),
            _ => Err(crate::EngineError::FormatNotSupported {
                name: self.name().to_string(),
                operation: "saving".to_string(),
            }),
        }
    }

    /// Save a region of a buffer to an image file.
    ///
    /// # Arguments
    /// * `buffer` - The text buffer to render
    /// * `path` - Output file path
    /// * `region` - The rectangular region to export
    pub fn save_buffer_region(&self, buffer: &TextBuffer, path: &Path, region: Rectangle) -> Result<()> {
        match self {
            ImageFormat::Png => self.save_png(buffer, path, region),
            ImageFormat::Gif => self.save_gif(buffer, path, region),
            _ => Err(crate::EngineError::FormatNotSupported {
                name: self.name().to_string(),
                operation: "saving".to_string(),
            }),
        }
    }

    fn save_png(&self, buffer: &TextBuffer, path: &Path, region: Rectangle) -> Result<()> {
        let options = RenderOptions {
            rect: region.into(),
            blink_on: true,
            ..Default::default()
        };

        let scan_lines = options.override_scan_lines.unwrap_or(false);
        let (size, pixels) = buffer.render_to_rgba(&options, scan_lines);

        // Create image buffer and save
        let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            image::RgbaImage::from_raw(size.width as u32, size.height as u32, pixels).ok_or(crate::EngineError::ImageBufferCreationFailed)?;

        img.save(path).map_err(|e| crate::EngineError::ImageSaveFailed { message: e.to_string() })?;

        Ok(())
    }

    fn save_gif(&self, buffer: &TextBuffer, path: &Path, region: Rectangle) -> Result<()> {
        use crate::gif_encoder::GifEncoder;

        let size = buffer.size();
        let dim = buffer.font_dimensions();
        let width = (region.width().min(size.width) * dim.width) as u16;
        let height = (region.height().min(size.height) * dim.height) as u16;

        // Get blink rate from the buffer's type (in milliseconds)
        let blink_rate_ms = buffer.buffer_type.blink_rate();

        // Frame 1: blink_on = true (visible)
        let options1 = RenderOptions {
            rect: region.into(),
            blink_on: true,
            ..Default::default()
        };
        let scan_lines = options1.override_scan_lines.unwrap_or(false);
        let (_frame1_size, frame1_data) = buffer.render_to_rgba(&options1, scan_lines);

        // Frame 2: blink_on = false (hidden) - use buffer's blink rate
        let options2 = RenderOptions {
            rect: region.into(),
            blink_on: false,
            ..Default::default()
        };
        let (_frame2_size, frame2_data) = buffer.render_to_rgba(&options2, scan_lines);

        // Use new GIF encoder
        let encoder = GifEncoder::new(width, height);
        encoder.encode_blink_animation(path, frame1_data, frame2_data, blink_rate_ms as u32)
    }
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}
