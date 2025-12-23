use icy_engine_gui::MonitorSettings;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf};

use crate::{ui::FKeySets, MostRecentlyUsedFiles};

// =============================================================================
// Project directory constants
// =============================================================================

const PROJECT_QUALIFIER: &str = "com";
const PROJECT_ORGANIZATION: &str = "GitHub";
const PROJECT_APPLICATION: &str = "icy_draw";

/// Lazily initialized project directories (computed once on first access)
pub(crate) static PROJECT_DIRS: Lazy<Option<directories::ProjectDirs>> =
    Lazy::new(|| directories::ProjectDirs::from(PROJECT_QUALIFIER, PROJECT_ORGANIZATION, PROJECT_APPLICATION));

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedSettings {
    #[serde(default)]
    pub monitor_settings: MonitorSettings,

    #[serde(default)]
    pub font_outline_style: usize,

    #[serde(default = "default_true")]
    pub show_layer_borders: bool,

    #[serde(default)]
    pub show_line_numbers: bool,

    #[serde(default)]
    pub collaboration: CollaborationSettings,

    /// Selected tag replacement list name (without extension)
    #[serde(default)]
    pub selected_taglist: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationSettings {
    /// Collaboration nickname
    #[serde(default = "default_nick")]
    pub nick: String,

    /// Collaboration group (like Moebius)
    #[serde(default)]
    pub group: String,

    /// Most recently used collaboration servers (last = most recent)
    #[serde(default = "default_servers")]
    pub servers: Vec<String>,
}

impl Default for CollaborationSettings {
    fn default() -> Self {
        Self {
            nick: default_nick(),
            group: String::new(),
            servers: default_servers(),
        }
    }
}

fn default_nick() -> String {
    "Anonymous".to_string()
}

fn default_servers() -> Vec<String> {
    vec!["localhost".to_string()]
}

fn default_true() -> bool {
    true
}

impl Default for PersistedSettings {
    fn default() -> Self {
        Self {
            monitor_settings: MonitorSettings::default(),
            font_outline_style: 0,
            show_layer_borders: true,
            show_line_numbers: false,
            collaboration: Default::default(),
            selected_taglist: String::new(),
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
    pub monitor_settings: MonitorSettings,

    /// Shared outline style for drawing/TDFFont outlines (persisted)
    pub font_outline_style: usize,

    /// Whether layer borders are shown (persisted, default: true)
    pub show_layer_borders: bool,

    /// Whether line numbers are shown (persisted, default: false)
    pub show_line_numbers: bool,

    /// Collaboration settings (persisted)
    pub collaboration: CollaborationSettings,

    /// Selected tag replacement list name (persisted)
    pub selected_taglist: String,
}

impl Settings {
    pub const FILE_NAME: &'static str = "settings.toml";

    pub fn load() -> Self {
        let persistent = Self::load_settings_file();
        Self {
            recent_files: MostRecentlyUsedFiles::load(),
            fkeys: FKeySets::load(),
            monitor_settings: persistent.monitor_settings,
            font_outline_style: persistent.font_outline_style,
            show_layer_borders: persistent.show_layer_borders,
            show_line_numbers: persistent.show_line_numbers,
            collaboration: persistent.collaboration,
            selected_taglist: persistent.selected_taglist,
        }
    }

    pub fn store_persistent(&self) {
        let settings = PersistedSettings {
            monitor_settings: self.monitor_settings.clone(),
            font_outline_style: self.font_outline_style,
            show_layer_borders: self.show_layer_borders,
            show_line_numbers: self.show_line_numbers,
            collaboration: self.collaboration.clone(),
            selected_taglist: self.selected_taglist.clone(),
        };
        Self::store_options_file(&settings);
    }

    fn load_settings_file() -> PersistedSettings {
        let Some(config_dir) = Self::config_dir() else {
            return PersistedSettings::default();
        };

        if !config_dir.exists() {
            if let Err(err) = fs::create_dir_all(&config_dir) {
                log::error!("Can't create configuration directory {:?}: {}", config_dir, err);
                return PersistedSettings::default();
            }
        }

        let options_file = config_dir.join(Self::FILE_NAME);
        if options_file.exists() {
            match fs::read_to_string(&options_file) {
                Ok(txt) => {
                    if let Ok(mut result) = toml::from_str::<PersistedSettings>(&txt) {
                        result.monitor_settings = normalize_monitor_settings(result.monitor_settings);
                        return result;
                    }
                }
                Err(err) => log::error!("Error reading options file: {}", err),
            }
        }

        PersistedSettings::default()
    }

    /// Atomically write settings to file (write to temp, then rename).
    /// This prevents data loss if the app crashes during write.
    fn store_options_file(options: &PersistedSettings) {
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

    /// Directory for Text-Art fonts (TDF/FIGlet).
    ///
    /// Migration behavior:
    /// - Prefer the new directory `data/text_art_fonts` if it exists.
    /// - Otherwise fall back to the legacy directory `data/fonts` if it exists.
    /// - If neither exists, return the new directory path.
    pub fn text_art_font_dir() -> Option<PathBuf> {
        let config_dir = Self::config_dir()?;
        let new_dir = config_dir.join("data/text_art_fonts");
        if new_dir.exists() {
            return Some(new_dir);
        }

        let legacy_dir = config_dir.join("data/fonts");
        if legacy_dir.exists() {
            return Some(legacy_dir);
        }

        Some(new_dir)
    }

    pub fn plugin_dir() -> Option<PathBuf> {
        Self::config_dir().map(|d| d.join("data/plugins"))
    }

    pub fn taglists_dir() -> Option<PathBuf> {
        let dir = Self::plugin_dir()?.join("taglists");

        if !dir.exists() {
            if let Err(err) = fs::create_dir_all(&dir) {
                log::error!("Can't create taglists directory {:?}: {}", dir, err);
            }
        }

        Some(dir)
    }

    /// Add a collaboration server to the MRU list
    pub fn add_collaboration_server(&mut self, url: &str) {
        const MAX_SERVERS: usize = 10;
        let url = url.trim().to_string();
        if url.is_empty() {
            return;
        }

        // Remove if exists (to move to end)
        self.collaboration.servers.retain(|s| s != &url);
        // Add to end (most recent)
        self.collaboration.servers.push(url);
        // Trim to max
        while self.collaboration.servers.len() > MAX_SERVERS {
            self.collaboration.servers.remove(0);
        }
    }

    /// Get the most recent collaboration server (last in list)
    pub fn last_collaboration_server(&self) -> Option<String> {
        self.collaboration.servers.last().cloned()
    }

    /// Get all collaboration servers (oldest first)
    pub fn collaboration_servers_list(&self) -> Vec<String> {
        self.collaboration.servers.clone()
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

    // icy_draw does not support scaling modes (Auto/Integer) - these are only for terminal viewers.
    // Force Manual scaling at 150% and disable integer scaling regardless of config file values.
    settings.scaling_mode = icy_engine_gui::ScalingMode::Manual(1.5);
    settings.use_integer_scaling = false;

    settings
}
