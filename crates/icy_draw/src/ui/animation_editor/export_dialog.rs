//! Export dialog for animation editor
//!
//! Provides a modal dialog for exporting animations to GIF or Asciicast format.

use std::path::PathBuf;
use std::sync::Arc;

use iced::{
    Alignment, Element, Length,
    widget::{Space, column, container, pick_list, row, text, text_input},
};
use icy_engine::{
    Position, RenderOptions, Screen,
    gif_encoder::{GifEncoder, GifFrame, RepeatCount},
};
use icy_engine_gui::ButtonType;
use icy_engine_gui::ui::{
    DIALOG_SPACING, DIALOG_WIDTH_MEDIUM, Dialog, DialogAction, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL, browse_button, button_row, dialog_area, dialog_title,
    left_label_small, modal_container, primary_button, secondary_button, separator,
};
use icy_engine_scripting::Animator;
use parking_lot::Mutex;

use crate::fl;
use crate::ui::Message;

/// Export format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Gif,
    Asciicast,
}

impl ExportFormat {
    pub fn all() -> &'static [ExportFormat] {
        &[ExportFormat::Gif, ExportFormat::Asciicast]
    }

    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Gif => "gif",
            ExportFormat::Asciicast => "cast",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ExportFormat::Gif => "GIF Animation",
            ExportFormat::Asciicast => "Asciicast v2",
        }
    }
}

impl std::fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
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
    /// Close dialog
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
    /// Success message
    success: Option<String>,
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
            success: None,
        }
    }

    /// Update the path extension based on format
    fn update_extension(&mut self) {
        if let Some(ref mut path) = self.export_path {
            path.set_extension(self.format.extension());
        }
    }

    /// Perform the export
    fn do_export(&mut self) -> Result<(), String> {
        let path = self.export_path.as_ref().ok_or("No export path specified")?;

        match self.format {
            ExportFormat::Gif => export_to_gif(&self.animator, path),
            ExportFormat::Asciicast => export_to_asciicast(&self.animator, path),
        }
    }
}

impl Dialog<Message> for AnimationExportDialog {
    fn view(&self) -> Element<'_, Message> {
        let title = dialog_title(fl!("menu-export").trim_end_matches('…').to_string());

        // Format selection
        let format_label = left_label_small(fl!("animation-export-format"));
        let format_picker = pick_list(ExportFormat::all(), Some(self.format), |f| {
            Message::AnimationExport(AnimationExportMessage::SetFormat(f))
        })
        .width(Length::Fill);

        let format_row = row![format_label, format_picker].spacing(DIALOG_SPACING).align_y(Alignment::Center);

        // Path input
        let path_label = left_label_small(fl!("animation-export-path"));
        let path_text = self.export_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();

        let path_input = text_input(&fl!("animation-export-no-path"), &path_text)
            .on_input(|s| Message::AnimationExport(AnimationExportMessage::SetPath(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        let browse_btn = browse_button(Message::AnimationExport(AnimationExportMessage::Browse));

        let file_row = row![path_label, path_input, browse_btn].spacing(DIALOG_SPACING).align_y(Alignment::Center);

        // Error/Success message
        let message_element: Element<'_, Message> = if let Some(ref err) = self.error {
            text(err)
                .size(TEXT_SIZE_SMALL)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().danger.base.color),
                })
                .into()
        } else if let Some(ref success) = self.success {
            text(success)
                .size(TEXT_SIZE_SMALL)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().success.base.color),
                })
                .into()
        } else {
            Space::new().height(0).into()
        };

        // Frame count info
        let frame_count = self.animator.lock().frames.len();
        let info_text = text(format!("{} frames", frame_count)).size(TEXT_SIZE_SMALL);

        // Content
        let content_column = column![format_row, file_row, Space::new().height(DIALOG_SPACING), info_text, message_element,].spacing(DIALOG_SPACING);

        // Buttons
        let can_export = self.export_path.is_some() && frame_count > 0;
        let buttons = button_row(vec![
            secondary_button(format!("{}", ButtonType::Cancel), Some(Message::AnimationExport(AnimationExportMessage::Close))).into(),
            primary_button(fl!("menu-export"), can_export.then(|| Message::AnimationExport(AnimationExportMessage::Export))).into(),
        ]);

        let dialog_content = dialog_area(column![title, Space::new().height(DIALOG_SPACING), content_column].into());
        let button_area = dialog_area(buttons.into());

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area].into(),
            DIALOG_WIDTH_MEDIUM,
        )
        .into()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        if let Message::AnimationExport(msg) = message {
            match msg {
                AnimationExportMessage::SetFormat(format) => {
                    self.format = *format;
                    self.update_extension();
                    self.error = None;
                    self.success = None;
                    Some(DialogAction::None)
                }
                AnimationExportMessage::SetPath(path) => {
                    self.export_path = Some(PathBuf::from(path));
                    self.error = None;
                    self.success = None;
                    Some(DialogAction::None)
                }
                AnimationExportMessage::Browse => {
                    // TODO: Open file dialog
                    Some(DialogAction::None)
                }
                AnimationExportMessage::PathSelected(path) => {
                    if let Some(p) = path {
                        self.export_path = Some(p.clone());
                    }
                    Some(DialogAction::None)
                }
                AnimationExportMessage::Export => {
                    match self.do_export() {
                        Ok(()) => {
                            self.success = Some(fl!("animation-export-success"));
                            // Keep dialog open to show success, user can close manually
                            Some(DialogAction::None)
                        }
                        Err(e) => {
                            self.error = Some(e);
                            Some(DialogAction::None)
                        }
                    }
                }
                AnimationExportMessage::Close => Some(DialogAction::Close),
            }
        } else {
            None
        }
    }
}

/// Export animation frames to GIF
pub fn export_to_gif(animator: &Arc<Mutex<Animator>>, path: &PathBuf) -> Result<(), String> {
    let animator = animator.lock();

    if animator.frames.is_empty() {
        return Err("No frames to export".to_string());
    }

    // Get dimensions from first frame
    let first_frame = &animator.frames[0].0;
    let size = first_frame.get_size();
    let dim = first_frame.get_font_dimensions();
    let width = (size.width * dim.width) as u16;
    let height = (size.height * dim.height) as u16;

    // Collect frames
    let mut gif_frames = Vec::with_capacity(animator.frames.len());

    for (screen, _settings, delay_ms) in &animator.frames {
        let options = RenderOptions {
            blink_on: true,
            ..Default::default()
        };

        let (_size, rgba_data) = screen.render_to_rgba(&options);
        gif_frames.push(GifFrame::new(rgba_data, *delay_ms));
    }

    // Create encoder and export
    let mut encoder = GifEncoder::new(width, height);
    encoder.set_repeat(RepeatCount::Infinite);

    encoder.encode_to_file(path, gif_frames).map_err(|e| format!("GIF encoding failed: {}", e))
}

/// Export animation frames to Asciicast v2 format
pub fn export_to_asciicast(animator: &Arc<Mutex<Animator>>, path: &PathBuf) -> Result<(), String> {
    use std::io::Write;

    let animator = animator.lock();

    if animator.frames.is_empty() {
        return Err("No frames to export".to_string());
    }

    let first_frame = &animator.frames[0].0;
    let size = first_frame.get_size();

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
    let mut timestamp = 0.0;

    for (screen, _settings, delay_ms) in &animator.frames {
        // Render frame to ANSI escape sequences
        let ansi_output = render_screen_to_ansi(screen.as_ref());

        // Clear screen and move cursor to home
        let frame_data = format!("\x1b[2J\x1b[H{}", ansi_output);

        // Escape the output for JSON
        let escaped = serde_json::to_string(&frame_data).map_err(|e| format!("JSON encoding failed: {}", e))?;

        // Write event: [timestamp, "o", data]
        writeln!(file, "[{}, \"o\", {}]", timestamp, escaped).map_err(|e| format!("Failed to write frame: {}", e))?;

        timestamp += *delay_ms as f64 / 1000.0;
    }

    Ok(())
}

/// Render a screen to ANSI escape sequences
fn render_screen_to_ansi(screen: &dyn Screen) -> String {
    let size = screen.get_size();
    let palette = screen.palette();
    let mut output = String::new();

    let mut last_fg: Option<u32> = None;
    let mut last_bg: Option<u32> = None;
    let mut last_bold = false;
    let mut last_blink = false;

    for y in 0..size.height {
        for x in 0..size.width {
            let ch = screen.get_char(Position::new(x, y));
            let attr = ch.attribute;

            // Build escape sequence for attribute changes
            let mut needs_reset = false;
            let bold = attr.is_bold();
            let blink = attr.is_blinking();

            if bold != last_bold || blink != last_blink {
                needs_reset = true;
            }

            if needs_reset {
                output.push_str("\x1b[0m");
                last_fg = None;
                last_bg = None;
                last_bold = false;
                last_blink = false;
            }

            // Set attributes
            if bold && !last_bold {
                output.push_str("\x1b[1m");
                last_bold = true;
            }
            if blink && !last_blink {
                output.push_str("\x1b[5m");
                last_blink = true;
            }

            // Get foreground color from palette
            let fg_idx = attr.get_foreground();
            if last_fg != Some(fg_idx) {
                let (r, g, b) = palette.get_rgb(fg_idx);
                output.push_str(&format!("\x1b[38;2;{};{};{}m", r, g, b));
                last_fg = Some(fg_idx);
            }

            // Get background color from palette
            let bg_idx = attr.get_background();
            if last_bg != Some(bg_idx) {
                let (r, g, b) = palette.get_rgb(bg_idx);
                output.push_str(&format!("\x1b[48;2;{};{};{}m", r, g, b));
                last_bg = Some(bg_idx);
            }

            // Output character
            let char_code = ch.ch;
            if char_code == '\0' || char_code == ' ' {
                output.push(' ');
            } else {
                output.push(char_code);
            }
        }
        output.push_str("\r\n");
    }

    // Reset attributes at end
    output.push_str("\x1b[0m");

    output
}
