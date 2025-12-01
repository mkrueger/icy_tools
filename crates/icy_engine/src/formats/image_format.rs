//! Image format registry for exporting ANSI art as images.
//!
//! This module provides image export functionality for PNG and GIF formats.
//! GIF export supports blink animation rendering two frames.
//!
//! # Example
//!
//! ```no_run
//! use icy_engine::formats::ImageFormat;
//! use icy_engine::{Buffer, Rectangle};
//! use std::path::Path;
//!
//! // Export as PNG
//! let buffer = Buffer::default();
//! ImageFormat::Png.save_buffer(&buffer, Path::new("output.png")).unwrap();
//!
//! // Export as animated GIF (with blink)
//! ImageFormat::Gif.save_buffer(&buffer, Path::new("output.gif")).unwrap();
//! ```

use std::path::Path;

use crate::{EngineResult, Rectangle, RenderOptions, Screen, TextBuffer, TextPane};

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
}

impl ImageFormat {
    /// All available image formats
    pub const ALL: &'static [ImageFormat] = &[ImageFormat::Png, ImageFormat::Gif, ImageFormat::Jpeg, ImageFormat::Bmp];

    /// Get the file extension for this image format.
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Gif => "gif",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Bmp => "bmp",
        }
    }

    /// Get a human-readable name for this format.
    pub fn name(&self) -> &'static str {
        match self {
            ImageFormat::Png => "PNG Image",
            ImageFormat::Gif => "GIF Animation",
            ImageFormat::Jpeg => "JPEG Image",
            ImageFormat::Bmp => "BMP Image",
        }
    }

    /// Get a description of this format's capabilities.
    pub fn description(&self) -> &'static str {
        match self {
            ImageFormat::Png => "Static PNG image",
            ImageFormat::Gif => "Animated GIF with blink support",
            ImageFormat::Jpeg => "JPEG image (recognition only)",
            ImageFormat::Bmp => "BMP image (recognition only)",
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
    pub fn save_screen(&self, screen: &dyn Screen, path: &Path) -> EngineResult<()> {
        let size = screen.get_size();
        let rect = Rectangle::from(0, 0, size.width, size.height);

        match self {
            ImageFormat::Png => self.save_screen_png(screen, path, rect),
            ImageFormat::Gif => self.save_screen_gif(screen, path, rect),
            ImageFormat::Jpeg | ImageFormat::Bmp => {
                anyhow::bail!("Saving to {} is not supported", self.name())
            }
        }
    }

    /// Save a region of a Screen to an image file.
    pub fn save_screen_region(&self, screen: &dyn Screen, path: &Path, region: Rectangle) -> EngineResult<()> {
        match self {
            ImageFormat::Png => self.save_screen_png(screen, path, region),
            ImageFormat::Gif => self.save_screen_gif(screen, path, region),
            ImageFormat::Jpeg | ImageFormat::Bmp => {
                anyhow::bail!("Saving to {} is not supported", self.name())
            }
        }
    }

    fn save_screen_png(&self, screen: &dyn Screen, path: &Path, region: Rectangle) -> EngineResult<()> {
        let options = RenderOptions {
            rect: region.into(),
            blink_on: true,
            ..Default::default()
        };

        let (size, pixels) = screen.render_to_rgba(&options);

        let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            image::RgbaImage::from_raw(size.width as u32, size.height as u32, pixels).ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;

        img.save(path).map_err(|e| anyhow::anyhow!("Failed to save PNG: {}", e))?;

        Ok(())
    }

    fn save_screen_gif(&self, screen: &dyn Screen, path: &Path, region: Rectangle) -> EngineResult<()> {
        use gifski::{Repeat, progress::NoProgress};

        let size = screen.get_size();
        let dim = screen.get_font_dimensions();
        let width = (region.get_width().min(size.width) * dim.width) as usize;
        let height = (region.get_height().min(size.height) * dim.height) as usize;

        // Get blink rate from the screen's buffer type (in milliseconds)
        let blink_rate_ms = screen.buffer_type().get_blink_rate();
        let blink_rate_secs = blink_rate_ms as f64 / 1000.0;

        let settings = gifski::Settings {
            width: Some(width as u32),
            height: Some(height as u32),
            quality: 100,
            fast: true,
            repeat: Repeat::Infinite,
        };

        let (collector, writer) = gifski::new(settings)?;

        let fs = std::fs::File::create(path)?;
        let mut pb = NoProgress {};

        let path_clone = path.to_path_buf();
        let writer_handle = std::thread::spawn(move || {
            if let Err(e) = writer.write(fs, &mut pb) {
                log::error!("GIF writer error for {:?}: {}", path_clone, e);
            }
        });

        // Frame 1: blink_on = true (visible)
        let options1 = RenderOptions {
            rect: region.into(),
            blink_on: true,
            ..Default::default()
        };
        let (frame1_size, frame1_data) = screen.render_to_rgba(&options1);
        let img1 = Self::create_imgref(frame1_data, frame1_size);
        collector.add_frame_rgba(0, img1, 0.0)?;

        // Frame 2: blink_on = false (hidden) - use screen's blink rate
        let options2 = RenderOptions {
            rect: region.into(),
            blink_on: false,
            ..Default::default()
        };
        let (frame2_size, frame2_data) = screen.render_to_rgba(&options2);
        let img2 = Self::create_imgref(frame2_data, frame2_size);
        collector.add_frame_rgba(1, img2, blink_rate_secs)?;

        drop(collector);
        writer_handle.join().map_err(|_| anyhow::anyhow!("GIF writer thread panicked"))?;

        Ok(())
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
    pub fn save_buffer(&self, buffer: &TextBuffer, path: &Path) -> EngineResult<()> {
        let rect = Rectangle::from(0, 0, buffer.get_width(), buffer.get_height());

        match self {
            ImageFormat::Png => self.save_png(buffer, path, rect),
            ImageFormat::Gif => self.save_gif(buffer, path, rect),
            ImageFormat::Jpeg | ImageFormat::Bmp => {
                anyhow::bail!("Saving to {} is not supported", self.name())
            }
        }
    }

    /// Save a region of a buffer to an image file.
    ///
    /// # Arguments
    /// * `buffer` - The text buffer to render
    /// * `path` - Output file path
    /// * `region` - The rectangular region to export
    pub fn save_buffer_region(&self, buffer: &TextBuffer, path: &Path, region: Rectangle) -> EngineResult<()> {
        match self {
            ImageFormat::Png => self.save_png(buffer, path, region),
            ImageFormat::Gif => self.save_gif(buffer, path, region),
            ImageFormat::Jpeg | ImageFormat::Bmp => {
                anyhow::bail!("Saving to {} is not supported", self.name())
            }
        }
    }

    fn save_png(&self, buffer: &TextBuffer, path: &Path, region: Rectangle) -> EngineResult<()> {
        let options = RenderOptions {
            rect: region.into(),
            blink_on: true,
            ..Default::default()
        };

        let scan_lines = options.override_scan_lines.unwrap_or(false);
        let (size, pixels) = buffer.render_to_rgba(&options, scan_lines);

        // Create image buffer and save
        let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            image::RgbaImage::from_raw(size.width as u32, size.height as u32, pixels).ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;

        img.save(path).map_err(|e| anyhow::anyhow!("Failed to save PNG: {}", e))?;

        Ok(())
    }

    fn save_gif(&self, buffer: &TextBuffer, path: &Path, region: Rectangle) -> EngineResult<()> {
        use gifski::{Repeat, progress::NoProgress};

        let size = buffer.get_size();
        let dim = buffer.get_font_dimensions();
        let width = (region.get_width().min(size.width) * dim.width) as usize;
        let height = (region.get_height().min(size.height) * dim.height) as usize;

        // Get blink rate from the buffer's type (in milliseconds)
        let blink_rate_ms = buffer.buffer_type.get_blink_rate();
        let blink_rate_secs = blink_rate_ms as f64 / 1000.0;

        let settings = gifski::Settings {
            width: Some(width as u32),
            height: Some(height as u32),
            quality: 100,
            fast: true,
            repeat: Repeat::Infinite,
        };

        let (collector, writer) = gifski::new(settings)?;

        let fs = std::fs::File::create(path)?;
        let mut pb = NoProgress {};

        // Spawn writer thread
        let path_clone = path.to_path_buf();
        let writer_handle = std::thread::spawn(move || {
            if let Err(e) = writer.write(fs, &mut pb) {
                log::error!("GIF writer error for {:?}: {}", path_clone, e);
            }
        });

        // Frame 1: blink_on = true (visible)
        let options1 = RenderOptions {
            rect: region.into(),
            blink_on: true,
            ..Default::default()
        };
        let scan_lines = options1.override_scan_lines.unwrap_or(false);
        let (frame1_size, frame1_data) = buffer.render_to_rgba(&options1, scan_lines);
        let img1 = Self::create_imgref(frame1_data, frame1_size);
        collector.add_frame_rgba(0, img1, 0.0)?;

        // Frame 2: blink_on = false (hidden) - use buffer's blink rate
        let options2 = RenderOptions {
            rect: region.into(),
            blink_on: false,
            ..Default::default()
        };
        let (frame2_size, frame2_data) = buffer.render_to_rgba(&options2, scan_lines);
        let img2 = Self::create_imgref(frame2_data, frame2_size);
        collector.add_frame_rgba(1, img2, blink_rate_secs)?;

        // Drop collector to signal completion
        drop(collector);

        // Wait for writer to finish
        writer_handle.join().map_err(|_| anyhow::anyhow!("GIF writer thread panicked"))?;

        Ok(())
    }

    fn create_imgref(data: Vec<u8>, size: crate::Size) -> imgref::Img<Vec<rgb::RGBA<u8>>> {
        let mut rgba_data = Vec::with_capacity(data.len() / 4);
        for chunk in data.chunks_exact(4) {
            rgba_data.push(rgb::RGBA::new(chunk[0], chunk[1], chunk[2], chunk[3]));
        }
        imgref::Img::new(rgba_data, size.width as usize, size.height as usize)
    }
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_detection() {
        assert_eq!(ImageFormat::from_extension("png"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_extension("PNG"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_extension("gif"), Some(ImageFormat::Gif));
        assert_eq!(ImageFormat::from_extension("GIF"), Some(ImageFormat::Gif));
        assert_eq!(ImageFormat::from_extension("jpg"), None);
    }

    #[test]
    fn test_path_detection() {
        assert_eq!(ImageFormat::from_path(Path::new("test.png")), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_path(Path::new("/path/to/file.gif")), Some(ImageFormat::Gif));
        assert_eq!(ImageFormat::from_path(Path::new("noext")), None);
    }

    #[test]
    fn test_animation_support() {
        assert!(!ImageFormat::Png.supports_animation());
        assert!(ImageFormat::Gif.supports_animation());
    }
}
