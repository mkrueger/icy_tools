//! Palette editor: title with import/export, the swatch grid, and a large preview with
//! RGB sliders and a hex field below.

use std::path::Path;

use eframe::egui::{self, Color32, Stroke, StrokeKind};
use icy_draw::{fl, palette_files};
use icy_engine::{Color, Palette};
use icy_engine_gui::egui::{
    appearance::{self, labels, Dialog, DialogButton, DialogSize},
    dialog::DANGER,
};

const SWATCH: f32 = 32.0;
const SMALL_SWATCH: f32 = 24.0;
const MIN_SWATCH: f32 = 12.0;
/// Rows shown before the grid scrolls, so large palettes fit the dialog body without scrolling it.
const VISIBLE_ROWS: usize = 6;
const PREVIEW: f32 = 96.0;
const CHANNEL_LABEL: f32 = 16.0;
const CHANNEL_VALUE: f32 = 28.0;
const ROW_HEIGHT: f32 = 24.0;
const SPACING: f32 = 8.0;

pub enum Action {
    Import,
    Export,
    Apply(Palette),
    Cancel,
}

#[derive(Clone)]
enum Button {
    Restore,
    Cancel,
    Apply,
}

pub struct PaletteEditor {
    palette: Palette,
    selected: usize,
    hex: String,
    error: Option<String>,
    /// Scrolls the selected swatch into view once the grid is shown.
    reveal: bool,
}

impl Default for PaletteEditor {
    fn default() -> Self {
        Self::new(Palette::dos_default(), 0)
    }
}

impl PaletteEditor {
    pub fn new(palette: Palette, selected: usize) -> Self {
        let palette = if palette.is_empty() { Palette::dos_default() } else { palette };
        let mut editor = Self {
            palette,
            selected: 0,
            hex: String::new(),
            error: None,
            reveal: true,
        };
        editor.select(selected);
        editor
    }

    #[cfg(test)]
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    #[cfg(test)]
    pub fn selected(&self) -> usize {
        self.selected
    }

    #[cfg(test)]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn rgb(&self) -> (u8, u8, u8) {
        self.palette.rgb(self.selected as u32)
    }

    fn sync_hex(&mut self) {
        let (red, green, blue) = self.rgb();
        self.hex = format!("{red:02X}{green:02X}{blue:02X}");
    }

    pub fn select(&mut self, index: usize) {
        self.selected = index.min(self.palette.len().saturating_sub(1));
        self.sync_hex();
        self.error = None;
    }

    pub fn set_rgb(&mut self, red: u8, green: u8, blue: u8) {
        self.palette.set_color(self.selected as u32, Color::new(red, green, blue));
        self.sync_hex();
        self.error = None;
    }

    fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
        let hex = hex.trim().trim_start_matches('#');
        if hex.len() != 6 || !hex.is_ascii() {
            return None;
        }
        let channel = |range: std::ops::Range<usize>| u8::from_str_radix(&hex[range], 16).ok();
        Some((channel(0..2)?, channel(2..4)?, channel(4..6)?))
    }

    /// Applies the hex field while typing; errors only show once enough digits were entered.
    pub fn set_hex(&mut self, hex: String) {
        self.hex = hex;
        if let Some((red, green, blue)) = Self::parse_hex(&self.hex) {
            self.palette.set_color(self.selected as u32, Color::new(red, green, blue));
            self.error = None;
        } else if self.hex.trim().trim_start_matches('#').len() >= 6 {
            self.error = Some(fl!("palette-editor-invalid-hex"));
        } else {
            self.error = None;
        }
    }

    /// Replaces the colors from a palette file, keeping the number of palette slots.
    pub fn import(&mut self, path: &Path) {
        match palette_files::load(path) {
            Ok(mut palette) => {
                palette.resize(self.palette.len());
                self.palette = palette;
                self.select(self.selected);
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn export(&mut self, path: &Path) {
        self.error = palette_files::save(&self.palette, path).err();
    }

    pub fn restore_defaults(&mut self) {
        self.palette = Palette::dos_default();
        self.select(self.selected);
    }

    /// `blocked` is set while a file picker is open.
    pub fn show(&mut self, context: &egui::Context, blocked: bool) -> Option<Action> {
        let mut action = None;
        let response = Dialog::new("palette-edit").size(DialogSize::Medium).show(context, |dialog| {
            dialog.content(|ui| {
                let title = fl!("menu-edit_palette");
                let title = appearance::bold(ui, title.trim_end_matches(['…', '.'])).size(18.0);
                let import = fl!("palette-editor-import");
                let export = fl!("palette-editor-export");
                let mut file_buttons = |ui: &mut egui::Ui| {
                    ui.add_enabled_ui(!blocked, |ui| {
                        if ui.button(export.as_str()).clicked() {
                            action = Some(Action::Export);
                        }
                        if ui.button(import.as_str()).clicked() {
                            action = Some(Action::Import);
                        }
                    });
                };
                let button_row = |ui: &mut egui::Ui, buttons: &mut dyn FnMut(&mut egui::Ui)| {
                    let size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y + 8.0);
                    ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), |ui| buttons(ui));
                };
                // The file buttons share the title row unless the dialog is narrow.
                if ui.available_width() >= 440.0 {
                    ui.horizontal(|ui| {
                        ui.label(title);
                        button_row(ui, &mut file_buttons);
                    });
                } else {
                    ui.label(title);
                    ui.add_space(SPACING / 2.0);
                    button_row(ui, &mut file_buttons);
                }
                ui.add_space(SPACING);
                self.swatches(ui);
                ui.add_space(SPACING * 2.0);
                ui.horizontal_top(|ui| {
                    self.preview(ui);
                    ui.add_space(SPACING * 2.0);
                    ui.vertical(|ui| self.channels(ui));
                });
            });
            dialog.buttons([
                DialogButton::secondary(labels::restore_defaults(), Button::Restore)
                    .leading()
                    .enabled(!self.palette.is_default()),
                DialogButton::cancel(labels::cancel(), Button::Cancel),
                DialogButton::primary(labels::ok(), Button::Apply),
            ]);
        });
        match response.action {
            Some(Button::Restore) => self.restore_defaults(),
            Some(Button::Apply) if !blocked => action = Some(Action::Apply(self.palette.clone())),
            Some(Button::Cancel) if !blocked => action = Some(Action::Cancel),
            _ if response.dismissed && !blocked => action = Some(Action::Cancel),
            _ => {}
        }
        action
    }

    fn swatches(&mut self, ui: &mut egui::Ui) {
        let (columns, preferred, spacing) = if self.palette.len() <= 16 {
            (8, SWATCH, 6.0)
        } else {
            (16, SMALL_SWATCH, 2.0)
        };
        // Shrinks the swatches instead of overflowing narrow dialogs; `swatch` adds a 2 px ring.
        let fit = (ui.available_width() + spacing) / columns as f32 - spacing - 4.0;
        let size = fit.clamp(MIN_SWATCH, preferred);
        let cell = size + 4.0;
        let rows = self.palette.len().div_ceil(columns);
        let height = rows as f32 * cell + rows.saturating_sub(1) as f32 * spacing;
        let reveal = std::mem::take(&mut self.reveal);
        let selected_row = (self.selected / columns) as f32;
        let mut grid = |ui: &mut egui::Ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::splat(spacing);
            for row in 0..rows {
                ui.horizontal(|ui| {
                    for index in row * columns..((row + 1) * columns).min(self.palette.len()) {
                        let (red, green, blue) = self.palette.rgb(index as u32);
                        let response = swatch(ui, Color32::from_rgb(red, green, blue), index == self.selected, size)
                            .on_hover_text(format!("{index}: #{red:02X}{green:02X}{blue:02X}"));
                        if response.clicked() {
                            self.select(index);
                        }
                    }
                });
            }
        };
        if rows <= VISIBLE_ROWS {
            ui.vertical(grid);
        } else {
            let visible = VISIBLE_ROWS as f32 * (cell + spacing) - spacing;
            let mut scroll = egui::ScrollArea::vertical()
                .id_salt("palette-swatches")
                .max_height(visible)
                .min_scrolled_height(visible.min(height))
                .auto_shrink([true, false]);
            if reveal {
                // Centers the selected row; computed, as the grid has no layout yet on the first frame.
                let offset = selected_row * (cell + spacing) - (visible - cell) / 2.0;
                scroll = scroll.vertical_scroll_offset(offset.clamp(0.0, (height - visible).max(0.0)));
            }
            scroll.show(ui, |ui| ui.vertical(&mut grid));
        }
    }

    fn preview(&self, ui: &mut egui::Ui) {
        let (red, green, blue) = self.rgb();
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(PREVIEW), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 6, Color32::from_rgb(red, green, blue));
        painter.rect_stroke(rect, 6, Stroke::new(1.0, Color32::from_gray(80)), StrokeKind::Inside);
        let luminance = 0.299 * f32::from(red) + 0.587 * f32::from(green) + 0.114 * f32::from(blue);
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("#{red:02X}{green:02X}{blue:02X}"),
            egui::FontId::proportional(12.0),
            if luminance > 128.0 { Color32::BLACK } else { Color32::WHITE },
        );
    }

    /// R, G, B and hex rows share one label column and one control column.
    fn channels(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(SPACING, SPACING);
        let (red, green, blue) = self.rgb();
        let mut rgb = [red, green, blue];
        let mut changed = false;
        let width = (ui.available_width() - CHANNEL_LABEL - CHANNEL_VALUE - SPACING * 2.0).max(60.0);
        let label = |ui: &mut egui::Ui, text: &str| {
            ui.allocate_ui_with_layout(egui::vec2(CHANNEL_LABEL, ROW_HEIGHT), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_min_size(egui::vec2(CHANNEL_LABEL, ROW_HEIGHT));
                ui.label(text);
            });
        };
        for (name, value) in ["R", "G", "B"].into_iter().zip(rgb.iter_mut()) {
            ui.horizontal(|ui| {
                label(ui, name);
                let mut level = f32::from(*value);
                if appearance::slider(ui, &mut level, 0.0..=255.0, width).changed() {
                    *value = level.round().clamp(0.0, 255.0) as u8;
                    changed = true;
                }
                ui.allocate_ui_with_layout(egui::vec2(CHANNEL_VALUE, ROW_HEIGHT), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.set_min_size(egui::vec2(CHANNEL_VALUE, ROW_HEIGHT));
                    ui.label(egui::RichText::new(value.to_string()).weak());
                });
            });
        }
        if changed {
            self.set_rgb(rgb[0], rgb[1], rgb[2]);
        }
        ui.horizontal(|ui| {
            label(ui, "#");
            let valid = self.hex.is_empty() || Self::parse_hex(&self.hex).is_some();
            let mut hex = self.hex.clone();
            let response = ui.add(appearance::text_edit(&mut hex).desired_width(80.0).char_limit(7).hint_text("RRGGBB"));
            if !valid {
                ui.painter().rect_stroke(response.rect, 4, Stroke::new(1.0, DANGER), StrokeKind::Inside);
            }
            if response.changed() {
                self.set_hex(hex);
            }
            if let Some(error) = &self.error {
                ui.colored_label(DANGER, error);
            }
        });
    }
}

/// Swatch with a white inner and black outer ring when selected, readable on any color.
fn swatch(ui: &mut egui::Ui, color: Color32, selected: bool, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(size + 4.0), egui::Sense::click());
    let inner = rect.shrink(2.0);
    let painter = ui.painter();
    painter.rect_filled(inner, 4, color);
    if selected {
        painter.rect_stroke(rect, 5, Stroke::new(2.0, Color32::BLACK), StrokeKind::Inside);
        painter.rect_stroke(inner, 3, Stroke::new(2.0, Color32::WHITE), StrokeKind::Inside);
    } else {
        let border = if response.hovered() {
            ui.visuals().text_color()
        } else {
            Color32::from_gray(50)
        };
        painter.rect_stroke(inner, 4, Stroke::new(1.0, border), StrokeKind::Inside);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_keep_hex_and_palette_in_sync() {
        let mut editor = PaletteEditor::new(Palette::dos_default(), 20);
        assert_eq!(editor.selected(), 15);
        editor.select(1);
        assert_eq!(editor.hex, "0000AA");
        editor.set_rgb(1, 2, 3);
        assert_eq!(editor.hex, "010203");
        assert_eq!(editor.palette().rgb(1), (1, 2, 3));

        editor.set_hex("#AbCd".into());
        assert_eq!(editor.palette().rgb(1), (1, 2, 3));
        assert!(editor.error().is_none());
        editor.set_hex("#abcdef".into());
        assert_eq!(editor.palette().rgb(1), (0xAB, 0xCD, 0xEF));
        editor.set_hex("zzzzzz".into());
        assert!(editor.error().is_some());
        assert_eq!(editor.palette().rgb(1), (0xAB, 0xCD, 0xEF));

        assert!(!editor.palette().is_default());
        editor.restore_defaults();
        assert!(editor.palette().is_default());
        assert_eq!(editor.hex, "0000AA");
    }

    #[test]
    fn import_keeps_slot_count() {
        let directory = std::env::temp_dir().join(format!("icy_draw_palette_editor_{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("palette.gpl");
        let mut source = Palette::dos_default();
        source.set_color(2, Color::new(9, 8, 7));
        palette_files::save(&source, &path).unwrap();

        let mut editor = PaletteEditor::new(Palette::dos_default(), 2);
        editor.import(&path);
        assert_eq!(editor.palette().len(), 16);
        assert_eq!(editor.palette().rgb(2), (9, 8, 7));
        assert_eq!(editor.hex, "090807");
        editor.import(&directory.join("missing.gpl"));
        assert!(editor.error().is_some());
        let _ = std::fs::remove_dir_all(directory);
    }
}
