use super::appearance;
use crate::{MonitorSettings, MonitorType, LANGUAGE_LOADER};
use i18n_embed_fl::fl;

fn monitor_name(monitor: MonitorType) -> String {
    match monitor {
        MonitorType::Color => fl!(LANGUAGE_LOADER, "settings-monitor-color"),
        MonitorType::Grayscale => fl!(LANGUAGE_LOADER, "settings-monitor-grayscale"),
        MonitorType::Amber => fl!(LANGUAGE_LOADER, "settings-monitor-amber"),
        MonitorType::Green => fl!(LANGUAGE_LOADER, "settings-monitor-green"),
        MonitorType::Apple2 => fl!(LANGUAGE_LOADER, "settings-monitor-apple2"),
        MonitorType::Futuristic => fl!(LANGUAGE_LOADER, "settings-monitor-futuristic"),
        MonitorType::CustomMonochrome => fl!(LANGUAGE_LOADER, "settings-monitor-custom"),
    }
}

/// Effect group whose sliders stay visible but inactive while the effect is off, so the page keeps its layout.
fn effect(ui: &mut ::egui::Ui, title: &str, enabled: &mut bool, sliders: impl FnOnce(&mut ::egui::Ui)) {
    appearance::group(ui, title, |ui| {
        appearance::check_row(ui, &fl!(LANGUAGE_LOADER, "settings-enabled-checkbox"), enabled);
        ui.add_enabled_ui(*enabled, sliders);
    });
}

pub fn fields(ui: &mut ::egui::Ui, settings: &mut MonitorSettings) {
    appearance::group(ui, &fl!(LANGUAGE_LOADER, "settings-appearance-section"), |ui| {
        appearance::combo_row(ui, &fl!(LANGUAGE_LOADER, "settings-monitor-type"), monitor_name(settings.monitor_type), |ui| {
            for mode in [
                MonitorType::Color,
                MonitorType::Grayscale,
                MonitorType::Amber,
                MonitorType::Green,
                MonitorType::Apple2,
                MonitorType::Futuristic,
                MonitorType::CustomMonochrome,
            ] {
                ui.selectable_value(&mut settings.monitor_type, mode, monitor_name(mode));
            }
        });
        if settings.monitor_type == MonitorType::CustomMonochrome {
            let (red, green, blue) = settings.custom_monitor_color.rgb();
            let mut color = [red, green, blue];
            appearance::form_row(ui, &fl!(LANGUAGE_LOADER, "settings-monitor-custom"), |ui| {
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.custom_monitor_color = icy_engine::Color::new(color[0], color[1], color[2]);
                }
            });
        }
        appearance::check_row(ui, &fl!(LANGUAGE_LOADER, "settings-integer-scaling-label"), &mut settings.use_integer_scaling);
        appearance::check_row(
            ui,
            &fl!(LANGUAGE_LOADER, "settings-bilinear-filtering-label"),
            &mut settings.use_bilinear_filtering,
        );
    });
    appearance::group(ui, &fl!(LANGUAGE_LOADER, "settings-color-tone-section"), |ui| {
        appearance::slider_row(ui, &fl!(LANGUAGE_LOADER, "settings-monitor-brightness"), &mut settings.brightness, 0.0..=200.0);
        appearance::slider_row(ui, &fl!(LANGUAGE_LOADER, "settings-monitor-contrast"), &mut settings.contrast, 0.0..=200.0);
        appearance::slider_row(ui, &fl!(LANGUAGE_LOADER, "settings-monitor-gamma"), &mut settings.gamma, 0.1..=4.0);
        appearance::slider_row(ui, &fl!(LANGUAGE_LOADER, "settings-monitor-saturation"), &mut settings.saturation, 0.0..=200.0);
    });
    effect(ui, &fl!(LANGUAGE_LOADER, "settings-scanlines-section"), &mut settings.use_scanlines, |ui| {
        appearance::slider_row(
            ui,
            &fl!(LANGUAGE_LOADER, "settings-scanline-thickness-label"),
            &mut settings.scanline_thickness,
            0.0..=1.0,
        );
        appearance::slider_row(
            ui,
            &fl!(LANGUAGE_LOADER, "settings-scanline-sharpness-label"),
            &mut settings.scanline_sharpness,
            0.0..=1.0,
        );
        appearance::slider_row(
            ui,
            &fl!(LANGUAGE_LOADER, "settings-scanline-phase-label"),
            &mut settings.scanline_phase,
            0.0..=1.0,
        );
    });
    effect(ui, &fl!(LANGUAGE_LOADER, "settings-bloom-glow-section"), &mut settings.use_bloom, |ui| {
        appearance::slider_row(
            ui,
            &fl!(LANGUAGE_LOADER, "settings-bloom-threshold-label"),
            &mut settings.bloom_threshold,
            0.0..=100.0,
        );
        appearance::slider_row(ui, &fl!(LANGUAGE_LOADER, "settings-bloom-radius-label"), &mut settings.bloom_radius, 0.0..=50.0);
        appearance::slider_row(
            ui,
            &fl!(LANGUAGE_LOADER, "settings-glow-strength-label"),
            &mut settings.glow_strength,
            0.0..=100.0,
        );
        appearance::slider_row(
            ui,
            &fl!(LANGUAGE_LOADER, "settings-phosphor-persistence-label"),
            &mut settings.phosphor_persistence,
            0.0..=100.0,
        );
    });
    effect(ui, &fl!(LANGUAGE_LOADER, "settings-geometry-section"), &mut settings.use_curvature, |ui| {
        appearance::slider_row(ui, &fl!(LANGUAGE_LOADER, "settings-curvature-x-label"), &mut settings.curvature_x, 0.0..=100.0);
        appearance::slider_row(ui, &fl!(LANGUAGE_LOADER, "settings-curvature-y-label"), &mut settings.curvature_y, 0.0..=100.0);
    });
    effect(ui, &fl!(LANGUAGE_LOADER, "settings-noise-artifacts-section"), &mut settings.use_noise, |ui| {
        appearance::slider_row(ui, &fl!(LANGUAGE_LOADER, "settings-noise-level-label"), &mut settings.noise_level, 0.0..=100.0);
        appearance::slider_row(ui, &fl!(LANGUAGE_LOADER, "settings-sync-wobble-label"), &mut settings.sync_wobble, 0.0..=100.0);
    });
}
