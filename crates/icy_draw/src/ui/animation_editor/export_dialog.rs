//! Export dialog for animation editor
//!
//! Provides a modal dialog for exporting animations to GIF or Asciicast format.

use std::path::PathBuf;
use std::sync::Arc;

use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, pick_list, progress_bar, row, text, text_input, Space},
};
use icy_engine::{Position, gif_encoder::{GifEncoder, GifFrame, RepeatCount}, RenderOptions, Screen};
use icy_engine_scripting::Animator;
use parking_lot::Mutex;

/// Export format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Gif,
    Asciicast,
}

impl ExportFormat {
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

/// Export dialog state
pub struct ExportDialog {
    /// Selected export format
    pub format: ExportFormat,
    /// Export file path
    pub path: PathBuf,
    /// Whether export is in progress
    pub is_exporting: bool,
    /// Current export progress (0.0 to 1.0)
    pub progress: f32,
    /// Error message if export failed
    pub error: Option<String>,
    /// Success message
    pub success: Option<String>,
}

impl Default for ExportDialog {
    fn default() -> Self {
        Self {
            format: ExportFormat::Gif,
            path: PathBuf::from("animation.gif"),
            is_exporting: false,
            progress: 0.0,
            error: None,
            success: None,
        }
    }
}

impl ExportDialog {
    /// Create a new export dialog with a default path based on source file
    pub fn new(source_path: Option<&PathBuf>) -> Self {
        let path = source_path
            .map(|p| p.with_extension("gif"))
            .unwrap_or_else(|| PathBuf::from("animation.gif"));

        Self {
            path,
            ..Default::default()
        }
    }

    /// Update the path extension based on format
    pub fn update_extension(&mut self) {
        self.path.set_extension(self.format.extension());
    }
}

/// Messages for export dialog
#[derive(Debug, Clone)]
pub enum ExportDialogMessage {
    /// Set export format
    SetFormat(ExportFormat),
    /// Set export path (from text input)
    SetPath(String),
    /// Browse for export path
    Browse,
    /// Path selected from file dialog
    PathSelected(Option<PathBuf>),
    /// Start export
    StartExport,
    /// Export progress update
    Progress(f32),
    /// Export completed successfully
    Complete,
    /// Export failed with error
    Error(String),
    /// Close dialog
    Close,
}

/// Export animation frames to GIF
pub fn export_to_gif(
    animator: &Arc<Mutex<Animator>>,
    path: &PathBuf,
) -> Result<(), String> {
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
    
    encoder.encode_to_file(path, gif_frames)
        .map_err(|e| format!("GIF encoding failed: {}", e))
}

/// Export animation frames to Asciicast v2 format
pub fn export_to_asciicast(
    animator: &Arc<Mutex<Animator>>,
    path: &PathBuf,
) -> Result<(), String> {
    use std::io::Write;
    
    let animator = animator.lock();
    
    if animator.frames.is_empty() {
        return Err("No frames to export".to_string());
    }

    let first_frame = &animator.frames[0].0;
    let size = first_frame.get_size();
    
    let mut file = std::fs::File::create(path)
        .map_err(|e| format!("Failed to create file: {}", e))?;
    
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
    
    writeln!(file, "{}", header)
        .map_err(|e| format!("Failed to write header: {}", e))?;
    
    // Write frames
    let mut timestamp = 0.0;
    
    for (screen, _settings, delay_ms) in &animator.frames {
        // Render frame to ANSI escape sequences
        let ansi_output = render_screen_to_ansi(screen.as_ref());
        
        // Clear screen and move cursor to home
        let frame_data = format!("\x1b[2J\x1b[H{}", ansi_output);
        
        // Escape the output for JSON
        let escaped = serde_json::to_string(&frame_data)
            .map_err(|e| format!("JSON encoding failed: {}", e))?;
        
        // Write event: [timestamp, "o", data]
        writeln!(file, "[{}, \"o\", {}]", timestamp, escaped)
            .map_err(|e| format!("Failed to write frame: {}", e))?;
        
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

/// Build the export dialog view
pub fn view_export_dialog<'a>(dialog: &'a ExportDialog) -> Element<'a, ExportDialogMessage> {
    static FORMAT_OPTIONS: &[ExportFormat] = &[ExportFormat::Gif, ExportFormat::Asciicast];
    
    let title = text("Export Animation").size(20);
    
    // Format selection
    let format_label = text("Format:").size(14);
    let format_picker = pick_list(
        FORMAT_OPTIONS,
        Some(dialog.format),
        ExportDialogMessage::SetFormat,
    ).width(200);
    
    let format_row = row![format_label, Space::new().width(8), format_picker]
        .align_y(Alignment::Center);
    
    // Path input
    let path_label = text("Path:").size(14);
    let path_input = text_input("Export path...", &dialog.path.display().to_string())
        .on_input(ExportDialogMessage::SetPath)
        .width(Length::Fill);
    let browse_btn = button(text("Browse").size(12))
        .padding([6, 12])
        .on_press(ExportDialogMessage::Browse);
    
    let path_row = row![
        path_label,
        Space::new().width(8),
        path_input,
        Space::new().width(8),
        browse_btn,
    ]
    .align_y(Alignment::Center);
    
    // Progress or status
    let status_element: Element<'_, ExportDialogMessage> = if dialog.is_exporting {
        column![
            text("Exporting...").size(12),
            container(progress_bar(0.0..=1.0, dialog.progress)).width(Length::Fill),
        ]
        .spacing(4)
        .into()
    } else if let Some(ref error) = dialog.error {
        text(format!("Error: {}", error))
            .size(12)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().danger.base.color),
            })
            .into()
    } else if let Some(ref success) = dialog.success {
        text(success)
            .size(12)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().success.base.color),
            })
            .into()
    } else {
        Space::new().height(20).into()
    };
    
    // Buttons
    let cancel_btn = button(text("Cancel").size(14))
        .padding([8, 16])
        .on_press(ExportDialogMessage::Close);
    
    let export_btn = button(text("Export").size(14))
        .padding([8, 16])
        .style(iced::widget::button::primary)
        .on_press_maybe((!dialog.is_exporting).then_some(ExportDialogMessage::StartExport));
    
    let button_row = row![
        Space::new().width(Length::Fill),
        cancel_btn,
        Space::new().width(8),
        export_btn,
    ];
    
    let content = column![
        title,
        Space::new().height(16),
        format_row,
        Space::new().height(8),
        path_row,
        Space::new().height(16),
        status_element,
        Space::new().height(16),
        button_row,
    ]
    .spacing(4)
    .padding(20)
    .width(500);
    
    container(content)
        .style(|theme: &iced::Theme| {
            container::Style {
                background: Some(iced::Background::Color(theme.extended_palette().background.base.color)),
                border: iced::Border {
                    color: theme.extended_palette().background.strong.color,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
}
