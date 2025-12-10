//! Font Export Dialog
//!
//! Provides a dialog for exporting bitmap fonts to various file formats:
//! - Image files (.png, .bmp) - 16x16 grid of characters
//! - PSF files (.psf) - Linux console font format
//! - Raw bitmap fonts (.fXX) - DOS bitmap font format
//! - YAFF files (.yaff) - Yet Another Font Format (text-based)

mod image_export;

use base64::{Engine as _, engine::general_purpose};
use std::path::PathBuf;

use iced::{
    Alignment, Element, Length, Task,
    widget::{Space, column, container, pick_list, row, text, text_input},
};
use icy_engine::BitFont;
use icy_engine_edit::bitfont::MAX_FONT_HEIGHT;
use icy_engine_gui::ui::{
    DIALOG_SPACING, DIALOG_WIDTH_MEDIUM, Dialog, DialogAction, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL, browse_button, button_row, dialog_area, dialog_title,
    left_label_small, modal_container, primary_button, secondary_button, separator,
};
use icy_engine_gui::{ButtonType, settings::effect_box};

use crate::fl;
use crate::ui::Message;

use super::font_import::FontPreviewCanvas;

/// Export format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontExportFormat {
    /// PNG image (16x16 grid)
    Png,
    /// BMP image (16x16 grid)
    Bmp,
    /// PSF2 format (Linux console)
    Psf,
    /// Raw bitmap font (.fXX)
    Raw,
    /// YAFF format (text-based)
    Yaff,
    /// ANSI DCS sequence (CTerm format, copies to clipboard)
    AnsiDcs,
    /// DOS COM executable (Fontraption Non-TSR format)
    Com,
}

/// COM subformat options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComExportFormat {
    /// Fontraption Non-TSR (simplest, checksum 0xEF10)
    #[default]
    NonTsr,
    /// Fontraption TSR - 40 column modes only
    Tsr40Col,
    /// Fontraption TSR - 80 column modes only
    Tsr80Col,
    /// Fontraption TSR - all text modes
    TsrAll,
}

impl ComExportFormat {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::NonTsr => "Non-TSR (simple)",
            Self::Tsr40Col => "TSR (40-column)",
            Self::Tsr80Col => "TSR (80-column)",
            Self::TsrAll => "TSR (all modes)",
        }
    }

    pub fn all() -> Vec<Self> {
        vec![Self::NonTsr, Self::Tsr40Col, Self::Tsr80Col, Self::TsrAll]
    }
}

impl std::fmt::Display for ComExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl FontExportFormat {
    /// Get the file extension for this format
    /// For Raw format, font_height is used to generate .fXX extension
    pub fn extension(&self, font_height: i32) -> String {
        match self {
            Self::Png => "png".to_string(),
            Self::Bmp => "bmp".to_string(),
            Self::Psf => "psf".to_string(),
            Self::Raw => format!("f{:02}", font_height),
            Self::Yaff => "yaff".to_string(),
            Self::AnsiDcs => "ans".to_string(),
            Self::Com => "com".to_string(),
        }
    }

    /// Get the display name for this format
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Png => "PNG Image",
            Self::Bmp => "BMP Image",
            Self::Psf => "PSF (Linux Console)",
            Self::Raw => "Raw Binary (.fXX)",
            Self::Yaff => "YAFF (Text-based)",
            Self::AnsiDcs => "ANSI DCS",
            Self::Com => "DOS COM Executable",
        }
    }

    /// All available export formats
    pub fn all() -> Vec<Self> {
        vec![Self::Png, Self::Bmp, Self::Psf, Self::Raw, Self::Yaff, Self::AnsiDcs, Self::Com]
    }
}

impl std::fmt::Display for FontExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
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

        match self.format {
            FontExportFormat::Png => image_export::export_font_to_image(&self.font, path, image::ImageFormat::Png),
            FontExportFormat::Bmp => image_export::export_font_to_image(&self.font, path, image::ImageFormat::Bmp),
            FontExportFormat::Psf => {
                let bytes = self.font.to_psf2_bytes().map_err(|e| e.to_string())?;
                std::fs::write(path, bytes).map_err(|e| e.to_string())
            }
            FontExportFormat::Raw => {
                let bytes = export_to_raw_bytes(&self.font)?;
                std::fs::write(path, bytes).map_err(|e| e.to_string())
            }
            FontExportFormat::Yaff => {
                let yaff_string = libyaff::to_yaff_string(&self.font.yaff_font);
                std::fs::write(path, yaff_string).map_err(|e| e.to_string())
            }
            FontExportFormat::AnsiDcs => {
                let ansi_string = encode_font_as_ansi(&self.font, 0);
                std::fs::write(path, ansi_string).map_err(|e| e.to_string())
            }
            FontExportFormat::Com => {
                let bytes = export_to_com(&self.font, self.com_format)?;
                std::fs::write(path, bytes).map_err(|e| e.to_string())
            }
        }
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
                            .add_filter("Font file", &extensions.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                            .save_file()
                            .await;
                        handle.map(|h| h.path().to_path_buf())
                    },
                    |path| Message::FontExport(FontExportMessage::FileSelected(path)),
                )))
            }
            FontExportMessage::FileSelected(path) => {
                self.export_path = path.clone();
                self.error = None;
                self.success = None;
                Some(DialogAction::None)
            }
            FontExportMessage::Export => match self.do_export() {
                Ok(()) => Some(DialogAction::CloseWith(Message::FontExported)),
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

        let canvas = iced::widget::Canvas::new(FontPreviewCanvas {
            font: &self.font,
            cell_width: cell_size,
            cell_height: cell_size,
            label_size,
        })
        .width(Length::Fixed(label_size + 16.0 * cell_size))
        .height(Length::Fixed(label_size + 16.0 * cell_size));

        container(canvas)
            .style(|theme: &iced::Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(palette.background.weak.color.into()),
                    border: iced::Border {
                        color: palette.background.strong.color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                }
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
        let format_picker = pick_list(FontExportFormat::all(), Some(self.format), |f| {
            Message::FontExport(FontExportMessage::SetFormat(f))
        })
        .width(Length::Fill);

        let format_row = row![format_label, format_picker].spacing(DIALOG_SPACING).align_y(Alignment::Center);

        // === COM SUBFORMAT (only shown when COM is selected) ===
        let com_format_element: Element<'_, Message> = if self.format == FontExportFormat::Com {
            let com_label = left_label_small(fl!("font-export-com-format"));
            let com_picker = pick_list(ComExportFormat::all(), Some(self.com_format), |f| {
                Message::FontExport(FontExportMessage::SetComFormat(f))
            })
            .width(Length::Fill);
            row![com_label, com_picker].spacing(DIALOG_SPACING).align_y(Alignment::Center).into()
        } else {
            Space::new().height(0).into()
        };

        // === FILE PATH ROW ===
        let path_label = left_label_small(fl!("font-export-path"));
        let path_text = self.export_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();

        let path_input = text_input(&fl!("font-export-no-path"), &path_text)
            .on_input(|s| Message::FontExport(FontExportMessage::SetPath(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        let browse_btn = browse_button(Message::FontExport(FontExportMessage::Browse));

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
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().danger.base.color),
                })
                .into()
        } else if let Some(success) = &self.success {
            text(success)
                .size(TEXT_SIZE_SMALL)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().success.base.color),
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
            secondary_button(format!("{}", ButtonType::Cancel), Some(Message::FontExport(FontExportMessage::Cancel))).into(),
            primary_button(fl!("font-export-button"), can_export.then(|| Message::FontExport(FontExportMessage::Export))).into(),
        ]);

        let dialog_content = dialog_area(column![title, Space::new().height(DIALOG_SPACING), content_box].into());
        let button_area = dialog_area(buttons.into());

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area].into(),
            DIALOG_WIDTH_MEDIUM,
        )
        .into()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        let Message::FontExport(msg) = message else {
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
                Ok(()) => DialogAction::CloseWith(Message::FontExported),
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

/// Export font to raw bitmap format (8 pixels wide)
fn export_to_raw_bytes(font: &BitFont) -> Result<Vec<u8>, String> {
    let height = font.size().height as usize;
    let width = font.size().width as usize;
    let mut data = Vec::with_capacity(256 * height);

    for ch_code in 0..256u32 {
        let glyph = font.get_glyph(unsafe { char::from_u32_unchecked(ch_code) });

        for row in 0..height {
            let mut row_byte: u8 = 0;

            if let Some(glyph_def) = &glyph {
                if let Some(row_pixels) = glyph_def.bitmap.pixels.get(row) {
                    for (x, &is_set) in row_pixels.iter().enumerate().take(8.min(width)) {
                        if is_set {
                            row_byte |= 1 << (7 - x);
                        }
                    }
                }
            }

            data.push(row_byte);
        }
    }

    Ok(data)
}

/// Encode font as ANSI DCS sequence (CTerm format)
///
/// Creates a Device Control String that can be used to upload a font
/// to terminals supporting CTerm font sequences.
/// Format: ESC P CTerm:Font:<slot>:<base64-data> ESC \
fn encode_font_as_ansi(font: &BitFont, font_slot: usize) -> String {
    let font_data = convert_font_to_u8_data(font);
    let data = general_purpose::STANDARD.encode(font_data);
    format!("\x1BPCTerm:Font:{font_slot}:{data}\x1B\\")
}

/// Convert font to raw u8 data for ANSI encoding
fn convert_font_to_u8_data(font: &BitFont) -> Vec<u8> {
    let size = font.size();
    let bytes_per_row = (size.width as usize + 7) / 8;
    let mut result = Vec::new();

    for ch_code in 0..256u32 {
        let ch = unsafe { char::from_u32_unchecked(ch_code) };
        if let Some(glyph) = font.get_glyph(ch) {
            let mut rows = vec![0u8; bytes_per_row * size.height as usize];
            for (y, row) in glyph.bitmap.pixels.iter().enumerate() {
                if y >= size.height as usize {
                    break;
                }
                for (x, &is_set) in row.iter().enumerate() {
                    if x >= size.width as usize {
                        break;
                    }
                    if is_set {
                        let byte_idx = y * bytes_per_row + x / 8;
                        let bit_idx = 7 - (x % 8);
                        rows[byte_idx] |= 1 << bit_idx;
                    }
                }
            }
            result.extend_from_slice(&rows);
        } else {
            // No glyph found, add empty rows
            result.extend_from_slice(&vec![0; bytes_per_row * size.height as usize]);
        }
    }
    result
}

/// Export font to DOS COM executable format (Fontraption format)
///
/// Supports two formats from Fontraption:
/// - Non-TSR: Simple executable that loads the font and exits
/// - TSR: Terminate-and-stay-resident, can be unloaded later
///
/// COM format headers are based on VileR's Fontraption source code.
fn export_to_com(font: &BitFont, com_format: ComExportFormat) -> Result<Vec<u8>, String> {
    let height = font.size().height;

    if height == 0 || height > MAX_FONT_HEIGHT as usize {
        return Err(format!(
            "Font height {} is not supported for COM export (must be 1-{})",
            height, MAX_FONT_HEIGHT
        ));
    }

    // Get raw font data (256 chars × height bytes)
    let font_data = export_to_raw_bytes(font)?;

    match com_format {
        ComExportFormat::NonTsr => export_non_tsr_com(height as u8, &font_data),
        ComExportFormat::Tsr40Col => export_tsr_com(height as u8, &font_data, true, false),
        ComExportFormat::Tsr80Col => export_tsr_com(height as u8, &font_data, false, true),
        ComExportFormat::TsrAll => export_tsr_com(height as u8, &font_data, true, true),
    }
}

/// Export font as Non-TSR COM (Fontraption simple format)
///
/// Format:
/// - Header: 25 bytes (0x19)
/// - Font height at offset 0x15
/// - Font data starts at offset 0x19
fn export_non_tsr_com(height: u8, font_data: &[u8]) -> Result<Vec<u8>, String> {
    // Header from Fontraption's FORMATS.inc (head_com)
    // This is a minimal DOS program that uses INT 10h AH=11h to load the font
    #[rustfmt::skip]
    let header: [u8; 25] = [
        0x56, 0x49, 0x4C, 0x45, 0x1A, 0x00, 0x83, 0xC4, 0x03, 0xB8,
        0x10, 0x11, 0xBD, 0x19, 0x01, 0xB9, 0x00, 0x01, 0x99, 0xBB,
        0x00, height, // height at offset 0x15
        0xCD, 0x10, 0xC3, // INT 10h, RET
    ];

    let mut result = Vec::with_capacity(25 + font_data.len());
    result.extend_from_slice(&header);
    result.extend_from_slice(font_data);

    Ok(result)
}

/// Export font as TSR COM (Fontraption TSR format)
///
/// This creates a terminate-and-stay-resident program that can intercept
/// video mode changes and reload the font automatically.
fn export_tsr_com(height: u8, font_data: &[u8], modes_40: bool, modes_80: bool) -> Result<Vec<u8>, String> {
    // Calculate offsets
    let font_size = font_data.len();
    let header_size = 0x63;
    let data_end = header_size + font_size;

    // OFFSET_INIT = 0x63 + (256 * height)
    let offset_init = data_end;

    // Header from Fontraption's FORMATS.inc (head_tsr)
    // This is a TSR that hooks INT 10h to reload the font on mode changes
    #[rustfmt::skip]
    let mut header: Vec<u8> = vec![
        0xE9, 0x00, 0x00, // JMP to init (patched below)
        0x00, 0x00, 0x80, 0xFC, 0x00, 0x75, 0x10, 0x3C, 0x03,
        0x77, 0xF2, 0x53, 0x89, 0xC3, 0x2E, 0x8A, 0x9F, 0x2D, 0x01, 0x4B, 0x5B,
        0x74, 0x17, 0x3D, 0x00, 0x12, 0x75, 0xE1, 0x80, 0xFB, 0x21, 0x75, 0xDC,
        0xB0, 0x21, 0xCF, 0x0D,
        0x56, 0x49, 0x4C, 0x45, // 'VILE' signature at 0x28
        0x1A,
        0x00, 0x00, 0x00, 0x00, // modes 0,1 and 2,3 flags (patched below)
        0x9C, 0x0E, 0xE8, 0xCA, 0xFF, 0x50, 0x51, 0x52, 0x53, 0x55, 0x56,
        0x57, 0x1E, 0x06, 0x0E, 0x0E, 0x1F, 0x07, 0xE8, 0x0F, 0x00, 0x9C, 0x0E,
        0xE8, 0xB5, 0xFF, 0x07, 0x1F, 0x5F, 0x5E, 0x5D, 0x5B, 0x5A, 0x59, 0x58,
        0xCF, 0xB8, 0x10, 0x11, 0xBD, 0x63, 0x01, 0xBB, 0x00,
        height, // font height at 0x5D
        0xB9, 0x00, 0x01, 0x99, 0xC3,
    ];

    // Patch the JMP offset (offset 0x01-0x02) = OFFSET_INIT - 3
    let jmp_offset = (offset_init - 3) as u16;
    header[1] = (jmp_offset & 0xFF) as u8;
    header[2] = ((jmp_offset >> 8) & 0xFF) as u8;

    // Patch mode flags at offsets 0x2D and 0x2F
    let mode_01 = if modes_40 { 0x01u8 } else { 0x00u8 };
    let mode_23 = if modes_80 { 0x01u8 } else { 0x00u8 };
    header[0x2D] = mode_01;
    header[0x2E] = mode_01;
    header[0x2F] = mode_23;
    header[0x30] = mode_23;

    // Tail from Fontraption's FORMATS.inc (tail_tsr)
    #[rustfmt::skip]
    let mut tail: Vec<u8> = vec![
        0xB8, 0x00, 0x12, 0xB3, 0x21, 0xCD, 0x10, 0x3C, 0x21, 0x75, 0x3E, 0xBA,
        0x00, 0x00, // OFFSET_TXT_PRE (patched)
        0xE8, 0x77, 0x00, 0x31, 0xC0, 0x8E, 0xD8, 0x8E, 0xC0, 0xC5,
        0x1E, 0x40, 0x00, 0x81, 0x7F, 0x23, 0x56, 0x49, 0x75, 0x1E, 0x81, 0x7F,
        0x25, 0x4C, 0x45, 0x75, 0x17, 0xFA, 0xBE, 0x01, 0x01, 0xBF, 0x40, 0x00,
        0xA5, 0xA5, 0xFB, 0x1E, 0x07, 0xB4, 0x49, 0xCD, 0x21, 0x72, 0x05, 0xBA,
        0x00, 0x00, // OFFSET_TXT_GOOD (patched)
        0xEB, 0x03, 0xBA,
        0x00, 0x00, // OFFSET_TXT_BAD (patched)
        0x0E, 0x1F, 0xE8, 0x40, 0x00,
        0xC3, 0xB8, 0x10, 0x35, 0xCD, 0x21, 0xFE, 0x06, 0x00, 0x01, 0x89, 0x1E,
        0x01, 0x01, 0x8C, 0x06, 0x03, 0x01, 0xBA, 0x05, 0x01, 0xB4, 0x25, 0xCD,
        0x21, 0xB4, 0x0F, 0xCD, 0x10, 0x3C, 0x03, 0x77, 0x0F, 0xBB, 0x2D, 0x01,
        0xD7, 0xFE, 0xC8, 0x75, 0x07, 0x0E, 0x07, 0xE8,
        0x00, 0x00, // DISPLC_PREP (patched)
        0xCD, 0x10,
        0x8E, 0x06, 0x2C, 0x00, 0xB4, 0x49, 0xCD, 0x21, 0xBA,
        0x00, 0x00, // NUMPAR (patched)
        0xB8,
        0x00, 0x31, 0xCD, 0x21, 0xB4, 0x09, 0xCD, 0x21, 0xC3,
        // Text strings
        b'L', b'a', b's', b't', b' ', b'T', b'S', b'R', b' ', b'f', b'o', b'n', b't', b' ', b'$',
        b'r', b'e', b'm', b'o', b'v', b'e', b'd', 13, 10, b'$',
        b'u', b'n', b'r', b'e', b'm', b'o', b'v', b'a', b'b', b'l', b'e', b'!', 13, 10, b'$',
    ];

    // Patch offsets in tail
    // OFFSET_TXT_PRE = OFFSET_INIT + 0x18D
    let offset_txt_pre = (offset_init + 0x18D) as u16;
    tail[0x0C] = (offset_txt_pre & 0xFF) as u8;
    tail[0x0D] = ((offset_txt_pre >> 8) & 0xFF) as u8;

    // OFFSET_TXT_GOOD = OFFSET_INIT + 0x19C
    let offset_txt_good = (offset_init + 0x19C) as u16;
    tail[0x3C] = (offset_txt_good & 0xFF) as u8;
    tail[0x3D] = ((offset_txt_good >> 8) & 0xFF) as u8;

    // OFFSET_TXT_BAD = OFFSET_INIT + 0x1A6
    let offset_txt_bad = (offset_init + 0x1A6) as u16;
    tail[0x41] = (offset_txt_bad & 0xFF) as u8;
    tail[0x42] = ((offset_txt_bad >> 8) & 0xFF) as u8;

    // DISPLC_PREP = -(OFFSET_INIT + 0x21)
    let displc_prep = (!((offset_init + 0x21) as u16)).wrapping_add(1);
    tail[0x74] = (displc_prep & 0xFF) as u8;
    tail[0x75] = ((displc_prep >> 8) & 0xFF) as u8;

    // NUMPAR = (OFFSET_INIT >> 4) + 1 (rounded up)
    let numpar = ((offset_init + 0x0F) >> 4) as u16;
    tail[0x81] = (numpar & 0xFF) as u8;
    tail[0x82] = ((numpar >> 8) & 0xFF) as u8;

    // Combine header + font data + tail
    let mut result = Vec::with_capacity(header.len() + font_data.len() + tail.len());
    result.extend_from_slice(&header);
    result.extend_from_slice(font_data);
    result.extend_from_slice(&tail);

    Ok(result)
}
