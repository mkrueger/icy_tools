use i18n_embed_fl::fl;
use iced::{
    Alignment, Element, Length,
    widget::{Space, checkbox, column, container, pick_list, row, text, text_input},
};
use icy_engine::{
    BufferType, SaveOptions, Screen,
    formats::{FileFormat, ImageFormat},
};
use icy_engine_gui::settings::effect_box;
use icy_engine_gui::ui::*;
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::ui::MainWindowMode;

#[derive(Debug, Clone)]
pub enum ExportScreenMsg {
    Export,
    ChangeDirectory(String),
    ChangeFileName(String),
    ChangeFormat(FileFormat),
    ToggleUtf8Output(bool),
    BrowseDirectory,
    RestoreDefaults,
    Cancel,
}

pub struct ExportScreenDialogState {
    pub export_directory: String,
    pub export_filename: String,
    pub export_format: FileFormat,
    pub utf8_output: bool,
    temp_directory: String,
    temp_filename: String,
    temp_format: FileFormat,
    temp_utf8_output: bool,
    /// The buffer type determines which export formats are available
    #[allow(dead_code)]
    buffer_type: BufferType,
    /// Cached list of available formats for this buffer type
    available_formats: Vec<FileFormat>,
}

impl ExportScreenDialogState {
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
        }
    }

    pub fn get_full_path(&self) -> PathBuf {
        let filename_with_ext = format!("{}.{}", self.export_filename, self.export_format.primary_extension());
        Path::new(&self.export_directory).join(filename_with_ext)
    }

    pub fn get_temp_full_path(&self) -> PathBuf {
        let filename_with_ext = format!("{}.{}", self.temp_filename, self.temp_format.primary_extension());
        Path::new(&self.temp_directory).join(filename_with_ext)
    }

    pub fn export_buffer(&self, edit_screen: Arc<Mutex<Box<dyn Screen>>>) -> Result<(), String> {
        let full_path = self.get_full_path();

        // Create directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&self.export_directory) {
            return Err(format!("Failed to create directory: {}", e));
        }

        // Get the buffer from edit state
        let mut screen = edit_screen.lock();

        // Handle image export using ImageFormat from icy_engine
        if let FileFormat::Image(img_format) = self.export_format {
            img_format
                .save_screen(screen.as_ref(), &full_path)
                .map_err(|e| format!("Failed to save image: {}", e))?;
            return Ok(());
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

        Ok(())
    }

    pub fn update(&mut self, message: ExportScreenMsg, edit_screen: Arc<Mutex<Box<dyn Screen>>>) -> Option<crate::ui::Message> {
        match message {
            ExportScreenMsg::Export => {
                // Update the actual values
                self.export_directory = self.temp_directory.clone();
                self.export_filename = self.temp_filename.clone();
                self.export_format = self.temp_format;
                self.utf8_output = self.temp_utf8_output;

                // Perform the export
                match self.export_buffer(edit_screen) {
                    Ok(_) => {
                        log::info!("Successfully exported to: {}", self.get_full_path().display());
                        // Close the dialog after successful export
                        Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)))
                    }
                    Err(e) => {
                        log::error!("Export failed: {}", e);
                        // Keep dialog open on error - maybe show error in UI
                        None
                    }
                }
            }
            ExportScreenMsg::ChangeDirectory(dir) => {
                self.temp_directory = dir;
                None
            }
            ExportScreenMsg::ChangeFileName(name) => {
                self.temp_filename = name;
                None
            }
            ExportScreenMsg::ChangeFormat(format) => {
                self.temp_format = format;
                // Disable UTF-8 output for binary/image formats
                if self.is_binary_format(format) {
                    self.temp_utf8_output = false;
                }
                None
            }
            ExportScreenMsg::ToggleUtf8Output(enabled) => {
                self.temp_utf8_output = enabled;
                None
            }
            ExportScreenMsg::BrowseDirectory => {
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
            ExportScreenMsg::RestoreDefaults => {
                let default_dir = crate::data::Options::default_capture_directory();
                if let Some(path_str) = default_dir.to_str() {
                    self.temp_directory = path_str.to_string();
                }
                None
            }
            ExportScreenMsg::Cancel => Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal))),
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

    pub fn view<'a>(&'a self, terminal_content: Element<'a, crate::ui::Message>) -> Element<'a, crate::ui::Message> {
        let overlay = self.create_modal_content();
        crate::ui::modal(terminal_content, overlay, crate::ui::Message::ExportDialog(ExportScreenMsg::Cancel))
    }

    fn create_modal_content(&self) -> Element<'_, crate::ui::Message> {
        let title = dialog_title(fl!(crate::LANGUAGE_LOADER, "export-dialog-title"));

        // Check directory validity
        let dir_path = Path::new(&self.temp_directory);
        let dir_valid = !self.temp_directory.is_empty() && dir_path.exists();
        let dir_error = if self.temp_directory.is_empty() {
            None
        } else if !dir_path.exists() {
            Some(fl!(crate::LANGUAGE_LOADER, "export-dialog-dir-not-exist"))
        } else if !dir_path.is_dir() {
            Some(fl!(crate::LANGUAGE_LOADER, "export-dialog-not-directory"))
        } else {
            None
        };

        // Directory input with browse button
        let dir_input = text_input("", &self.temp_directory)
            .on_input(|s| crate::ui::Message::ExportDialog(ExportScreenMsg::ChangeDirectory(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        let browse_btn = browse_button(crate::ui::Message::ExportDialog(ExportScreenMsg::BrowseDirectory));

        let dir_input_row = row![dir_input, Space::new().width(4.0), browse_btn].align_y(Alignment::Center);

        let dir_row = row![left_label_small(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-folder")), dir_input_row,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Filename input with format picker (uses dynamically filtered formats)
        let file_input = text_input("", &self.temp_filename)
            .on_input(|s| crate::ui::Message::ExportDialog(ExportScreenMsg::ChangeFileName(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        let format_picker = pick_list(self.available_formats.clone(), Some(self.temp_format), |format| {
            crate::ui::Message::ExportDialog(ExportScreenMsg::ChangeFormat(format))
        })
        .padding(6)
        .width(Length::Fixed(160.0));

        let file_row = row![
            left_label_small(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-file")),
            file_input,
            Space::new().width(4.0),
            format_picker,
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        // Warning/Preview area - show either directory error or path preview
        let warning_content = if let Some(error) = dir_error {
            let error_msg = error.clone();
            row![
                error_tooltip(error),
                Space::new().width(4.0),
                text(error_msg).size(TEXT_SIZE_SMALL).style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().danger.base.color),
                })
            ]
        } else {
            row![
                text(format!("→ {}", self.get_temp_full_path().display()))
                    .size(TEXT_SIZE_SMALL)
                    .style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().secondary.base.color),
                    })
            ]
        };

        let preview_row = row![Space::new().width(LABEL_SMALL_WIDTH + DIALOG_SPACING), warning_content];

        // UTF-8 output checkbox (disabled for binary/image formats)
        let is_binary = self.is_binary_format(self.temp_format);
        let utf8_checkbox_enabled = !is_binary;
        let utf8_checkbox = checkbox(self.temp_utf8_output)
            .on_toggle_maybe(if utf8_checkbox_enabled {
                Some(|checked| crate::ui::Message::ExportDialog(ExportScreenMsg::ToggleUtf8Output(checked)))
            } else {
                None
            })
            .size(18);

        let utf8_row = row![left_label_small(fl!(crate::LANGUAGE_LOADER, "export-dialog-utf8-output")), utf8_checkbox]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Check if settings are at defaults for restore button
        let default_dir = crate::data::Options::default_capture_directory();
        let is_at_defaults = default_dir.to_str().map(|s| s == self.temp_directory).unwrap_or(true);

        let restore_btn = secondary_button(
            fl!(crate::LANGUAGE_LOADER, "settings-restore-defaults-button"),
            if !is_at_defaults {
                Some(crate::ui::Message::ExportDialog(ExportScreenMsg::RestoreDefaults))
            } else {
                None
            },
        );

        // Action buttons
        let export_enabled = !self.temp_directory.is_empty() && !self.temp_filename.is_empty() && dir_valid;

        let export_btn = primary_button(
            fl!(crate::LANGUAGE_LOADER, "export-dialog-export-button"),
            if export_enabled {
                Some(crate::ui::Message::ExportDialog(ExportScreenMsg::Export))
            } else {
                None
            },
        );

        let cancel_btn = secondary_button(
            format!("{}", icy_engine_gui::ButtonType::Cancel),
            Some(crate::ui::Message::ExportDialog(ExportScreenMsg::Cancel)),
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

        iced::widget::container(modal)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}
