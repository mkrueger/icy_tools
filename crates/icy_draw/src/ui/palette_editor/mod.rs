//! Palette Editor Dialog
//!
//! Editor for 16-color palettes with streamlined UX:
//! - Visual color grid with large swatches
//! - RGB sliders + hex input
//! - Single-click import/export (file dialog triggers action immediately)

use std::path::PathBuf;

use iced::{
    Alignment, Element, Length, Task,
    widget::{Space, button, column, container, mouse_area, row, slider, text, text_input},
};

use icy_engine::formats::PaletteFormat;
use icy_engine::{Color, DOS_DEFAULT_PALETTE, FileFormat, Palette, SaveOptions, Screen, TextBuffer};
use icy_engine_gui::{
    ButtonType,
    ui::{
        DIALOG_SPACING, DIALOG_WIDTH_MEDIUM, Dialog, DialogAction, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL, dialog_area, dialog_title, modal_container,
        primary_button, restore_defaults_button, secondary_button, separator, validated_input_style,
    },
};

use crate::{fl, ui::Message};

#[derive(Debug, Clone)]
pub enum PaletteEditorMessage {
    SelectIndex(usize),
    SetR(f32),
    SetG(f32),
    SetB(f32),
    SetHex(String),

    Import,
    ImportFile(Option<PathBuf>),

    Export,
    ExportFile(Option<PathBuf>),

    ResetToDefaults,

    Apply,
    Cancel,
}

pub struct PaletteEditorDialog {
    palette: Palette,
    selected_index: usize,
    hex_input: String,
    error: Option<String>,
}

impl PaletteEditorDialog {
    pub fn new(mut palette: Palette) -> Self {
        palette.resize(16);
        let (r, g, b) = palette.rgb(0);
        let hex_input = format!("{:02X}{:02X}{:02X}", r, g, b);

        Self {
            palette,
            selected_index: 0,
            hex_input,
            error: None,
        }
    }

    fn selected_rgb(&self) -> (u8, u8, u8) {
        self.palette.rgb(self.selected_index as u32)
    }

    fn set_selected_rgb(&mut self, r: u8, g: u8, b: u8) {
        self.palette.set_color(self.selected_index as u32, Color::new(r, g, b));
        self.palette.resize(16);
        self.hex_input = format!("{:02X}{:02X}{:02X}", r, g, b);
    }

    fn update_hex_from_selection(&mut self) {
        let (r, g, b) = self.selected_rgb();
        self.hex_input = format!("{:02X}{:02X}{:02X}", r, g, b);
    }

    fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some((r, g, b))
    }

    fn is_dos_default(&self) -> bool {
        if self.palette.len() != 16 {
            return false;
        }
        for i in 0..16 {
            let (r, g, b) = self.palette.rgb(i as u32);
            let (dr, dg, db) = DOS_DEFAULT_PALETTE[i].rgb();
            if r != dr || g != dg || b != db {
                return false;
            }
        }
        true
    }

    fn try_load_palette_from_path(path: &PathBuf) -> Result<Palette, String> {
        let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());

        if matches!(ext.as_deref(), Some("xb")) {
            let screen = FileFormat::XBin.load(path.as_path(), None).map_err(|e| e.to_string())?;
            let mut pal = screen.palette().clone();
            pal.resize(16);
            return Ok(pal);
        }

        let fmt = match ext.as_deref() {
            Some("pal") => FileFormat::Palette(PaletteFormat::Pal),
            Some("gpl") => FileFormat::Palette(PaletteFormat::Gpl),
            Some("hex") => FileFormat::Palette(PaletteFormat::Hex),
            Some("txt") => FileFormat::Palette(PaletteFormat::Txt),
            Some("ice") | Some("icepal") => FileFormat::Palette(PaletteFormat::Ice),
            Some("ase") => {
                return Err("ASE palette loading is not implemented".to_string());
            }
            _ => {
                return Err("Unsupported palette file type".to_string());
            }
        };

        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let mut pal = fmt.load_palette(&bytes).map_err(|e| e.to_string())?;
        pal.resize(16);
        Ok(pal)
    }

    fn export_to_path(&self, path: &PathBuf) -> Result<(), String> {
        let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());

        match ext.as_deref() {
            Some("xb") => {
                let mut buffer = TextBuffer::new((1, 1));
                buffer.palette = self.palette.clone();
                let mut options = SaveOptions::default();
                options.compress = false;
                let bytes = FileFormat::XBin.to_bytes(&buffer, &options).map_err(|e| e.to_string())?;
                std::fs::write(path, bytes).map_err(|e| e.to_string())
            }
            Some("gpl") => {
                let bytes = self
                    .palette
                    .export_palette(&FileFormat::Palette(PaletteFormat::Gpl))
                    .map_err(|e| e.to_string())?;
                std::fs::write(path, bytes).map_err(|e| e.to_string())
            }
            Some("pal") => {
                let bytes = self
                    .palette
                    .export_palette(&FileFormat::Palette(PaletteFormat::Pal))
                    .map_err(|e| e.to_string())?;
                std::fs::write(path, bytes).map_err(|e| e.to_string())
            }
            Some("hex") => {
                let bytes = self
                    .palette
                    .export_palette(&FileFormat::Palette(PaletteFormat::Hex))
                    .map_err(|e| e.to_string())?;
                std::fs::write(path, bytes).map_err(|e| e.to_string())
            }
            Some("txt") => {
                let bytes = self
                    .palette
                    .export_palette(&FileFormat::Palette(PaletteFormat::Txt))
                    .map_err(|e| e.to_string())?;
                std::fs::write(path, bytes).map_err(|e| e.to_string())
            }
            Some("ice") | Some("icepal") => {
                let bytes = self
                    .palette
                    .export_palette(&FileFormat::Palette(PaletteFormat::Ice))
                    .map_err(|e| e.to_string())?;
                std::fs::write(path, bytes).map_err(|e| e.to_string())
            }
            _ => Err("Unsupported palette file type".to_string()),
        }
    }

    fn update_internal(&mut self, msg: &PaletteEditorMessage) -> Option<DialogAction<Message>> {
        match msg {
            PaletteEditorMessage::SelectIndex(idx) => {
                self.selected_index = (*idx).min(15);
                self.update_hex_from_selection();
                self.error = None;
                Some(DialogAction::None)
            }
            PaletteEditorMessage::SetR(v) => {
                let (_r, g, b) = self.selected_rgb();
                self.set_selected_rgb((*v).round().clamp(0.0, 255.0) as u8, g, b);
                Some(DialogAction::None)
            }
            PaletteEditorMessage::SetG(v) => {
                let (r, _g, b) = self.selected_rgb();
                self.set_selected_rgb(r, (*v).round().clamp(0.0, 255.0) as u8, b);
                Some(DialogAction::None)
            }
            PaletteEditorMessage::SetB(v) => {
                let (r, g, _b) = self.selected_rgb();
                self.set_selected_rgb(r, g, (*v).round().clamp(0.0, 255.0) as u8);
                Some(DialogAction::None)
            }
            PaletteEditorMessage::SetHex(hex) => {
                // Limit to 8 characters (allows # prefix + 6 hex digits with some room for editing)
                let hex = if hex.len() > 8 { hex[..8].to_string() } else { hex.clone() };
                self.hex_input = hex.clone();
                if let Some((r, g, b)) = Self::parse_hex(&hex) {
                    self.palette.set_color(self.selected_index as u32, Color::new(r, g, b));
                    self.palette.resize(16);
                    self.error = None;
                } else if !hex.is_empty() {
                    // Only show error if there's actual input (not while typing)
                    let clean = hex.trim_start_matches('#');
                    if clean.len() >= 6 {
                        self.error = Some(fl!("palette-editor-invalid-hex"));
                    } else {
                        self.error = None; // Still typing
                    }
                } else {
                    self.error = None;
                }
                Some(DialogAction::None)
            }

            // Single-click import: open dialog, load immediately
            PaletteEditorMessage::Import => {
                let task = Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .add_filter("Palette", &["gpl", "pal", "hex", "txt", "ice", "icepal", "xb"])
                            .pick_file()
                            .await
                            .map(|h| h.path().to_path_buf())
                    },
                    |path| Message::PaletteEditor(PaletteEditorMessage::ImportFile(path)),
                );
                Some(DialogAction::RunTask(task))
            }
            PaletteEditorMessage::ImportFile(path) => {
                if let Some(path) = path {
                    match Self::try_load_palette_from_path(path) {
                        Ok(mut pal) => {
                            pal.resize(16);
                            self.palette = pal;
                            self.selected_index = self.selected_index.min(15);
                            self.update_hex_from_selection();
                            self.error = None;
                        }
                        Err(e) => {
                            self.error = Some(e);
                        }
                    }
                }
                Some(DialogAction::None)
            }

            // Single-click export: open save dialog with all formats, write immediately
            PaletteEditorMessage::Export => {
                let task = Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .set_file_name("palette.gpl")
                            .add_filter("GIMP Palette", &["gpl"])
                            .add_filter("PAL", &["pal"])
                            .add_filter("Hex", &["hex"])
                            .add_filter("Text", &["txt"])
                            .add_filter("ICE Palette", &["ice"])
                            .add_filter("XBin", &["xb"])
                            .save_file()
                            .await
                            .map(|h| h.path().to_path_buf())
                    },
                    |path| Message::PaletteEditor(PaletteEditorMessage::ExportFile(path)),
                );
                Some(DialogAction::RunTask(task))
            }
            PaletteEditorMessage::ExportFile(path) => {
                if let Some(path) = path {
                    match self.export_to_path(path) {
                        Ok(()) => self.error = None,
                        Err(e) => self.error = Some(e),
                    }
                }
                Some(DialogAction::None)
            }

            PaletteEditorMessage::ResetToDefaults => {
                self.palette = Palette::from_slice(&DOS_DEFAULT_PALETTE);
                self.palette.resize(16);
                self.update_hex_from_selection();
                self.error = None;
                Some(DialogAction::None)
            }

            PaletteEditorMessage::Apply => Some(DialogAction::CloseWith(Message::PaletteEditorApplied(self.palette.clone()))),
            PaletteEditorMessage::Cancel => Some(DialogAction::Close),
        }
    }

    fn color_swatch(&self, idx: usize, size: f32) -> Element<'_, Message> {
        let (r, g, b) = self.palette.rgb(idx as u32);
        let is_selected = idx == self.selected_index;

        // Fixed size container - selection border is drawn INSIDE
        let swatch = container(Space::new().width(Length::Fixed(size - 2.0)).height(Length::Fixed(size - 2.0)))
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .center_x(Length::Fixed(size))
            .center_y(Length::Fixed(size))
            .style(move |_theme: &iced::Theme| {
                if is_selected {
                    // Selected: white inner border + black outer border (marching ants style)
                    iced::widget::container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgb8(r, g, b))),
                        border: iced::Border {
                            color: iced::Color::WHITE,
                            width: 2.0,
                            radius: 3.0.into(),
                        },
                        ..Default::default()
                    }
                } else {
                    iced::widget::container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgb8(r, g, b))),
                        border: iced::Border {
                            color: iced::Color::from_rgb8(50, 50, 50),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    }
                }
            });

        // Wrap selected swatch in black border container
        let final_swatch: Element<'_, Message> = if is_selected {
            container(swatch)
                .width(Length::Fixed(size + 4.0))
                .height(Length::Fixed(size + 4.0))
                .center_x(Length::Fixed(size + 4.0))
                .center_y(Length::Fixed(size + 4.0))
                .style(|_theme: &iced::Theme| iced::widget::container::Style {
                    background: None,
                    border: iced::Border {
                        color: iced::Color::BLACK,
                        width: 2.0,
                        radius: 5.0.into(),
                    },
                    ..Default::default()
                })
                .into()
        } else {
            // Non-selected: wrap in invisible container of same size as selected
            container(swatch)
                .width(Length::Fixed(size + 4.0))
                .height(Length::Fixed(size + 4.0))
                .center_x(Length::Fixed(size + 4.0))
                .center_y(Length::Fixed(size + 4.0))
                .into()
        };

        mouse_area(final_swatch)
            .on_press(Message::PaletteEditor(PaletteEditorMessage::SelectIndex(idx)))
            .into()
    }

    fn color_preview(&self) -> Element<'_, Message> {
        let (r, g, b) = self.selected_rgb();
        container(
            text(format!("#{:02X}{:02X}{:02X}", r, g, b))
                .size(TEXT_SIZE_SMALL)
                .style(move |_theme: &iced::Theme| {
                    // Choose text color based on luminance
                    let lum = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
                    let text_color = if lum > 128.0 { iced::Color::BLACK } else { iced::Color::WHITE };
                    iced::widget::text::Style { color: Some(text_color) }
                }),
        )
        .width(Length::Fixed(72.0))
        .height(Length::Fixed(72.0))
        .center_x(Length::Fixed(72.0))
        .center_y(Length::Fixed(72.0))
        .style(move |_theme: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgb8(r, g, b))),
            border: iced::Border {
                color: iced::Color::from_rgb8(80, 80, 80),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .into()
    }
}

impl Dialog<Message> for PaletteEditorDialog {
    fn view(&self) -> Element<'_, Message> {
        let title = dialog_title(fl!("menu-edit_palette").trim_end_matches('…').to_string());
        let swatch_size = 32.0;
        let swatch_spacing = 2.0;

        // 16 swatches in 2 rows of 8
        let swatches_top = row![
            self.color_swatch(0, swatch_size),
            self.color_swatch(1, swatch_size),
            self.color_swatch(2, swatch_size),
            self.color_swatch(3, swatch_size),
            self.color_swatch(4, swatch_size),
            self.color_swatch(5, swatch_size),
            self.color_swatch(6, swatch_size),
            self.color_swatch(7, swatch_size),
        ]
        .spacing(swatch_spacing)
        .align_y(Alignment::Center);

        let swatches_bottom = row![
            self.color_swatch(8, swatch_size),
            self.color_swatch(9, swatch_size),
            self.color_swatch(10, swatch_size),
            self.color_swatch(11, swatch_size),
            self.color_swatch(12, swatch_size),
            self.color_swatch(13, swatch_size),
            self.color_swatch(14, swatch_size),
            self.color_swatch(15, swatch_size),
        ]
        .spacing(swatch_spacing)
        .align_y(Alignment::Center);

        let palette_grid = column![swatches_top, swatches_bottom].spacing(swatch_spacing);

        // Import/Export buttons next to the swatches (top right)
        let import_btn = secondary_button(fl!("palette-editor-import"), Some(Message::PaletteEditor(PaletteEditorMessage::Import)));

        let export_btn = secondary_button(fl!("palette-editor-export"), Some(Message::PaletteEditor(PaletteEditorMessage::Export)));

        let io_buttons = column![import_btn, export_btn].spacing(DIALOG_SPACING);

        let top_section = row![palette_grid, Space::new().width(Length::Fill), io_buttons]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Color editing section: preview + sliders + hex
        let (r, g, b) = self.selected_rgb();

        let r_slider = slider(0.0..=255.0, r as f32, |v| Message::PaletteEditor(PaletteEditorMessage::SetR(v)))
            .step(1.0)
            .width(Length::Fill);
        let g_slider = slider(0.0..=255.0, g as f32, |v| Message::PaletteEditor(PaletteEditorMessage::SetG(v)))
            .step(1.0)
            .width(Length::Fill);
        let b_slider = slider(0.0..=255.0, b as f32, |v| Message::PaletteEditor(PaletteEditorMessage::SetB(v)))
            .step(1.0)
            .width(Length::Fill);

        let rgb_sliders = column![
            row![
                text("R").size(TEXT_SIZE_NORMAL).width(Length::Fixed(20.0)),
                r_slider,
                text(format!("{r:3}")).size(TEXT_SIZE_SMALL).width(Length::Fixed(32.0))
            ]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center),
            row![
                text("G").size(TEXT_SIZE_NORMAL).width(Length::Fixed(20.0)),
                g_slider,
                text(format!("{g:3}")).size(TEXT_SIZE_SMALL).width(Length::Fixed(32.0))
            ]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center),
            row![
                text("B").size(TEXT_SIZE_NORMAL).width(Length::Fixed(20.0)),
                b_slider,
                text(format!("{b:3}")).size(TEXT_SIZE_SMALL).width(Length::Fixed(32.0))
            ]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center),
        ]
        .spacing(DIALOG_SPACING);

        let is_hex_valid = self.hex_input.is_empty() || Self::parse_hex(&self.hex_input).is_some();

        let hex_input = text_input("#RRGGBB", &self.hex_input)
            .on_input(|s| Message::PaletteEditor(PaletteEditorMessage::SetHex(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(90.0))
            .style(validated_input_style(is_hex_valid));

        // Error text next to hex input
        let hex_error: Element<'_, Message> = if let Some(err) = &self.error {
            text(err)
                .size(TEXT_SIZE_SMALL)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().danger.base.color),
                })
                .into()
        } else {
            Space::new().width(0.0).into()
        };

        let hex_row = row![text("#").size(TEXT_SIZE_NORMAL), hex_input, Space::new().width(DIALOG_SPACING), hex_error]
            .spacing(2)
            .align_y(Alignment::Center);

        let color_edit_section = row![
            self.color_preview(),
            Space::new().width(DIALOG_SPACING),
            column![rgb_sliders, hex_row].spacing(DIALOG_SPACING)
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Start);

        let content = column![top_section, Space::new().height(DIALOG_SPACING), color_edit_section,].spacing(DIALOG_SPACING);

        let content_box = container(content).padding(0);
        let dialog_content = dialog_area(column![title, Space::new().height(DIALOG_SPACING), content_box].into());

        // Reset to defaults button (enabled only if palette differs from DOS default)
        let reset_btn = restore_defaults_button(!self.is_dos_default(), Message::PaletteEditor(PaletteEditorMessage::ResetToDefaults));

        let buttons = row![
            reset_btn,
            Space::new().width(Length::Fill),
            secondary_button(format!("{}", ButtonType::Cancel), Some(Message::PaletteEditor(PaletteEditorMessage::Cancel)),),
            primary_button(format!("{}", ButtonType::Ok), Some(Message::PaletteEditor(PaletteEditorMessage::Apply)),),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        let button_area = dialog_area(buttons.into());

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area].into(),
            DIALOG_WIDTH_MEDIUM,
        )
        .into()
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        if let Message::PaletteEditor(msg) = message {
            return self.update_internal(msg);
        }
        None
    }

    fn request_cancel(&mut self) -> DialogAction<Message> {
        DialogAction::Close
    }

    fn request_confirm(&mut self) -> DialogAction<Message> {
        DialogAction::CloseWith(Message::PaletteEditorApplied(self.palette.clone()))
    }
}
