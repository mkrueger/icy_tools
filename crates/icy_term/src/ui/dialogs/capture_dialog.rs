use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length,
    widget::{Space, button, column, container, row, text, text_input, tooltip},
};
use iced_engine_gui::settings::effect_box;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::ui::MainWindowMode;

const MODAL_WIDTH: f32 = 550.0;
const MODAL_HEIGHT: f32 = 224.0;
const DIALOG_LABEL_WIDTH: f32 = 100.0;

#[derive(Debug, Clone)]
pub enum CaptureMsg {
    StartCapture,
    StopCapture,
    ChangeDirectory(String),
    ChangeFileName(String),
    BrowseDirectory,
    ResetDirectory,
    Cancel,
}

pub struct CaptureDialogState {
    pub capture_session: bool,
    pub capture_directory: String,
    pub capture_filename: String,
    temp_directory: String,
    temp_filename: String,
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

    pub fn update(&mut self, message: CaptureMsg) -> Option<crate::ui::Message> {
        match message {
            CaptureMsg::StartCapture => {
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
            CaptureMsg::ResetDirectory => {
                let default_dir = crate::data::Options::default_capture_directory();
                if let Some(path_str) = default_dir.to_str() {
                    self.temp_directory = path_str.to_string();
                }
                None
            }
            CaptureMsg::Cancel => {
                // Don't save changes, just close
                Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)))
            }
        }
    }

    pub fn view<'a>(&'a self, terminal_content: Element<'a, crate::ui::Message>) -> Element<'a, crate::ui::Message> {
        let overlay = self.create_modal_content();
        crate::ui::modal(terminal_content, overlay, crate::ui::Message::CaptureDialog(CaptureMsg::Cancel))
    }

    fn create_modal_content(&self) -> Element<'_, crate::ui::Message> {
        let title = text(if self.capture_session {
            fl!(crate::LANGUAGE_LOADER, "toolbar-stop-capture")
        } else {
            fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-title")
        })
        .size(18)
        .font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..iced::Font::default()
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
            .size(14)
            .width(Length::Fill);

        let browse_button = button(text("…").size(14))
            .on_press(crate::ui::Message::CaptureDialog(CaptureMsg::BrowseDirectory))
            .padding([6, 12]);

        // Check if directory is different from default
        let default_dir = crate::data::Options::default_capture_directory();
        let is_default = default_dir.to_str().map(|s| s == self.temp_directory).unwrap_or(true);

        let mut dir_input_row = row![dir_input, Space::new().width(4.0), browse_button];

        if !is_default {
            let reset_button = button(text("↻").size(14))
                .on_press(crate::ui::Message::CaptureDialog(CaptureMsg::ResetDirectory))
                .padding([6, 12])
                .style(button::secondary);
            dir_input_row = dir_input_row.push(Space::new().width(4.0)).push(reset_button);
        }

        dir_input_row = dir_input_row.align_y(Alignment::Center);

        if let Some(error) = dir_error {
            let warning_icon = tooltip(
                text("⚠").size(16).style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().danger.weak.color),
                }),
                container(text(error)).style(container::rounded_box),
                tooltip::Position::Top,
            );
            dir_input_row = dir_input_row.push(Space::new().width(4.0)).push(warning_icon);
        }

        let dir_row = row![
            container(text(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-folder")).size(14))
                .width(Length::Fixed(DIALOG_LABEL_WIDTH))
                .align_x(iced::alignment::Horizontal::Right),
            dir_input_row,
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // Check if file exists
        let full_path = Path::new(&self.temp_directory).join(&self.temp_filename);
        let file_exists = full_path.exists();

        // Filename input
        let file_input = text_input("", &self.temp_filename)
            .on_input(|s| crate::ui::Message::CaptureDialog(CaptureMsg::ChangeFileName(s)))
            .size(14)
            .width(Length::Fill);

        let file_row = row![
            container(text(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-file")).size(14))
                .width(Length::Fixed(DIALOG_LABEL_WIDTH))
                .align_x(iced::alignment::Horizontal::Right),
            file_input,
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // File exists warning
        let file_warning = row![
            Space::new().width(DIALOG_LABEL_WIDTH + 8.0),
            if file_exists && !self.capture_session {
                text(fl!(crate::LANGUAGE_LOADER, "capture-dialog-file-exists"))
                    .size(12)
                    .style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().warning.base.color),
                    })
            } else {
                text(String::new()).size(12)
            }
        ];

        // Action buttons
        let action_button: button::Button<'_, crate::ui::Message> = if self.capture_session {
            button(text(fl!(crate::LANGUAGE_LOADER, "toolbar-stop-capture")))
                .on_press(crate::ui::Message::StopCapture)
                .padding([6, 12])
                .style(button::danger)
        } else {
            let mut btn = button(text(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-button")))
                .padding([6, 12])
                .style(button::primary);
            if dir_valid {
                btn = btn.on_press(crate::ui::Message::CaptureDialog(CaptureMsg::StartCapture));
            }
            btn
        };

        let cancel_button = button(text(fl!(crate::LANGUAGE_LOADER, "dialog-cancel_button")))
            .on_press(crate::ui::Message::CaptureDialog(CaptureMsg::Cancel))
            .padding([6, 12])
            .style(button::secondary);

        let button_row = row![Space::new().width(Length::Fill), cancel_button, Space::new().width(8.0), action_button,];

        // Visual separator
        let separator = container(Space::new())
            .width(Length::Fill)
            .height(1.0)
            .style(|theme: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(theme.palette().text.scale_alpha(0.06))),
                ..Default::default()
            });

        // Main content wrapped in effect_box
        let mut content_column = column![dir_row, Space::new().height(8.0), file_row];

        content_column = content_column.push(Space::new().height(8.0)).push(file_warning);

        let content_box = effect_box(content_column.spacing(0).into());

        let modal_content = container(
            column![
                title,
                Space::new().height(8.0),
                content_box,
                Space::new().height(8.0),
                separator,
                Space::new().height(8.0),
                button_row,
            ]
            .padding(10),
        )
        .width(Length::Fixed(MODAL_WIDTH))
        .height(Length::Fixed(MODAL_HEIGHT))
        .style(|theme: &iced::Theme| {
            let palette = theme.palette();
            container::Style {
                background: Some(iced::Background::Color(palette.background)),
                border: Border {
                    color: palette.text,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                text_color: Some(palette.text),
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
                snap: false,
            }
        });

        container(modal_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}
