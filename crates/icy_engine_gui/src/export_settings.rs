use icy_engine::{formats::SixelSettings, AnsiCompatibilityLevel, ScreenPreperation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSettings {
    #[serde(default)]
    pub export_format_ext: Option<String>,
    #[serde(default)]
    pub ansi_level: AnsiCompatibilityLevel,
    #[serde(default)]
    pub ansi_rgb_output: bool,
    #[serde(default)]
    pub screen_prep: ScreenPreperation,
    #[serde(default)]
    pub max_line_length_enabled: bool,
    #[serde(default = "default_max_line_length")]
    pub max_line_length: u16,
    #[serde(default)]
    pub utf8_output: bool,
    #[serde(default)]
    pub compress: bool,
    #[serde(default)]
    pub sixel_settings: SixelSettings,
}

fn default_max_line_length() -> u16 {
    80
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            export_format_ext: None,
            ansi_level: AnsiCompatibilityLevel::default(),
            ansi_rgb_output: false,
            screen_prep: ScreenPreperation::None,
            max_line_length_enabled: false,
            max_line_length: 80,
            utf8_output: false,
            compress: false,
            sixel_settings: SixelSettings::default(),
        }
    }
}
