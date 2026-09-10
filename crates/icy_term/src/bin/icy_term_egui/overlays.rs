//! Overlay dialogs that the legacy client offered: shortcuts, about and BPS emulation.

use eframe::egui;
use icy_parser_core::BaudEmulation;

use super::{appearance, hotkeys};

/// Same list the legacy BPS dialog offered.
const RATES: [u32; 11] = [300, 1200, 2400, 4800, 9600, 14400, 19200, 28800, 38400, 57600, 115_200];

pub fn help(context: &egui::Context, open: &mut bool) {
    let mut close = false;
    egui::Modal::new(egui::Id::new("shortcuts"))
        .frame(appearance::dialog_frame(context))
        .show(context, |ui| {
            let height = (context.content_rect().height() - 64.0).clamp(140.0, 560.0);
            ui.set_width((context.content_rect().width() - 48.0).clamp(240.0, 620.0));
            ui.heading(&*tr!("help-title"));
            ui.weak(&*tr!("help-subtitle"));
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("shortcut-list")
                .auto_shrink([false, false])
                .max_height(height)
                .show(ui, |ui| {
                    for (category, entries) in hotkeys::help_entries() {
                        ui.add_space(4.0);
                        ui.strong(category);
                        for (name, shortcut) in entries {
                            ui.horizontal(|ui| {
                                ui.allocate_ui_with_layout(egui::vec2(150.0, 22.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                    ui.set_min_width(150.0);
                                    ui.add(egui::Label::new(egui::RichText::new(shortcut).monospace()).wrap());
                                });
                                ui.add(egui::Label::new(name).wrap());
                            });
                        }
                    }
                });
            ui.separator();
            close = ui.add(appearance::primary_button(tr!("egui-close"))).clicked();
        });
    if close {
        *open = false;
    }
}

pub fn about(context: &egui::Context, open: &mut bool) -> Option<String> {
    let mut link = None;
    let mut close = false;
    egui::Modal::new(egui::Id::new("about"))
        .frame(appearance::dialog_frame(context))
        .show(context, |ui| {
            ui.set_width((context.content_rect().width() - 48.0).clamp(240.0, 460.0));
            ui.heading("Icy Term");
            ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
            if let Some(date) = option_env!("ICY_BUILD_DATE") {
                ui.weak(date);
            }
            ui.separator();
            ui.label(&*tr!("egui-about-description"));
            ui.add_space(4.0);
            for (label, url) in [
                ("github.com/mkrueger/icy_tools", "https://github.com/mkrueger/icy_tools"),
                (&*tr!("menu-item-discuss"), "https://github.com/mkrueger/icy_tools/discussions"),
                (&*tr!("menu-item-report-bug"), "https://github.com/mkrueger/icy_tools/issues"),
            ] {
                if ui.link(label).clicked() {
                    link = Some(url.to_string());
                }
            }
            ui.separator();
            close = ui.add(appearance::primary_button(tr!("egui-close"))).clicked();
        });
    if close {
        *open = false;
    }
    link
}

pub fn baud(context: &egui::Context, open: &mut bool, current: BaudEmulation) -> Option<BaudEmulation> {
    let id = egui::Id::new("bps-custom");
    let mut selected = None;
    let mut close = false;
    egui::Modal::new(egui::Id::new("bps"))
        .frame(appearance::dialog_frame(context))
        .show(context, |ui| {
            ui.set_width((context.content_rect().width() - 48.0).clamp(220.0, 420.0));
            ui.heading(&*tr!("select-bps-dialog-heading"));
            ui.separator();
            let mut custom = ui.data(|data| data.get_temp::<u32>(id)).unwrap_or(match current {
                BaudEmulation::Rate(rate) if !RATES.contains(&rate) => rate,
                _ => 2400,
            });
            egui::ScrollArea::vertical()
                .id_salt("bps-list")
                .auto_shrink([false, false])
                .max_height((context.content_rect().height() - 200.0).clamp(80.0, 320.0))
                .show(ui, |ui| {
                    if ui.radio(current == BaudEmulation::Off, &*tr!("select-bps-dialog-bps-max")).clicked() {
                        selected = Some(BaudEmulation::Off);
                    }
                    for rate in RATES {
                        if ui
                            .radio(current == BaudEmulation::Rate(rate), tr!("select-bps-dialog-bps", bps = rate))
                            .clicked()
                        {
                            selected = Some(BaudEmulation::Rate(rate));
                        }
                    }
                    ui.horizontal(|ui| {
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
            ui.separator();
            close = ui.button(&*tr!("egui-close")).clicked();
        });
    if close || selected.is_some() {
        *open = false;
    }
    selected
}
