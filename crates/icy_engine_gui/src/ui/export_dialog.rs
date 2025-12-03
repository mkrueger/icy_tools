//! Export dialog for saving buffers to various file formats.
//!
//! This dialog provides a UI for exporting screen buffers to different formats
//! including ANSI, ASCII, XBin, and image formats (PNG, GIF, etc.).

use i18n_embed_fl::fl;
use iced::{
    Alignment, Element, Length,
    widget::{Space, checkbox, column, container, pick_list, row, text, text_input},
};
use icy_engine::{
    BufferType, SaveOptions, Screen,
    formats::{FileFormat, ImageFormat},
};
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{
    browse_button, button_row_with_left, dialog_area, dialog_title, error_tooltip,
    left_label_small, modal_container, primary_button, secondary_button, separator,
    DIALOG_SPACING, DIALOG_WIDTH_LARGE, LABEL_SMALL_WIDTH, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL,
};
use crate::settings::effect_box;
use crate::LANGUAGE_LOADER;

/// Messages for the export dialog
#[derive(Debug, Clone)]
pub enum ExportDialogMessage {
    /// Perform the export
    Export,
    /// Change the export directory
    ChangeDirectory(String),
    /// Change the export filename
    ChangeFileName(String),
    /// Change the export format
    ChangeFormat(FileFormat),
    /// Toggle UTF-8 output option
    ToggleUtf8Output(bool),
    /// Open directory browser
    BrowseDirectory,
    /// Restore default settings
    RestoreDefaults,
    /// Cancel the dialog
    Cancel,
}

/// State for the export dialog
pub struct ExportDialogState {
    /// Current export directory
    pub export_directory: String,
    /// Current export filename (without extension)
    pub export_filename: String,
    /// Current export format
    pub export_format: FileFormat,
    /// Whether to use UTF-8 output
    pub utf8_output: bool,
    /// Temporary directory (edited in dialog)
    temp_directory: String,
    /// Temporary filename (edited in dialog)
    temp_filename: String,
    /// Temporary format (edited in dialog)
    temp_format: FileFormat,
    /// Temporary UTF-8 output setting
    temp_utf8_output: bool,
    /// The buffer type determines which export formats are available
    #[allow(dead_code)]
    buffer_type: BufferType,
    /// Cached list of available formats for this buffer type
    available_formats: Vec<FileFormat>,
    /// Default directory provider function
    default_directory_fn: Option<Box<dyn Fn() -> PathBuf + Send + Sync>>,
}

impl ExportDialogState {
    /// Create a new export dialog state
    ///
    /// # Arguments
    /// * `initial_path` - Initial file path (can include directory and filename)
    /// * `buffer_type` - The type of buffer being exported (determines available formats)
    pub fn new(initial_path: String, buffer_type: BufferType) -> Self {
        let path: &Path = Path::new(&initial_path);

        // Get available formats for this buffer type (including image formats)
        let available_formats = FileFormat::save_formats_with_images_for_buffer_type(buffer_type);

        // Determine format from extension if present, or use first available
        let format = path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| FileFormat::from_extension(&ext.to_lowercase()))
            .and_then(|fmt| {
                // Only use the detected format if it's compatible with the buffer type
                if available_formats.contains(&fmt) { Some(fmt) } else { None }
            })
            .unwrap_or_else(|| available_formats.first().copied().unwrap_or(FileFormat::Image(ImageFormat::Png)));

        let (dir, file) = if path.is_absolute() {
            (
                path.parent().and_then(|p| p.to_str()).unwrap_or("").to_string(),
                path.file_stem().and_then(|f| f.to_str()).unwrap_or("export").to_string(),
            )
        } else {
            (
                std::env::current_dir()
                    .ok()
                    .and_then(|p| p.to_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| ".".to_string()),
                path.file_stem().and_then(|f| f.to_str()).unwrap_or("export").to_string(),
            )
        };

        Self {
            export_directory: dir.clone(),
            export_filename: file.clone(),
            export_format: format,
            utf8_output: false,
            temp_directory: dir,
            temp_filename: file,
            temp_format: format,
            temp_utf8_output: false,
            buffer_type,
            available_formats,
            default_directory_fn: None,
        }
    }

    /// Set a function that provides the default directory
    pub fn with_default_directory_fn<F>(mut self, f: F) -> Self
    where
        F: Fn() -> PathBuf + Send + Sync + 'static,
    {
        self.default_directory_fn = Some(Box::new(f));
        self
    }

    /// Get the full export path
    pub fn get_full_path(&self) -> PathBuf {
        let filename_with_ext = format!("{}.{}", self.export_filename, self.export_format.primary_extension());
        Path::new(&self.export_directory).join(filename_with_ext)
    }

    /// Get the temporary full path (while editing)
    pub fn get_temp_full_path(&self) -> PathBuf {
        let filename_with_ext = format!("{}.{}", self.temp_filename, self.temp_format.primary_extension());
        Path::new(&self.temp_directory).join(filename_with_ext)
    }

    /// Export the buffer to file
    pub fn export_buffer(&self, screen: Arc<Mutex<Box<dyn Screen>>>) -> Result<PathBuf, String> {
        let full_path = self.get_full_path();

        // Create directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&self.export_directory) {
            return Err(format!("Failed to create directory: {}", e));
        }

        // Get the buffer from edit state
        let mut screen = screen.lock();

        // Handle image export using ImageFormat from icy_engine
        if let FileFormat::Image(img_format) = self.export_format {
            img_format
                .save_screen(screen.as_ref(), &full_path)
                .map_err(|e| format!("Failed to save image: {}", e))?;
            return Ok(full_path);
        }

        // Get the file extension for format
        let ext = self.export_format.primary_extension();

        // Create save options with UTF-8 setting
        let mut options = SaveOptions::new();
        options.modern_terminal_output = self.utf8_output;

        // Convert buffer to bytes based on format
        let content = screen.to_bytes(ext, &options).map_err(|e| format!("Failed to convert buffer: {}", e))?;

        // Write the bytes to file
        std::fs::write(&full_path, &content).map_err(|e| format!("Failed to write file: {}", e))?;

        Ok(full_path)
    }

    /// Update the dialog state based on a message
    ///
    /// Returns `Some(true)` if export was successful, `Some(false)` if cancelled,
    /// `None` if no action needed (keep dialog open)
    pub fn update<F>(&mut self, message: ExportDialogMessage, export_fn: F) -> Option<bool>
    where
        F: FnOnce(&Self) -> Result<PathBuf, String>,
    {
        match message {
            ExportDialogMessage::Export => {
                // Update the actual values
                self.export_directory = self.temp_directory.clone();
                self.export_filename = self.temp_filename.clone();
                self.export_format = self.temp_format;
                self.utf8_output = self.temp_utf8_output;

                // Perform the export
                match export_fn(self) {
                    Ok(path) => {
                        log::info!("Successfully exported to: {}", path.display());
                        Some(true)
                    }
                    Err(e) => {
                        log::error!("Export failed: {}", e);
                        // Keep dialog open on error
                        None
                    }
                }
            }
            ExportDialogMessage::ChangeDirectory(dir) => {
                self.temp_directory = dir;
                None
            }
            ExportDialogMessage::ChangeFileName(name) => {
                self.temp_filename = name;
                None
            }
            ExportDialogMessage::ChangeFormat(format) => {
                self.temp_format = format;
                // Disable UTF-8 output for binary/image formats
                if self.is_binary_format(format) {
                    self.temp_utf8_output = false;
                }
                None
            }
            ExportDialogMessage::ToggleUtf8Output(enabled) => {
                self.temp_utf8_output = enabled;
                None
            }
            ExportDialogMessage::BrowseDirectory => {
                let initial_dir = if Path::new(&self.temp_directory).exists() {
                    Some(PathBuf::from(&self.temp_directory))
                } else {
                    std::env::current_dir().ok()
                };

                let mut dialog = rfd::FileDialog::new();
                if let Some(dir) = initial_dir {
                    dialog = dialog.set_directory(dir);
                }

                if let Some(path) = dialog.pick_folder() {
                    if let Some(path_str) = path.to_str() {
                        self.temp_directory = path_str.to_string();
                    }
                }
                None
            }
            ExportDialogMessage::RestoreDefaults => {
                if let Some(ref default_fn) = self.default_directory_fn {
                    let default_dir = default_fn();
                    if let Some(path_str) = default_dir.to_str() {
                        self.temp_directory = path_str.to_string();
                    }
                }
                None
            }
            ExportDialogMessage::Cancel => Some(false),
        }
    }

    /// Check if the format is a binary format (no UTF-8 option available)
    fn is_binary_format(&self, format: FileFormat) -> bool {
        matches!(
            format,
            FileFormat::Image(_)
                | FileFormat::IcyDraw
                | FileFormat::XBin
                | FileFormat::Bin
                | FileFormat::IceDraw
                | FileFormat::TundraDraw
                | FileFormat::Artworx
        )
    }

    /// Create the modal content for the dialog
    pub fn view<'a, Message: Clone + 'static>(
        &'a self,
        on_message: impl Fn(ExportDialogMessage) -> Message + 'a + Clone,
    ) -> Element<'a, Message> {
        let title = dialog_title(fl!(LANGUAGE_LOADER, "export-dialog-title"));

        // Check directory validity
        let dir_path = Path::new(&self.temp_directory);
        let dir_valid = !self.temp_directory.is_empty() && dir_path.exists();
        let dir_error = if self.temp_directory.is_empty() {
            None
        } else if !dir_path.exists() {
            Some(fl!(LANGUAGE_LOADER, "export-dialog-dir-not-exist"))
        } else if !dir_path.is_dir() {
            Some(fl!(LANGUAGE_LOADER, "export-dialog-not-directory"))
        } else {
            None
        };

        // Directory input with browse button
        let on_msg = on_message.clone();
        let dir_input = text_input("", &self.temp_directory)
            .on_input(move |s| on_msg(ExportDialogMessage::ChangeDirectory(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        let browse_btn = browse_button(on_message.clone()(ExportDialogMessage::BrowseDirectory));

        let dir_input_row = row![dir_input, Space::new().width(4.0), browse_btn].align_y(Alignment::Center);

        let dir_row = row![left_label_small(fl!(LANGUAGE_LOADER, "export-dialog-folder")), dir_input_row,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Filename input with format picker
        let on_msg = on_message.clone();
        let file_input = text_input("", &self.temp_filename)
            .on_input(move |s| on_msg(ExportDialogMessage::ChangeFileName(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        let on_msg = on_message.clone();
        let format_picker = pick_list(self.available_formats.clone(), Some(self.temp_format), move |format| {
            on_msg(ExportDialogMessage::ChangeFormat(format))
        })
        .padding(6)
        .width(Length::Fixed(160.0));

        let file_row = row![
            left_label_small(fl!(LANGUAGE_LOADER, "export-dialog-file")),
            file_input,
            Space::new().width(4.0),
            format_picker,
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        // Warning/Preview area
        let warning_content: Element<'a, Message> = if let Some(error) = dir_error {
            let error_msg = error.clone();
            row![
                error_tooltip(error),
                Space::new().width(4.0),
                text(error_msg).size(TEXT_SIZE_SMALL).style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().danger.base.color),
                })
            ]
            .into()
        } else {
            row![
                text(format!("→ {}", self.get_temp_full_path().display()))
                    .size(TEXT_SIZE_SMALL)
                    .style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().secondary.base.color),
                    })
            ]
            .into()
        };

        let preview_row = row![Space::new().width(LABEL_SMALL_WIDTH + DIALOG_SPACING), warning_content];

        // UTF-8 output checkbox
        let is_binary = self.is_binary_format(self.temp_format);
        let utf8_checkbox_enabled = !is_binary;
        let on_msg = on_message.clone();
        let utf8_checkbox = checkbox(self.temp_utf8_output)
            .on_toggle_maybe(if utf8_checkbox_enabled {
                Some(move |checked| on_msg(ExportDialogMessage::ToggleUtf8Output(checked)))
            } else {
                None
            })
            .size(18);

        let utf8_row = row![left_label_small(fl!(LANGUAGE_LOADER, "export-dialog-utf8-output")), utf8_checkbox]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Check if settings are at defaults
        let is_at_defaults = if let Some(ref default_fn) = self.default_directory_fn {
            let default_dir = default_fn();
            default_dir.to_str().map(|s| s == self.temp_directory).unwrap_or(true)
        } else {
            true
        };

        let restore_btn = secondary_button(
            fl!(LANGUAGE_LOADER, "settings-restore-defaults-button"),
            if !is_at_defaults {
                Some(on_message.clone()(ExportDialogMessage::RestoreDefaults))
            } else {
                None
            },
        );

        // Action buttons
        let export_enabled = !self.temp_directory.is_empty() && !self.temp_filename.is_empty() && dir_valid;

        let export_btn = primary_button(
            fl!(LANGUAGE_LOADER, "export-dialog-export-button"),
            if export_enabled {
                Some(on_message.clone()(ExportDialogMessage::Export))
            } else {
                None
            },
        );

        let cancel_btn = secondary_button(
            fl!(LANGUAGE_LOADER, "dialog-cancel-button"),
            Some(on_message(ExportDialogMessage::Cancel)),
        );

        let buttons_left = vec![restore_btn.into()];
        let buttons_right = vec![cancel_btn.into(), export_btn.into()];

        let buttons = button_row_with_left(buttons_left, buttons_right);

        // Main content wrapped in effect_box
        let content_box = effect_box(
            column![
                dir_row,
                Space::new().height(DIALOG_SPACING),
                file_row,
                Space::new().height(DIALOG_SPACING),
                utf8_row,
                Space::new().height(DIALOG_SPACING),
                preview_row,
            ]
            .spacing(0)
            .into(),
        );

        let dialog_content = dialog_area(column![title, Space::new().height(DIALOG_SPACING), content_box, Space::new().height(DIALOG_SPACING),].into());

        let button_area = dialog_area(buttons.into());

        let modal = modal_container(
            column![container(dialog_content).height(Length::Fill), separator(), button_area,].into(),
            DIALOG_WIDTH_LARGE,
        );

        container(modal)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}
