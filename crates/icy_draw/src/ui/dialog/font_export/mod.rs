//! Font Export Dialog
//!
//! Provides a dialog for exporting bitmap fonts to various file formats:
//! - Image files (.png, .bmp) - 16x16 grid of characters
//! - PSF files (.psf) - Linux console font format
//! - Raw bitmap fonts (.fXX) - DOS bitmap font format
//! - YAFF files (.yaff) - Yet Another Font Format (text-based)

pub use icy_draw::font_export::{ComExportFormat, FontExportFormat};
use std::path::PathBuf;

use icy_engine::BitFont;
use icy_engine_gui::ui::{
    browse_button, button_row, dialog_area, dialog_title, left_label_small, modal_container, primary_button, secondary_button, separator, Dialog, DialogAction,
    DIALOG_SPACING, DIALOG_WIDTH_MEDIUM, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL,
};
use icy_engine_gui::{settings::effect_box, ButtonType};
use icy_ui::{
    widget::{column, container, pick_list, row, text, text_input, Space},
    Alignment, Element, Length, Task,
};

use crate::fl;
use crate::ui::editor::bitfont::BitFontEditorMessage;
use crate::ui::Message;

use super::font_import::FontPreviewCanvas;

/// Helper to wrap `FontExportMessage` in Message
fn msg(m: FontExportMessage) -> Message {
    Message::BitFontEditor(BitFontEditorMessage::FontExportDialog(m))
}

/// Messages for the Font Export dialog
#[derive(Debug, Clone)]
pub enum FontExportMessage {
    /// Format selection changed
    SetFormat(FontExportFormat),
    /// COM subformat selection changed
    SetComFormat(ComExportFormat),
    /// Path text input changed
    SetPath(String),
    /// Browse for export location
    Browse,
    /// File path selected from browser
    FileSelected(Option<PathBuf>),
    /// Export the font
    Export,
    /// Cancel the dialog
    Cancel,
}

/// State for the Font Export dialog
pub struct FontExportDialog {
    /// The font to export
    pub font: BitFont,
    /// Selected export format
    pub format: FontExportFormat,
    /// Selected COM subformat (when format is Com)
    pub com_format: ComExportFormat,
    /// Selected export path (if any)
    pub export_path: Option<PathBuf>,
    /// Error message (if any)
    pub error: Option<String>,
    /// Success message (if any)
    pub success: Option<String>,
}

impl FontExportDialog {
    /// Create a new Font Export dialog
    pub fn new(font: BitFont) -> Self {
        Self {
            font,
            format: FontExportFormat::Png,
            com_format: ComExportFormat::default(),
            export_path: None,
            error: None,
            success: None,
        }
    }

    /// Get the file filter for the current format
    fn get_file_filter(&self) -> Vec<String> {
        vec![self.format.extension(self.font.size().height)]
    }

    /// Get the default filename for export
    fn get_default_filename(&self) -> String {
        format!("{}.{}", self.font.name(), self.format.extension(self.font.size().height))
    }

    /// Export the font to the selected path
    fn do_export(&mut self) -> Result<(), String> {
        let Some(path) = &self.export_path else {
            return Err("No export path selected".to_string());
        };

        let bytes = icy_draw::font_export::encode(&self.font, self.format, self.com_format)?;
        std::fs::write(path, bytes).map_err(|error| error.to_string())
    }

    /// Handle internal messages
    fn update_internal(&mut self, message: &FontExportMessage) -> Option<DialogAction<Message>> {
        match message {
            FontExportMessage::SetFormat(format) => {
                self.format = *format;
                self.error = None;
                self.success = None;
                // Update path extension if we have a path
                if let Some(path) = &self.export_path {
                    let new_path = path.with_extension(self.format.extension(self.font.size().height));
                    self.export_path = Some(new_path);
                }
                Some(DialogAction::None)
            }
            FontExportMessage::SetComFormat(com_format) => {
                self.com_format = *com_format;
                self.error = None;
                self.success = None;
                Some(DialogAction::None)
            }
            FontExportMessage::SetPath(path) => {
                if path.is_empty() {
                    self.export_path = None;
                } else {
                    self.export_path = Some(PathBuf::from(path));
                }
                self.error = None;
                self.success = None;
                Some(DialogAction::None)
            }
            FontExportMessage::Browse => {
                let extensions = self.get_file_filter();
                let default_name = self.get_default_filename();

                Some(DialogAction::RunTask(Task::perform(
                    async move {
                        let handle = rfd::AsyncFileDialog::new()
                            .set_file_name(&default_name)
                            .add_filter("Font file", &extensions.iter().map(std::string::String::as_str).collect::<Vec<_>>())
                            .save_file()
                            .await;
                        handle.map(|h| h.path().to_path_buf())
                    },
                    |path| msg(FontExportMessage::FileSelected(path)),
                )))
            }
            FontExportMessage::FileSelected(path) => {
                self.export_path = path.clone();
                self.error = None;
                self.success = None;
                Some(DialogAction::None)
            }
            FontExportMessage::Export => match self.do_export() {
                Ok(()) => Some(DialogAction::CloseWith(Message::BitFontEditor(BitFontEditorMessage::FontExported))),
                Err(e) => {
                    self.error = Some(e);
                    Some(DialogAction::None)
                }
            },
            FontExportMessage::Cancel => Some(DialogAction::Close),
        }
    }

    /// View the font preview
    fn view_preview(&self) -> Element<'_, Message> {
        // Calculate cell size to fit 16x16 grid in ~240 pixels (leaving room for labels)
        let available: f32 = 240.0;
        let cell_size: f32 = (available / 16.0).floor();
        let label_size: f32 = 16.0;

        let canvas = icy_ui::widget::Canvas::new(FontPreviewCanvas {
            font: &self.font,
            cell_width: cell_size,
            cell_height: cell_size,
            label_size,
        })
        .width(Length::Fixed(label_size + 16.0 * cell_size))
        .height(Length::Fixed(label_size + 16.0 * cell_size));

        container(canvas)
            .style(|theme: &icy_ui::Theme| container::Style {
                background: Some(theme.secondary.base.into()),
                border: icy_ui::Border {
                    color: theme.primary.divider,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
            .padding(8)
            .into()
    }

    /// View font info
    fn view_info(&self) -> Element<'_, Message> {
        let font_name = text(format!("Font: {}", self.font.name())).size(TEXT_SIZE_NORMAL);
        let font_size = text(format!("Size: {}×{} pixels", self.font.size().width, self.font.size().height)).size(TEXT_SIZE_SMALL);
        let glyph_count = text(format!("Glyphs: {}", 256)).size(TEXT_SIZE_SMALL);

        column![font_name, font_size, glyph_count].spacing(4).into()
    }
}

impl Dialog<Message> for FontExportDialog {
    fn view(&self) -> Element<'_, Message> {
        let title = dialog_title(fl!("menu-export-font").trim_end_matches('…').to_string());

        // === FORMAT SELECTION ===
        let format_label = left_label_small(fl!("font-export-format"));
        let format_picker = pick_list(FontExportFormat::all(), Some(self.format), |f| msg(FontExportMessage::SetFormat(f))).width(Length::Fill);

        let format_row = row![format_label, format_picker].spacing(DIALOG_SPACING).align_y(Alignment::Center);

        // === COM SUBFORMAT (only shown when COM is selected) ===
        let com_format_element: Element<'_, Message> = if self.format == FontExportFormat::Com {
            let com_label = left_label_small(fl!("font-export-com-format"));
            let com_picker = pick_list(ComExportFormat::all(), Some(self.com_format), |f| msg(FontExportMessage::SetComFormat(f))).width(Length::Fill);
            row![com_label, com_picker].spacing(DIALOG_SPACING).align_y(Alignment::Center).into()
        } else {
            Space::new().height(0).into()
        };

        // === FILE PATH ROW ===
        let path_label = left_label_small(fl!("font-export-path"));
        let path_text = self.export_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();

        let path_input = text_input(&fl!("font-export-no-path"), &path_text)
            .on_input(|s| msg(FontExportMessage::SetPath(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        let browse_btn = browse_button(msg(FontExportMessage::Browse));

        let file_row = row![path_label, path_input, browse_btn].spacing(DIALOG_SPACING).align_y(Alignment::Center);

        // === PREVIEW ===
        let preview_element = self.view_preview();
        let info_element = self.view_info();

        let preview_row = row![
            container(preview_element).width(Length::Shrink),
            Space::new().width(DIALOG_SPACING * 2.0),
            container(info_element).width(Length::Fill),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Start);

        // === ERROR/SUCCESS MESSAGE ===
        let message_element: Element<'_, Message> = if let Some(err) = &self.error {
            text(err)
                .size(TEXT_SIZE_SMALL)
                .style(|theme: &icy_ui::Theme| icy_ui::widget::text::Style {
                    color: Some(theme.destructive.base),
                })
                .into()
        } else if let Some(success) = &self.success {
            text(success)
                .size(TEXT_SIZE_SMALL)
                .style(|theme: &icy_ui::Theme| icy_ui::widget::text::Style {
                    color: Some(theme.success.base),
                })
                .into()
        } else {
            Space::new().height(0).into()
        };

        // === CONTENT ===
        let content_column = column![
            format_row,
            com_format_element,
            file_row,
            Space::new().height(DIALOG_SPACING),
            preview_row,
            message_element,
        ]
        .spacing(DIALOG_SPACING);

        let content_box = effect_box(content_column.into());

        // === BUTTONS ===
        let can_export = self.export_path.is_some();
        let buttons = button_row(vec![
            secondary_button(format!("{}", ButtonType::Cancel), Some(msg(FontExportMessage::Cancel))).into(),
            primary_button(fl!("font-export-button"), can_export.then(|| msg(FontExportMessage::Export))).into(),
        ]);

        let dialog_content = dialog_area(column![title, Space::new().height(DIALOG_SPACING), content_box].into());
        let button_area = dialog_area(buttons);

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area].into(),
            DIALOG_WIDTH_MEDIUM,
        )
        .into()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        let Message::BitFontEditor(BitFontEditorMessage::FontExportDialog(msg)) = message else {
            return None;
        };
        self.update_internal(msg)
    }

    fn request_cancel(&mut self) -> DialogAction<Message> {
        DialogAction::Close
    }

    fn request_confirm(&mut self) -> DialogAction<Message> {
        if self.export_path.is_some() {
            match self.do_export() {
                Ok(()) => DialogAction::CloseWith(Message::BitFontEditor(BitFontEditorMessage::FontExported)),
                Err(e) => {
                    self.error = Some(e);
                    DialogAction::None
                }
            }
        } else {
            self.error = Some("No export path selected".to_string());
            DialogAction::None
        }
    }
}
