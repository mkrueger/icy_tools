use i18n_embed_fl::fl;
use iced::{
    Alignment, Element, Length,
    widget::{Space, column, container, row, text, text_input},
};
use icy_engine_gui::settings::effect_box;
use icy_engine_gui::ui::*;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::ui::MainWindowMode;

#[derive(Debug, Clone)]
pub enum CaptureMsg {
    StartCapture,
    StopCapture,
    ChangeDirectory(String),
    ChangeFileName(String),
    BrowseDirectory,
    RestoreDefaults,
    Cancel,
    ConfirmOverwrite,
    CancelOverwrite,
}

pub struct CaptureDialogState {
    pub capture_session: bool,
    pub capture_directory: String,
    pub capture_filename: String,
    temp_directory: String,
    temp_filename: String,
    pending_overwrite: bool,
}

impl CaptureDialogState {
    pub fn new(initial_path: String) -> Self {
        // initial_path is now just a directory, not a full file path
        let dir = if initial_path.is_empty() {
            std::env::current_dir()
                .ok()
                .and_then(|p| p.to_str().map(|s| s.to_string()))
                .unwrap_or_else(|| ".".to_string())
        } else {
            initial_path
        };

        let file = "capture.txt".to_string();

        Self {
            capture_session: false,
            capture_directory: dir.clone(),
            capture_filename: file.clone(),
            temp_directory: dir,
            temp_filename: file,
            pending_overwrite: false,
        }
    }

    pub fn reset(&mut self, capture_dir: &str, is_capturing: bool) {
        // capture_dir is now just a directory path, not a full file path
        let dir = if capture_dir.is_empty() {
            self.capture_directory.clone()
        } else {
            capture_dir.to_string()
        };

        self.temp_directory = dir.clone();
        self.capture_directory = dir;
        // Keep existing filename
        self.temp_filename = self.capture_filename.clone();
        self.capture_session = is_capturing;
    }

    pub fn is_capturing(&self) -> bool {
        self.capture_session
    }

    pub fn get_full_path(&self) -> PathBuf {
        Path::new(&self.capture_directory).join(&self.capture_filename)
    }

    pub fn append_data(&mut self, data: &[u8]) {
        if self.capture_session {
            let full_path = self.get_full_path();
            if let Ok(mut data_file) = std::fs::OpenOptions::new().create(true).append(true).open(&full_path) {
                if let Err(err) = data_file.write_all(data) {
                    log::error!("Failed to write capture data to file {}: {}", full_path.display(), err);
                }
            }
        }
    }

    fn start_capture_internal(&mut self) -> Option<crate::ui::Message> {
        self.capture_directory = self.temp_directory.clone();
        self.capture_filename = self.temp_filename.clone();
        self.capture_session = true;

        // Create directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&self.capture_directory) {
            log::error!("Failed to create directory: {}", e);
            return None;
        }

        // Save the full path to options
        let full_path = self.get_full_path();
        if let Some(path_str) = full_path.to_str() {
            Some(crate::ui::Message::StartCapture(path_str.to_string()))
        } else {
            None
        }
    }

    pub fn update(&mut self, message: CaptureMsg) -> Option<crate::ui::Message> {
        match message {
            CaptureMsg::StartCapture => {
                // Check if file exists
                let full_path = PathBuf::from(&self.temp_directory).join(&self.temp_filename);
                if full_path.exists() && !self.capture_session {
                    // Show confirmation dialog
                    self.pending_overwrite = true;
                    None
                } else {
                    self.start_capture_internal()
                }
            }
            CaptureMsg::ConfirmOverwrite => {
                self.pending_overwrite = false;
                self.start_capture_internal()
            }
            CaptureMsg::CancelOverwrite => {
                self.pending_overwrite = false;
                None
            }
            CaptureMsg::StopCapture => {
                // This message is no longer used - use Message::StopCapture directly
                self.capture_session = false;
                Some(crate::ui::Message::StopCapture)
            }
            CaptureMsg::ChangeDirectory(dir) => {
                self.temp_directory = dir;
                None
            }
            CaptureMsg::ChangeFileName(name) => {
                self.temp_filename = name;
                None
            }
            CaptureMsg::BrowseDirectory => {
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
            CaptureMsg::RestoreDefaults => {
                let default_dir = crate::data::Options::default_capture_directory();
                if let Some(path_str) = default_dir.to_str() {
                    self.temp_directory = path_str.to_string();
                }
                None
            }
            CaptureMsg::Cancel => {
                // Don't save changes, just close
                self.pending_overwrite = false;
                Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)))
            }
        }
    }

    pub fn view<'a>(&'a self, terminal_content: Element<'a, crate::ui::Message>) -> Element<'a, crate::ui::Message> {
        let overlay = self.create_modal_content();
        let modal = crate::ui::modal(terminal_content, overlay, crate::ui::Message::CaptureDialog(CaptureMsg::Cancel));

        if self.pending_overwrite {
            let filename = self.temp_filename.clone();
            let dialog = icy_engine_gui::ConfirmationDialog::new(
                fl!(crate::LANGUAGE_LOADER, "capture-dialog-overwrite-title"),
                fl!(crate::LANGUAGE_LOADER, "capture-dialog-overwrite-message", filename = filename),
            )
            .dialog_type(icy_engine_gui::DialogType::Warning)
            .secondary_message(fl!(crate::LANGUAGE_LOADER, "capture-dialog-overwrite-secondary"))
            .buttons(icy_engine_gui::ButtonSet::OverwriteCancel);

            dialog.view(modal, |result| match result {
                icy_engine_gui::DialogResult::Overwrite => crate::ui::Message::CaptureDialog(CaptureMsg::ConfirmOverwrite),
                _ => {
                    // Reset pending_overwrite flag when user cancels
                    crate::ui::Message::CaptureDialog(CaptureMsg::CancelOverwrite)
                }
            })
        } else {
            modal
        }
    }

    fn create_modal_content(&self) -> Element<'_, crate::ui::Message> {
        let title = dialog_title(if self.capture_session {
            fl!(crate::LANGUAGE_LOADER, "toolbar-stop-capture")
        } else {
            fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-title")
        });

        // Check directory validity
        let dir_path = Path::new(&self.temp_directory);
        let dir_valid = !self.temp_directory.is_empty() && dir_path.exists();
        let dir_error = if self.temp_directory.is_empty() {
            None
        } else if !dir_path.exists() {
            Some(fl!(crate::LANGUAGE_LOADER, "capture-dialog-dir-not-exist"))
        } else if !dir_path.is_dir() {
            Some(fl!(crate::LANGUAGE_LOADER, "capture-dialog-not-directory"))
        } else {
            None
        };

        // Directory input with browse button
        let dir_input = text_input("", &self.temp_directory)
            .on_input(|s| crate::ui::Message::CaptureDialog(CaptureMsg::ChangeDirectory(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        let browse_btn = browse_button(crate::ui::Message::CaptureDialog(CaptureMsg::BrowseDirectory));

        let dir_input_row = row![dir_input, Space::new().width(4.0), browse_btn].align_y(Alignment::Center);

        let dir_row = row![left_label_small(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-folder")), dir_input_row,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Check if file exists
        let full_path = Path::new(&self.temp_directory).join(&self.temp_filename);
        let file_exists = full_path.exists();

        // Filename input
        let file_input: text_input::TextInput<'_, crate::ui::Message> = text_input("", &self.temp_filename)
            .on_input(|s| crate::ui::Message::CaptureDialog(CaptureMsg::ChangeFileName(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        let file_row = row![left_label_small(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-file")), file_input,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Warning area - show either directory error or file exists warning
        let warning_content = if let Some(error) = dir_error {
            let error_msg = error.clone();
            row![
                error_tooltip(error),
                Space::new().width(4.0),
                text(error_msg).size(TEXT_SIZE_SMALL).style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().danger.base.color),
                })
            ]
            .align_y(Alignment::Center)
        } else if file_exists && !self.capture_session {
            let file_warning_msg = fl!(crate::LANGUAGE_LOADER, "capture-dialog-file-exists");
            row![
                warning_tooltip(file_warning_msg.clone()),
                Space::new().width(4.0),
                text(file_warning_msg)
                    .size(TEXT_SIZE_SMALL)
                    .style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().warning.base.color),
                    })
            ]
        } else {
            row![text(String::new()).size(TEXT_SIZE_SMALL)]
        };

        let warning_row = row![Space::new().width(LABEL_SMALL_WIDTH + DIALOG_SPACING), warning_content];

        // Check if settings are at defaults for restore button
        let default_dir = crate::data::Options::default_capture_directory();
        let is_at_defaults = default_dir.to_str().map(|s| s == self.temp_directory).unwrap_or(true);

        let restore_btn = if !self.capture_session {
            Some(icy_engine_gui::ui::restore_defaults_button(
                !is_at_defaults,
                crate::ui::Message::CaptureDialog(CaptureMsg::RestoreDefaults),
            ))
        } else {
            None
        };

        // Action buttons
        let action_btn = if self.capture_session {
            danger_button(fl!(crate::LANGUAGE_LOADER, "toolbar-stop-capture"), Some(crate::ui::Message::StopCapture))
        } else {
            primary_button(
                fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-button"),
                if dir_valid {
                    Some(crate::ui::Message::CaptureDialog(CaptureMsg::StartCapture))
                } else {
                    None
                },
            )
        };

        let cancel_btn = secondary_button(
            format!("{}", icy_engine_gui::ButtonType::Cancel),
            Some(crate::ui::Message::CaptureDialog(CaptureMsg::Cancel)),
        );

        let buttons = if let Some(restore) = restore_btn {
            button_row_with_left(vec![restore.into()], vec![cancel_btn.into(), action_btn.into()])
        } else {
            button_row(vec![cancel_btn.into(), action_btn.into()])
        };

        // Main content wrapped in effect_box
        let mut content_column = column![dir_row, Space::new().height(DIALOG_SPACING), file_row];

        content_column = content_column.push(Space::new().height(DIALOG_SPACING)).push(warning_row);

        let content_box = effect_box(content_column.spacing(0).into());

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
