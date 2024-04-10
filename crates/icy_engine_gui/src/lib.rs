pub mod animations;
use icy_engine::Color;
use serde::{Deserialize, Serialize};

#[cfg(feature = "ui")]
pub mod ui;
#[cfg(feature = "ui")]
pub use ui::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorSettings {
    pub use_filter: bool,

    pub monitor_type: usize,
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
            raster_color: Color::new(0xAB, 0xAB, 0xAB),
            guide_alpha: 0.2,
            guide_color: Color::new(0xAB, 0xAB, 0xAB),
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
            monitor_type: 0,
            gamma: 50.,
            contrast: 50.,
            saturation: 50.,
            brightness: 30.,
            light: 40.,
            blur: 30.,
            curvature: 10.,
            scanlines: 10.,
            background_effect: BackgroundEffect::None,
            selection_fg: Color::new(0xAB, 0x00, 0xAB),
            selection_bg: Color::new(0xAB, 0xAB, 0xAB),
            border_color: Color::new(64, 69, 74),
        }
    }
}

impl MonitorSettings {
    pub fn neutral() -> Self {
        Self {
            use_filter: true,
            monitor_type: 0,
            gamma: 50.,
            contrast: 50.,
            saturation: 50.,
            brightness: 29.,
            light: 50.,
            blur: 0.,
            curvature: 0.,
            scanlines: 0.,
            background_effect: BackgroundEffect::None,
            selection_fg: Color::new(0xAB, 0x00, 0xAB),
            selection_bg: Color::new(0xAB, 0xAB, 0xAB),
            border_color: Color::new(64, 69, 74),
        }
    }
}
