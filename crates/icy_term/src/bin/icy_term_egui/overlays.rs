//! Overlay dialogs that the legacy client offered: shortcuts, about and BPS emulation.

use eframe::egui;
use icy_parser_core::BaudEmulation;

use super::appearance::{self, Dialog, DialogButton, DialogSize};
use super::hotkeys;
use icy_engine_gui::egui::shortcuts;

/// Same list the legacy BPS dialog offered.
const RATES: [u32; 11] = [300, 1200, 2400, 4800, 9600, 14400, 19200, 28800, 38400, 57600, 115_200];

pub fn help(context: &egui::Context, open: &mut bool) {
    if shortcuts::shortcuts_dialog(context, &tr!("help-title"), &tr!("help-subtitle"), &hotkeys::help_entries()) {
        *open = false;
    }
}

pub fn baud(context: &egui::Context, open: &mut bool, current: BaudEmulation) -> Option<BaudEmulation> {
    let id = egui::Id::new("bps-custom");
    let mut selected = None;
    let response = Dialog::new("bps").size(DialogSize::Small).fixed_height(390.0).show(context, |dialog| {
        let mut custom = dialog.ui().data(|data| data.get_temp::<u32>(id)).unwrap_or(match current {
            BaudEmulation::Rate(rate) if !RATES.contains(&rate) => rate,
            _ => 2400,
        });
        dialog.content(|ui| {
            appearance::group(ui, &tr!("select-bps-dialog-heading"), |ui| {
                if ui.radio(current == BaudEmulation::Off, &*tr!("select-bps-dialog-bps-max")).clicked() {
                    selected = Some(BaudEmulation::Off);
                }
                for row in RATES.chunks(3) {
                    ui.columns(3, |columns| {
                        for (column, rate) in columns.iter_mut().zip(row) {
                            if column.selectable_label(current == BaudEmulation::Rate(*rate), rate.to_string()).clicked() {
                                selected = Some(BaudEmulation::Rate(*rate));
                            }
                        }
                    });
                }
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .radio(
                            matches!(current, BaudEmulation::Rate(rate) if !RATES.contains(&rate)),
                            &*tr!("select-bps-dialog-bps-custom"),
                        )
                        .clicked()
                    {
                        selected = Some(BaudEmulation::Rate(custom));
                    }
                    if ui.add(egui::DragValue::new(&mut custom).range(50..=1_000_000)).changed() {
                        ui.data_mut(|data| data.insert_temp(id, custom));
                    }
                });
            });
        });
        dialog.buttons([DialogButton::cancel(tr!("egui-close"), ())]);
    });
    if response.action.is_some() || response.dismissed || selected.is_some() {
        *open = false;
    }
    selected
}
