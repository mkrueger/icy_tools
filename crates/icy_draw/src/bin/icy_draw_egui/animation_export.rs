//! Animation export to GIF, AV1 video (IVF container) and Asciicast v2.

use icy_draw::fl;
use icy_engine::Screen;
use icy_engine_scripting::Animator;
use parking_lot::Mutex;
use rayon::prelude::*;
use std::{
    io::Write,
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Gif,
    Av1,
    Cast,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 3] = [ExportFormat::Gif, ExportFormat::Av1, ExportFormat::Cast];

    pub fn extension(self) -> &'static str {
        match self {
            Self::Gif => "gif",
            Self::Av1 => "ivf",
            Self::Cast => "cast",
        }
    }

    pub fn name(self) -> String {
        match self {
            Self::Gif => fl!("animation-format-gif"),
            Self::Av1 => fl!("animation-format-av1"),
            Self::Cast => fl!("animation-format-cast"),
        }
    }
}

/// Shared between the export thread and the dialog showing its progress.
#[derive(Default)]
pub struct ExportProgress {
    /// Frames rendered (or written) so far.
    pub frame: AtomicUsize,
    pub total: AtomicUsize,
    /// Set once all frames are handed to the video encoder, which then takes a while on its own.
    pub encoding: AtomicBool,
    pub cancelled: AtomicBool,
}

impl ExportProgress {
    fn cancelled(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err(fl!("animation-export-cancelled"))
        } else {
            Ok(())
        }
    }
}

/// Writes all frames to `path`. A temporary file is used, so a cancelled or failed export leaves an
/// existing target untouched.
pub fn export_frames(animator: &Arc<Mutex<Animator>>, path: &Path, format: ExportFormat, progress: &Arc<ExportProgress>) -> Result<(), String> {
    let frames: Vec<_> = animator.lock().frames.iter().map(|(screen, _, delay)| (screen.clone_box(), *delay)).collect();
    if frames.is_empty() {
        return Err(fl!("animation-export-no-frames"));
    }
    progress.total.store(frames.len(), Ordering::Relaxed);
    progress.frame.store(0, Ordering::Relaxed);
    let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    match format {
        ExportFormat::Gif => export_gif(&frames, temporary.path(), progress)?,
        ExportFormat::Av1 => export_av1(&frames, temporary.as_file_mut(), progress)?,
        ExportFormat::Cast => export_cast(frames, temporary.as_file_mut(), progress)?,
    }
    progress.cancelled()?;
    temporary.as_file().sync_all().map_err(|error| error.to_string())?;
    temporary.persist(path).map_err(|error| error.to_string())?;
    Ok(())
}

fn render(screen: &dyn Screen) -> (icy_engine::Size, Vec<u8>) {
    let options = icy_engine::RenderOptions {
        rect: icy_engine::Rectangle::from_coords(0, 0, screen.width(), screen.height()).into(),
        blink_on: true,
        ..Default::default()
    };
    screen.render_to_rgba(&options)
}

fn export_gif(frames: &[(Box<dyn Screen>, u32)], path: &Path, progress: &Arc<ExportProgress>) -> Result<(), String> {
    use icy_engine::gif_encoder::{GifEncoder, GifFrame, RepeatCount};
    let mut images = Vec::with_capacity(frames.len());
    let mut dimensions = None;
    for (screen, delay) in frames {
        progress.cancelled()?;
        let (size, pixels) = render(screen.as_ref());
        if size.width <= 0 || size.height <= 0 || size.width > u16::MAX as i32 || size.height > u16::MAX as i32 {
            return Err("Invalid GIF frame dimensions".into());
        }
        if dimensions.is_some_and(|previous| previous != size) {
            return Err("GIF export requires frames of equal size.".into());
        }
        dimensions = Some(size);
        images.push(GifFrame::new(pixels, *delay));
    }
    let size = dimensions.unwrap();
    let mut encoder = GifEncoder::new(size.width as u16, size.height as u16);
    encoder.set_repeat(RepeatCount::Infinite);
    let counter = progress.clone();
    encoder
        .encode_to_file_with_progress(
            path,
            images,
            move |current, _| counter.frame.store(current, Ordering::Relaxed),
            || progress.cancelled.load(Ordering::Relaxed),
        )
        .map_err(|error| error.to_string())
}

fn export_cast(mut frames: Vec<(Box<dyn Screen>, u32)>, file: &mut std::fs::File, progress: &Arc<ExportProgress>) -> Result<(), String> {
    let header = serde_json::json!({ "version": 2, "width": frames[0].0.width(), "height": frames[0].0.height() });
    writeln!(file, "{header}").map_err(|error| error.to_string())?;
    let mut timestamp = 0.0;
    for (index, (screen, delay)) in frames.iter_mut().enumerate() {
        progress.cancelled()?;
        let options = icy_engine::SaveOptions::ansi(icy_engine::AnsiCompatibilityLevel::Utf8Terminal);
        let bytes = screen.to_bytes("ans", &options).map_err(|error| error.to_string())?;
        let text = String::from_utf8(bytes).map_err(|error| error.to_string())?;
        let event = serde_json::json!([timestamp, "o", format!("\u{001b}[2J\u{001b}[H{text}")]);
        writeln!(file, "{event}").map_err(|error| error.to_string())?;
        timestamp += f64::from(*delay) / 1000.0;
        progress.frame.store(index + 1, Ordering::Relaxed);
    }
    Ok(())
}

/// AV1 in an IVF container, at the frame rate of the average frame delay. Frames are sized like
/// the first one, padded to even dimensions as 4:2:0 chroma requires.
fn export_av1(frames: &[(Box<dyn Screen>, u32)], file: &mut std::fs::File, progress: &Arc<ExportProgress>) -> Result<(), String> {
    use rav1e::prelude::*;
    let rendered: Vec<_> = frames
        .par_iter()
        .map(|(screen, _)| {
            if progress.cancelled.load(Ordering::Relaxed) {
                return None;
            }
            let (size, pixels) = render(screen.as_ref());
            Some((pixels, size.width.max(1) as usize, size.height.max(1) as usize))
        })
        .collect::<Option<_>>()
        .ok_or_else(|| fl!("animation-export-cancelled"))?;
    let (_, first_width, first_height) = rendered[0];
    let width = first_width.next_multiple_of(2);
    let height = first_height.next_multiple_of(2);
    if width > usize::from(u16::MAX) || height > usize::from(u16::MAX) {
        return Err("Invalid video frame dimensions".into());
    }
    let total_delay: u64 = frames.iter().map(|(_, delay)| u64::from(*delay)).sum();
    let fps = ((1000.0 * frames.len() as f64 / total_delay.max(1) as f64).round() as u64).clamp(1, 60);
    let config = EncoderConfig {
        width,
        height,
        speed_settings: SpeedSettings::from_preset(10),
        time_base: Rational::new(1, fps),
        bit_depth: 8,
        chroma_sampling: ChromaSampling::Cs420,
        pixel_range: PixelRange::Full,
        min_key_frame_interval: 0,
        max_key_frame_interval: frames.len() as u64,
        low_latency: true,
        quantizer: 100,
        min_quantizer: 0,
        tune: Tune::Psychovisual,
        ..Default::default()
    };
    let mut context: Context<u8> = Config::new()
        .with_encoder_config(config)
        .with_threads(num_cpus::get())
        .new_context()
        .map_err(|error| format!("Failed to create the AV1 encoder: {error:?}"))?;

    let mut header = [0u8; 32];
    header[0..4].copy_from_slice(b"DKIF");
    header[6..8].copy_from_slice(&32u16.to_le_bytes());
    header[8..12].copy_from_slice(b"AV01");
    header[12..14].copy_from_slice(&(width as u16).to_le_bytes());
    header[14..16].copy_from_slice(&(height as u16).to_le_bytes());
    header[16..20].copy_from_slice(&(fps as u32).to_le_bytes());
    header[20..24].copy_from_slice(&1u32.to_le_bytes());
    header[24..28].copy_from_slice(&(frames.len() as u32).to_le_bytes());
    file.write_all(&header).map_err(|error| error.to_string())?;

    for (index, (pixels, source_width, source_height)) in rendered.into_iter().enumerate() {
        progress.cancelled()?;
        let mut frame = context.new_frame();
        let rgb = |x: usize, y: usize| {
            let offset = (y.min(source_height - 1) * source_width + x.min(source_width - 1)) * 4;
            let channel = |index: usize| i32::from(pixels.get(offset + index).copied().unwrap_or(0));
            (channel(0), channel(1), channel(2))
        };
        // BT.601 with integer arithmetic; chroma averages each 2×2 block.
        let stride = frame.planes[0].cfg.stride;
        let luma = frame.planes[0].data_origin_mut();
        for y in 0..height {
            for x in 0..width {
                let (red, green, blue) = rgb(x, y);
                luma[y * stride + x] = ((77 * red + 150 * green + 29 * blue) >> 8) as u8;
            }
        }
        let (stride_u, stride_v) = (frame.planes[1].cfg.stride, frame.planes[2].cfg.stride);
        let (_, chroma) = frame.planes.split_at_mut(1);
        let (u_plane, v_plane) = chroma.split_at_mut(1);
        let (u_plane, v_plane) = (u_plane[0].data_origin_mut(), v_plane[0].data_origin_mut());
        for y in 0..height / 2 {
            for x in 0..width / 2 {
                let (mut red, mut green, mut blue) = (0, 0, 0);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (r, g, b) = rgb(x * 2 + dx, y * 2 + dy);
                    red += r;
                    green += g;
                    blue += b;
                }
                let (red, green, blue) = (red >> 2, green >> 2, blue >> 2);
                u_plane[y * stride_u + x] = (128 + ((-43 * red - 85 * green + 128 * blue) >> 8)).clamp(0, 255) as u8;
                v_plane[y * stride_v + x] = (128 + ((128 * red - 107 * green - 21 * blue) >> 8)).clamp(0, 255) as u8;
            }
        }
        context
            .send_frame(frame)
            .map_err(|error| format!("Failed to encode frame {index}: {error:?}"))?;
        progress.frame.store(index + 1, Ordering::Relaxed);
    }
    context.flush();
    progress.encoding.store(true, Ordering::Relaxed);
    let mut timestamp = 0u64;
    loop {
        progress.cancelled()?;
        match context.receive_packet() {
            Ok(packet) => {
                file.write_all(&(packet.data.len() as u32).to_le_bytes())
                    .and_then(|()| file.write_all(&timestamp.to_le_bytes()))
                    .and_then(|()| file.write_all(&packet.data))
                    .map_err(|error| error.to_string())?;
                timestamp += 1;
            }
            Err(EncoderStatus::Encoded) => {}
            Err(EncoderStatus::LimitReached) => break,
            Err(error) => return Err(format!("AV1 encoding failed: {error:?}")),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn animator(frames: usize) -> Arc<Mutex<Animator>> {
        let animator = Arc::new(Mutex::new(Animator::default()));
        for _ in 0..frames {
            animator.lock().frames.push((
                Box::new(icy_engine::TextScreen::new((10, 3))),
                icy_engine_scripting::MonitorSettings::neutral(),
                100,
            ));
        }
        animator
    }

    #[test]
    fn exports_every_format_and_keeps_target_on_cancellation() {
        let animator = animator(2);
        let directory = tempfile::tempdir().unwrap();
        let progress = Arc::new(ExportProgress::default());
        let gif = directory.path().join("test.gif");
        export_frames(&animator, &gif, ExportFormat::Gif, &progress).unwrap();
        assert_eq!(image::open(gif).unwrap().width(), 80);

        let ivf = directory.path().join("test.ivf");
        export_frames(&animator, &ivf, ExportFormat::Av1, &progress).unwrap();
        let video = std::fs::read(&ivf).unwrap();
        assert_eq!(&video[0..4], b"DKIF");
        assert_eq!(&video[8..12], b"AV01");
        assert_eq!(u16::from_le_bytes([video[12], video[13]]), 80);
        assert_eq!(u32::from_le_bytes(video[24..28].try_into().unwrap()), 2);
        assert!(video.len() > 32 + 12, "the video carries encoded frames");
        assert!(progress.encoding.load(Ordering::Relaxed));

        let cast = directory.path().join("test.cast");
        export_frames(&animator, &cast, ExportFormat::Cast, &progress).unwrap();
        let original = std::fs::read(&cast).unwrap();
        let events: Vec<serde_json::Value> = std::str::from_utf8(&original)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0]["version"], 2);
        assert_eq!(events[2][0], 0.1);
        assert_eq!(progress.frame.load(Ordering::Relaxed), 2);

        progress.cancelled.store(true, Ordering::Relaxed);
        for (path, format) in [(&cast, ExportFormat::Cast), (&ivf, ExportFormat::Av1)] {
            let before = std::fs::read(path).unwrap();
            assert!(export_frames(&animator, path, format, &progress).is_err());
            assert_eq!(std::fs::read(path).unwrap(), before);
        }
        assert!(export_frames(&self::animator(0), &cast, ExportFormat::Gif, &Arc::new(ExportProgress::default())).is_err());
    }
}
