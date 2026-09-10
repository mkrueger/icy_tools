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
            ui.set_min_height(height);
            close |= appearance::dialog_header(ui, &tr!("help-title"));
            egui::ScrollArea::vertical()
                .id_salt("shortcut-list")
                .auto_shrink([false, false])
                .min_scrolled_height(0.0)
                .max_height((height - 100.0).max(0.0))
                .show(ui, |ui| {
                    for (category, entries) in hotkeys::help_entries() {
                        appearance::section(ui, &category);
                        for (name, shortcut) in entries {
                            ui.horizontal(|ui| {
                                let width = (ui.available_width() * 0.40).min(150.0);
                                ui.allocate_ui_with_layout(egui::vec2(width, 22.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                    ui.set_min_width(width);
                                    ui.add(egui::Label::new(egui::RichText::new(shortcut).monospace()).wrap());
                                });
                                ui.add(egui::Label::new(name).wrap());
                            });
                        }
                    }
                });
            ui.separator();
            close |= ui.button(tr!("egui-close")).clicked();
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
            close |= appearance::dialog_header(ui, "Icy Term");
            egui::ScrollArea::vertical()
                .min_scrolled_height(0.0)
                .max_height((context.content_rect().height() - 160.0).max(0.0))
                .show(ui, |ui| {
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
                });
            ui.separator();
            close |= ui.button(tr!("egui-close")).clicked();
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
            let height = (context.content_rect().height() - 64.0).clamp(140.0, 360.0);
            ui.set_width((context.content_rect().width() - 48.0).clamp(220.0, 420.0));
            ui.set_min_height(height);
            close |= appearance::dialog_header(ui, &tr!("select-bps-dialog-heading"));
            let mut custom = ui.data(|data| data.get_temp::<u32>(id)).unwrap_or(match current {
                BaudEmulation::Rate(rate) if !RATES.contains(&rate) => rate,
                _ => 2400,
            });
            egui::ScrollArea::vertical()
                .id_salt("bps-list")
                .auto_shrink([false, false])
                .min_scrolled_height(0.0)
                .max_height((height - 104.0).max(0.0))
                .show(ui, |ui| {
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
            ui.separator();
            close |= ui.button(&*tr!("egui-close")).clicked();
        });
    if close || selected.is_some() {
        *open = false;
    }
    selected
}
