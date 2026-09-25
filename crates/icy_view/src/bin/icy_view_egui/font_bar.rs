//! Controls above TheDraw/FIGlet previews: pick a font of the bundle, type a sample, adjust
//! spacing, colours and a column guide. The preview is re-rendered on every change.

use super::{preview::Preview, text};
use eframe::egui;
use icy_engine::{formats::FileFormat, DOS_DEFAULT_PALETTE};
use icy_view::format_preview::{self, FontInfo, FontSampleOptions, OUTLINE_STYLES};
use std::sync::Arc;

const RULERS: &[i32] = &[0, 40, 80, 132];

#[derive(Default)]
pub struct FontBar {
    pub options: FontSampleOptions,
    pub selected: Option<usize>,
    pub fonts: Vec<FontInfo>,
    file: String,
    data: Arc<Vec<u8>>,
    applied: Option<(Option<usize>, FontSampleOptions)>,
}

pub fn is_font_file(file: &str) -> bool {
    matches!(FileFormat::from_path(std::path::Path::new(file)), Some(FileFormat::CharacterFont(_)))
}

impl FontBar {
    /// Follows the previewed file and re-renders it when the settings changed.
    /// Returns whether the preview is a font with controls to show.
    pub fn sync(&mut self, preview: &mut Preview) -> bool {
        if preview.file.is_empty() || !is_font_file(&preview.file) {
            self.file.clear();
            self.fonts.clear();
            self.applied = None;
            return false;
        }
        if self.file != preview.file || !Arc::ptr_eq(&self.data, &preview.data) {
            self.file = preview.file.clone();
            self.data = preview.data.clone();
            self.fonts = format_preview::font_list(icy_sauce::strip_sauce(&self.data, icy_sauce::StripMode::All));
            self.selected = None;
            self.applied = None;
        }
        if self.fonts.is_empty() {
            return false;
        }
        if preview.loading || preview.error.is_some() {
            return true;
        }
        let key = (self.selected, self.options.clone());
        if self.applied.as_ref() == Some(&key) {
            return true;
        }
        // The view thread already rendered the default overview.
        if self.applied.is_some() || key != (None, FontSampleOptions::default()) {
            let reset_scroll = self.applied.as_ref().is_none_or(|(selected, _)| *selected != self.selected);
            let data = icy_sauce::strip_sauce(&self.data, icy_sauce::StripMode::All);
            match format_preview::render_font_sample(data, self.selected, &self.options) {
                Ok(buffer) => preview.show_buffer(buffer, reset_scroll),
                Err(error) => preview.error = Some(error.to_string()),
            }
        }
        self.applied = Some(key);
        true
    }

    /// The shown sample differs from the file's thumbnail.
    pub fn customized(&self) -> bool {
        !self.fonts.is_empty() && (self.selected.is_some() || self.options != FontSampleOptions::default())
    }

    fn font_label(&self, selected: Option<usize>) -> String {
        match selected.and_then(|index| self.fonts.get(index)) {
            Some(font) => format!("{}  ·  {}", font.name, font.kind),
            None => format!("{} ({})", text("egui-font-all"), self.fonts.len()),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let mut selected = self.selected;
        let combo = egui::ComboBox::from_id_salt("font-select")
            .selected_text(self.font_label(selected))
            .width(200.0)
            .height(400.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut selected, None, self.font_label(None));
                for (index, font) in self.fonts.iter().enumerate() {
                    ui.selectable_value(&mut selected, Some(index), format!("{}  ·  {}  ·  {}", font.name, font.kind, font.glyphs));
                }
            })
            .response;
        let wheel = if combo.hovered() { ui.input(|input| input.raw_scroll_delta.y) } else { 0.0 };
        if wheel != 0.0 {
            let last = self.fonts.len() - 1;
            selected = match (selected, wheel < 0.0) {
                (None, true) => Some(0),
                (None, false) => None,
                (Some(index), true) => Some((index + 1).min(last)),
                (Some(0), false) => None,
                (Some(index), false) => Some(index - 1),
            };
        }
        combo.on_hover_text(text("egui-font-select"));
        self.selected = selected;

        ui.add(
            egui::TextEdit::multiline(&mut self.options.text)
                .desired_rows(1)
                .desired_width(140.0)
                .hint_text(text("egui-font-sample")),
        )
        .on_hover_text(text("egui-font-sample-hint"));

        let options = &mut self.options;
        ui.label(text("egui-font-spacing"));
        ui.add(egui::DragValue::new(&mut options.spacing).range(-8..=16));
        ui.label(text("egui-font-line-gap"));
        ui.add(egui::DragValue::new(&mut options.line_gap).range(-8..=16));
        let shown = match self.selected.and_then(|index| self.fonts.get(index)) {
            Some(font) => std::slice::from_ref(font),
            None => &self.fonts[..],
        };
        if shown.iter().any(|font| font.kind == "outline") {
            ui.label(text("egui-font-outline"));
            ui.add(egui::DragValue::new(&mut options.outline_style).range(0..=OUTLINE_STYLES - 1));
        }
        // Colour fonts carry their own attributes.
        if shown.iter().any(|font| font.kind != "color") {
            color_combo(ui, "font-fg", &text("egui-font-foreground"), &mut options.foreground);
            color_combo(ui, "font-bg", &text("egui-font-background"), &mut options.background);
        }

        let ruler_label = |columns: i32| {
            if columns == 0 {
                text("egui-baud-off")
            } else {
                format!("{columns} {}", text("egui-columns"))
            }
        };
        egui::ComboBox::from_id_salt("font-ruler")
            .selected_text(ruler_label(options.ruler))
            .show_ui(ui, |ui| {
                for columns in RULERS {
                    ui.selectable_value(&mut options.ruler, *columns, ruler_label(*columns));
                }
            })
            .response
            .on_hover_text(text("egui-font-ruler"));
        if *options != FontSampleOptions::default() && ui.button(text("egui-font-reset")).clicked() {
            *options = FontSampleOptions::default();
        }
    }
}

fn paint_swatch(ui: &egui::Ui, rect: egui::Rect, color: Option<u8>) {
    let painter = ui.painter();
    let visuals = &ui.visuals().widgets.inactive;
    match color {
        Some(index) => {
            let (r, g, b) = DOS_DEFAULT_PALETTE[index as usize & 15].rgb();
            painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(r, g, b));
        }
        None => {
            painter.line_segment([rect.left_bottom(), rect.right_top()], visuals.fg_stroke);
        }
    }
    painter.rect_stroke(rect, 2.0, visuals.fg_stroke, egui::StrokeKind::Inside);
}

/// A DOS colour picker; `None` keeps the font's own colours.
fn color_combo(ui: &mut egui::Ui, id: &str, label: &str, color: &mut Option<u8>) {
    let swatch = egui::Id::new(id).with("swatch");
    let button = egui::Button::new((egui::Atom::custom(swatch, egui::vec2(14.0, 14.0)), label)).atom_ui(ui);
    if let Some(rect) = button.rect(swatch) {
        paint_swatch(ui, rect, *color);
    }
    egui::Popup::menu(&button.response).id(egui::Id::new(id).with("popup")).show(|ui| {
        if ui.selectable_label(color.is_none(), text("egui-font-default-color")).clicked() {
            *color = None;
        }
        egui::Grid::new(format!("{id}-grid")).spacing([2.0, 2.0]).show(ui, |ui| {
            for index in 0..16u8 {
                let (r, g, b) = DOS_DEFAULT_PALETTE[index as usize].rgb();
                let fill = egui::Color32::from_rgb(r, g, b);
                let button = egui::Button::new("")
                    .fill(fill)
                    .min_size(egui::vec2(22.0, 18.0))
                    .selected(*color == Some(index));
                if ui.add(button).clicked() {
                    *color = Some(index);
                }
                if index % 8 == 7 {
                    ui.end_row();
                }
            }
        });
    });
}
