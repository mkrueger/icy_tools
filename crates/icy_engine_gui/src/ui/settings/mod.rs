use egui::{color_picker, Color32, RichText};
use i18n_embed_fl::fl;
use icy_engine::Color;
use lazy_static::lazy_static;

use crate::{ui::LANGUAGE_LOADER, MonitorSettings};
lazy_static! {
    static ref MONITOR_NAMES: [String; 6] = [
        fl!(LANGUAGE_LOADER, "settings-monitor-color"),
        fl!(LANGUAGE_LOADER, "settings-monitor-grayscale"),
        fl!(LANGUAGE_LOADER, "settings-monitor-amber"),
        fl!(LANGUAGE_LOADER, "settings-monitor-green"),
        fl!(LANGUAGE_LOADER, "settings-monitor-apple2"),
        fl!(LANGUAGE_LOADER, "settings-monitor-futuristic"),
    ];
}

pub fn show_monitor_settings(ui: &mut egui::Ui, old_settings: &MonitorSettings) -> Option<MonitorSettings> {
    let mut result = None;

    let mut monitor_settings = old_settings.clone();

    let cur_color = monitor_settings.monitor_type;
    egui::ComboBox::from_label(fl!(LANGUAGE_LOADER, "settings-monitor-type"))
        .width(150.)
        .selected_text(&MONITOR_NAMES[cur_color])
        .show_ui(ui, |ui| {
            (0..MONITOR_NAMES.len()).for_each(|i| {
                let label = RichText::new(&MONITOR_NAMES[i]);
                ui.selectable_value(&mut monitor_settings.monitor_type, i, label);
            });
        });

    ui.horizontal(|ui| {
        ui.label(fl!(LANGUAGE_LOADER, "settings-background_color-label"));
        let (r, g, b) = monitor_settings.border_color.get_rgb();
        let mut color = Color32::from_rgb(r, g, b);
        color_picker::color_edit_button_srgba(ui, &mut color, color_picker::Alpha::Opaque);
        monitor_settings.border_color = Color::new(color.r(), color.g(), color.b());
    });
    let use_filter = monitor_settings.use_filter;

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    ui.checkbox(
        &mut monitor_settings.use_filter,
        fl!(LANGUAGE_LOADER, "settings-monitor-use-crt-filter-checkbox"),
    );

    ui.add_enabled_ui(use_filter, |ui| {
        // todo: that should take full with, but doesn't work - egui bug ?
        ui.vertical_centered_justified(|ui| {
            ui.add(egui::Slider::new(&mut monitor_settings.brightness, 0.0..=100.0).text(fl!(LANGUAGE_LOADER, "settings-monitor-brightness")));
            ui.add(egui::Slider::new(&mut monitor_settings.contrast, 0.0..=100.0).text(fl!(LANGUAGE_LOADER, "settings-monitor-contrast")));
            ui.add(egui::Slider::new(&mut monitor_settings.saturation, 0.0..=100.0).text(fl!(LANGUAGE_LOADER, "settings-monitor-saturation")));
            ui.add(egui::Slider::new(&mut monitor_settings.gamma, 0.0..=100.0).text(fl!(LANGUAGE_LOADER, "settings-monitor-gamma")));
            /*  ui.add_enabled(
                use_filter,
                egui::Slider::new(
                    &mut window.buffer_view.lock().monitor_settings.light,
                    0.0..=100.0,
                )
                .text("Light"),
            );*/
            ui.add(egui::Slider::new(&mut monitor_settings.blur, 0.0..=100.0).text(fl!(LANGUAGE_LOADER, "settings-monitor-blur")));
            ui.add(egui::Slider::new(&mut monitor_settings.curvature, 0.0..=100.0).text(fl!(LANGUAGE_LOADER, "settings-monitor-curve")));
            ui.add(egui::Slider::new(&mut monitor_settings.scanlines, 0.0..=100.0).text(fl!(LANGUAGE_LOADER, "settings-monitor-scanlines")));
        });
    });

    ui.add_space(8.0);
    if monitor_settings != *old_settings {
        result = Some(monitor_settings);
    }
    result
}
