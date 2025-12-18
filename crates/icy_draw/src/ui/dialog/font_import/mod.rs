//! Font Import Dialog
//!
//! Provides a dialog for importing fonts from various file formats:
//! - Native font files (.yaff, .psf, .f08, etc.) - direct import
//! - XB files - import font from XBin with font selection (1 or 2 fonts)
//! - Image files - convert raster image to bitmap font
//! - TTF/OTF files - rasterize TrueType/OpenType fonts to bitmap

mod canvas;
mod image_import;
mod ttf_import;

pub use canvas::*;

use std::path::PathBuf;

use iced::{
    Alignment, Element, Length, Task,
    widget::{Space, checkbox, column, container, pick_list, row, text, text_input},
};
use icy_engine::BitFont;
use icy_engine_edit::bitfont::MAX_FONT_HEIGHT;
use icy_engine_gui::ui::{
    DIALOG_SPACING, DIALOG_WIDTH_LARGE, Dialog, DialogAction, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL, browse_button, button_row, dialog_area, dialog_title,
    left_label_small, modal_container, primary_button, secondary_button, separator,
};
use icy_engine_gui::{ButtonType, settings::effect_box};

use crate::fl;
use crate::ui::Message;
use crate::ui::editor::bitfont::BitFontEditorMessage;

/// Helper to wrap FontImportMessage in Message
fn msg(m: FontImportMessage) -> Message {
    Message::BitFontEditor(BitFontEditorMessage::FontImportDialog(m))
}

/// Type of font source being imported
#[derive(Debug, Clone, PartialEq)]
pub enum FontSourceType {
    /// Native font file (.yaff, .psf, .fXX)
    NativeFont,
    /// XB file with embedded font(s)
    XBin { has_second_font: bool },
    /// Image file to convert to font
    Image,
}

/// Messages for the Font Import dialog
#[derive(Debug, Clone)]
pub enum FontImportMessage {
    /// File path input changed
    SetFilePath(String),
    /// Browse button clicked
    Browse,
    /// File selected from browser
    FileSelected(Option<PathBuf>),
    /// For XB files: select which font to import (0 or 1)
    SelectXBFont(usize),
    /// For image import: set font width
    SetFontWidth(String),
    /// For image import: set font height
    SetFontHeight(String),
    /// For image import: toggle dithering
    SetDithering(bool),
    /// Import the font
    Import,
    /// Cancel the dialog
    Cancel,
}

/// State for the Font Import dialog
pub struct FontImportDialog {
    /// Current file path
    pub file_path: String,
    /// Detected font source type
    pub source_type: Option<FontSourceType>,
    /// Loaded font preview (if available)
    pub preview_font: Option<BitFont>,
    /// For XB files: available fonts
    pub xb_fonts: Vec<BitFont>,
    /// For XB files: selected font index
    pub xb_selected_font: usize,
    /// For image import: target font width
    pub image_width: String,
    /// For image import: target font height
    pub image_height: String,
    /// For image import: use dithering
    pub use_dithering: bool,
    /// Error message (if any)
    pub error: Option<String>,
}

impl Default for FontImportDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl FontImportDialog {
    /// Create a new Font Import dialog
    pub fn new() -> Self {
        Self {
            file_path: String::new(),
            source_type: None,
            preview_font: None,
            xb_fonts: Vec::new(),
            xb_selected_font: 0,
            image_width: "8".to_string(),
            image_height: "16".to_string(),
            use_dithering: true,
            error: None,
        }
    }

    /// Parse font width for image import
    fn parsed_font_width(&self) -> Option<i32> {
        self.image_width.parse::<i32>().ok().filter(|&w| w >= 1)
    }

    /// Parse font height for image import
    fn parsed_font_height(&self) -> Option<i32> {
        self.image_height.parse::<i32>().ok().filter(|&h| h >= 1)
    }

    /// Check if the dialog is ready for import
    fn can_import(&self) -> bool {
        match &self.source_type {
            Some(FontSourceType::NativeFont) => self.preview_font.is_some(),
            Some(FontSourceType::XBin { .. }) => !self.xb_fonts.is_empty(),
            Some(FontSourceType::Image) => self.parsed_font_width().is_some() && self.parsed_font_height().is_some() && self.preview_font.is_some(),
            None => false,
        }
    }

    /// Load file and detect type
    fn load_file(&mut self, path: &std::path::Path) {
        self.error = None;
        self.preview_font = None;
        self.xb_fonts.clear();
        self.xb_selected_font = 0;

        let ext = path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()).unwrap_or_default();

        // Determine file type and load accordingly
        if is_native_font_extension(&ext) {
            self.source_type = Some(FontSourceType::NativeFont);
            self.load_native_font(path);
        } else if ext == "xb" {
            self.load_xb_file(path);
        } else if ext == "com" {
            self.source_type = Some(FontSourceType::NativeFont);
            self.load_com_file(path);
        } else if is_ttf_extension(&ext) {
            self.source_type = Some(FontSourceType::Image); // Reuse Image type for dimension config
            self.load_ttf_file(path);
        } else if is_image_extension(&ext) {
            self.source_type = Some(FontSourceType::Image);
            self.auto_detect_image_dimensions(path);
            self.load_image_file(path);
        } else {
            self.error = Some(format!("Unsupported file type: .{}", ext));
            self.source_type = None;
        }
    }

    /// Load a native font file
    fn load_native_font(&mut self, path: &std::path::Path) {
        match std::fs::read(path) {
            Ok(data) => {
                let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Font").to_string();
                match BitFont::from_bytes(name, &data) {
                    Ok(font) => {
                        self.preview_font = Some(font);
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to parse font: {}", e));
                    }
                }
            }
            Err(e) => {
                self.error = Some(format!("Failed to read file: {}", e));
            }
        }
    }

    /// Load a DOS COM file and extract font
    ///
    /// Supports multiple COM font formats:
    /// - PCMag FontEdit .COM: checksum 0x8696, height at 0x32, data at 0x63
    /// - Fontraption Non-TSR .COM: checksum 0xEF10, height at 0x15, data at 0x19  
    /// - Fontraption TSR .COM: 'VILE' at 0x28, height at 0x5D, data at 0x63
    fn load_com_file(&mut self, path: &std::path::Path) {
        match std::fs::read(path) {
            Ok(data) => {
                let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Font").to_string();

                match parse_com_font(&name, &data) {
                    Ok(font) => {
                        self.preview_font = Some(font);
                    }
                    Err(e) => {
                        self.error = Some(e);
                    }
                }
            }
            Err(e) => {
                self.error = Some(format!("Failed to read file: {}", e));
            }
        }
    }

    /// Load an XB file and extract fonts
    fn load_xb_file(&mut self, path: &std::path::Path) {
        match std::fs::read(path) {
            Ok(data) => {
                // Parse XB file using icy_engine
                match icy_engine::formats::FileFormat::XBin.from_bytes(&data, None) {
                    Ok(screen) => {
                        let buffer = screen.buffer;
                        // Extract fonts from the loaded buffer
                        let mut fonts = Vec::new();

                        if let Some(font) = buffer.font(0) {
                            fonts.push(font.clone());
                        }
                        if let Some(font) = buffer.font(1) {
                            fonts.push(font.clone());
                        }

                        if fonts.is_empty() {
                            self.error = Some("XB file contains no custom fonts".to_string());
                            self.source_type = None;
                        } else {
                            self.source_type = Some(FontSourceType::XBin {
                                has_second_font: fonts.len() > 1,
                            });
                            self.preview_font = Some(fonts[0].clone());
                            self.xb_fonts = fonts;
                        }
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to parse XB file: {}", e));
                        self.source_type = None;
                    }
                }
            }
            Err(e) => {
                self.error = Some(format!("Failed to read file: {}", e));
            }
        }
    }

    /// Load a TTF/OTF file and rasterize to bitmap font
    fn load_ttf_file(&mut self, path: &std::path::Path) {
        let width = self.parsed_font_width().unwrap_or(8);
        let height = self.parsed_font_height().unwrap_or(16);

        match ttf_import::import_font_from_ttf(path, width, height) {
            Ok(font) => {
                self.preview_font = Some(font);
            }
            Err(e) => {
                self.error = Some(e);
            }
        }
    }

    /// Load an image file and convert to font preview
    fn load_image_file(&mut self, path: &std::path::Path) {
        let width = self.parsed_font_width().unwrap_or(8);
        let height = self.parsed_font_height().unwrap_or(16);

        match image_import::import_font_from_image(path, width, height, self.use_dithering) {
            Ok(font) => {
                self.preview_font = Some(font);
            }
            Err(e) => {
                self.error = Some(e);
            }
        }
    }

    /// Auto-detect font dimensions from image size
    fn auto_detect_image_dimensions(&mut self, path: &std::path::Path) {
        if let Ok(dim) = image::image_dimensions(path) {
            let detected_width = (dim.0 / 16) as i32;
            let detected_height = (dim.1 / 16) as i32;

            // Use detected dimensions, minimum 1
            let width = detected_width.max(1);
            let height = detected_height.max(1);

            self.image_width = width.to_string();
            self.image_height = height.to_string();
        }
    }

    /// Reload image or TTF with new dimensions
    fn reload_image(&mut self) {
        if let Some(FontSourceType::Image) = &self.source_type {
            let path = PathBuf::from(&self.file_path);
            if path.exists() {
                let ext = path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()).unwrap_or_default();
                if is_ttf_extension(&ext) {
                    self.load_ttf_file(&path);
                } else {
                    self.load_image_file(&path);
                }
            }
        }
    }

    /// Update the dialog state (internal method)
    fn update_internal(&mut self, message: &FontImportMessage) -> Option<DialogAction<Message>> {
        match message {
            FontImportMessage::SetFilePath(path) => {
                self.file_path = path.clone();
                let path_buf = PathBuf::from(&path);
                if path_buf.exists() {
                    self.load_file(&path_buf);
                }
                Some(DialogAction::None)
            }
            FontImportMessage::Browse => {
                // Return a task to open file dialog
                Some(DialogAction::RunTask(Task::perform(
                    async {
                        let file = rfd::AsyncFileDialog::new()
                            .add_filter(
                                "All Fonts",
                                &[
                                    // Native fonts
                                    "yaff", "psf", "psfu", "f08", "f14", "f16", "f19", "f06", "f07", "f09", "f10", "f11", "f12", "f13", "f15", "f17", "f18",
                                    "f20", "f22", "f24", "f26", "f28", "f30", "f32", // TrueType/OpenType
                                    "ttf", "otf", "ttc", "otc", // XBin
                                    "xb",  // DOS COM
                                    "com", // Images
                                    "png", "jpg", "jpeg", "gif", "bmp", "webp",
                                ],
                            )
                            .add_filter("Font Files", &["yaff", "psf", "psfu", "f08", "f14", "f16", "f19"])
                            .add_filter("TrueType/OpenType", &["ttf", "otf", "ttc", "otc"])
                            .add_filter("XBin Files", &["xb"])
                            .add_filter("DOS COM Fonts", &["com"])
                            .add_filter("Image Files", &["png", "jpg", "jpeg", "gif", "bmp", "webp"])
                            .add_filter("All Files", &["*"])
                            .set_title("Import Font")
                            .pick_file()
                            .await;
                        file.map(|f| f.path().to_path_buf())
                    },
                    |path| msg(FontImportMessage::FileSelected(path)),
                )))
            }
            FontImportMessage::FileSelected(path) => {
                if let Some(path) = path {
                    self.file_path = path.to_string_lossy().to_string();
                    self.load_file(path);
                }
                Some(DialogAction::None)
            }
            FontImportMessage::SelectXBFont(index) => {
                self.xb_selected_font = *index;
                if *index < self.xb_fonts.len() {
                    self.preview_font = Some(self.xb_fonts[*index].clone());
                }
                Some(DialogAction::None)
            }
            FontImportMessage::SetFontWidth(w) => {
                self.image_width = w.clone();
                self.reload_image();
                Some(DialogAction::None)
            }
            FontImportMessage::SetFontHeight(h) => {
                self.image_height = h.clone();
                self.reload_image();
                Some(DialogAction::None)
            }
            FontImportMessage::SetDithering(enabled) => {
                self.use_dithering = *enabled;
                self.reload_image();
                Some(DialogAction::None)
            }
            FontImportMessage::Import => {
                if let Some(font) = self.preview_font.take() {
                    Some(DialogAction::CloseWith(Message::BitFontEditor(BitFontEditorMessage::FontImported(font))))
                } else {
                    Some(DialogAction::None)
                }
            }
            FontImportMessage::Cancel => Some(DialogAction::Close),
        }
    }
}

impl Dialog<Message> for FontImportDialog {
    fn view(&self) -> Element<'_, Message> {
        let title = dialog_title(fl!("menu-import-font").trim_end_matches('…').to_string());

        // === FILE PATH ROW ===
        let placeholder = fl!("font-import-file-placeholder");
        let file_input = text_input(&placeholder, &self.file_path)
            .on_input(|s| msg(FontImportMessage::SetFilePath(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        let browse_btn = browse_button(msg(FontImportMessage::Browse));

        let file_row = row![left_label_small(fl!("font-import-file")), file_input, browse_btn,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // === PREVIEW AND OPTIONS ===
        let preview_element = self.view_preview_internal();
        let options_element = self.view_options_internal();

        let preview_options_row = row![
            container(preview_element).width(Length::FillPortion(3)),
            Space::new().width(DIALOG_SPACING),
            container(options_element).width(Length::FillPortion(2)),
        ]
        .spacing(DIALOG_SPACING);

        // === ERROR MESSAGE ===
        let error_element: Element<'_, Message> = if let Some(err) = &self.error {
            text(err)
                .size(TEXT_SIZE_SMALL)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().danger.base.color),
                })
                .into()
        } else {
            Space::new().height(0).into()
        };

        // === CONTENT ===
        let content_column = column![file_row, Space::new().height(DIALOG_SPACING), preview_options_row, error_element,].spacing(0);

        let content_box = effect_box(content_column.into());

        // === BUTTONS ===
        let can_import = self.can_import();
        let buttons = button_row(vec![
            secondary_button(format!("{}", ButtonType::Cancel), Some(msg(FontImportMessage::Cancel))).into(),
            primary_button(fl!("font-import-button"), can_import.then(|| msg(FontImportMessage::Import))).into(),
        ]);

        let dialog_content = dialog_area(column![title, Space::new().height(DIALOG_SPACING), content_box].into());
        let button_area = dialog_area(buttons.into());

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area,].into(),
            DIALOG_WIDTH_LARGE,
        )
        .into()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        let Message::BitFontEditor(BitFontEditorMessage::FontImportDialog(msg)) = message else {
            return None;
        };
        self.update_internal(msg)
    }

    fn request_cancel(&mut self) -> DialogAction<Message> {
        DialogAction::Close
    }

    fn request_confirm(&mut self) -> DialogAction<Message> {
        if let Some(font) = self.preview_font.take() {
            DialogAction::CloseWith(Message::BitFontEditor(BitFontEditorMessage::FontImported(font)))
        } else {
            DialogAction::None
        }
    }
}

impl FontImportDialog {
    /// View for the font preview (charset-like grid)
    fn view_preview_internal(&self) -> Element<'_, Message> {
        use iced::widget::Canvas;

        if let Some(font) = &self.preview_font {
            let (font_width, font_height) = (font.size().width, font.size().height);

            // Calculate scale to fit in preview area (similar to charset view)
            let scale = 2.0_f32.min(280.0 / (16.0 * font_width as f32));
            let cell_width = font_width as f32 * scale;
            let cell_height = font_height as f32 * scale;
            let label_size = 20.0;

            let grid_width = label_size + 16.0 * cell_width;
            let grid_height = label_size + 16.0 * cell_height;

            let canvas = Canvas::new(FontPreviewCanvas {
                font,
                cell_width,
                cell_height,
                label_size,
            })
            .width(Length::Fixed(grid_width))
            .height(Length::Fixed(grid_height));

            column![text(fl!("font-import-preview")).size(TEXT_SIZE_SMALL), Space::new().height(4), canvas,].into()
        } else {
            container(text(fl!("font-import-no-preview")).size(TEXT_SIZE_NORMAL))
                .width(Length::Fill)
                .height(Length::Fixed(200.0))
                .center_x(Length::Fill)
                .center_y(Length::Fixed(200.0))
                .into()
        }
    }

    /// View for options (depends on source type)
    fn view_options_internal(&self) -> Element<'_, Message> {
        match &self.source_type {
            Some(FontSourceType::NativeFont) => {
                // Simple info for native fonts
                if let Some(font) = &self.preview_font {
                    let size = font.size();
                    column![
                        text(fl!("font-import-native-info")).size(TEXT_SIZE_SMALL),
                        Space::new().height(8),
                        text(format!("{}×{}", size.width, size.height)).size(TEXT_SIZE_NORMAL),
                    ]
                    .into()
                } else {
                    Space::new().into()
                }
            }
            Some(FontSourceType::XBin { has_second_font }) => {
                // XB font selector
                let options: Vec<String> = if *has_second_font {
                    vec![fl!("font-import-xb-font-1"), fl!("font-import-xb-font-2")]
                } else {
                    vec![fl!("font-import-xb-font-1")]
                };

                let selected = options.get(self.xb_selected_font).cloned();

                let picker = pick_list(options, selected, |s: String| {
                    let index = if s == fl!("font-import-xb-font-2") { 1 } else { 0 };
                    msg(FontImportMessage::SelectXBFont(index))
                })
                .width(Length::Fill);

                column![
                    text(fl!("font-import-xb-info")).size(TEXT_SIZE_SMALL),
                    Space::new().height(8),
                    left_label_small(fl!("font-import-select-font")),
                    picker,
                ]
                .spacing(4)
                .into()
            }
            Some(FontSourceType::Image) => {
                // Image import options: width and height
                let width_valid = self.parsed_font_width().is_some();
                let height_valid = self.parsed_font_height().is_some();

                let width_input = text_input("8", &self.image_width)
                    .on_input(|s| msg(FontImportMessage::SetFontWidth(s)))
                    .size(TEXT_SIZE_NORMAL)
                    .width(Length::Fixed(60.0));

                let height_input = text_input("16", &self.image_height)
                    .on_input(|s| msg(FontImportMessage::SetFontHeight(s)))
                    .size(TEXT_SIZE_NORMAL)
                    .width(Length::Fixed(60.0));

                let width_error = if !width_valid && !self.image_width.is_empty() {
                    text("> 0").size(TEXT_SIZE_SMALL).style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().danger.base.color),
                    })
                } else {
                    text("").size(TEXT_SIZE_SMALL)
                };

                let height_error = if !height_valid && !self.image_height.is_empty() {
                    text("> 0").size(TEXT_SIZE_SMALL).style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().danger.base.color),
                    })
                } else {
                    text("").size(TEXT_SIZE_SMALL)
                };

                let dither_checkbox = checkbox(self.use_dithering)
                    .on_toggle(|b| msg(FontImportMessage::SetDithering(b)))
                    .size(16);

                let dither_row = row![dither_checkbox, text(fl!("font-import-dithering")).size(TEXT_SIZE_NORMAL),]
                    .spacing(6)
                    .align_y(Alignment::Center);

                column![
                    text(fl!("font-import-image-info")).size(TEXT_SIZE_SMALL),
                    Space::new().height(8),
                    row![left_label_small(fl!("font-size-width")), width_input, width_error,]
                        .spacing(4)
                        .align_y(Alignment::Center),
                    Space::new().height(4),
                    row![left_label_small(fl!("font-size-height")), height_input, height_error,]
                        .spacing(4)
                        .align_y(Alignment::Center),
                    Space::new().height(4),
                    dither_row,
                ]
                .spacing(4)
                .into()
            }
            None => Space::new().into(),
        }
    }
}

/// Check if extension is a native font format
fn is_native_font_extension(ext: &str) -> bool {
    matches!(
        ext,
        "yaff"
            | "psf"
            | "psfu"
            | "f08"
            | "f14"
            | "f16"
            | "f19"
            | "f06"
            | "f07"
            | "f09"
            | "f10"
            | "f11"
            | "f12"
            | "f13"
            | "f15"
            | "f17"
            | "f18"
            | "f20"
            | "f22"
            | "f24"
            | "f26"
            | "f28"
            | "f30"
            | "f32"
    )
}

/// Check if extension is a TTF/OTF font format
fn is_ttf_extension(ext: &str) -> bool {
    matches!(ext, "ttf" | "otf" | "ttc" | "otc")
}

/// Check if extension is an image format
fn is_image_extension(ext: &str) -> bool {
    matches!(ext, "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tga" | "tiff" | "ico")
}

/// Parse a DOS COM file containing font data
///
/// Supports multiple COM font formats used by various DOS font editors.
fn parse_com_font(name: &str, data: &[u8]) -> Result<BitFont, String> {
    if data.len() < 0x64 {
        return Err("COM file too small to contain font data".to_string());
    }

    // Calculate checksum of first 16 bytes (like Fontraption does)
    let checksum: u16 = data[0..16]
        .chunks(2)
        .map(|chunk| {
            if chunk.len() == 2 {
                u16::from_le_bytes([chunk[0], chunk[1]])
            } else {
                chunk[0] as u16
            }
        })
        .fold(0u16, |acc, val| acc.wrapping_add(val));

    // Try to detect the format
    let (height, data_offset) = if checksum == 0x8696 {
        // PCMag FontEdit .COM
        let h = data[0x32];
        (h, 0x63usize)
    } else if checksum == 0xEF10 {
        // Fontraption Non-TSR .COM
        let h = data[0x15];
        (h, 0x19usize)
    } else if data.len() >= 0x2C && data[0x28] == b'V' && data[0x29] == b'I' && data[0x2A] == b'L' && data[0x2B] == b'E' {
        // Fontraption TSR .COM (has 'VILE' signature)
        let h = data[0x5D];
        (h, 0x63usize)
    } else {
        return Err("Unknown COM font format (not PCMag FontEdit or Fontraption)".to_string());
    };

    // Validate height
    if height == 0 || height as i32 > MAX_FONT_HEIGHT {
        return Err(format!("Invalid font height: {} (must be 1-{})", height, MAX_FONT_HEIGHT));
    }

    // Check if we have enough data
    let font_size = 256 * height as usize;
    if data.len() < data_offset + font_size {
        return Err(format!(
            "COM file too small: need {} bytes for font data, file has {}",
            data_offset + font_size,
            data.len()
        ));
    }

    // Extract font data
    let font_data = &data[data_offset..data_offset + font_size];

    Ok(BitFont::create_8(name, 8, height, font_data))
}
