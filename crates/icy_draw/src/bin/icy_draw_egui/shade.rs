//! Shade ramps of the shading brush: toolbar pickers and the ramp editor.

use super::*;
use icy_draw::brush::{char_ramp_from_text, char_ramp_text, CharRamp, ColorRamp, Ramp, ShadeRamps};

const RAMP_CELL: f32 = 18.0;
/// Narrower cells for the toolbar, which also shows the size and filter controls.
const TOOLBAR_CELL: f32 = 13.0;

/// The ramps being edited in the ramp dialog.
#[derive(Clone)]
pub(super) struct RampDraft {
    pub ramps: ShadeRamps,
    /// Color ramp whose "add color" palette is open.
    adding_color: Option<usize>,
}

fn color32((red, green, blue): (u8, u8, u8)) -> Color32 {
    Color32::from_rgb(red, green, blue)
}

/// A ramp preview button: the characters, or color swatches, in a toolbar-sized frame.
fn ramp_button(ui: &mut egui::Ui, cells: usize, cell_width: f32, placeholder: &str, paint: impl Fn(&egui::Ui, usize, egui::Rect)) -> egui::Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = (cells == 0).then(|| ui.fonts_mut(|fonts| fonts.layout_no_wrap(placeholder.to_owned(), font, Color32::PLACEHOLDER)));
    let width = galley.as_ref().map_or(cells as f32 * cell_width, |galley| galley.size().x) + 12.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width.max(36.0), widgets::CONTROL_HEIGHT), egui::Sense::click());
    let visuals = ui.visuals();
    let fill = if ui.is_enabled() && response.hovered() {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        visuals.widgets.inactive.weak_bg_fill
    };
    ui.painter().rect_filled(rect, 6, fill);
    ui.painter()
        .rect_stroke(rect, 6, visuals.widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
    match galley {
        Some(galley) => ui.painter().galley(rect.center() - galley.size() / 2.0, galley, visuals.text_color()),
        None => {
            let left = rect.center().x - cells as f32 * cell_width / 2.0;
            for index in 0..cells {
                let cell = egui::Rect::from_min_size(
                    egui::pos2(left + index as f32 * cell_width, rect.center().y - RAMP_CELL / 2.0),
                    egui::vec2(cell_width, RAMP_CELL),
                );
                paint(ui, index, cell);
            }
        }
    }
    response
}

fn paint_chars(ui: &egui::Ui, font: Option<&icy_engine::BitFont>, chars: &[char], index: usize, cell: egui::Rect) {
    let Some(font) = font else {
        return;
    };
    let size = font.size();
    let scale = ((cell.height() - 2.0) / size.width.max(size.height).max(1) as f32).max(0.1);
    let target = egui::Rect::from_center_size(cell.center(), egui::vec2(size.width as f32, size.height as f32) * scale);
    widgets::paint_glyph(ui, font, chars[index], target, ui.visuals().text_color());
}

fn paint_swatch(ui: &egui::Ui, palette: &icy_engine::Palette, colors: &[u8], index: usize, cell: egui::Rect) {
    let swatch = egui::Rect::from_center_size(cell.center(), egui::Vec2::splat(cell.width().min(cell.height()) - 3.0));
    ui.painter().rect_filled(swatch, 3, color32(palette.rgb(colors[index] as u32)));
    ui.painter()
        .rect_stroke(swatch, 3, ui.visuals().widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
}

impl DrawApp {
    /// Toolbar controls of the shading brush: the character and the color ramp.
    pub(super) fn shade_options(&mut self, ui: &mut egui::Ui, font: Option<&icy_engine::BitFont>) {
        let palette = self.document.with_state(|state| state.get_buffer().palette.clone());
        let chars = self.document.brush.shade_chars;
        let colors = self.document.brush.shade_colors;
        let mut edit = false;

        let button = ramp_button(ui, chars.len(), TOOLBAR_CELL, &fl!("shade-keep-characters"), |ui, index, cell| {
            paint_chars(ui, font, chars.as_slice(), index, cell)
        })
        .on_hover_text(fl!("shade-characters-tooltip"));
        egui::Popup::menu(&button).id(egui::Id::new("shade-char-ramps")).show(|ui| {
            for ramp in self.settings.shade_ramps.char_ramps() {
                let row = ramp_button(ui, ramp.len(), RAMP_CELL, "", |ui, index, cell| {
                    paint_chars(ui, font, ramp.as_slice(), index, cell)
                });
                let row = if ramp == chars { row.highlight() } else { row };
                if row.on_hover_text(char_ramp_text(&ramp)).clicked() {
                    self.document.brush.shade_chars = ramp;
                    ui.close();
                }
            }
            if ui.selectable_label(chars.is_empty(), fl!("shade-keep-characters")).clicked() {
                self.document.brush.shade_chars = CharRamp::default();
                ui.close();
            }
            ui.separator();
            if ui.button(fl!("shade-edit-ramps")).clicked() {
                edit = true;
                ui.close();
            }
        });

        let button = ramp_button(ui, colors.len(), TOOLBAR_CELL, &fl!("shade-brush-color"), |ui, index, cell| {
            paint_swatch(ui, &palette, colors.as_slice(), index, cell)
        })
        .on_hover_text(fl!("shade-colors-tooltip"));
        egui::Popup::menu(&button).id(egui::Id::new("shade-color-ramps")).show(|ui| {
            if ui.selectable_label(colors.is_empty(), fl!("shade-brush-color")).clicked() {
                self.document.brush.shade_colors = ColorRamp::default();
                ui.close();
            }
            for ramp in self.settings.shade_ramps.color_ramps() {
                let row = ramp_button(ui, ramp.len(), RAMP_CELL, "", |ui, index, cell| {
                    paint_swatch(ui, &palette, ramp.as_slice(), index, cell)
                });
                let row = if ramp == colors { row.highlight() } else { row };
                if row.clicked() {
                    self.document.brush.shade_colors = ramp;
                    ui.close();
                }
            }
            ui.separator();
            if ui.button(fl!("shade-edit-ramps")).clicked() {
                edit = true;
                ui.close();
            }
        });
        if edit {
            self.open_shade_ramps();
        }
    }

    pub(super) fn open_shade_ramps(&mut self) {
        self.dialog = Some(Dialog::ShadeRamps(Box::new(RampDraft {
            ramps: self.settings.shade_ramps.clone(),
            adding_color: None,
        })));
    }

    /// Stores edited ramps. A brush using an edited ramp switches to its new version.
    pub(super) fn apply_shade_ramps(&mut self, ramps: ShadeRamps) {
        let old = &self.settings.shade_ramps;
        let brush = &mut self.document.brush;
        if let Some(index) = old.characters.iter().position(|text| char_ramp_from_text(text).ok() == Some(brush.shade_chars)) {
            if let Some(ramp) = ramps
                .characters
                .get(index)
                .and_then(|text| char_ramp_from_text(text).ok())
                .filter(|ramp| !ramp.is_empty())
            {
                brush.shade_chars = ramp;
            }
        }
        if let Some(index) = old.colors.iter().position(|colors| Ramp::new(colors) == brush.shade_colors) {
            if let Some(colors) = ramps.colors.get(index).filter(|colors| !colors.is_empty()) {
                brush.shade_colors = Ramp::new(colors);
            }
        }
        self.settings.shade_ramps = ramps;
        if self.persist_settings {
            self.settings.store_persistent();
        }
    }

    /// The ramp editor; returns whether it stays open.
    pub(super) fn shade_ramps_dialog(&mut self, context: &egui::Context, draft: &mut RampDraft) -> bool {
        #[derive(Clone, Copy)]
        enum Action {
            Defaults,
            Cancel,
            Apply,
        }
        let font = self
            .document
            .with_state(|state| state.get_buffer().font(state.get_caret().attribute.font_page()).cloned());
        let palette = self.document.with_state(|state| state.get_buffer().palette.clone());
        let valid = draft
            .ramps
            .characters
            .iter()
            .all(|text| char_ramp_from_text(text).is_ok_and(|ramp| !ramp.is_empty()));
        let response = appearance::Dialog::new("shade-ramps")
            .title(fl!("shade-ramps-title"))
            .subtitle(fl!("shade-ramps-subtitle"))
            .size(DialogSize::Large)
            .max_height(620.0)
            .scroll(true)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    appearance::group(ui, &fl!("shade-character-ramps"), |ui| {
                        let mut remove = None;
                        for (index, text) in draft.ramps.characters.iter_mut().enumerate() {
                            ui.push_id(("char-ramp", index), |ui| {
                                ui.horizontal(|ui| {
                                    ui.add(appearance::text_edit(text).desired_width(180.0).hint_text(fl!("shade-character-ramp-hint")));
                                    match char_ramp_from_text(text) {
                                        Ok(ramp) if ramp.is_empty() => {
                                            ui.colored_label(ui.visuals().error_fg_color, fl!("shade-ramp-empty"));
                                        }
                                        Ok(ramp) => {
                                            for &code in ramp.as_slice() {
                                                let (cell, _) = ui.allocate_exact_size(egui::Vec2::splat(RAMP_CELL), egui::Sense::hover());
                                                paint_chars(ui, font.as_ref(), &[code], 0, cell);
                                            }
                                        }
                                        Err(invalid) => {
                                            ui.colored_label(ui.visuals().error_fg_color, fl!("shade-invalid-character", character = invalid.to_string()));
                                        }
                                    }
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button(fl!("shade-remove-ramp")).clicked() {
                                            remove = Some(index);
                                        }
                                    });
                                });
                            });
                        }
                        if let Some(index) = remove {
                            draft.ramps.characters.remove(index);
                        }
                        if ui.button(fl!("shade-add-character-ramp")).clicked() {
                            draft.ramps.characters.push("░▒▓█".into());
                        }
                    });
                    appearance::group(ui, &fl!("shade-color-ramps"), |ui| {
                        let mut remove = None;
                        for (index, colors) in draft.ramps.colors.iter_mut().enumerate() {
                            ui.push_id(("color-ramp", index), |ui| {
                                ui.horizontal(|ui| {
                                    let mut remove_color = None;
                                    for (position, &color) in colors.iter().enumerate() {
                                        let (cell, response) = ui.allocate_exact_size(egui::Vec2::splat(RAMP_CELL + 4.0), egui::Sense::click());
                                        paint_swatch(ui, &palette, &[color], 0, cell);
                                        if response.on_hover_text(fl!("shade-remove-color", color = color)).clicked() {
                                            remove_color = Some(position);
                                        }
                                    }
                                    if let Some(position) = remove_color {
                                        colors.remove(position);
                                    }
                                    if colors.len() < icy_draw::brush::MAX_RAMP_LEN {
                                        let add = ui.button("+").on_hover_text(fl!("shade-add-color"));
                                        if add.clicked() {
                                            draft.adding_color = if draft.adding_color == Some(index) { None } else { Some(index) };
                                        }
                                    }
                                    if colors.is_empty() {
                                        ui.colored_label(ui.visuals().error_fg_color, fl!("shade-ramp-empty"));
                                    }
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button(fl!("shade-remove-ramp")).clicked() {
                                            remove = Some(index);
                                        }
                                    });
                                });
                                if draft.adding_color == Some(index) {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing = egui::Vec2::splat(2.0);
                                        for color in 0..palette.len().min(256) {
                                            let (cell, response) = ui.allocate_exact_size(egui::Vec2::splat(RAMP_CELL + 4.0), egui::Sense::click());
                                            paint_swatch(ui, &palette, &[color as u8], 0, cell);
                                            if response.on_hover_text(color.to_string()).clicked() && colors.len() < icy_draw::brush::MAX_RAMP_LEN {
                                                colors.push(color as u8);
                                            }
                                        }
                                    });
                                }
                            });
                        }
                        if let Some(index) = remove {
                            draft.ramps.colors.remove(index);
                            draft.adding_color = None;
                        }
                        if ui.button(fl!("shade-add-color-ramp")).clicked() {
                            draft.ramps.colors.push(Vec::new());
                            draft.adding_color = Some(draft.ramps.colors.len() - 1);
                        }
                    });
                });
                dialog.buttons([
                    DialogButton::secondary(labels::restore_defaults(), Action::Defaults).leading(),
                    DialogButton::cancel(labels::cancel(), Action::Cancel),
                    DialogButton::primary(fl!("button-apply"), Action::Apply).enabled(valid),
                ]);
            });
        match response.action {
            Some(Action::Defaults) => {
                draft.ramps = ShadeRamps::default();
                draft.adding_color = None;
                true
            }
            Some(Action::Apply) => {
                let mut ramps = draft.ramps.clone();
                ramps.colors.retain(|colors| !colors.is_empty());
                self.apply_shade_ramps(ramps);
                false
            }
            Some(Action::Cancel) => false,
            None => !response.dismissed,
        }
    }
}
