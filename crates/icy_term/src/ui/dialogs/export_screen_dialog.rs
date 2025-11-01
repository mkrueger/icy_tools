use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length,
    widget::{Space, button, column, container, pick_list, row, text, text_input},
};
use icy_engine::{SaveOptions, editor::EditState};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const MODAL_WIDTH: f32 = 500.0;
const MODAL_HEIGHT: f32 = 200.0;

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

    pub fn export_buffer(&self, edit_state: Arc<Mutex<EditState>>) -> Result<(), String> {
        let full_path = self.get_full_path();

        // Create directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&self.export_directory) {
            return Err(format!("Failed to create directory: {}", e));
        }

        // Get the buffer from edit state
        let mut edit_state = edit_state.lock().map_err(|e| format!("Failed to lock edit state: {}", e))?;
        let buffer = edit_state.get_buffer_mut();

        // Get the file extension for format
        let ext = self.export_format.extension();

        // Convert buffer to bytes based on format
        let content = buffer
            .to_bytes(ext, &SaveOptions::new())
            .map_err(|e| format!("Failed to convert buffer: {}", e))?;

        // Write the bytes to file
        std::fs::write(&full_path, &content).map_err(|e| format!("Failed to write file: {}", e))?;

        Ok(())
    }

    pub fn update(&mut self, message: ExportScreenMsg, edit_state: Arc<Mutex<EditState>>) -> Option<crate::ui::Message> {
        match message {
            ExportScreenMsg::Export => {
                // Update the actual values
                self.export_directory = self.temp_directory.clone();
                self.export_filename = self.temp_filename.clone();
                self.export_format = self.temp_format;

                // Perform the export
                match self.export_buffer(edit_state) {
                    Ok(_) => {
                        log::info!("Successfully exported to: {}", self.get_full_path().display());
                        // Close the dialog after successful export
                        Some(crate::ui::Message::CloseDialog)
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
            ExportScreenMsg::Cancel => Some(crate::ui::Message::CloseDialog),
        }
    }

    pub fn view<'a>(&'a self, terminal_content: Element<'a, crate::ui::Message>) -> Element<'a, crate::ui::Message> {
        let overlay = self.create_modal_content();
        crate::ui::modal(terminal_content, overlay, crate::ui::Message::ExportDialog(ExportScreenMsg::Cancel))
    }

    fn create_modal_content(&self) -> Element<'_, crate::ui::Message> {
        let title = text(fl!(crate::LANGUAGE_LOADER, "export-dialog-title"))
            .size(20)
            .width(Length::Fill)
            .align_x(Alignment::Center);

        // Directory input with browse button
        let dir_input = text_input("", &self.temp_directory)
            .on_input(|s| crate::ui::Message::ExportDialog(ExportScreenMsg::ChangeDirectory(s)))
            .padding(6)
            .width(Length::Fill);

        let browse_button = button(text("📁").size(14))
            .on_press(crate::ui::Message::ExportDialog(ExportScreenMsg::BrowseDirectory))
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

        // Add warning if directory doesn't exist
        if !self.temp_directory.is_empty() && !Path::new(&self.temp_directory).exists() {
            dir_row = dir_row.push(
                container(text("⚠").size(18).style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().danger.base.color),
                }))
                .padding([0, 4]),
            );
        }

        // Filename input with format picker
        let file_input = text_input("", &self.temp_filename)
            .on_input(|s| crate::ui::Message::ExportDialog(ExportScreenMsg::ChangeFileName(s)))
            .padding(6)
            .width(Length::Fill);

        let format_picker = pick_list(&ExportFormat::ALL[..], Some(self.temp_format), |format| {
            crate::ui::Message::ExportDialog(ExportScreenMsg::ChangeFormat(format))
        })
        .padding(6)
        .width(Length::Fixed(120.0));

        let file_row = row![
            container(text(fl!(crate::LANGUAGE_LOADER, "capture-dialog-capture-file")).size(14))
                .width(Length::Fixed(80.0))
                .align_x(Alignment::End),
            Space::new().width(8.0),
            file_input,
            Space::new().width(4.0),
            format_picker,
        ]
        .align_y(Alignment::Center);

        // Display the full path that will be saved
        let full_path_preview = text(format!("→ {}", self.get_temp_full_path().display()))
            .size(12)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().secondary.base.color),
            });

        let preview_row = row![Space::new().width(88.0), full_path_preview,];

        // Action buttons
        let export_enabled = !self.temp_directory.is_empty() && !self.temp_filename.is_empty() && Path::new(&self.temp_directory).exists();

        let mut export_button = button(text(fl!(crate::LANGUAGE_LOADER, "export-dialog-export-button")))
            .padding([8, 16])
            .style(button::primary);

        if export_enabled {
            export_button = export_button.on_press(crate::ui::Message::ExportDialog(ExportScreenMsg::Export));
        }

        let cancel_button = button(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-cancel-button")))
            .on_press(crate::ui::Message::ExportDialog(ExportScreenMsg::Cancel))
            .padding([8, 16])
            .style(button::secondary);

        let button_row = row![Space::new().width(Length::Fill), cancel_button, Space::new().width(8.0), export_button,];

        // Main content
        let modal_content = container(
            column![
                title,
                Space::new().height(12.0),
                dir_row,
                Space::new().height(8.0),
                file_row,
                Space::new().height(4.0),
                preview_row,
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
