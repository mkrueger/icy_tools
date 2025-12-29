use i18n_embed_fl::fl;
use iced::{
    widget::{column, container, row, text, text_input, Space},
    Alignment, Element, Length,
};
use icy_engine_gui::dialog_wrapper;
use icy_engine_gui::settings::effect_box;
use icy_engine_gui::ui::*;
use icy_engine_gui::StateResult;
use std::path::{Path, PathBuf};

/// Result from the capture dialog
#[derive(Debug, Clone)]
pub enum CaptureDialogResult {
    /// Start capturing to the given file path
    StartCapture(String),
    /// Stop the current capture
    StopCapture,
}

#[derive(Debug, Clone)]
pub enum CaptureDialogMessage {
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

#[dialog_wrapper(close_on_blur = true, result_type = CaptureDialogResult)]
pub struct CaptureDialogState {
    is_capturing: bool,
    capture_directory: String,
    capture_filename: String,
    temp_directory: String,
    temp_filename: String,
    pending_overwrite: bool,
}

impl CaptureDialogState {
    pub fn new(initial_dir: String, is_capturing: bool) -> Self {
        let dir = if initial_dir.is_empty() {
            std::env::current_dir()
                .ok()
                .and_then(|p| p.to_str().map(|s| s.to_string()))
                .unwrap_or_else(|| ".".to_string())
        } else {
            initial_dir
        };

        let file = "capture.txt".to_string();

        Self {
            is_capturing,
            capture_directory: dir.clone(),
            capture_filename: file.clone(),
            temp_directory: dir,
            temp_filename: file,
            pending_overwrite: false,
        }
    }

    fn get_full_path(&self) -> PathBuf {
        Path::new(&self.capture_directory).join(&self.capture_filename)
    }

    fn start_capture_internal(&mut self) -> StateResult<CaptureDialogResult> {
        self.capture_directory = self.temp_directory.clone();
        self.capture_filename = self.temp_filename.clone();

        // Create directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&self.capture_directory) {
            log::error!("Failed to create directory: {}", e);
            return StateResult::None;
        }

        // Save the full path to options
        let full_path = self.get_full_path();
        if let Some(path_str) = full_path.to_str() {
            StateResult::Success(CaptureDialogResult::StartCapture(path_str.to_string()))
        } else {
            StateResult::None
        }
    }

    pub fn handle_message(&mut self, message: CaptureDialogMessage) -> StateResult<CaptureDialogResult> {
        match message {
            CaptureDialogMessage::StartCapture => {
                // Check if file exists
                let full_path = PathBuf::from(&self.temp_directory).join(&self.temp_filename);
                if full_path.exists() && !self.is_capturing {
                    // Show confirmation dialog
                    self.pending_overwrite = true;
                    StateResult::None
                } else {
                    self.start_capture_internal()
                }
            }
            CaptureDialogMessage::ConfirmOverwrite => {
                self.pending_overwrite = false;
                self.start_capture_internal()
            }
            CaptureDialogMessage::CancelOverwrite => {
                self.pending_overwrite = false;
                StateResult::None
            }
            CaptureDialogMessage::StopCapture => StateResult::Success(CaptureDialogResult::StopCapture),
            CaptureDialogMessage::ChangeDirectory(dir) => {
                self.temp_directory = dir;
                StateResult::None
            }
            CaptureDialogMessage::ChangeFileName(name) => {
                self.temp_filename = name;
                StateResult::None
            }
            CaptureDialogMessage::BrowseDirectory => {
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
                StateResult::None
            }
            CaptureDialogMessage::RestoreDefaults => {
                let default_dir = crate::data::Options::default_capture_directory();
                if let Some(path_str) = default_dir.to_str() {
                    self.temp_directory = path_str.to_string();
                }
                StateResult::None
            }
            CaptureDialogMessage::Cancel => StateResult::Close,
        }
    }

    pub fn view<'a, M: Clone + 'static>(&'a self, on_message: impl Fn(CaptureDialogMessage) -> M + 'static + Clone) -> Element<'a, M> {
        let content = self.create_modal_content(on_message.clone());

        if self.pending_overwrite {
            let filename = self.temp_filename.clone();
            let on_msg = on_message.clone();
            let dialog = icy_engine_gui::ConfirmationDialog::new(
                fl!(crate::LANGUAGE_LOADER, "capture-dialog-overwrite-title"),
                fl!(crate::LANGUAGE_LOADER, "capture-dialog-overwrite-message", filename = filename),
            )
            .dialog_type(icy_engine_gui::DialogType::Warning)
            .secondary_message(fl!(crate::LANGUAGE_LOADER, "capture-dialog-overwrite-secondary"))
            .buttons(icy_engine_gui::ButtonSet::OverwriteCancel);

            dialog.view(content, move |result| match result {
                icy_engine_gui::DialogResult::Overwrite => on_msg(CaptureDialogMessage::ConfirmOverwrite),
                _ => on_msg(CaptureDialogMessage::CancelOverwrite),
            })
        } else {
            content
        }
    }

    fn create_modal_content<'a, M: Clone + 'static>(&'a self, on_message: impl Fn(CaptureDialogMessage) -> M + 'static + Clone) -> Element<'a, M> {
        let title = dialog_title(if self.is_capturing {
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
        let on_msg = on_message.clone();
        let dir_input = text_input("", &self.temp_directory)
            .on_input(move |s| on_msg(CaptureDialogMessage::ChangeDirectory(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        let on_msg = on_message.clone();
        let browse_btn = browse_button(on_msg(CaptureDialogMessage::BrowseDirectory));

        let dir_input_row = row![dir_input, Space::new().width(4.0), browse_btn].align_y(Alignment::Center);

        let dir_row = row![left_label_small(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-folder")), dir_input_row,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Check if file exists
        let full_path = Path::new(&self.temp_directory).join(&self.temp_filename);
        let file_exists = full_path.exists();

        // Filename input
        let on_msg = on_message.clone();
        let file_input: text_input::TextInput<'_, M> = text_input("", &self.temp_filename)
            .on_input(move |s| on_msg(CaptureDialogMessage::ChangeFileName(s)))
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
                    color: Some(theme.destructive.base),
                })
            ]
            .align_y(Alignment::Center)
        } else if file_exists && !self.is_capturing {
            let file_warning_msg = fl!(crate::LANGUAGE_LOADER, "capture-dialog-file-exists");
            row![
                warning_tooltip(file_warning_msg.clone()),
                Space::new().width(4.0),
                text(file_warning_msg)
                    .size(TEXT_SIZE_SMALL)
                    .style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.warning.base),
                    })
            ]
        } else {
            row![text(String::new()).size(TEXT_SIZE_SMALL)]
        };

        let warning_row = row![Space::new().width(LABEL_SMALL_WIDTH + DIALOG_SPACING), warning_content];

        // Check if settings are at defaults for restore button
        let default_dir = crate::data::Options::default_capture_directory();
        let is_at_defaults = default_dir.to_str().map(|s| s == self.temp_directory).unwrap_or(true);

        let restore_btn = if !self.is_capturing {
            let on_msg = on_message.clone();
            Some(icy_engine_gui::ui::restore_defaults_button(
                !is_at_defaults,
                on_msg(CaptureDialogMessage::RestoreDefaults),
            ))
        } else {
            None
        };

        // Action buttons
        let on_msg = on_message.clone();
        let action_btn = if self.is_capturing {
            danger_button(
                fl!(crate::LANGUAGE_LOADER, "toolbar-stop-capture"),
                Some(on_msg(CaptureDialogMessage::StopCapture)),
            )
        } else {
            primary_button(
                fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-button"),
                if dir_valid { Some(on_msg(CaptureDialogMessage::StartCapture)) } else { None },
            )
        };

        let on_msg = on_message.clone();
        let cancel_btn = secondary_button(format!("{}", icy_engine_gui::ButtonType::Cancel), Some(on_msg(CaptureDialogMessage::Cancel)));

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

/// Create a capture dialog for the dialog stack
pub fn capture_dialog_from_msg<M, F, E>(initial_dir: String, is_capturing: bool, (on_message, extract_message): (F, E)) -> CaptureDialogWrapper<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(CaptureDialogMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&CaptureDialogMessage> + Clone + 'static,
{
    CaptureDialogWrapper::new(CaptureDialogState::new(initial_dir, is_capturing), on_message, extract_message)
}
