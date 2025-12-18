use icy_engine_gui::MonitorSettings;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf, sync::Arc};

use crate::{MostRecentlyUsedFiles, ui::FKeySets};

// =============================================================================
// Project directory constants
// =============================================================================

const PROJECT_QUALIFIER: &str = "com";
const PROJECT_ORGANIZATION: &str = "GitHub";
const PROJECT_APPLICATION: &str = "icy_draw";

/// Lazily initialized project directories (computed once on first access)
pub(crate) static PROJECT_DIRS: Lazy<Option<directories::ProjectDirs>> =
    Lazy::new(|| directories::ProjectDirs::from(PROJECT_QUALIFIER, PROJECT_ORGANIZATION, PROJECT_APPLICATION));

// =============================================================================
// TagRenderMode
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagRenderMode {
    Buffer,
    Overlay,
}

impl Default for TagRenderMode {
    fn default() -> Self {
        Self::Buffer
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedOptions {
    #[serde(default)]
    pub monitor_settings: MonitorSettings,

    #[serde(default)]
    pub font_outline_style: usize,

    #[serde(default)]
    pub tag_render_mode: TagRenderMode,

    #[serde(default = "default_true")]
    pub show_layer_borders: bool,

    #[serde(default)]
    pub show_line_numbers: bool,
}

fn default_true() -> bool {
    true
}

impl Default for PersistedOptions {
    fn default() -> Self {
        Self {
            monitor_settings: MonitorSettings::default(),
            font_outline_style: 0,
            tag_render_mode: TagRenderMode::default(),
            show_layer_borders: true,
            show_line_numbers: false,
        }
    }
}

/// Shared options between all windows.
///
/// Persisted values are stored in `settings.toml`.
/// Some values (MRU, F-keys) are stored separately (see their modules).
pub struct Settings {
    /// Most recently used files
    pub recent_files: MostRecentlyUsedFiles,
    /// F-key character sets
    pub fkeys: FKeySets,

    /// Shared monitor/CRT settings (persisted)
    pub monitor_settings: Arc<RwLock<MonitorSettings>>,

    /// Shared outline style for drawing/TDFFont outlines (persisted)
    pub font_outline_style: Arc<RwLock<usize>>,

    /// Tag rendering mode (persisted)
    pub tag_render_mode: Arc<RwLock<TagRenderMode>>,

    /// Whether layer borders are shown (persisted, default: true)
    pub show_layer_borders: Arc<RwLock<bool>>,

    /// Whether line numbers are shown (persisted, default: false)
    pub show_line_numbers: Arc<RwLock<bool>>,
}

impl Settings {
    pub const FILE_NAME: &'static str = "settings.toml";

    pub fn load() -> Self {
        let persistent = Self::load_settings_file();
        Self {
            recent_files: MostRecentlyUsedFiles::load(),
            fkeys: FKeySets::load(),
            monitor_settings: Arc::new(RwLock::new(persistent.monitor_settings)),
            font_outline_style: Arc::new(RwLock::new(persistent.font_outline_style)),
            tag_render_mode: Arc::new(RwLock::new(persistent.tag_render_mode)),
            show_layer_borders: Arc::new(RwLock::new(persistent.show_layer_borders)),
            show_line_numbers: Arc::new(RwLock::new(persistent.show_line_numbers)),
        }
    }

    pub fn store_persistent(&self) {
        let settings = PersistedOptions {
            monitor_settings: self.monitor_settings.read().clone(),
            font_outline_style: *self.font_outline_style.read(),
            tag_render_mode: *self.tag_render_mode.read(),
            show_layer_borders: *self.show_layer_borders.read(),
            show_line_numbers: *self.show_line_numbers.read(),
        };
        Self::store_options_file(&settings);
    }

    fn load_settings_file() -> PersistedOptions {
        let Some(config_dir) = Self::config_dir() else {
            return PersistedOptions::default();
        };

        if !config_dir.exists() {
            if let Err(err) = fs::create_dir_all(&config_dir) {
                log::error!("Can't create configuration directory {:?}: {}", config_dir, err);
                return PersistedOptions::default();
            }
        }

        let options_file = config_dir.join(Self::FILE_NAME);
        if options_file.exists() {
            match fs::read_to_string(&options_file) {
                Ok(txt) => {
                    if let Ok(mut result) = toml::from_str::<PersistedOptions>(&txt) {
                        result.monitor_settings = normalize_monitor_settings(result.monitor_settings);
                        return result;
                    }
                }
                Err(err) => log::error!("Error reading options file: {}", err),
            }
        }

        PersistedOptions::default()
    }

    /// Atomically write settings to file (write to temp, then rename).
    /// This prevents data loss if the app crashes during write.
    fn store_options_file(options: &PersistedOptions) {
        let Some(config_dir) = Self::config_dir() else {
            log::error!("Cannot determine config directory for saving settings");
            return;
        };

        let file_path = config_dir.join(Self::FILE_NAME);
        let temp_path = config_dir.join(format!(".{}.tmp", Self::FILE_NAME));

        match toml::to_string_pretty(options) {
            Ok(text) => {
                // Write to temporary file first
                let write_result = (|| -> std::io::Result<()> {
                    let mut file = fs::File::create(&temp_path)?;
                    file.write_all(text.as_bytes())?;
                    file.sync_all()?; // Ensure data is flushed to disk
                    Ok(())
                })();

                if let Err(err) = write_result {
                    log::error!("Error writing temp settings file: {}", err);
                    let _ = fs::remove_file(&temp_path); // Clean up temp file
                    return;
                }

                // Atomically rename temp file to final destination
                if let Err(err) = fs::rename(&temp_path, &file_path) {
                    log::error!("Error renaming settings file: {}", err);
                    let _ = fs::remove_file(&temp_path); // Clean up temp file
                }
            }
            Err(err) => log::error!("Error serializing options: {}", err),
        }
    }

    pub fn config_dir() -> Option<PathBuf> {
        PROJECT_DIRS.as_ref().map(|p| p.config_dir().to_path_buf())
    }

    pub fn config_file() -> Option<PathBuf> {
        Self::config_dir().map(|d| d.join(Self::FILE_NAME))
    }

    pub fn log_file() -> Option<PathBuf> {
        Self::config_dir().map(|d| {
            if cfg!(windows) {
                d.join("icy_draw_rCURRENT.log")
            } else {
                d.join("icy_draw.log")
            }
        })
    }

    pub fn font_dir() -> Option<PathBuf> {
        Self::config_dir().map(|d| d.join("data/fonts"))
    }

    pub fn plugin_dir() -> Option<PathBuf> {
        Self::config_dir().map(|d| d.join("data/plugins"))
    }
}

fn normalize_monitor_settings(mut settings: MonitorSettings) -> MonitorSettings {
    // Migration: older versions used 0.0..=2.0 (neutral=1.0) for brightness/contrast/saturation.
    // Current shader expects 0.0..=200.0 (neutral=100.0) and divides by 100.
    let looks_like_legacy_scale = settings.brightness <= 4.0 && settings.contrast <= 4.0 && settings.saturation <= 4.0;
    if looks_like_legacy_scale {
        settings.brightness *= 100.0;
        settings.contrast *= 100.0;
        settings.saturation *= 100.0;
    }

    // Keep values in a sane range even if the config is corrupted.
    settings.brightness = settings.brightness.clamp(0.0, 200.0);
    settings.contrast = settings.contrast.clamp(0.0, 200.0);
    settings.saturation = settings.saturation.clamp(0.0, 200.0);
    settings.gamma = settings.gamma.clamp(0.0, 4.0);

    settings
}
