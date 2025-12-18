use crate::{MonitorSettings, MonitorType, ScalingMode, settings::iced_to_icy_color};
use iced::{Color, Theme};

#[derive(Debug, Clone)]
pub enum MonitorSettingsMessage {
    // Appearance / basic
    ThemeChanged(Theme),
    MonitorTypeChanged(MonitorType),
    CustomColorChanged(Color),
    IntegerScalingChanged(bool),
    BilinearFilteringChanged(bool),
    ScalingModeChanged(ScalingMode),

    // Tone (always applied now)
    BrightnessChanged(f32),
    ContrastChanged(f32),
    GammaChanged(f32),
    SaturationChanged(f32),

    // Bloom / glow
    BloomToggleChanged(bool),
    BloomThresholdChanged(f32),
    BloomRadiusChanged(f32),
    GlowStrengthChanged(f32),
    PhosphorPersistenceChanged(f32),

    // Scanlines
    ScanlinesToggleChanged(bool),
    ScanlineThicknessChanged(f32),
    ScanlineSharpnessChanged(f32),
    ScanlinePhaseChanged(f32),

    // Geometry
    CurvatureToggleChanged(bool),
    CurvatureXChanged(f32),
    CurvatureYChanged(f32),

    // Noise / artifacts
    NoiseToggleChanged(bool),
    NoiseLevelChanged(f32),
    SyncWobbleChanged(f32),

    Noop,
}

pub fn update_monitor_settings(settings: &mut MonitorSettings, message: MonitorSettingsMessage) {
    match message {
        MonitorSettingsMessage::ThemeChanged(t) => settings.set_theme(t),
        MonitorSettingsMessage::MonitorTypeChanged(m) => settings.monitor_type = m,
        MonitorSettingsMessage::CustomColorChanged(c) => settings.custom_monitor_color = iced_to_icy_color(c),
        MonitorSettingsMessage::IntegerScalingChanged(v) => settings.use_integer_scaling = v,
        MonitorSettingsMessage::BilinearFilteringChanged(v) => settings.use_bilinear_filtering = v,
        MonitorSettingsMessage::ScalingModeChanged(mode) => settings.scaling_mode = mode,

        MonitorSettingsMessage::BrightnessChanged(v) => settings.brightness = v,
        MonitorSettingsMessage::ContrastChanged(v) => settings.contrast = v,
        MonitorSettingsMessage::GammaChanged(v) => settings.gamma = v,
        MonitorSettingsMessage::SaturationChanged(v) => settings.saturation = v,

        // Bloom / glow
        MonitorSettingsMessage::BloomToggleChanged(v) => settings.use_bloom = v,
        MonitorSettingsMessage::BloomThresholdChanged(v) => settings.bloom_threshold = v,
        MonitorSettingsMessage::BloomRadiusChanged(v) => settings.bloom_radius = v,
        MonitorSettingsMessage::GlowStrengthChanged(v) => settings.glow_strength = v,
        MonitorSettingsMessage::PhosphorPersistenceChanged(v) => settings.phosphor_persistence = v,

        // Scanlines
        MonitorSettingsMessage::ScanlinesToggleChanged(v) => settings.use_scanlines = v,
        MonitorSettingsMessage::ScanlineThicknessChanged(v) => settings.scanline_thickness = v,
        MonitorSettingsMessage::ScanlineSharpnessChanged(v) => settings.scanline_sharpness = v,
        MonitorSettingsMessage::ScanlinePhaseChanged(v) => settings.scanline_phase = v,

        // Geometry
        MonitorSettingsMessage::CurvatureToggleChanged(v) => settings.use_curvature = v,
        MonitorSettingsMessage::CurvatureXChanged(v) => settings.curvature_x = v,
        MonitorSettingsMessage::CurvatureYChanged(v) => settings.curvature_y = v,

        // Noise / artifacts
        MonitorSettingsMessage::NoiseToggleChanged(v) => settings.use_noise = v,
        MonitorSettingsMessage::NoiseLevelChanged(v) => settings.noise_level = v,
        MonitorSettingsMessage::SyncWobbleChanged(v) => settings.sync_wobble = v,

        MonitorSettingsMessage::Noop => { /* Do nothing */ }
    }
}
