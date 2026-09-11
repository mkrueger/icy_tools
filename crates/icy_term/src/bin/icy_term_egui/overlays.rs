//! Overlay dialogs that the legacy client offered: shortcuts, about and BPS emulation.

use eframe::egui;
use icy_parser_core::BaudEmulation;

use super::{appearance, hotkeys};

/// Same list the legacy BPS dialog offered.
const RATES: [u32; 11] = [300, 1200, 2400, 4800, 9600, 14400, 19200, 28800, 38400, 57600, 115_200];

fn key_pill(ui: &mut egui::Ui, label: &str) {
    let accent = ui.visuals().selection.stroke.color;
    let bright = u32::from(accent.r()) + u32::from(accent.g()) + u32::from(accent.b()) > 380;
    let text = if bright { egui::Color32::from_rgb(18, 22, 28) } else { egui::Color32::WHITE };
    let font = egui::FontId::proportional(12.0);
    let width = ui.fonts_mut(|fonts| fonts.layout_no_wrap(label.to_owned(), font.clone(), text).size().x);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width + 18.0, 24.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 8.0, accent);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, font, text);
}

fn category_band(ui: &mut egui::Ui, name: &str) {
    egui::Frame::new()
        .fill(ui.visuals().widgets.inactive.bg_fill.gamma_multiply(0.6))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(12, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(name).size(15.0).strong());
        });
}

fn shortcut_row(ui: &mut egui::Ui, entry: &hotkeys::HelpEntry, shaded: bool, compact: bool) {
    let fill = if shaded { ui.visuals().faint_bg_color } else { egui::Color32::TRANSPARENT };
    egui::Frame::new().fill(fill).inner_margin(egui::Margin::symmetric(12, 5)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            let keys = if compact { 132.0 } else { 186.0 };
            ui.allocate_ui_with_layout(egui::vec2(keys, 0.0), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                ui.set_min_width(keys);
                for (index, part) in hotkeys::key_parts(&entry.shortcut).iter().enumerate() {
                    if index > 0 {
                        ui.label("+");
                    }
                    key_pill(ui, part);
                }
            });
            let action = if compact {
                ui.available_width()
            } else {
                (ui.available_width() * 0.40).min(150.0)
            };
            ui.allocate_ui_with_layout(egui::vec2(action, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_min_width(action);
                ui.add(egui::Label::new(&entry.action).wrap());
            });
            if !compact && !entry.description.is_empty() {
                ui.add(egui::Label::new(egui::RichText::new(&entry.description).color(ui.visuals().weak_text_color())).wrap());
            }
        });
    });
}

pub fn help(context: &egui::Context, open: &mut bool) {
    let mut close = false;
    egui::Modal::new(egui::Id::new("shortcuts"))
        .frame(appearance::dialog_frame(context))
        .show(context, |ui| {
            let width = (context.content_rect().width() - 48.0).clamp(240.0, 700.0);
            let height = (context.content_rect().height() - 64.0).clamp(140.0, 560.0);
            let compact = width < 520.0;
            ui.set_width(width);
            ui.set_min_height(height);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("\u{2328}").size(24.0));
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.add(egui::Label::new(egui::RichText::new(tr!("help-title")).size(20.0).strong()).truncate());
                    ui.add(egui::Label::new(egui::RichText::new(tr!("help-subtitle")).size(12.0).weak()).truncate());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close |= ui
                        .add_sized([24.0, 24.0], egui::Button::new(egui::RichText::new("\u{00d7}").size(20.0)).frame(false))
                        .on_hover_text(tr!("egui-close"))
                        .clicked();
                });
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("shortcut-list")
                .auto_shrink([false, false])
                .min_scrolled_height(0.0)
                .max_height((height - 110.0).max(0.0))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (category, entries) in hotkeys::help_entries() {
                        category_band(ui, &category);
                        for (index, entry) in entries.iter().enumerate() {
                            shortcut_row(ui, entry, index % 2 == 0, compact);
                        }
                        ui.add_space(6.0);
                    }
                });
            ui.separator();
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close |= ui.add(appearance::primary_button(tr!("egui-close"))).clicked();
                });
            });
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
