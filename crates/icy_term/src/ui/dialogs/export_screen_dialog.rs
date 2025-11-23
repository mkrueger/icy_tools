use i18n_embed_fl::fl;
use iced::{
    Alignment, Element, Length,
    widget::{Space, column, container, pick_list, row, text, text_input},
};
use iced_engine_gui::settings::effect_box;
use iced_engine_gui::ui::*;
use icy_engine::{SaveOptions, Screen};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::ui::MainWindowMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    IcyDraw,
    Ansi,
}

impl ExportFormat {
    const ALL: [ExportFormat; 2] = [ExportFormat::IcyDraw, ExportFormat::Ansi];

    fn extension(&self) -> &str {
        match self {
            ExportFormat::IcyDraw => "icy",
            ExportFormat::Ansi => "ans",
        }
    }
}

impl std::fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportFormat::IcyDraw => write!(f, ".icy (IcyDraw)"),
            ExportFormat::Ansi => write!(f, ".ans (ANSI)"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExportScreenMsg {
    Export,
    ChangeDirectory(String),
    ChangeFileName(String),
    ChangeFormat(ExportFormat),
    BrowseDirectory,
    RestoreDefaults,
    Cancel,
}

pub struct ExportScreenDialogState {
    pub export_directory: String,
    pub export_filename: String,
    pub export_format: ExportFormat,
    temp_directory: String,
    temp_filename: String,
    temp_format: ExportFormat,
}

impl ExportScreenDialogState {
    pub fn new(initial_path: String) -> Self {
        let path: &Path = Path::new(&initial_path);

        // Determine format from extension if present
        let format = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| match ext.to_lowercase().as_str() {
                "icy" => ExportFormat::IcyDraw,
                "ans" => ExportFormat::Ansi,
                _ => ExportFormat::Ansi,
            })
            .unwrap_or(ExportFormat::Ansi);

        let (dir, mut file) = if path.is_absolute() {
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

        // Remove extension from filename if it's there
        if file.ends_with(".icy") || file.ends_with(".ans") {
            file = file.rsplit_once('.').map(|(name, _)| name.to_string()).unwrap_or(file);
        }

        Self {
            export_directory: dir.clone(),
            export_filename: file.clone(),
            export_format: format,
            temp_directory: dir,
            temp_filename: file,
            temp_format: format,
        }
    }

    pub fn get_full_path(&self) -> PathBuf {
        let filename_with_ext = format!("{}.{}", self.export_filename, self.export_format.extension());
        Path::new(&self.export_directory).join(filename_with_ext)
    }

    pub fn get_temp_full_path(&self) -> PathBuf {
        let filename_with_ext = format!("{}.{}", self.temp_filename, self.temp_format.extension());
        Path::new(&self.temp_directory).join(filename_with_ext)
    }

    pub fn export_buffer(&self, edit_screen: Arc<Mutex<Box<dyn Screen>>>) -> Result<(), String> {
        let full_path = self.get_full_path();

        // Create directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&self.export_directory) {
            return Err(format!("Failed to create directory: {}", e));
        }

        // Get the buffer from edit state
        let mut screen = edit_screen.lock().map_err(|e| format!("Failed to lock edit state: {}", e))?;

        // Get the file extension for format
        let ext = self.export_format.extension();

        // Convert buffer to bytes based on format
        let content = screen
            .to_bytes(ext, &SaveOptions::new())
            .map_err(|e| format!("Failed to convert buffer: {}", e))?;

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
            .size(14)
            .width(Length::Fill);

        let browse_btn = browse_button(crate::ui::Message::ExportDialog(ExportScreenMsg::BrowseDirectory));

        let dir_input_row = row![dir_input, Space::new().width(4.0), browse_btn].align_y(Alignment::Center);

        let dir_row = row![left_label_small(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-folder")), dir_input_row,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Filename input with format picker
        let file_input = text_input("", &self.temp_filename)
            .on_input(|s| crate::ui::Message::ExportDialog(ExportScreenMsg::ChangeFileName(s)))
            .size(14)
            .width(Length::Fill);

        let format_picker = pick_list(&ExportFormat::ALL[..], Some(self.temp_format), |format| {
            crate::ui::Message::ExportDialog(ExportScreenMsg::ChangeFormat(format))
        })
        .padding(6)
        .width(Length::Fixed(120.0));

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
                text(error_msg).size(12).style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().danger.base.color),
                })
            ]
        } else {
            row![
                text(format!("→ {}", self.get_temp_full_path().display()))
                    .size(12)
                    .style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().secondary.base.color),
                    })
            ]
        };

        let preview_row = row![Space::new().width(LABEL_SMALL_WIDTH + DIALOG_SPACING), warning_content];

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
            format!("{}", iced_engine_gui::ButtonType::Cancel),
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
