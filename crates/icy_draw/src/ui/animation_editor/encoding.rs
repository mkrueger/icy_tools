//! Animation encoding for export

use std::path::Path;
use std::sync::mpsc::Sender;

/// Result type for encoding operations
pub type EncodingResult<T> = Result<T, EncodingError>;

/// Errors that can occur during encoding
#[derive(Debug, Clone)]
pub enum EncodingError {
    IoError(String),
    EncodingFailed(String),
    Cancelled,
}

impl std::fmt::Display for EncodingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodingError::IoError(e) => write!(f, "IO error: {}", e),
            EncodingError::EncodingFailed(e) => write!(f, "Encoding failed: {}", e),
            EncodingError::Cancelled => write!(f, "Encoding cancelled"),
        }
    }
}

impl std::error::Error for EncodingError {}

/// Trait for animation encoders
pub trait AnimationEncoder: Send + Sync {
    /// Get the display label for this encoder
    fn label(&self) -> &'static str;

    /// Get the file extension for this encoder
    fn extension(&self) -> &'static str;

    /// Encode the animation frames to a file
    fn encode(&self, path: &Path, frames: Vec<FrameData>, width: usize, height: usize, progress: Sender<usize>) -> EncodingResult<()>;
}

/// Data for a single animation frame
pub struct FrameData {
    /// Raw RGBA pixel data
    pub pixels: Vec<u8>,
    /// Frame delay in milliseconds
    pub delay_ms: u32,
}

/// GIF encoder
pub struct GifEncoder;

impl AnimationEncoder for GifEncoder {
    fn label(&self) -> &'static str {
        "GIF"
    }

    fn extension(&self) -> &'static str {
        "gif"
    }

    fn encode(&self, path: &Path, frames: Vec<FrameData>, width: usize, height: usize, progress: Sender<usize>) -> EncodingResult<()> {
        use std::fs::File;

        let file = File::create(path).map_err(|e| EncodingError::IoError(e.to_string()))?;

        let mut encoder = gif::Encoder::new(file, width as u16, height as u16, &[]).map_err(|e| EncodingError::EncodingFailed(e.to_string()))?;

        encoder
            .set_repeat(gif::Repeat::Infinite)
            .map_err(|e| EncodingError::EncodingFailed(e.to_string()))?;

        for (i, frame_data) in frames.iter().enumerate() {
            // Convert RGBA to indexed color (simplified - real impl would use color quantization)
            let mut indexed = Vec::with_capacity(width * height);
            let mut palette = Vec::new();
            let mut color_map = std::collections::HashMap::new();

            for pixel in frame_data.pixels.chunks(4) {
                let r = pixel[0];
                let g = pixel[1];
                let b = pixel[2];
                let color = (r, g, b);

                let index = if let Some(&idx) = color_map.get(&color) {
                    idx
                } else if palette.len() < 256 {
                    let idx = palette.len() as u8;
                    palette.push(r);
                    palette.push(g);
                    palette.push(b);
                    color_map.insert(color, idx);
                    idx
                } else {
                    // Find closest color (simplified)
                    0u8
                };

                indexed.push(index);
            }

            // Pad palette to power of 2
            while palette.len() < 3 * 256 {
                palette.push(0);
            }

            // Convert delay from ms to centiseconds
            let delay_cs = (frame_data.delay_ms / 10).max(1) as u16;

            let mut frame = gif::Frame::from_indexed_pixels(width as u16, height as u16, indexed, None);
            frame.delay = delay_cs;
            frame.palette = Some(palette);

            encoder.write_frame(&frame).map_err(|e| EncodingError::EncodingFailed(e.to_string()))?;

            let _ = progress.send(i + 1);
        }

        Ok(())
    }
}

/// PNG sequence encoder
pub struct PngSequenceEncoder;

impl AnimationEncoder for PngSequenceEncoder {
    fn label(&self) -> &'static str {
        "PNG Sequence"
    }

    fn extension(&self) -> &'static str {
        "png"
    }

    fn encode(&self, path: &Path, frames: Vec<FrameData>, width: usize, height: usize, progress: Sender<usize>) -> EncodingResult<()> {
        let base_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("frame");
        let parent = path.parent().unwrap_or(Path::new("."));

        for (i, frame_data) in frames.iter().enumerate() {
            let frame_path = parent.join(format!("{}_{:04}.png", base_name, i + 1));

            let img = image::RgbaImage::from_raw(width as u32, height as u32, frame_data.pixels.clone())
                .ok_or_else(|| EncodingError::EncodingFailed("Failed to create image".to_string()))?;

            img.save(&frame_path).map_err(|e| EncodingError::IoError(e.to_string()))?;

            let _ = progress.send(i + 1);
        }

        Ok(())
    }
}

/// Asciicast (asciinema) encoder for terminal recordings
pub struct AsciicastEncoder;

impl AnimationEncoder for AsciicastEncoder {
    fn label(&self) -> &'static str {
        "Asciicast"
    }

    fn extension(&self) -> &'static str {
        "cast"
    }

    fn encode(&self, path: &Path, frames: Vec<FrameData>, width: usize, height: usize, progress: Sender<usize>) -> EncodingResult<()> {
        use std::io::Write;

        let file = std::fs::File::create(path).map_err(|e| EncodingError::IoError(e.to_string()))?;
        let mut writer = std::io::BufWriter::new(file);

        // Write header
        writeln!(writer, r#"{{"version": 2, "width": {}, "height": {}, "timestamp": 0}}"#, width / 8, height / 16)
            .map_err(|e| EncodingError::IoError(e.to_string()))?;

        let mut time = 0.0;
        for (i, frame_data) in frames.iter().enumerate() {
            // This is a simplified version - real impl would output ANSI escape codes
            let delay_sec = frame_data.delay_ms as f64 / 1000.0;
            writeln!(writer, r#"[{}, "o", "Frame {}"]"#, time, i + 1).map_err(|e| EncodingError::IoError(e.to_string()))?;

            time += delay_sec;
            let _ = progress.send(i + 1);
        }

        Ok(())
    }
}

/// Get all available encoders
pub fn get_encoders() -> Vec<Box<dyn AnimationEncoder>> {
    vec![Box::new(GifEncoder), Box::new(PngSequenceEncoder), Box::new(AsciicastEncoder)]
}

/// Get encoder labels for UI display
pub fn get_encoder_labels() -> Vec<&'static str> {
    vec!["GIF", "PNG Sequence", "Asciicast"]
}

/// Get encoder extension by index
pub fn get_encoder_extension(index: usize) -> &'static str {
    match index {
        0 => "gif",
        1 => "png",
        2 => "cast",
        _ => "gif",
    }
}
