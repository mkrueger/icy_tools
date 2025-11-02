use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length,
    widget::{Space, button, column, container, row, text, text_input},
};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::ui::MainWindowMode;

const MODAL_WIDTH: f32 = 500.0;
const MODAL_HEIGHT: f32 = 200.0;

#[derive(Debug, Clone)]
pub enum CaptureMsg {
    StartCapture,
    StopCapture,
    ChangeDirectory(String),
    ChangeFileName(String),
    BrowseDirectory,
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
        let path: &Path = Path::new(&initial_path);
        let (dir, file) = if path.is_absolute() {
            (
                path.parent().and_then(|p| p.to_str()).unwrap_or("").to_string(),
                path.file_name().and_then(|f| f.to_str()).unwrap_or("capture.txt").to_string(),
            )
        } else {
            (
                std::env::current_dir()
                    .ok()
                    .and_then(|p| p.to_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| ".".to_string()),
                initial_path.clone(),
            )
        };

        Self {
            capture_session: false,
            capture_directory: dir.clone(),
            capture_filename: file.clone(),
            temp_directory: dir,
            temp_filename: file,
        }
    }

    pub fn reset(&mut self, full_path: &str, is_capturing: bool) {
        let path = Path::new(full_path);
        let (dir, file) = if path.is_absolute() {
            (
                path.parent().and_then(|p| p.to_str()).unwrap_or(&self.capture_directory).to_string(),
                path.file_name().and_then(|f| f.to_str()).unwrap_or(&self.capture_filename).to_string(),
            )
        } else if !full_path.is_empty() {
            (self.capture_directory.clone(), full_path.to_string())
        } else {
            (self.capture_directory.clone(), self.capture_filename.clone())
        };

        self.temp_directory = dir.clone();
        self.temp_filename = file.clone();
        self.capture_directory = dir;
        self.capture_filename = file;
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
                self.capture_session = false;
                Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)))
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
        let title = if self.capture_session {
            text(fl!(crate::LANGUAGE_LOADER, "toolbar-stop-capture"))
        } else {
            text(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-title"))
        }
        .size(20)
        .width(Length::Fill)
        .align_x(Alignment::Center);

        // Directory input with browse button
        let dir_input = text_input("", &self.temp_directory)
            .on_input(|s| crate::ui::Message::CaptureDialog(CaptureMsg::ChangeDirectory(s)))
            .padding(6)
            .width(Length::Fill);

        let browse_button = button(text("📁").size(14))
            .on_press(crate::ui::Message::CaptureDialog(CaptureMsg::BrowseDirectory))
            .padding([6, 12]);

        let mut dir_row = row![
            container(text(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-folder")).size(14))
                .width(Length::Fixed(80.0))
                .align_x(Alignment::End),
            Space::new().width(8.0),
            dir_input,
            Space::new().width(4.0),
            browse_button,
        ]
        .align_y(Alignment::Center);

        if !self.temp_directory.is_empty() && !Path::new(&self.temp_directory).exists() {
            dir_row = dir_row.push(
                container(text("⚠").size(18).style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().danger.base.color),
                }))
                .padding([0, 4]),
            );
        }

        // Filename input
        let file_input = text_input("", &self.temp_filename)
            .on_input(|s| crate::ui::Message::CaptureDialog(CaptureMsg::ChangeFileName(s)))
            .padding(6)
            .width(Length::Fill);

        let file_row = row![
            container(text(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-file")).size(14))
                .width(Length::Fixed(80.0))
                .align_x(Alignment::End),
            Space::new().width(8.0),
            file_input,
        ]
        .align_y(Alignment::Center);

        // Action buttons
        let action_button: button::Button<'_, crate::ui::Message> = if self.capture_session {
            button(text(fl!(crate::LANGUAGE_LOADER, "toolbar-stop-capture")))
                .on_press(crate::ui::Message::CaptureDialog(CaptureMsg::StopCapture))
                .padding([8, 16])
                .style(button::danger)
        } else {
            button(text(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-button")))
                .on_press(crate::ui::Message::CaptureDialog(CaptureMsg::StartCapture))
                .padding([8, 16])
                .style(button::primary)
        };

        let cancel_button = button(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-cancel-button")))
            .on_press(crate::ui::Message::CaptureDialog(CaptureMsg::Cancel))
            .padding([8, 16])
            .style(button::secondary);

        let button_row = row![Space::new().width(Length::Fill), cancel_button, Space::new().width(8.0), action_button,];

        // Main content
        let modal_content = container(
            column![
                title,
                Space::new().height(12.0),
                dir_row,
                Space::new().height(8.0),
                file_row,
                Space::new().height(Length::Fill),
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
