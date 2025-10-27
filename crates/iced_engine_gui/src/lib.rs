pub mod scene;
use iced::Color;
pub use scene::*;
use serde::{Deserialize, Serialize};

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MonitorType {
    Color = 0,
    BlackAndWhite = 1,
    Amber = 2,
    Green = 3,
    Apple2 = 4,
    Futuristic = 5,
    CustomMonochrome = 6,
}

impl MonitorType {
    pub fn get_color(&self) -> Color {
        match self {
            MonitorType::Color => Color::from_rgb8(0, 0, 0),
            MonitorType::BlackAndWhite => Color::from_rgb8(0xFF, 0xFF, 0xFF),
            MonitorType::Amber => Color::from_rgb8(0xFF, 0x81, 0x00),
            MonitorType::Green => Color::from_rgb8(0x0C, 0xCC, 0x68),
            MonitorType::Apple2 => Color::from_rgb8(0x00, 0xD5, 0x6D),
            MonitorType::Futuristic => Color::from_rgb8(0x72, 0x9F, 0xCF),
            MonitorType::CustomMonochrome => Color::from_rgb8(0, 0, 0),
        }
    }

    fn is_monochrome(&self) -> bool {
        *self != MonitorType::Color
    }
}

impl Into<i32> for MonitorType {
    fn into(self) -> i32 {
        match self {
            MonitorType::Color => 0,
            MonitorType::BlackAndWhite => 1,
            MonitorType::Amber => 2,
            MonitorType::Green => 3,
            MonitorType::Apple2 => 4,
            MonitorType::Futuristic => 5,
            MonitorType::CustomMonochrome => 6,
        }
    }
}

impl From<i32> for MonitorType {
    fn from(value: i32) -> Self {
        match value {
            0 => MonitorType::Color,
            1 => MonitorType::BlackAndWhite,
            2 => MonitorType::Amber,
            3 => MonitorType::Green,
            4 => MonitorType::Apple2,
            5 => MonitorType::Futuristic,
            _ => MonitorType::CustomMonochrome,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorSettings {
    pub use_filter: bool,

    pub monitor_type: MonitorType,
    pub custom_monitor_color: Color,
    pub border_color: Color,

    pub gamma: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub brightness: f32,
    pub light: f32,
    pub blur: f32,
    pub curvature: f32,
    pub scanlines: f32,
    pub background_effect: BackgroundEffect,
    pub selection_fg: Color,
    pub selection_bg: Color,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkerSettings {
    pub reference_image_alpha: f32,

    pub raster_alpha: f32,
    pub raster_color: Color,

    pub guide_alpha: f32,
    pub guide_color: Color,
}

impl Default for MarkerSettings {
    fn default() -> Self {
        Self {
            reference_image_alpha: 0.2,
            raster_alpha: 0.2,
            raster_color: Color::from_rgb8(0xBB, 0xBB, 0xBB),
            guide_alpha: 0.2,
            guide_color: Color::from_rgb8(0xAB, 0xAB, 0xAB),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BackgroundEffect {
    None,
    Checkers,
}

unsafe impl Send for MonitorSettings {}
unsafe impl Sync for MonitorSettings {}

impl Default for MonitorSettings {
    fn default() -> Self {
        Self {
            use_filter: false,
            monitor_type: MonitorType::Color,
            gamma: 50.,
            contrast: 50.,
            saturation: 50.,
            brightness: 30.,
            light: 40.,
            blur: 30.,
            curvature: 10.,
            scanlines: 10.,
            background_effect: BackgroundEffect::None,
            custom_monitor_color: Color::from_rgb8(0xFF, 0xFF, 0xFF),
            selection_fg: Color::from_rgb8(0xAB, 0x00, 0xAB),
            selection_bg: Color::from_rgb8(0xAB, 0xAB, 0xAB),
            border_color: Color::from_rgb8(64, 69, 74),
        }
    }
}

impl MonitorSettings {
    pub fn neutral() -> Self {
        Self {
            use_filter: true,
            monitor_type: MonitorType::Color,
            gamma: 50.,
            contrast: 50.,
            saturation: 50.,
            brightness: 29.,
            light: 50.,
            blur: 0.,
            curvature: 0.,
            scanlines: 0.,
            background_effect: BackgroundEffect::None,
            custom_monitor_color: Color::from_rgb8(0xFF, 0xFF, 0xFF),
            selection_fg: Color::from_rgb8(0xAB, 0x00, 0xAB),
            selection_bg: Color::from_rgb8(0xAB, 0xAB, 0xAB),
            border_color: Color::from_rgb8(64, 69, 74),
        }
    }

    fn get_monochrome_color(&self) -> Color {
        match self.monitor_type {
            MonitorType::CustomMonochrome => self.custom_monitor_color.clone(),
            _ => self.monitor_type.get_color(),
        }
    }
}
