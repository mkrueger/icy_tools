use iced::{Color, Theme};

use crate::{MonitorSettings, MonitorType, settings::iced_to_icy_color};

#[derive(Debug, Clone)]
pub enum MonitorSettingsMessage {
    MonitorTypeChanged(MonitorType),
    CustomColorChanged(Color),
    BorderColorChanged(Color),
    UseFilterChanged(bool),
    BrightnessChanged(f32),
    ContrastChanged(f32),
    SaturationChanged(f32),
    GammaChanged(f32),
    BlurChanged(f32),
    CurvatureChanged(f32),
    ScanlinesChanged(f32),
    ThemeChanged(Theme),
    PixelPerfectScalingChanged(bool),
}

// Update function to handle monitor settings messages
pub fn update_monitor_settings(settings: &mut MonitorSettings, message: MonitorSettingsMessage) {
    match message {
        MonitorSettingsMessage::MonitorTypeChanged(monitor_type) => {
            settings.monitor_type = monitor_type;
        }
        MonitorSettingsMessage::CustomColorChanged(color) => {
            settings.custom_monitor_color = iced_to_icy_color(color);
        }
        MonitorSettingsMessage::BorderColorChanged(color) => {
            settings.border_color = iced_to_icy_color(color);
        }
        MonitorSettingsMessage::UseFilterChanged(use_filter) => {
            settings.use_filter = use_filter;
        }
        MonitorSettingsMessage::BrightnessChanged(brightness) => {
            settings.brightness = brightness;
        }
        MonitorSettingsMessage::ContrastChanged(contrast) => {
            settings.contrast = contrast;
        }
        MonitorSettingsMessage::SaturationChanged(saturation) => {
            settings.saturation = saturation;
        }
        MonitorSettingsMessage::GammaChanged(gamma) => {
            settings.gamma = gamma;
        }
        MonitorSettingsMessage::BlurChanged(blur) => {
            settings.blur = blur;
        }
        MonitorSettingsMessage::CurvatureChanged(curvature) => {
            settings.curvature = curvature;
        }
        MonitorSettingsMessage::ScanlinesChanged(scanlines) => {
            settings.scanlines = scanlines;
        }
        MonitorSettingsMessage::ThemeChanged(theme) => {
            settings.set_theme(theme);
        }
        MonitorSettingsMessage::PixelPerfectScalingChanged(val) => {
            settings.use_pixel_perfect_scaling = val;
        }
    }
}
