use serde::{Deserialize, Serialize};

/// Color type for monitor settings (RGB)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MonitorType {
    Color = 0,
    Grayscale = 1,
    Amber = 2,
    Green = 3,
    Apple2 = 4,
    Futuristic = 5,
    CustomMonochrome = 6,
}

impl MonitorType {
    pub fn color(&self) -> Color {
        match self {
            MonitorType::Color => Color::new(0, 0, 0),
            MonitorType::Grayscale => Color::new(0xFF, 0xFF, 0xFF),
            MonitorType::Amber => Color::new(0xFF, 0x81, 0x00),
            MonitorType::Green => Color::new(0x0C, 0xCC, 0x68),
            MonitorType::Apple2 => Color::new(0x00, 0xD5, 0x6D),
            MonitorType::Futuristic => Color::new(0x72, 0x9F, 0xCF),
            MonitorType::CustomMonochrome => Color::new(0, 0, 0),
        }
    }

    pub fn is_monochrome(&self) -> bool {
        *self != MonitorType::Color
    }
}

impl From<i32> for MonitorType {
    fn from(value: i32) -> Self {
        match value {
            0 => MonitorType::Color,
            1 => MonitorType::Grayscale,
            2 => MonitorType::Amber,
            3 => MonitorType::Green,
            4 => MonitorType::Apple2,
            5 => MonitorType::Futuristic,
            _ => MonitorType::CustomMonochrome,
        }
    }
}

impl From<MonitorType> for i32 {
    fn from(value: MonitorType) -> Self {
        match value {
            MonitorType::Color => 0,
            MonitorType::Grayscale => 1,
            MonitorType::Amber => 2,
            MonitorType::Green => 3,
            MonitorType::Apple2 => 4,
            MonitorType::Futuristic => 5,
            MonitorType::CustomMonochrome => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BackgroundEffect {
    None,
    Checkers,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorSettings {
    pub monitor_type: MonitorType,
    pub custom_monitor_color: Color,
    pub border_color: Color,

    pub brightness: f32,
    pub contrast: f32,
    pub gamma: f32,
    pub saturation: f32,

    pub background_effect: BackgroundEffect,

    pub use_pixel_perfect_scaling: bool,
    pub use_bilinear_filtering: bool,

    pub use_bloom: bool,
    pub bloom_threshold: f32,
    pub bloom_radius: f32,
    pub glow_strength: f32,
    pub phosphor_persistence: f32,

    pub use_scanlines: bool,
    pub scanline_thickness: f32,
    pub scanline_sharpness: f32,
    pub scanline_phase: f32,

    pub use_curvature: bool,
    pub curvature_x: f32,
    pub curvature_y: f32,

    pub use_noise: bool,
    pub noise_level: f32,
    pub sync_wobble: f32,

    // Legacy fields for backward compatibility with old scripts
    #[serde(default)]
    pub blur: f32,
    #[serde(default)]
    pub curvature: f32,
    #[serde(default)]
    pub scanlines: f32,
}

unsafe impl Send for MonitorSettings {}
unsafe impl Sync for MonitorSettings {}

impl Default for MonitorSettings {
    fn default() -> Self {
        MonitorSettings::neutral()
    }
}

impl MonitorSettings {
    pub fn neutral() -> Self {
        Self {
            monitor_type: MonitorType::Color,
            custom_monitor_color: Color::new(0xFF, 0xFF, 0xFF),
            border_color: Color::new(64, 69, 74),

            brightness: 100.0,
            contrast: 100.0,
            gamma: 1.0,
            saturation: 100.0,

            background_effect: BackgroundEffect::None,

            use_pixel_perfect_scaling: false,
            use_bilinear_filtering: false,

            use_bloom: false,
            bloom_threshold: 25.0,
            bloom_radius: 3.0,
            glow_strength: 15.0,
            phosphor_persistence: 10.0,

            use_scanlines: false,
            scanline_thickness: 0.5,
            scanline_sharpness: 0.5,
            scanline_phase: 0.0,

            use_curvature: false,
            curvature_x: 60.0,
            curvature_y: 60.0,

            use_noise: false,
            noise_level: 20.0,
            sync_wobble: 20.0,

            // Legacy fields
            blur: 0.0,
            curvature: 0.0,
            scanlines: 0.0,
        }
    }

    pub fn get_monochrome_color(&self) -> Color {
        match self.monitor_type {
            MonitorType::CustomMonochrome => self.custom_monitor_color.clone(),
            _ => self.monitor_type.color(),
        }
    }
}
