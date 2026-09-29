//! IGS pens and colors: the pen to palette register mapping of each resolution, pen pickers
//! and the palette dialog that writes `S` (set pen color) commands.

use eframe::egui::{self, Color32, Stroke};
use icy_draw::fl;
use icy_engine_gui::egui::appearance::{self, labels, DialogButton, DialogSize};
use icy_parser_core::{IgsCommand, TerminalResolution};

// VDI pens map to hardware color registers like the engine's IGS runner does.
const LOW_PENS: [u8; 16] = [0, 15, 1, 2, 4, 6, 3, 5, 7, 8, 9, 10, 12, 14, 11, 13];
const MEDIUM_PENS: [u8; 4] = [0, 3, 1, 2];
const HIGH_PENS: [u8; 2] = [0, 1];

/// The pens a resolution has, each with the palette register it draws with.
pub fn pens(resolution: TerminalResolution) -> &'static [u8] {
    match resolution {
        TerminalResolution::Low => &LOW_PENS,
        TerminalResolution::Medium => &MEDIUM_PENS,
        TerminalResolution::High => &HIGH_PENS,
    }
}

pub fn pen_count(resolution: TerminalResolution) -> u8 {
    pens(resolution).len() as u8
}

/// The color `pen` draws with.
pub fn pen_color(palette: &icy_engine::Palette, resolution: TerminalResolution, pen: u8) -> Color32 {
    let pens = pens(resolution);
    let register = pens.get(pen as usize).copied().unwrap_or(pens[pens.len() - 1]);
    let (red, green, blue) = palette.rgb(u32::from(register));
    Color32::from_rgb(red, green, blue)
}

/// The 0–7 Atari ST levels of `pen`, as `S` commands set them.
pub fn pen_levels(palette: &icy_engine::Palette, resolution: TerminalResolution, pen: u8) -> [u8; 3] {
    let color = pen_color(palette, resolution, pen);
    [color.r(), color.g(), color.b()].map(|channel| ((f32::from(channel) / 34.0).round() as u8).min(7))
}

/// A swatch that opens the pens of the resolution.
pub fn pen_picker(ui: &mut egui::Ui, id: &str, palette: &icy_engine::Palette, resolution: TerminalResolution, value: &mut u8, size: egui::Vec2) -> bool {
    let mut changed = false;
    let response = ui.add(
        egui::Button::new("")
            .fill(pen_color(palette, resolution, *value))
            .min_size(size)
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke),
    );
    egui::Popup::menu(&response).id(egui::Id::new(("igs-pen", id))).show(|ui| {
        egui::Grid::new(("igs-pen-grid", id)).spacing(egui::vec2(6.0, 6.0)).show(ui, |ui| {
            for pen in 0..pen_count(resolution) {
                let response = pen_swatch(ui, pen_color(palette, resolution, pen), *value == pen);
                if response.on_hover_text(fl!("igs-pen", pen = pen)).clicked() {
                    *value = pen;
                    changed = true;
                    ui.close();
                }
                if pen % 8 == 7 {
                    ui.end_row();
                }
            }
        });
    });
    changed
}

/// A square pen swatch framed with the accent while selected and highlighted while hovered.
fn pen_swatch(ui: &mut egui::Ui, color: Color32, selected: bool) -> egui::Response {
    const SIZE: f32 = 22.0;
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(SIZE), egui::Sense::click());
    let hovered = response.hovered();
    let hover = ui.ctx().animate_bool_responsive(response.id, hovered);
    let visuals = ui.visuals();
    let accent = visuals.selection.stroke.color;
    let rect = rect.expand(hover * 1.5);
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, color);
    painter.rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
        egui::StrokeKind::Inside,
    );
    if selected {
        painter.rect_stroke(rect.expand(3.0), 5.0, Stroke::new(2.0, accent), egui::StrokeKind::Inside);
    } else if hovered {
        painter.rect_stroke(rect.expand(3.0), 5.0, Stroke::new(1.5, visuals.strong_text_color()), egui::StrokeKind::Inside);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn level_color([red, green, blue]: [u8; 3]) -> Color32 {
    Color32::from_rgb(red * 34, green * 34, blue * 34)
}

pub struct PaletteDialog {
    resolution: TerminalResolution,
    values: Vec<[u8; 3]>,
    original: Vec<[u8; 3]>,
    pen: usize,
}

pub enum PaletteResult {
    Open,
    Cancel,
    Apply,
}

impl PaletteDialog {
    pub fn new(palette: &icy_engine::Palette, resolution: TerminalResolution) -> Self {
        let values: Vec<[u8; 3]> = (0..pen_count(resolution)).map(|pen| pen_levels(palette, resolution, pen)).collect();
        Self {
            resolution,
            original: values.clone(),
            values,
            pen: 0,
        }
    }

    /// One `S` command for every changed pen.
    pub fn commands(&self) -> Vec<IgsCommand> {
        self.values
            .iter()
            .zip(&self.original)
            .enumerate()
            .filter(|(_, (value, original))| value != original)
            .map(|(pen, (&[red, green, blue], _))| IgsCommand::SetPenColor {
                pen: pen as u8,
                red,
                green,
                blue,
            })
            .collect()
    }

    pub fn show(&mut self, context: &egui::Context) -> PaletteResult {
        #[derive(Clone, Copy)]
        enum Action {
            Revert,
            Cancel,
            Apply,
        }
        let response = appearance::Dialog::new("igs-palette")
            .title(fl!("igs-palette-title"))
            .subtitle(fl!("igs-palette-subtitle"))
            .size(DialogSize::Width(520.0))
            .show(context, |dialog| {
                dialog.content(|ui| {
                    appearance::group(ui, &fl!("igs-palette-pens"), |ui| {
                        egui::Grid::new("igs-palette-pens").spacing(egui::vec2(6.0, 6.0)).show(ui, |ui| {
                            for pen in 0..self.values.len() {
                                let selected = self.pen == pen;
                                let color = level_color(self.values[pen]);
                                let luminance = 0.299 * f32::from(color.r()) + 0.587 * f32::from(color.g()) + 0.114 * f32::from(color.b());
                                let text = if luminance > 140.0 { Color32::BLACK } else { Color32::WHITE };
                                let swatch = egui::Button::new(egui::RichText::new(pen.to_string()).size(11.0).color(text))
                                    .fill(color)
                                    .min_size(egui::vec2(48.0, 34.0))
                                    .stroke(Stroke::new(
                                        if selected { 3.0 } else { 1.0 },
                                        if selected {
                                            ui.visuals().selection.stroke.color
                                        } else {
                                            ui.visuals().widgets.noninteractive.bg_stroke.color
                                        },
                                    ));
                                let changed = if self.values[pen] != self.original[pen] { " *" } else { "" };
                                if ui.add(swatch).on_hover_text(format!("{}{changed}", fl!("igs-pen", pen = pen))).clicked() {
                                    self.pen = pen;
                                }
                                if pen % 8 == 7 {
                                    ui.end_row();
                                }
                            }
                        });
                    });
                    appearance::group(ui, &fl!("igs-pen", pen = self.pen), |ui| {
                        let pen = self.pen;
                        for (index, name) in [fl!("igs-palette-red"), fl!("igs-palette-green"), fl!("igs-palette-blue")]
                            .into_iter()
                            .enumerate()
                        {
                            appearance::form_row(ui, &name, |ui| {
                                ui.add(egui::Slider::new(&mut self.values[pen][index], 0..=7));
                            });
                        }
                    });
                });
                dialog.buttons([
                    DialogButton::secondary(fl!("igs-palette-revert"), Action::Revert).leading(),
                    DialogButton::cancel(labels::cancel(), Action::Cancel),
                    DialogButton::primary(fl!("button-apply"), Action::Apply),
                ]);
            });
        match response.action {
            Some(Action::Revert) => {
                self.values = self.original.clone();
                PaletteResult::Open
            }
            Some(Action::Apply) => PaletteResult::Apply,
            Some(Action::Cancel) => PaletteResult::Cancel,
            None if response.dismissed => PaletteResult::Cancel,
            None => PaletteResult::Open,
        }
    }

    pub fn resolution(&self) -> TerminalResolution {
        self.resolution
    }

    #[cfg(test)]
    pub fn set_levels(&mut self, pen: usize, levels: [u8; 3]) {
        self.values[pen] = levels;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_pens_become_set_pen_color_commands() {
        let palette = icy_engine::ATARI_ST_MEDIUM_PALETTE.clone();
        let mut dialog = PaletteDialog::new(&palette, TerminalResolution::Medium);
        assert_eq!(dialog.values.len(), 4);
        assert!(dialog.commands().is_empty());
        dialog.values[2] = [0, 0, 7];
        assert_eq!(
            dialog.commands(),
            vec![IgsCommand::SetPenColor {
                pen: 2,
                red: 0,
                green: 0,
                blue: 7
            }]
        );
    }

    #[test]
    fn pens_use_the_resolution_registers() {
        assert_eq!(pen_count(TerminalResolution::Low), 16);
        assert_eq!(pens(TerminalResolution::Low)[1], 15);
        assert_eq!(pen_count(TerminalResolution::High), 2);
    }
}
