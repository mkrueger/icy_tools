//! The RIP palette: 16 color slots mapped to the 64 EGA colors (`|Q` and `|a`).

use eframe::egui::{self, Color32, Stroke};
use icy_draw::fl;
use icy_engine_gui::egui::appearance::{self, labels, DialogButton, DialogSize};
use icy_parser_core::RipCommand;

/// The EGA colors of the 16 slots after a reset, matching the DOS palette.
pub const DEFAULT_PALETTE: [u16; 16] = [0, 1, 2, 3, 4, 5, 20, 7, 56, 57, 58, 59, 60, 61, 62, 63];

/// The slot mapping in effect after `commands`.
pub fn palette_at_end(commands: &[RipCommand]) -> [u16; 16] {
    let mut palette = DEFAULT_PALETTE;
    for command in commands {
        match command {
            RipCommand::SetPalette { colors } => {
                for (slot, color) in palette.iter_mut().zip(colors) {
                    *slot = (*color).min(63);
                }
            }
            RipCommand::OnePalette { color, value } if *color < 16 => palette[*color as usize] = (*value).min(63),
            RipCommand::ResetWindows => palette = DEFAULT_PALETTE,
            _ => {}
        }
    }
    palette
}

pub fn ega_color(value: u16) -> Color32 {
    let (red, green, blue) = icy_engine::EGA_PALETTE[value.min(63) as usize].rgb();
    Color32::from_rgb(red, green, blue)
}

/// The command that changes `from` into `to`: one `|a` for a single slot, otherwise a full `|Q`.
pub fn palette_command(from: &[u16; 16], to: &[u16; 16]) -> Option<RipCommand> {
    let changed: Vec<usize> = (0..16).filter(|slot| from[*slot] != to[*slot]).collect();
    match changed.as_slice() {
        [] => None,
        [slot] => Some(RipCommand::OnePalette {
            color: *slot as u16,
            value: to[*slot],
        }),
        _ => Some(RipCommand::SetPalette { colors: to.to_vec() }),
    }
}

pub struct PaletteDialog {
    pub values: [u16; 16],
    /// The mapping before the dialog, for appending a change.
    pub original: [u16; 16],
    /// A `|Q` command edited in place.
    pub target: Option<usize>,
    slot: usize,
}

pub enum PaletteResult {
    Open,
    Cancel,
    Apply,
}

impl PaletteDialog {
    pub fn new(values: [u16; 16], target: Option<usize>) -> Self {
        Self {
            values,
            original: values,
            target,
            slot: 0,
        }
    }

    /// The commands the dialog currently stands for, for the live preview.
    pub fn command(&self) -> Option<RipCommand> {
        match self.target {
            Some(_) => Some(RipCommand::SetPalette { colors: self.values.to_vec() }),
            None => palette_command(&self.original, &self.values),
        }
    }

    pub fn show(&mut self, context: &egui::Context) -> PaletteResult {
        #[derive(Clone, Copy)]
        enum Action {
            Default,
            Cancel,
            Apply,
        }
        let response = appearance::Dialog::new("rip-palette")
            .title(fl!("rip-palette-title"))
            .subtitle(fl!("rip-palette-subtitle"))
            .size(DialogSize::Width(560.0))
            .show(context, |dialog| {
                dialog.content(|ui| {
                    appearance::group(ui, &fl!("rip-palette-slots"), |ui| {
                        egui::Grid::new("rip-palette-slots").spacing(egui::vec2(6.0, 6.0)).show(ui, |ui| {
                            for slot in 0..16 {
                                let selected = self.slot == slot;
                                let swatch = egui::Button::new(egui::RichText::new(slot.to_string()).size(11.0).color(contrast(ega_color(self.values[slot]))))
                                    .fill(ega_color(self.values[slot]))
                                    .min_size(egui::vec2(48.0, 34.0))
                                    .stroke(Stroke::new(
                                        if selected { 3.0 } else { 1.0 },
                                        if selected {
                                            ui.visuals().selection.stroke.color
                                        } else {
                                            ui.visuals().widgets.noninteractive.bg_stroke.color
                                        },
                                    ));
                                let changed = self.values[slot] != self.original[slot];
                                let tooltip = fl!("rip-palette-slot-tooltip", slot = slot, color = self.values[slot]);
                                let response = ui.add(swatch).on_hover_text(if changed { format!("{tooltip} *") } else { tooltip });
                                if response.clicked() {
                                    self.slot = slot;
                                }
                                if slot % 8 == 7 {
                                    ui.end_row();
                                }
                            }
                        });
                    });
                    appearance::group(ui, &fl!("rip-palette-ega", slot = self.slot), |ui| {
                        egui::Grid::new("rip-palette-ega").spacing(egui::vec2(4.0, 4.0)).show(ui, |ui| {
                            for value in 0..64u16 {
                                let selected = self.values[self.slot] == value;
                                let (red, green, blue) = icy_engine::EGA_PALETTE[value as usize].rgb();
                                let swatch = egui::Button::new("")
                                    .fill(ega_color(value))
                                    .min_size(egui::vec2(52.0, 22.0))
                                    .stroke(Stroke::new(
                                        if selected { 3.0 } else { 0.5 },
                                        if selected {
                                            ui.visuals().selection.stroke.color
                                        } else {
                                            ui.visuals().widgets.noninteractive.bg_stroke.color
                                        },
                                    ));
                                if ui.add(swatch).on_hover_text(format!("EGA {value} · #{red:02X}{green:02X}{blue:02X}")).clicked() {
                                    self.values[self.slot] = value;
                                }
                                if value % 8 == 7 {
                                    ui.end_row();
                                }
                            }
                        });
                    });
                });
                dialog.buttons([
                    DialogButton::secondary(fl!("rip-palette-default"), Action::Default).leading(),
                    DialogButton::cancel(labels::cancel(), Action::Cancel),
                    DialogButton::primary(fl!("button-apply"), Action::Apply),
                ]);
            });
        match response.action {
            Some(Action::Default) => {
                self.values = DEFAULT_PALETTE;
                PaletteResult::Open
            }
            Some(Action::Apply) => PaletteResult::Apply,
            Some(Action::Cancel) => PaletteResult::Cancel,
            None if response.dismissed => PaletteResult::Cancel,
            None => PaletteResult::Open,
        }
    }
}

/// Black or white, whichever reads better on `color`.
fn contrast(color: Color32) -> Color32 {
    let luminance = 0.299 * color.r() as f32 + 0.587 * color.g() as f32 + 0.114 * color.b() as f32;
    if luminance > 140.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_palette_follows_set_one_and_reset_commands() {
        assert_eq!(palette_at_end(&[]), DEFAULT_PALETTE);
        let mut colors = DEFAULT_PALETTE.to_vec();
        colors[1] = 9;
        let commands = vec![RipCommand::SetPalette { colors }, RipCommand::OnePalette { color: 4, value: 36 }];
        let palette = palette_at_end(&commands);
        assert_eq!((palette[1], palette[4], palette[6]), (9, 36, 20));
        let mut reset = commands.clone();
        reset.push(RipCommand::ResetWindows);
        assert_eq!(palette_at_end(&reset), DEFAULT_PALETTE);
    }

    #[test]
    fn changes_become_one_or_all_slots() {
        let mut changed = DEFAULT_PALETTE;
        assert_eq!(palette_command(&DEFAULT_PALETTE, &changed), None);
        changed[3] = 11;
        assert_eq!(
            palette_command(&DEFAULT_PALETTE, &changed),
            Some(RipCommand::OnePalette { color: 3, value: 11 })
        );
        changed[5] = 13;
        assert_eq!(
            palette_command(&DEFAULT_PALETTE, &changed),
            Some(RipCommand::SetPalette { colors: changed.to_vec() })
        );
        // The default mapping shows the DOS colors.
        assert_eq!(ega_color(DEFAULT_PALETTE[6]), Color32::from_rgb(0xAA, 0x55, 0x00));
    }
}
