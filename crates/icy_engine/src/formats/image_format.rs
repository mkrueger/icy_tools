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
}

impl ImageFormat {
    /// All available image formats
    pub const ALL: &'static [ImageFormat] = &[ImageFormat::Png, ImageFormat::Gif, ImageFormat::Jpeg, ImageFormat::Bmp, ImageFormat::Sixel];

    /// Get the file extension for this image format.
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Gif => "gif",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Bmp => "bmp",
            ImageFormat::Sixel => "six",
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
            _ => None,
        }
    }

    /// Detect image format from file path.
    pub fn from_path(path: &Path) -> Option<ImageFormat> {
        path.extension().and_then(|ext| ext.to_str()).and_then(ImageFormat::from_extension)
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
            ImageFormat::Jpeg | ImageFormat::Bmp | ImageFormat::Sixel => Err(crate::EngineError::FormatNotSupported {
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
            ImageFormat::Jpeg | ImageFormat::Bmp | ImageFormat::Sixel => Err(crate::EngineError::FormatNotSupported {
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

    /// Save a TextBuffer to an image file.
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
            ImageFormat::Jpeg | ImageFormat::Bmp | ImageFormat::Sixel => Err(crate::EngineError::FormatNotSupported {
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
            ImageFormat::Jpeg | ImageFormat::Bmp | ImageFormat::Sixel => Err(crate::EngineError::FormatNotSupported {
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
