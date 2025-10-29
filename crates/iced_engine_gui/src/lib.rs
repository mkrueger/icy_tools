pub mod terminal;
use icy_engine::Color;
pub use terminal::*;

pub mod terminal_shader;
pub use terminal_shader::*;

pub mod terminal_view;
pub use terminal_view::*;

pub mod key_map;
pub mod settings;

pub mod blink;
pub use blink::*;

//pub mod terminal_shader_widget;

use serde::{Deserialize, Serialize};

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
    pub fn get_color(&self) -> Color {
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

    fn _is_monochrome(&self) -> bool {
        *self != MonitorType::Color
    }
}

impl Into<i32> for MonitorType {
    fn into(self) -> i32 {
        match self {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorSettings {
    pub theme: String,

    pub monitor_type: MonitorType,
    pub custom_monitor_color: Color,
    pub border_color: Color,

    pub brightness: f32,
    pub contrast: f32,
    pub gamma: f32,
    pub saturation: f32,

    pub background_effect: BackgroundEffect,
    pub selection_fg: Color,
    pub selection_bg: Color,

    pub use_pixel_perfect_scaling: bool,

    pub use_bloom: bool,
    pub bloom_threshold: f32,
    pub bloom_radius: f32,
    pub glow_strength: f32,
    pub phosphor_persistence: f32, // decay speed (higher = longer afterglow)

    pub use_scanlines: bool,
    pub scanline_thickness: f32, // 0..1 relative thickness
    pub scanline_sharpness: f32, // exponent/style
    pub scanline_phase: f32,     // offset for anim/flicker

    pub use_curvature: bool,
    pub curvature_x: f32,
    pub curvature_y: f32,

    pub use_noise: bool,
    pub noise_level: f32,
    pub sync_wobble: f32,
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
            raster_color: Color::new(0xBB, 0xBB, 0xBB),
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
        MonitorSettings::neutral()
    }
}

impl MonitorSettings {
    pub fn neutral() -> Self {
        Self {
            theme: "Dark".to_string(),

            // Display settings
            monitor_type: MonitorType::Color,
            custom_monitor_color: Color::new(0xFF, 0xFF, 0xFF),
            border_color: Color::new(64, 69, 74),

            // Color adjustments - neutral values
            brightness: 100.0, // 100% = 1.0 multiplier (neutral)
            contrast: 100.0,   // 100% = 1.0 multiplier (neutral)
            gamma: 1.0,
            saturation: 100.0, // 100% = 1.0 multiplier (full saturation)

            // Effects
            background_effect: BackgroundEffect::None,
            selection_fg: Color::new(0xAB, 0x00, 0xAB),
            selection_bg: Color::new(0xAB, 0xAB, 0xAB),

            // Scaling
            use_pixel_perfect_scaling: true,

            // CRT effects - all disabled for neutral
            use_bloom: false,
            bloom_threshold: 0.7,
            bloom_radius: 0.0,
            glow_strength: 0.0,
            phosphor_persistence: 0.0,

            use_scanlines: false,
            scanline_thickness: 0.5,
            scanline_sharpness: 0.5,
            scanline_phase: 0.0,

            use_curvature: false,
            curvature_x: 60.0,
            curvature_y: 60.0,

            use_noise: false,
            noise_level: 0.0,
            sync_wobble: 0.0,
        }
    }

    fn _get_monochrome_color(&self) -> Color {
        match self.monitor_type {
            MonitorType::CustomMonochrome => self.custom_monitor_color.clone(),
            _ => self.monitor_type.get_color(),
        }
    }

    pub fn get_theme(&self) -> iced::Theme {
        match self.theme.as_str() {
            "Light" => iced::Theme::Light,
            "Dark" => iced::Theme::Dark,
            "Dracula" => iced::Theme::Dracula,
            "Nord" => iced::Theme::Nord,
            "SolarizedLight" => iced::Theme::SolarizedLight,
            "SolarizedDark" => iced::Theme::SolarizedDark,
            "GruvboxLight" => iced::Theme::GruvboxLight,
            "GruvboxDark" => iced::Theme::GruvboxDark,
            "Ferra" => iced::Theme::Ferra,
            "CatppuccinLatte" => iced::Theme::CatppuccinLatte,
            "CatppuccinFrappe" => iced::Theme::CatppuccinFrappe,
            "CatppuccinMacchiato" => iced::Theme::CatppuccinMacchiato,
            "CatppuccinMocha" => iced::Theme::CatppuccinMocha,
            "TokyoNight" => iced::Theme::TokyoNight,
            "TokyoNightStorm" => iced::Theme::TokyoNightStorm,
            "TokyoNightLight" => iced::Theme::TokyoNightLight,
            "KanagawaWave" => iced::Theme::KanagawaWave,
            "KanagawaDragon" => iced::Theme::KanagawaDragon,
            "KanagawaLotus" => iced::Theme::KanagawaLotus,
            "Moonfly" => iced::Theme::Moonfly,
            "Nightfly" => iced::Theme::Nightfly,
            "Oxocarbon" => iced::Theme::Oxocarbon,
            // Default to Dark theme if theme string is empty or unrecognized
            _ => iced::Theme::Dark,
        }
    }

    pub fn set_theme(&mut self, theme: iced::Theme) {
        self.theme = match theme {
            iced::Theme::Light => "Light",
            iced::Theme::Dark => "Dark",
            iced::Theme::Dracula => "Dracula",
            iced::Theme::Nord => "Nord",
            iced::Theme::SolarizedLight => "SolarizedLight",
            iced::Theme::SolarizedDark => "SolarizedDark",
            iced::Theme::GruvboxLight => "GruvboxLight",
            iced::Theme::GruvboxDark => "GruvboxDark",
            iced::Theme::CatppuccinLatte => "CatppuccinLatte",
            iced::Theme::CatppuccinFrappe => "CatppuccinFrappe",
            iced::Theme::CatppuccinMacchiato => "CatppuccinMacchiato",
            iced::Theme::CatppuccinMocha => "CatppuccinMocha",
            iced::Theme::TokyoNight => "TokyoNight",
            iced::Theme::TokyoNightStorm => "TokyoNightStorm",
            iced::Theme::TokyoNightLight => "TokyoNightLight",
            iced::Theme::KanagawaWave => "KanagawaWave",
            iced::Theme::KanagawaDragon => "KanagawaDragon",
            iced::Theme::KanagawaLotus => "KanagawaLotus",
            iced::Theme::Moonfly => "Moonfly",
            iced::Theme::Nightfly => "Nightfly",
            iced::Theme::Oxocarbon => "Oxocarbon",
            iced::Theme::Custom(_) => "Dark",
            iced::Theme::Ferra => "Ferra",
        }
        .to_string();
    }
}

use i18n_embed::{
    DesktopLanguageRequester,
    fluent::{FluentLanguageLoader, fluent_language_loader},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "i18n"] // path to the compiled localization resources
struct Localizations;

use once_cell::sync::Lazy;
static LANGUAGE_LOADER: Lazy<FluentLanguageLoader> = Lazy::new(|| {
    let loader = fluent_language_loader!();
    let requested_languages = DesktopLanguageRequester::requested_languages();
    let _result = i18n_embed::select(&loader, &Localizations, &requested_languages);
    loader
});
