use super::appearance;
use crate::{MonitorSettings, MonitorType};

pub fn fields(ui: &mut ::egui::Ui, settings: &mut MonitorSettings, label: impl Fn(&str) -> String) {
    appearance::combo_row(ui, &label("egui-monitor-type"), format!("{:?}", settings.monitor_type), |ui| {
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
        appearance::form_row(ui, &label("egui-colors"), |ui| {
            if ui.color_edit_button_srgb(&mut color).changed() {
                settings.custom_monitor_color = icy_engine::Color::new(color[0], color[1], color[2]);
            }
        });
    }
    ui.checkbox(&mut settings.use_integer_scaling, label("egui-integer-scaling"));
    ui.checkbox(&mut settings.use_bilinear_filtering, label("egui-bilinear-filtering"));
    appearance::section(ui, &label("egui-picture"));
    appearance::slider_row(ui, &label("settings-monitor-brightness"), &mut settings.brightness, 0.0..=200.0);
    appearance::slider_row(ui, &label("settings-monitor-contrast"), &mut settings.contrast, 0.0..=200.0);
    appearance::slider_row(ui, &label("settings-monitor-gamma"), &mut settings.gamma, 0.1..=4.0);
    appearance::slider_row(ui, &label("settings-monitor-saturation"), &mut settings.saturation, 0.0..=200.0);
    appearance::section(ui, &label("egui-effects"));
    ui.checkbox(&mut settings.use_scanlines, label("settings-monitor-scanlines"));
    if settings.use_scanlines {
        appearance::slider_row(ui, &label("egui-thickness"), &mut settings.scanline_thickness, 0.0..=1.0);
        appearance::slider_row(ui, &label("egui-sharpness"), &mut settings.scanline_sharpness, 0.0..=1.0);
        appearance::slider_row(ui, &label("egui-phase"), &mut settings.scanline_phase, 0.0..=1.0);
    }
    ui.checkbox(&mut settings.use_bloom, label("egui-bloom"));
    if settings.use_bloom {
        appearance::slider_row(ui, &label("egui-threshold"), &mut settings.bloom_threshold, 0.0..=100.0);
        appearance::slider_row(ui, &label("egui-radius"), &mut settings.bloom_radius, 0.0..=50.0);
        appearance::slider_row(ui, &label("egui-glow"), &mut settings.glow_strength, 0.0..=100.0);
        appearance::slider_row(ui, &label("egui-persistence"), &mut settings.phosphor_persistence, 0.0..=100.0);
    }
    ui.checkbox(&mut settings.use_curvature, label("egui-curvature"));
    if settings.use_curvature {
        appearance::slider_row(ui, &label("egui-horizontal"), &mut settings.curvature_x, 0.0..=100.0);
        appearance::slider_row(ui, &label("egui-vertical"), &mut settings.curvature_y, 0.0..=100.0);
    }
    ui.checkbox(&mut settings.use_noise, label("egui-noise"));
    if settings.use_noise {
        appearance::slider_row(ui, &label("egui-noise-level"), &mut settings.noise_level, 0.0..=100.0);
        appearance::slider_row(ui, &label("egui-sync-wobble"), &mut settings.sync_wobble, 0.0..=100.0);
    }
}
