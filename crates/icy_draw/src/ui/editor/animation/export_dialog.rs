//! Export dialog for animation editor
//!
//! Provides a modal dialog for exporting animations to GIF or Asciicast format.
//! Supports async export with progress indication and cancellation.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use iced::{
    Alignment, Element, Length, Task,
    widget::{Space, column, container, pick_list, progress_bar, row, text, text_input},
};
use icy_engine::{Position, Rectangle, RenderOptions, Screen};
use icy_engine_gui::ButtonType;
use icy_engine_gui::ui::{
    DIALOG_SPACING, DIALOG_WIDTH_MEDIUM, Dialog, DialogAction, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL, browse_button, button_row, dialog_area, dialog_title,
    left_label_small, modal_container, primary_button, secondary_button, separator,
};
use icy_engine_scripting::Animator;
use parking_lot::Mutex;

use crate::fl;
use crate::ui::Message;
use super::AnimationEditorMessage;

/// Helper function to wrap export dialog messages
fn msg(m: AnimationExportMessage) -> Message {
    Message::AnimationEditor(AnimationEditorMessage::ExportDialog(m))
}

/// Export format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Gif,
    Mp4,
    Asciicast,
}

impl ExportFormat {
    pub fn all() -> &'static [ExportFormat] {
        &[ExportFormat::Gif, ExportFormat::Mp4, ExportFormat::Asciicast]
    }

    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Gif => "gif",
            ExportFormat::Mp4 => "mp4",
            ExportFormat::Asciicast => "cast",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ExportFormat::Gif => "GIF Animation",
            ExportFormat::Mp4 => "MP4 Video (H.264)",
            ExportFormat::Asciicast => "Asciicast v2",
        }
    }
}

impl std::fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Export progress state shared between dialog and background thread
pub struct ExportProgress {
    /// Current frame being processed
    pub current_frame: AtomicUsize,
    /// Total number of frames
    pub total_frames: AtomicUsize,
    /// Whether export should be cancelled
    pub cancelled: AtomicBool,
    /// Whether export is complete
    pub complete: AtomicBool,
    /// Error message if export failed
    pub error: Mutex<Option<String>>,
}

impl ExportProgress {
    pub fn new(total_frames: usize) -> Self {
        Self {
            current_frame: AtomicUsize::new(0),
            total_frames: AtomicUsize::new(total_frames),
            cancelled: AtomicBool::new(false),
            complete: AtomicBool::new(false),
            error: Mutex::new(None),
        }
    }
}

/// Messages for export dialog
#[derive(Debug, Clone)]
pub enum AnimationExportMessage {
    /// Set export format
    SetFormat(ExportFormat),
    /// Set export path (from text input)
    SetPath(String),
    /// Browse for export path
    Browse,
    /// Path selected from file dialog
    PathSelected(Option<PathBuf>),
    /// Start export
    Export,
    /// Export progress tick (poll for updates)
    Tick,
    /// Close dialog (also cancels export)
    Close,
}

/// Export dialog state
pub struct AnimationExportDialog {
    /// The animator to export from
    animator: Arc<Mutex<Animator>>,
    /// Selected export format
    format: ExportFormat,
    /// Export file path
    export_path: Option<PathBuf>,
    /// Error message if export failed
    error: Option<String>,
    /// Export progress (Some when exporting)
    progress: Option<Arc<ExportProgress>>,
}

impl AnimationExportDialog {
    /// Create a new export dialog
    pub fn new(animator: Arc<Mutex<Animator>>, source_path: Option<&PathBuf>) -> Self {
        let export_path = source_path.map(|p| p.with_extension("gif"));
        Self {
            animator,
            format: ExportFormat::Gif,
            export_path,
            error: None,
            progress: None,
        }
    }

    /// Update the path extension based on format
    fn update_extension(&mut self) {
        if let Some(ref mut path) = self.export_path {
            path.set_extension(self.format.extension());
        }
    }

    /// Check if currently exporting
    fn is_exporting(&self) -> bool {
        self.progress.is_some()
    }

    /// Start async export
    fn start_export(&mut self) -> Task<Message> {
        let Some(path) = self.export_path.clone() else {
            self.error = Some("No export path specified".to_string());
            return Task::none();
        };

        let frame_count = self.animator.lock().frames.len();
        if frame_count == 0 {
            self.error = Some("No frames to export".to_string());
            return Task::none();
        }

        // Create progress tracker
        let progress = Arc::new(ExportProgress::new(frame_count));
        self.progress = Some(progress.clone());
        self.error = None;

        // Clone what we need for the background thread
        let animator = self.animator.clone();
        let format = self.format;

        // Spawn background thread
        thread::spawn(move || {
            let result = match format {
                ExportFormat::Gif => export_to_gif_with_progress(&animator, &path, &progress),
                ExportFormat::Mp4 => export_to_mp4_with_progress(&animator, &path, &progress),
                ExportFormat::Asciicast => export_to_asciicast_with_progress(&animator, &path, &progress),
            };

            if let Err(e) = result {
                *progress.error.lock() = Some(e);
            }
            progress.complete.store(true, Ordering::Relaxed);
        });

        // Start polling for progress updates
        self.create_tick_task()
    }

    /// Create a task that polls for progress updates
    fn create_tick_task(&self) -> Task<Message> {
        Task::perform(
            async {
                // Use tokio sleep which doesn't block the UI thread
                tokio::time::sleep(Duration::from_millis(50)).await;
            },
            |_| msg(AnimationExportMessage::Tick),
        )
    }
}

impl Dialog<Message> for AnimationExportDialog {
    fn view(&self) -> Element<'_, Message> {
        let title = dialog_title(fl!("menu-export").trim_end_matches('…').to_string());

        // Format selection
        let format_label = left_label_small(fl!("animation-export-format"));
        let format_picker = pick_list(ExportFormat::all(), Some(self.format), |f| {
            msg(AnimationExportMessage::SetFormat(f))
        })
        .width(Length::Fill);

        let format_row = row![format_label, format_picker].spacing(DIALOG_SPACING).align_y(Alignment::Center);

        // Path input
        let path_label = left_label_small(fl!("animation-export-path"));
        let path_text = self.export_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();

        let path_input = text_input(&fl!("animation-export-no-path"), &path_text)
            .on_input(|s| msg(AnimationExportMessage::SetPath(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        let browse_btn = browse_button(msg(AnimationExportMessage::Browse));

        let file_row = row![path_label, path_input, browse_btn].spacing(DIALOG_SPACING).align_y(Alignment::Center);

        // Progress or error message
        let status_element: Element<'_, Message> = if let Some(ref progress) = self.progress {
            let current = progress.current_frame.load(Ordering::Relaxed);
            let total = progress.total_frames.load(Ordering::Relaxed);
            let percentage = if total > 0 { current as f32 / total as f32 } else { 0.0 };

            column![
                row![
                    text(fl!("animation-export-exporting-frame", current = (current as i32), total = (total as i32))).size(TEXT_SIZE_SMALL),
                    Space::new().width(Length::Fill),
                    text(format!("{}%", (percentage * 100.0) as u32)).size(TEXT_SIZE_SMALL),
                ]
                .align_y(Alignment::Center),
                container(progress_bar(0.0..=1.0, percentage)).height(Length::Fixed(8.0)),
            ]
            .spacing(4)
            .into()
        } else if let Some(ref err) = self.error {
            text(err)
                .size(TEXT_SIZE_SMALL)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().danger.base.color),
                })
                .into()
        } else {
            Space::new().height(Length::Fixed(0.0)).into()
        };

        // Content
        let frame_count = self.animator.lock().frames.len();
        let is_exporting = self.is_exporting();

        let content_column = if is_exporting {
            // When exporting, show only progress
            column![status_element].spacing(DIALOG_SPACING)
        } else {
            column![format_row, file_row, status_element].spacing(DIALOG_SPACING)
        };

        // Buttons
        let buttons = if is_exporting {
            // Show cancel button during export
            button_row(vec![
                secondary_button(format!("{}", ButtonType::Cancel), Some(msg(AnimationExportMessage::Close))).into(),
            ])
        } else {
            let can_export = self.export_path.is_some() && frame_count > 0;
            button_row(vec![
                secondary_button(format!("{}", ButtonType::Cancel), Some(msg(AnimationExportMessage::Close))).into(),
                primary_button(fl!("menu-export"), can_export.then(|| msg(AnimationExportMessage::Export))).into(),
            ])
        };

        let dialog_content = dialog_area(column![title, Space::new().height(Length::Fixed(DIALOG_SPACING as f32)), content_column].into());
        let button_area = dialog_area(buttons.into());

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area].into(),
            DIALOG_WIDTH_MEDIUM,
        )
        .into()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        if let Message::AnimationEditor(AnimationEditorMessage::ExportDialog(dialog_msg)) = message {
            match dialog_msg {
                AnimationExportMessage::SetFormat(format) => {
                    if !self.is_exporting() {
                        self.format = *format;
                        self.update_extension();
                        self.error = None;
                    }
                    Some(DialogAction::None)
                }
                AnimationExportMessage::SetPath(path) => {
                    if !self.is_exporting() {
                        self.export_path = Some(PathBuf::from(path));
                        self.error = None;
                    }
                    Some(DialogAction::None)
                }
                AnimationExportMessage::Browse => {
                    if self.is_exporting() {
                        return Some(DialogAction::None);
                    }

                    let format = self.format;
                    let initial_path = self.export_path.clone();

                    let task = Task::perform(
                        async move {
                            let (filter_name, ext) = match format {
                                ExportFormat::Gif => ("GIF Animation", "gif"),
                                ExportFormat::Mp4 => ("MP4 Video", "mp4"),
                                ExportFormat::Asciicast => ("Asciicast", "cast"),
                            };

                            let mut dialog = rfd::AsyncFileDialog::new().add_filter(filter_name, &[ext]).set_title("Export Animation");

                            if let Some(ref path) = initial_path {
                                if let Some(parent) = path.parent() {
                                    dialog = dialog.set_directory(parent);
                                }
                                if let Some(name) = path.file_name() {
                                    dialog = dialog.set_file_name(name.to_string_lossy());
                                }
                            }

                            dialog.save_file().await.map(|f| f.path().to_path_buf())
                        },
                        |result| msg(AnimationExportMessage::PathSelected(result)),
                    );

                    Some(DialogAction::RunTask(task))
                }
                AnimationExportMessage::PathSelected(path) => {
                    if let Some(p) = path {
                        self.export_path = Some(p.clone());
                    }
                    Some(DialogAction::None)
                }
                AnimationExportMessage::Export => {
                    let task = self.start_export();
                    Some(DialogAction::RunTask(task))
                }
                AnimationExportMessage::Tick => {
                    // Check if export is still running
                    let is_complete = self.progress.as_ref().map(|p| p.complete.load(Ordering::Relaxed)).unwrap_or(false);

                    if is_complete {
                        // Export finished, check for error
                        let error = self.progress.as_ref().and_then(|p| p.error.lock().take());
                        if let Some(err) = error {
                            self.error = Some(err);
                            self.progress = None;
                            return Some(DialogAction::None);
                        }
                        // Success - close dialog
                        self.progress = None;
                        return Some(DialogAction::Close);
                    } else if self.progress.is_some() {
                        // Still running, schedule another tick
                        return Some(DialogAction::RunTask(self.create_tick_task()));
                    }
                    Some(DialogAction::None)
                }
                AnimationExportMessage::Close => {
                    // Cancel export if running
                    if let Some(ref progress) = self.progress {
                        progress.cancelled.store(true, Ordering::Relaxed);
                    }
                    Some(DialogAction::Close)
                }
            }
        } else {
            None
        }
    }
}

/// Export animation frames to GIF with progress tracking
fn export_to_gif_with_progress(animator: &Arc<Mutex<Animator>>, path: &PathBuf, progress: &Arc<ExportProgress>) -> Result<(), String> {
    use icy_engine::gif_encoder::{GifEncoder, GifFrame, RepeatCount};

    let animator_guard = animator.lock();

    if animator_guard.frames.is_empty() {
        return Err("No frames to export".to_string());
    }

    let frame_count = animator_guard.frames.len();
    // Only encoding phase counts for progress (rendering is fast)
    progress.total_frames.store(frame_count, Ordering::Relaxed);
    progress.current_frame.store(0, Ordering::Relaxed);

    // Render all frames to RGBA (fast, no progress needed)
    let mut gif_frames: Vec<GifFrame> = Vec::with_capacity(frame_count);
    let mut width: u16 = 0;
    let mut height: u16 = 0;

    for (i, (screen, _settings, delay_ms)) in animator_guard.frames.iter().enumerate() {
        // Check for cancellation
        if progress.cancelled.load(Ordering::Relaxed) {
            return Err("Export cancelled".to_string());
        }

        // Create render options with rect covering the entire screen
        let screen_width = screen.width();
        let screen_height = screen.height();
        let full_rect = Rectangle::from_coords(0, 0, screen_width, screen_height);
        let options = RenderOptions {
            rect: full_rect.into(),
            blink_on: true,
            ..Default::default()
        };

        let (render_size, rgba_data) = screen.render_to_rgba(&options);

        // Use actual rendered dimensions from first frame
        if i == 0 {
            width = render_size.width as u16;
            height = render_size.height as u16;
        }

        gif_frames.push(GifFrame::new(rgba_data, *delay_ms));
    }

    // Drop the animator lock before encoding
    drop(animator_guard);

    if width == 0 || height == 0 {
        return Err("Invalid frame dimensions".to_string());
    }

    // Check for cancellation before encoding
    if progress.cancelled.load(Ordering::Relaxed) {
        return Err("Export cancelled".to_string());
    }

    // Encode GIF with progress callback
    let mut encoder = GifEncoder::new(width, height);
    encoder.set_repeat(RepeatCount::Infinite);

    let progress_ref = progress.clone();
    encoder
        .encode_to_file_with_progress(
            path,
            gif_frames,
            move |current, _total| {
                progress_ref.current_frame.store(current, Ordering::Relaxed);
            },
            || progress.cancelled.load(Ordering::Relaxed),
        )
        .map_err(|e| format!("GIF encoding failed: {}", e))
}

/// Export animation frames to MP4 (H.264) with progress tracking
fn export_to_mp4_with_progress(animator: &Arc<Mutex<Animator>>, path: &PathBuf, progress: &Arc<ExportProgress>) -> Result<(), String> {
    use minimp4::Mp4Muxer;
    use openh264::OpenH264API;
    use openh264::encoder::{Encoder, EncoderConfig, FrameRate, UsageType};
    use openh264::formats::YUVBuffer;
    use std::fs::File;
    use std::io::BufWriter;

    let animator_guard = animator.lock();

    if animator_guard.frames.is_empty() {
        return Err("No frames to export".to_string());
    }

    let frame_count = animator_guard.frames.len();
    progress.total_frames.store(frame_count, Ordering::Relaxed);
    progress.current_frame.store(0, Ordering::Relaxed);

    // Get dimensions from first frame
    let first_frame = &animator_guard.frames[0].0;
    let screen_width = first_frame.width();
    let screen_height = first_frame.height();
    let full_rect = Rectangle::from_coords(0, 0, screen_width, screen_height);
    let options = RenderOptions {
        rect: full_rect.into(),
        blink_on: true,
        ..Default::default()
    };

    let (render_size, _) = first_frame.render_to_rgba(&options);
    // Ensure dimensions are even (required for H.264)
    let width = (render_size.width as usize + 1) & !1;
    let height = (render_size.height as usize + 1) & !1;

    // Calculate average FPS from frame delays
    let total_delay_ms: u32 = animator_guard.frames.iter().map(|(_, _, delay)| *delay).sum();
    let avg_delay_ms = if frame_count > 0 { total_delay_ms / frame_count as u32 } else { 100 };
    let fps = (1000.0 / avg_delay_ms as f64).round() as u32;
    let fps = fps.clamp(1, 60);

    // Create H.264 encoder (openh264 0.9 API)
    // Width/height are taken from the YUV source passed to `encode()`.
    let api = OpenH264API::from_source();
    let config = EncoderConfig::new()
        .usage_type(UsageType::ScreenContentRealTime)
        .max_frame_rate(FrameRate::from_hz(fps as f32));
    let mut h264_encoder = Encoder::with_api_config(api, config).map_err(|e| format!("Failed to create H.264 encoder: {:?}", e))?;

    // Create MP4 file
    let file = File::create(path).map_err(|e| format!("Failed to create MP4 file: {}", e))?;
    let writer = BufWriter::new(file);
    let mut mp4_muxer = Mp4Muxer::new(writer);
    mp4_muxer.init_video(width as i32, height as i32, false, "icy_draw animation");

    // Encode each frame
    for (i, (screen, _settings, _delay_ms)) in animator_guard.frames.iter().enumerate() {
        // Check for cancellation
        if progress.cancelled.load(Ordering::Relaxed) {
            return Err("Export cancelled".to_string());
        }

        progress.current_frame.store(i + 1, Ordering::Relaxed);

        // Render frame to RGBA
        let full_rect = Rectangle::from_coords(0, 0, screen.width(), screen.height());
        let options = RenderOptions {
            rect: full_rect.into(),
            blink_on: true,
            ..Default::default()
        };
        let (_, rgba_data) = screen.render_to_rgba(&options);

        // Convert RGBA to YUV (I420) and encode frame
        let yuv_i420 = rgba_to_yuv420(&rgba_data, width, height);
        let yuv = YUVBuffer::from_vec(yuv_i420, width, height);

        let bitstream = h264_encoder.encode(&yuv).map_err(|e| format!("H.264 encoding failed: {:?}", e))?;

        // Write NAL units to MP4
        let nal_data = bitstream.to_vec();
        if !nal_data.is_empty() {
            mp4_muxer.write_video_with_fps(&nal_data, fps);
        }
    }

    // Drop the animator lock before finalizing
    drop(animator_guard);

    // Finalize MP4 file
    mp4_muxer.close();

    Ok(())
}

/// Convert RGBA image data to YUV420 (I420) format
fn rgba_to_yuv420(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    let y_size = width * height;
    let uv_size = (width / 2) * (height / 2);

    let mut y_plane = vec![0u8; y_size];
    let mut u_plane = vec![0u8; uv_size];
    let mut v_plane = vec![0u8; uv_size];

    // Convert each pixel to Y
    for y in 0..height {
        for x in 0..width {
            let rgba_idx = (y * width + x) * 4;
            if rgba_idx + 2 < rgba.len() {
                let r = rgba[rgba_idx] as f32;
                let g = rgba[rgba_idx + 1] as f32;
                let b = rgba[rgba_idx + 2] as f32;

                // BT.601 conversion
                let y_val = (0.299 * r + 0.587 * g + 0.114 * b).clamp(0.0, 255.0) as u8;
                y_plane[y * width + x] = y_val;
            }
        }
    }

    // Convert 2x2 blocks to U and V (subsampled)
    for y in (0..height).step_by(2) {
        for x in (0..width).step_by(2) {
            let mut r_sum = 0.0f32;
            let mut g_sum = 0.0f32;
            let mut b_sum = 0.0f32;
            let mut count = 0.0f32;

            // Average 2x2 block
            for dy in 0..2 {
                for dx in 0..2 {
                    let px = x + dx;
                    let py = y + dy;
                    if px < width && py < height {
                        let rgba_idx = (py * width + px) * 4;
                        if rgba_idx + 2 < rgba.len() {
                            r_sum += rgba[rgba_idx] as f32;
                            g_sum += rgba[rgba_idx + 1] as f32;
                            b_sum += rgba[rgba_idx + 2] as f32;
                            count += 1.0;
                        }
                    }
                }
            }

            if count > 0.0 {
                let r = r_sum / count;
                let g = g_sum / count;
                let b = b_sum / count;

                // BT.601 conversion for U and V
                let u_val = (128.0 + (-0.169 * r - 0.331 * g + 0.500 * b)).clamp(0.0, 255.0) as u8;
                let v_val = (128.0 + (0.500 * r - 0.419 * g - 0.081 * b)).clamp(0.0, 255.0) as u8;

                let uv_idx = (y / 2) * (width / 2) + (x / 2);
                u_plane[uv_idx] = u_val;
                v_plane[uv_idx] = v_val;
            }
        }
    }

    let mut out = Vec::with_capacity(y_size + uv_size * 2);
    out.extend_from_slice(&y_plane);
    out.extend_from_slice(&u_plane);
    out.extend_from_slice(&v_plane);
    out
}

/// Export animation frames to Asciicast v2 format with progress tracking
fn export_to_asciicast_with_progress(animator: &Arc<Mutex<Animator>>, path: &PathBuf, progress: &Arc<ExportProgress>) -> Result<(), String> {
    use std::io::Write;

    let animator = animator.lock();

    if animator.frames.is_empty() {
        return Err("No frames to export".to_string());
    }

    let first_frame = &animator.frames[0].0;
    let size = first_frame.size();

    let mut file = std::fs::File::create(path).map_err(|e| format!("Failed to create file: {}", e))?;

    // Write header (Asciicast v2 format)
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let header = serde_json::json!({
        "version": 2,
        "width": size.width,
        "height": size.height,
        "timestamp": timestamp,
        "title": path.file_stem().and_then(|s| s.to_str()).unwrap_or("animation"),
        "env": {
            "TERM": "xterm-256color",
            "SHELL": "/bin/bash"
        }
    });

    writeln!(file, "{}", header).map_err(|e| format!("Failed to write header: {}", e))?;

    // Write frames
    let mut time_offset = 0.0;

    for (i, (screen, _settings, delay_ms)) in animator.frames.iter().enumerate() {
        // Check for cancellation
        if progress.cancelled.load(Ordering::Relaxed) {
            return Err("Export cancelled".to_string());
        }

        // Update progress
        progress.current_frame.store(i + 1, Ordering::Relaxed);

        // Render frame to ANSI escape sequences
        let ansi_output = render_screen_to_ansi(screen.as_ref());

        // Clear screen and move cursor to home
        let frame_data = format!("\x1b[2J\x1b[H{}", ansi_output);

        // Write event as JSON array: [time, "o", data]
        let escaped = serde_json::to_string(&frame_data).map_err(|e| format!("Failed to encode frame: {}", e))?;
        writeln!(file, "[{:.6}, \"o\", {}]", time_offset, escaped).map_err(|e| format!("Failed to write frame: {}", e))?;

        time_offset += *delay_ms as f64 / 1000.0;
    }

    Ok(())
}

/// Render a screen to ANSI escape sequences
fn render_screen_to_ansi(screen: &dyn Screen) -> String {
    let mut result = String::new();
    let width = screen.width();
    let height = screen.height();

    let mut last_attr = icy_engine::TextAttribute::default();

    for y in 0..height {
        for x in 0..width {
            let ch = screen.char_at(Position::new(x, y));

            // Check if attributes changed
            if ch.attribute != last_attr {
                // Reset and set new attributes
                result.push_str("\x1b[0m");

                let fg = ch.attribute.foreground();
                let bg = ch.attribute.background();

                // Set foreground color (256 color mode)
                result.push_str(&format!("\x1b[38;5;{}m", fg));

                // Set background color (256 color mode)
                result.push_str(&format!("\x1b[48;5;{}m", bg));

                if ch.attribute.is_bold() {
                    result.push_str("\x1b[1m");
                }
                if ch.attribute.is_blinking() {
                    result.push_str("\x1b[5m");
                }

                last_attr = ch.attribute;
            }

            // Output character
            let c = if ch.ch == '\0' || ch.ch == ' ' { ' ' } else { ch.ch };
            result.push(c);
        }
        result.push_str("\r\n");
    }

    result.push_str("\x1b[0m");
    result
}
