use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length,
    widget::{Space, button, column, container, pick_list, row, text, text_input, tooltip},
};
use iced_engine_gui::settings::effect_box;
use icy_engine::{EditableScreen, SaveOptions};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::ui::MainWindowMode;

const MODAL_WIDTH: f32 = 550.0;
const MODAL_HEIGHT: f32 = 224.0;
const DIALOG_LABEL_WIDTH: f32 = 100.0;

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

    pub fn export_buffer(&self, edit_screen: Arc<Mutex<Box<dyn EditableScreen>>>) -> Result<(), String> {
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

    pub fn update(&mut self, message: ExportScreenMsg, edit_screen: Arc<Mutex<Box<dyn EditableScreen>>>) -> Option<crate::ui::Message> {
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
            ExportScreenMsg::Cancel => Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal))),
        }
    }

    pub fn view<'a>(&'a self, terminal_content: Element<'a, crate::ui::Message>) -> Element<'a, crate::ui::Message> {
        let overlay = self.create_modal_content();
        crate::ui::modal(terminal_content, overlay, crate::ui::Message::ExportDialog(ExportScreenMsg::Cancel))
    }

    fn create_modal_content(&self) -> Element<'_, crate::ui::Message> {
        let title = text(fl!(crate::LANGUAGE_LOADER, "export-dialog-title")).size(16).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..iced::Font::default()
        });

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

        let browse_button = button(text("…").size(14))
            .on_press(crate::ui::Message::ExportDialog(ExportScreenMsg::BrowseDirectory))
            .padding([6, 12]);

        let mut dir_input_row = row![dir_input, Space::new().width(4.0), browse_button].align_y(Alignment::Center);

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
            container(text(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-file")).size(14))
                .width(Length::Fixed(DIALOG_LABEL_WIDTH))
                .align_x(iced::alignment::Horizontal::Right),
            file_input,
            Space::new().width(4.0),
            format_picker,
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // Display the full path that will be saved
        let full_path_preview = text(format!("→ {}", self.get_temp_full_path().display()))
            .size(12)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().secondary.base.color),
            });

        let preview_row = row![Space::new().width(DIALOG_LABEL_WIDTH + 8.0), full_path_preview,];

        // Action buttons
        let export_enabled = !self.temp_directory.is_empty() && !self.temp_filename.is_empty() && dir_valid;

        let mut export_button = button(text(fl!(crate::LANGUAGE_LOADER, "export-dialog-export-button")))
            .padding([6, 12])
            .style(button::primary);

        if export_enabled {
            export_button = export_button.on_press(crate::ui::Message::ExportDialog(ExportScreenMsg::Export));
        }

        let cancel_button = button(text(fl!(crate::LANGUAGE_LOADER, "dialog-cancel_button")))
            .on_press(crate::ui::Message::ExportDialog(ExportScreenMsg::Cancel))
            .padding([6, 12])
            .style(button::secondary);

        let button_row = row![Space::new().width(Length::Fill), cancel_button, Space::new().width(8.0), export_button,];

        // Visual separator
        let separator = container(Space::new())
            .width(Length::Fill)
            .height(1.0)
            .style(|theme: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(theme.palette().text.scale_alpha(0.06))),
                ..Default::default()
            });

        // Main content wrapped in effect_box
        let content_box = effect_box(
            column![dir_row, Space::new().height(8.0), file_row, Space::new().height(8.0), preview_row,]
                .spacing(0)
                .into(),
        );

        // Main content
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
