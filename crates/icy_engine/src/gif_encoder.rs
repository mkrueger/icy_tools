//! GIF encoding utilities for animated and static GIF export.
//!
//! This module provides helper functions for creating GIF files with
//! proper color quantization and frame timing support.
//!
//! # Features
//! - High-quality color quantization using quantette (Wu's algorithm)
//! - Support for animated GIFs with precise frame timing
//! - Configurable repeat count (infinite, N times, or none)
//!
//! # Example
//! ```no_run
//! use icy_engine::gif_encoder::{GifEncoder, GifFrame, RepeatCount};
//!
//! # let rgba_data_1 = vec![0u8; 80 * 25 * 4];
//! # let rgba_data_2 = vec![0u8; 80 * 25 * 4];
//! let frames = vec![
//!     GifFrame::new(rgba_data_1, 500), // 500ms
//!     GifFrame::new(rgba_data_2, 500), // 500ms
//! ];
//!
//! let mut encoder = GifEncoder::new(80, 25);
//! encoder.set_repeat(RepeatCount::Infinite);
//! encoder.encode_to_file("output.gif", frames).unwrap();
//! ```

use std::io::Write;
use std::path::Path;

use crate::EngineError;

/// How many times the GIF animation should repeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatCount {
    /// Loop forever
    Infinite,
    /// Play once, no repeat
    Once,
    /// Repeat N times (0 = infinite in GIF spec, but we handle that with Infinite)
    Times(u16),
}

impl Default for RepeatCount {
    fn default() -> Self {
        Self::Infinite
    }
}

/// A single frame in a GIF animation.
#[derive(Clone)]
pub struct GifFrame {
    /// RGBA pixel data (4 bytes per pixel: R, G, B, A)
    pub rgba_data: Vec<u8>,
    /// Frame duration in milliseconds
    pub duration_ms: u32,
}

impl GifFrame {
    /// Create a new GIF frame.
    ///
    /// # Arguments
    /// * `rgba_data` - RGBA pixel data (width * height * 4 bytes)
    /// * `duration_ms` - Frame duration in milliseconds
    pub fn new(rgba_data: Vec<u8>, duration_ms: u32) -> Self {
        Self { rgba_data, duration_ms }
    }

    /// Create a frame from raw RGBA data with duration in seconds.
    pub fn from_secs(rgba_data: Vec<u8>, duration_secs: f64) -> Self {
        Self {
            rgba_data,
            duration_ms: (duration_secs * 1000.0) as u32,
        }
    }
}

/// GIF encoder with support for animation and color quantization.
pub struct GifEncoder {
    /// Image width in pixels
    pub width: u16,
    /// Image height in pixels
    pub height: u16,
    /// Repeat count for animation
    pub repeat: RepeatCount,
}

impl GifEncoder {
    /// Create a new GIF encoder for the given dimensions.
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            repeat: RepeatCount::Infinite,
        }
    }

    /// Set the repeat count for animation.
    pub fn set_repeat(&mut self, repeat: RepeatCount) {
        self.repeat = repeat;
    }

    /// Encode frames to a GIF file.
    ///
    /// # Arguments
    /// * `path` - Output file path
    /// * `frames` - Vector of frames to encode
    ///
    /// # Returns
    /// `Ok(())` on success, or an error if encoding fails.
    pub fn encode_to_file(&self, path: impl AsRef<Path>, frames: Vec<GifFrame>) -> crate::Result<()> {
        let file = std::fs::File::create(path.as_ref())?;
        self.encode_to_writer(file, frames)
    }

    /// Encode frames to a writer (e.g., file or buffer).
    pub fn encode_to_writer<W: Write>(&self, writer: W, frames: Vec<GifFrame>) -> crate::Result<()> {
        if frames.is_empty() {
            return Err(EngineError::Generic("No frames to encode".to_string()));
        }

        // Build global palette from first frame
        let first_frame = &frames[0];
        let (global_palette, _) = self.quantize_frame(first_frame)?;

        let mut encoder = gif::Encoder::new(writer, self.width, self.height, &global_palette)
            .map_err(|e| EngineError::Generic(format!("GIF encoder creation failed: {e}")))?;

        // Set repeat count
        match self.repeat {
            RepeatCount::Infinite => encoder
                .set_repeat(gif::Repeat::Infinite)
                .map_err(|e| EngineError::Generic(format!("Failed to set repeat: {e}")))?,
            RepeatCount::Once => {} // No repeat extension needed
            RepeatCount::Times(n) => encoder
                .set_repeat(gif::Repeat::Finite(n))
                .map_err(|e| EngineError::Generic(format!("Failed to set repeat: {e}")))?,
        }

        for frame in &frames {
            self.encode_frame(&mut encoder, frame)?;
        }

        Ok(())
    }

    /// Quantize frame colors to 256-color palette using image crate integration
    fn quantize_frame(&self, frame: &GifFrame) -> crate::Result<(Vec<u8>, Vec<u8>)> {
        // Convert RGBA data to RGB image for quantette
        let rgb_data: Vec<u8> = frame.rgba_data.chunks(4).flat_map(|rgba| [rgba[0], rgba[1], rgba[2]]).collect();

        let img: image::RgbImage = image::RgbImage::from_raw(self.width as u32, self.height as u32, rgb_data)
            .ok_or_else(|| EngineError::Generic("Failed to create RGB image for quantization".to_string()))?;

        let mut pipeline = quantette::ImagePipeline::try_from(&img).map_err(|e| EngineError::Generic(format!("Quantization pipeline error: {e}")))?;

        let (palette, indexed_pixels) = pipeline.palette_size(255).indexed_palette();

        // Convert palette to flat RGB array for gif crate
        // palette is Vec<Srgb<u8>> from palette crate
        let flat_palette: Vec<u8> = palette.iter().flat_map(|c| [c.red, c.green, c.blue]).collect();

        Ok((flat_palette, indexed_pixels))
    }

    /// Encode a single frame to the GIF.
    fn encode_frame<W: Write>(&self, encoder: &mut gif::Encoder<W>, frame: &GifFrame) -> crate::Result<()> {
        let (_palette, indexed_pixels) = self.quantize_frame(frame)?;

        // Convert duration from milliseconds to GIF centiseconds (1/100th of a second)
        let delay_cs = (frame.duration_ms / 10).max(1) as u16;

        // Create GIF frame
        let mut gif_frame = gif::Frame::default();
        gif_frame.width = self.width;
        gif_frame.height = self.height;
        gif_frame.delay = delay_cs;
        gif_frame.dispose = gif::DisposalMethod::Keep;
        gif_frame.buffer = std::borrow::Cow::Owned(indexed_pixels);

        encoder
            .write_frame(&gif_frame)
            .map_err(|e| EngineError::Generic(format!("Failed to write GIF frame: {e}")))?;

        Ok(())
    }

    /// Convenience method to encode a blink animation (2 frames).
    ///
    /// This is commonly used for ANSI art with blinking text.
    ///
    /// # Arguments
    /// * `path` - Output file path
    /// * `frame_on` - RGBA data for blink "on" state
    /// * `frame_off` - RGBA data for blink "off" state
    /// * `blink_rate_ms` - Duration of each frame in milliseconds
    pub fn encode_blink_animation(&self, path: impl AsRef<Path>, frame_on: Vec<u8>, frame_off: Vec<u8>, blink_rate_ms: u32) -> crate::Result<()> {
        let frames = vec![GifFrame::new(frame_on, blink_rate_ms), GifFrame::new(frame_off, blink_rate_ms)];
        self.encode_to_file(path, frames)
    }
}

/// Encode a single static image as a GIF (no animation).
///
/// # Arguments
/// * `path` - Output file path
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
/// * `rgba_data` - RGBA pixel data
pub fn encode_static_gif(path: impl AsRef<Path>, width: u16, height: u16, rgba_data: Vec<u8>) -> crate::Result<()> {
    let encoder = GifEncoder::new(width, height);
    encoder.encode_to_file(path, vec![GifFrame::new(rgba_data, 0)])
}

/// Encode an animated GIF with the given frames.
///
/// # Arguments
/// * `path` - Output file path
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
/// * `frames` - Vector of (RGBA data, duration in ms) tuples
/// * `repeat` - How many times to repeat the animation
pub fn encode_animated_gif(path: impl AsRef<Path>, width: u16, height: u16, frames: Vec<(Vec<u8>, u32)>, repeat: RepeatCount) -> crate::Result<()> {
    let mut encoder = GifEncoder::new(width, height);
    encoder.set_repeat(repeat);

    let gif_frames: Vec<GifFrame> = frames.into_iter().map(|(data, duration)| GifFrame::new(data, duration)).collect();

    encoder.encode_to_file(path, gif_frames)
}

/// Encode an animated GIF with frames from an iterator and progress callback.
///
/// This is useful for encoding large animations where you want to show progress.
///
/// # Arguments
/// * `path` - Output file path
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
/// * `frames` - Iterator of (RGBA data, duration in ms) tuples
/// * `repeat` - How many times to repeat the animation
/// * `progress_callback` - Called with frame index after each frame is encoded
pub fn encode_animated_gif_with_progress<F>(
    path: impl AsRef<Path>,
    width: u16,
    height: u16,
    frames: Vec<(Vec<u8>, u32)>,
    repeat: RepeatCount,
    mut progress_callback: F,
) -> crate::Result<()>
where
    F: FnMut(usize),
{
    let mut encoder = GifEncoder::new(width, height);
    encoder.set_repeat(repeat);

    let file = std::fs::File::create(path.as_ref())?;

    if frames.is_empty() {
        return Err(EngineError::Generic("No frames to encode".to_string()));
    }

    // Build global palette from first frame
    let first_frame = GifFrame::new(frames[0].0.clone(), frames[0].1);
    let (global_palette, _) = encoder.quantize_frame(&first_frame)?;

    let mut gif_encoder = gif::Encoder::new(file, encoder.width, encoder.height, &global_palette)
        .map_err(|e| EngineError::Generic(format!("GIF encoder creation failed: {e}")))?;

    // Set repeat count
    match repeat {
        RepeatCount::Infinite => gif_encoder
            .set_repeat(gif::Repeat::Infinite)
            .map_err(|e| EngineError::Generic(format!("Failed to set repeat: {e}")))?,
        RepeatCount::Once => {}
        RepeatCount::Times(n) => gif_encoder
            .set_repeat(gif::Repeat::Finite(n))
            .map_err(|e| EngineError::Generic(format!("Failed to set repeat: {e}")))?,
    }

    for (idx, (data, duration_ms)) in frames.into_iter().enumerate() {
        progress_callback(idx);

        let frame = GifFrame::new(data, duration_ms);
        encoder.encode_frame(&mut gif_encoder, &frame)?;
    }

    Ok(())
}
