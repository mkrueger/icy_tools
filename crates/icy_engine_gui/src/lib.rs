pub mod terminal;
use icy_engine::Color;
pub use terminal::*;

pub mod commands;
pub use commands::{
    CommandDef, CommandSet, Hotkey, KeyCode, Modifiers, 
    cmd, create_common_commands, 
    load_commands_from_str, load_commands_from_file, CommandLoadError
};

pub mod render_info;
pub use render_info::*;

pub mod clipboard;
pub use clipboard::*;

pub mod viewport;
pub use viewport::*;

pub mod scrollbar_state;
pub use scrollbar_state::*;

pub mod scrollbar_overlay;
pub use scrollbar_overlay::*;

pub mod horizontal_scrollbar_overlay;
pub use horizontal_scrollbar_overlay::*;

pub mod scrollbar_info;
pub use scrollbar_info::*;

pub mod terminal_shader;
pub use terminal_shader::*;

pub mod terminal_view;
pub use terminal_view::*;

// Re-export mouse event types from icy_engine
pub use icy_engine::{KeyModifiers, MouseButton, MouseEvent, MouseEventType};

// Re-export ScrollDelta for ZoomMessage
pub use iced::mouse::ScrollDelta;

pub mod key_map;
pub mod settings;

pub mod blink;
pub use blink::*;

pub mod theme;
pub use theme::*;

pub mod render_unicode;
pub use render_unicode::*;

pub mod unicode_glyph_cache;
pub use unicode_glyph_cache::*;

pub mod crt_shader_state;
pub use crt_shader_state::*;

pub mod crt_shader_program;
pub use crt_shader_program::*;

pub mod ui;
pub use ui::*;

pub mod util;

pub mod music;

//pub mod terminal_shader_widget;

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

// ============================================================================
// Default auto-scaling behavior - set once at application startup
// ============================================================================
// 0 = Auto (fit both dimensions) - good for terminals with various screen modes
// 1 = AutoScaleX (fit width only) - good for viewers with long scrollable content
static DEFAULT_AUTO_SCALE_XY: AtomicBool = AtomicBool::new(false);

/// Set the default auto-scaling mode for this application.
pub fn set_default_auto_scaling_xy(scale_xy: bool) {
    DEFAULT_AUTO_SCALE_XY.store(scale_xy, Ordering::Relaxed);
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

/// Scaling mode for terminal/viewer content
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum ScalingMode {
    /// Automatically scale to fit the available space
    /// With integer_scaling: uses largest integer factor that fits
    /// Without integer_scaling: uses exact fit factor
    #[default]
    Auto,

    /// Manual zoom level (1.0 = 100%, 2.0 = 200%, etc.)
    /// With integer_scaling: rounds to nearest integer
    Manual(f32),
}

impl ScalingMode {
    /// Minimum zoom level (50%)
    pub const MIN_ZOOM: f32 = 0.5;
    /// Maximum zoom level (400%)
    pub const MAX_ZOOM: f32 = 4.0;
    /// Zoom step for each zoom in/out action (25%)
    pub const ZOOM_STEP: f32 = 0.25;
    /// Zoom step for integer scaling
    pub const ZOOM_STEP_INT: f32 = 1.0;

    /// Clamp a zoom value to valid range
    pub fn clamp_zoom(zoom: f32) -> f32 {
        zoom.clamp(Self::MIN_ZOOM, Self::MAX_ZOOM)
    }

    /// Calculate the next zoom level when zooming in
    pub fn zoom_in(current: f32, use_integer: bool) -> f32 {
        let step = if use_integer { Self::ZOOM_STEP_INT } else { Self::ZOOM_STEP };
        let new_zoom = if use_integer { (current + step).floor() } else { current + step };
        Self::clamp_zoom(new_zoom)
    }

    /// Calculate the next zoom level when zooming out
    pub fn zoom_out(current: f32, use_integer: bool) -> f32 {
        let step = if use_integer { Self::ZOOM_STEP_INT } else { Self::ZOOM_STEP };
        let new_zoom = if use_integer { (current - step).ceil().max(1.0) } else { current - step };
        Self::clamp_zoom(new_zoom)
    }

    /// Get the effective zoom factor for given content and viewport sizes
    /// Returns the zoom factor to use for rendering
    pub fn compute_zoom(&self, content_width: f32, content_height: f32, viewport_width: f32, viewport_height: f32, use_integer_scaling: bool) -> f32 {
        match self {
            ScalingMode::Auto => {
                // Calculate the scale that fits content in viewport
                let scale_x = viewport_width / content_width;

                let scale_y = viewport_height / content_height;
                let fit_scale = if DEFAULT_AUTO_SCALE_XY.load(Ordering::Relaxed) {
                    scale_x.min(scale_y).max(0.1) // Use smaller to fit both dimensions
                } else {
                    scale_x
                };

                if use_integer_scaling {
                    // Use largest integer that still fits
                    fit_scale.floor().max(1.0)
                } else {
                    fit_scale
                }
            }

            ScalingMode::Manual(zoom) => {
                if use_integer_scaling {
                    zoom.round().max(1.0)
                } else {
                    *zoom
                }
            }
        }
    }

    /// Check if in auto mode
    pub fn is_auto(&self) -> bool {
        matches!(self, ScalingMode::Auto)
    }

    /// Get manual zoom value, or 1.0 if auto
    pub fn get_manual_zoom(&self) -> f32 {
        match self {
            ScalingMode::Auto => 1.0,
            ScalingMode::Manual(z) => *z,
        }
    }

    /// Format zoom info for display in window title
    /// Returns "[AUTO]" for auto mode or "[N%]" for manual mode with clamped value
    pub fn format_zoom_string(&self) -> String {
        match self {
            ScalingMode::Auto => "[AUTO]".to_string(),
            ScalingMode::Manual(zoom) => {
                let clamped = Self::clamp_zoom(*zoom);
                format!("[{:.0}%]", clamped * 100.0)
            }
        }
    }

    /// Apply a zoom message and return the new scaling mode
    /// This is the central zoom handling logic for all applications
    pub fn apply_zoom(&self, msg: ZoomMessage, current_zoom: f32, use_integer_scaling: bool) -> ScalingMode {
        match msg {
            ZoomMessage::In => {
                let new_zoom = Self::zoom_in(current_zoom, use_integer_scaling);
                ScalingMode::Manual(new_zoom)
            }
            ZoomMessage::Out => {
                let new_zoom = Self::zoom_out(current_zoom, use_integer_scaling);
                ScalingMode::Manual(new_zoom)
            }
            ZoomMessage::Reset => ScalingMode::Manual(1.0),
            ZoomMessage::AutoFit => ScalingMode::Auto,
            ZoomMessage::Set(zoom) => ScalingMode::Manual(Self::clamp_zoom(zoom)),
            ZoomMessage::Wheel(delta) => {
                // Extract y-axis delta and determine zoom behavior
                let (y_delta, is_smooth) = match delta {
                    ScrollDelta::Lines { y, .. } => {
                        // Discrete scroll wheel - use sign for step-based zoom
                        let sign = if y > 0.0 {
                            1.0
                        } else if y < 0.0 {
                            -1.0
                        } else {
                            0.0
                        };
                        (sign, false)
                    }
                    ScrollDelta::Pixels { y, .. } => {
                        // Pixel-based scroll (macOS trackpad) - smooth zooming
                        (y / 200.0, true)
                    }
                };

                if y_delta == 0.0 {
                    return *self; // No change
                }

                let new_zoom = if is_smooth {
                    // Smooth scroll - apply delta directly
                    Self::clamp_zoom(current_zoom + y_delta)
                } else {
                    // Discrete scroll wheel - use step-based zoom
                    if y_delta > 0.0 {
                        Self::zoom_in(current_zoom, use_integer_scaling)
                    } else {
                        Self::zoom_out(current_zoom, use_integer_scaling)
                    }
                };
                ScalingMode::Manual(new_zoom)
            }
        }
    }
}

/// Unified zoom message for all icy_tools applications
/// Used by ScalingMode::apply_zoom() for consistent zoom handling
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZoomMessage {
    /// Zoom in by one step (respects integer scaling if enabled)
    In,
    /// Zoom out by one step (respects integer scaling if enabled)
    Out,
    /// Reset zoom to 100% (1:1 pixel mapping)
    Reset,
    /// Auto-fit content to viewport
    AutoFit,
    /// Set specific zoom level (1.0 = 100%)
    Set(f32),
    /// Mouse wheel zoom (raw delta from Cmd/Ctrl+scroll)
    /// Positive delta = zoom in, negative = zoom out
    /// |delta| >= 1.0: discrete scroll wheel (use step-based zoom)
    /// |delta| < 1.0: smooth trackpad (apply delta directly)
    Wheel(ScrollDelta),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorSettings {
    pub theme: String,

    pub monitor_type: MonitorType,
    pub custom_monitor_color: Color,

    pub brightness: f32,
    pub contrast: f32,
    pub gamma: f32,
    pub saturation: f32,

    pub background_effect: BackgroundEffect,

    /// Use integer scaling (1x, 2x, 3x) for sharp bitmap fonts
    #[serde(alias = "use_pixel_perfect_scaling")]
    pub use_integer_scaling: bool,

    pub use_bilinear_filtering: bool,

    /// Scaling mode: Auto (fit-to-window) or Manual (user-defined zoom)
    #[serde(default)]
    pub scaling_mode: ScalingMode,

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

            // Color adjustments - neutral values
            brightness: 100.0, // 100% = 1.0 multiplier (neutral)
            contrast: 100.0,   // 100% = 1.0 multiplier (neutral)
            gamma: 1.0,
            saturation: 100.0, // 100% = 1.0 multiplier (full saturation)

            // Effects
            background_effect: BackgroundEffect::None,

            // Scaling - auto-fit with integer scaling for sharp fonts
            use_integer_scaling: true,
            use_bilinear_filtering: false,
            scaling_mode: ScalingMode::Auto,

            // CRT effects - all disabled for neutral
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
