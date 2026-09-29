//! The fill pattern dialog: one of the eight 16 × 16 user patterns that `X 7` commands load
//! and `A 4,n` fills use.

use eframe::egui::{self, Color32, Stroke};
use icy_draw::fl;
use icy_engine_gui::egui::appearance::{self, labels, DialogButton, DialogSize};
use icy_parser_core::IgsCommand;

const CELL: f32 = 16.0;

pub enum PatternResult {
    Open,
    Cancel,
    Apply,
}

pub struct PatternDialog {
    slot: u8,
    data: [u16; 16],
    /// The pattern command edited in place.
    pub target: Option<usize>,
    /// The value cells are set to while the pointer is held down.
    painting: Option<bool>,
}

/// The user pattern of `slot` in effect before the end of `commands`.
pub fn pattern_before<'a>(commands: impl Iterator<Item = &'a IgsCommand>, slot: u8) -> [u16; 16] {
    let mut pattern = [0; 16];
    for command in commands {
        if let IgsCommand::LoadFillPattern { pattern: loaded, data } = command {
            if *loaded == slot {
                pattern = [0; 16];
                for (row, word) in pattern.iter_mut().zip(data) {
                    *row = *word;
                }
            }
        }
    }
    pattern
}

impl PatternDialog {
    pub fn new(slot: u8, data: [u16; 16], target: Option<usize>) -> Self {
        Self {
            slot: slot.min(7),
            data,
            target,
            painting: None,
        }
    }

    pub fn slot(&self) -> u8 {
        self.slot
    }

    pub fn command(&self) -> IgsCommand {
        IgsCommand::LoadFillPattern {
            pattern: self.slot,
            data: self.data.to_vec(),
        }
    }

    fn cell(&self, x: usize, y: usize) -> bool {
        self.data[y] & (0x8000 >> x) != 0
    }

    fn set_cell(&mut self, x: usize, y: usize, on: bool) {
        if on {
            self.data[y] |= 0x8000 >> x;
        } else {
            self.data[y] &= !(0x8000 >> x);
        }
    }

    #[cfg(test)]
    pub fn toggle(&mut self, x: usize, y: usize) {
        let on = !self.cell(x, y);
        self.set_cell(x, y, on);
    }

    fn grid(&mut self, ui: &mut egui::Ui, color: Color32) {
        let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(CELL * 16.0), egui::Sense::click_and_drag());
        let cell_at = |pos: egui::Pos2| {
            let offset = (pos - rect.min) / CELL;
            ((0.0..16.0).contains(&offset.x) && (0.0..16.0).contains(&offset.y)).then_some((offset.x as usize, offset.y as usize))
        };
        if let Some((x, y)) = response.interact_pointer_pos().and_then(cell_at) {
            let value = *self.painting.get_or_insert(!self.cell(x, y));
            self.set_cell(x, y, value);
        }
        if !ui.input(|input| input.pointer.any_down()) {
            self.painting = None;
        }
        let painter = ui.painter_at(rect);
        let off = ui.visuals().extreme_bg_color;
        for y in 0..16 {
            for x in 0..16 {
                let cell = egui::Rect::from_min_size(rect.min + egui::vec2(x as f32, y as f32) * CELL, egui::Vec2::splat(CELL));
                painter.rect_filled(cell.shrink(0.5), 0.0, if self.cell(x, y) { color } else { off });
            }
        }
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
            egui::StrokeKind::Inside,
        );
    }

    /// The pattern tiled three times in each direction, at two screen points per pixel.
    fn tiled(&self, ui: &mut egui::Ui, color: Color32) {
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(2.0 * 48.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::BLACK);
        for y in 0..48 {
            for x in 0..48 {
                if self.cell(x % 16, y % 16) {
                    painter.rect_filled(
                        egui::Rect::from_min_size(rect.min + egui::vec2(x as f32, y as f32) * 2.0, egui::Vec2::splat(2.0)),
                        0.0,
                        color,
                    );
                }
            }
        }
    }

    /// `load` gives the pattern a slot has in the drawing, used when the slot changes.
    pub fn show(&mut self, context: &egui::Context, color: Color32, load: impl Fn(u8) -> [u16; 16]) -> PatternResult {
        #[derive(Clone, Copy)]
        enum Action {
            Clear,
            Invert,
            Cancel,
            Apply,
        }
        let response = appearance::Dialog::new("igs-pattern")
            .title(fl!("igs-pattern-title"))
            .subtitle(fl!("igs-pattern-subtitle"))
            .size(DialogSize::Width(520.0))
            .show(context, |dialog| {
                dialog.content(|ui| {
                    appearance::form_row(ui, &fl!("igs-pattern-slot"), |ui| {
                        let mut slot = self.slot;
                        ui.add_enabled(self.target.is_none(), egui::DragValue::new(&mut slot).range(0..=7));
                        if slot != self.slot {
                            self.slot = slot;
                            self.data = load(slot);
                        }
                    });
                    ui.horizontal_top(|ui| {
                        self.grid(ui, color);
                        ui.add_space(12.0);
                        ui.vertical(|ui| {
                            ui.weak(fl!("igs-pattern-preview"));
                            self.tiled(ui, color);
                        });
                    });
                });
                dialog.buttons([
                    DialogButton::secondary(fl!("igs-pattern-clear"), Action::Clear).leading(),
                    DialogButton::secondary(fl!("igs-pattern-invert"), Action::Invert).leading(),
                    DialogButton::cancel(labels::cancel(), Action::Cancel),
                    DialogButton::primary(fl!("button-apply"), Action::Apply),
                ]);
            });
        match response.action {
            Some(Action::Clear) => {
                self.data = [0; 16];
                PatternResult::Open
            }
            Some(Action::Invert) => {
                self.data = self.data.map(|row| !row);
                PatternResult::Open
            }
            Some(Action::Apply) => PatternResult::Apply,
            Some(Action::Cancel) => PatternResult::Cancel,
            None if response.dismissed => PatternResult::Cancel,
            None => PatternResult::Open,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_follow_the_last_load_of_their_slot() {
        let commands = [
            IgsCommand::LoadFillPattern {
                pattern: 1,
                data: vec![0xFFFF; 16],
            },
            IgsCommand::LoadFillPattern {
                pattern: 2,
                data: vec![0x00FF; 16],
            },
            IgsCommand::LoadFillPattern {
                pattern: 1,
                data: vec![0x8000; 16],
            },
        ];
        assert_eq!(pattern_before(commands.iter(), 1), [0x8000; 16]);
        assert_eq!(pattern_before(commands.iter(), 2), [0x00FF; 16]);
        assert_eq!(pattern_before(commands.iter(), 3), [0; 16]);
        let mut dialog = PatternDialog::new(3, [0; 16], None);
        dialog.toggle(0, 0);
        dialog.toggle(15, 15);
        let IgsCommand::LoadFillPattern { pattern: 3, data } = dialog.command() else {
            panic!("expected a pattern")
        };
        assert_eq!((data[0], data[15]), (0x8000, 0x0001));
    }
}
