//! Monitor and CRT effect controls, shared by the monitor window and the settings dialog.

use eframe::egui;
use icy_engine_gui::{MonitorSettings, MonitorType};

use super::appearance;

pub fn fields(ui: &mut egui::Ui, settings: &mut MonitorSettings) {
    appearance::combo_row(ui, &tr!("egui-monitor-type"), format!("{:?}", settings.monitor_type), |ui| {
        for mode in [
            MonitorType::Color,
            MonitorType::Grayscale,
            MonitorType::Amber,
            MonitorType::Green,
            MonitorType::Apple2,
            MonitorType::Futuristic,
            MonitorType::CustomMonochrome,
        ] {
            ui.selectable_value(&mut settings.monitor_type, mode, format!("{mode:?}"));
        }
    });
    if settings.monitor_type == MonitorType::CustomMonochrome {
        let (red, green, blue) = settings.custom_monitor_color.rgb();
        let mut color = [red, green, blue];
        appearance::form_row(ui, &tr!("egui-colors"), |ui| {
            if ui.color_edit_button_srgb(&mut color).changed() {
                settings.custom_monitor_color = icy_engine::Color::new(color[0], color[1], color[2]);
            }
        });
    }
    ui.checkbox(&mut settings.use_integer_scaling, &*tr!("egui-integer-scaling"));
    ui.checkbox(&mut settings.use_bilinear_filtering, &*tr!("egui-bilinear-filtering"));

    appearance::section(ui, &tr!("egui-picture"));
    appearance::slider_row(ui, &tr!("settings-monitor-brightness"), &mut settings.brightness, 0.0..=200.0);
    appearance::slider_row(ui, &tr!("settings-monitor-contrast"), &mut settings.contrast, 0.0..=200.0);
    appearance::slider_row(ui, &tr!("settings-monitor-gamma"), &mut settings.gamma, 0.1..=4.0);
    appearance::slider_row(ui, &tr!("settings-monitor-saturation"), &mut settings.saturation, 0.0..=200.0);

    appearance::section(ui, &tr!("egui-effects"));
    ui.checkbox(&mut settings.use_scanlines, &*tr!("settings-monitor-scanlines"));
    if settings.use_scanlines {
        appearance::slider_row(ui, &tr!("egui-thickness"), &mut settings.scanline_thickness, 0.0..=1.0);
        appearance::slider_row(ui, &tr!("egui-sharpness"), &mut settings.scanline_sharpness, 0.0..=1.0);
        appearance::slider_row(ui, &tr!("egui-phase"), &mut settings.scanline_phase, 0.0..=1.0);
    }
    ui.checkbox(&mut settings.use_bloom, &*tr!("egui-bloom"));
    if settings.use_bloom {
        appearance::slider_row(ui, &tr!("egui-threshold"), &mut settings.bloom_threshold, 0.0..=100.0);
        appearance::slider_row(ui, &tr!("egui-radius"), &mut settings.bloom_radius, 0.0..=50.0);
        appearance::slider_row(ui, &tr!("egui-glow"), &mut settings.glow_strength, 0.0..=100.0);
        appearance::slider_row(ui, &tr!("egui-persistence"), &mut settings.phosphor_persistence, 0.0..=100.0);
    }
    ui.checkbox(&mut settings.use_curvature, &*tr!("egui-curvature"));
    if settings.use_curvature {
        appearance::slider_row(ui, &tr!("egui-horizontal"), &mut settings.curvature_x, 0.0..=100.0);
        appearance::slider_row(ui, &tr!("egui-vertical"), &mut settings.curvature_y, 0.0..=100.0);
    }
    ui.checkbox(&mut settings.use_noise, &*tr!("egui-noise"));
    if settings.use_noise {
        appearance::slider_row(ui, &tr!("egui-noise-level"), &mut settings.noise_level, 0.0..=100.0);
        appearance::slider_row(ui, &tr!("egui-sync-wobble"), &mut settings.sync_wobble, 0.0..=100.0);
    }
}
