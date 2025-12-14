use icy_engine_gui::MonitorSettings;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, sync::Arc};

use super::{FKeySets, MostRecentlyUsedFiles};

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
}

impl Default for PersistedOptions {
    fn default() -> Self {
        Self {
            monitor_settings: MonitorSettings::default(),
            font_outline_style: 0,
            tag_render_mode: TagRenderMode::default(),
        }
    }
}

/// Shared options between all windows.
///
/// Persisted values are stored in `options.toml`.
/// Some values (MRU, F-keys) are stored separately (see their modules).
pub struct Options {
    /// Most recently used files
    pub recent_files: MostRecentlyUsedFiles,
    /// Moebius-style F-key character sets
    pub fkeys: FKeySets,

    /// Shared monitor/CRT settings (persisted)
    pub monitor_settings: Arc<RwLock<MonitorSettings>>,

    /// Shared outline style for drawing/TDFFont outlines (persisted)
    pub font_outline_style: Arc<RwLock<usize>>,

    /// Tag rendering mode (persisted)
    pub tag_render_mode: Arc<RwLock<TagRenderMode>>,
}

impl Options {
    pub const FILE_NAME: &'static str = "options.toml";

    pub fn load() -> Self {
        let persistent = Self::load_options_file();
        Self {
            recent_files: MostRecentlyUsedFiles::load(),
            fkeys: FKeySets::load(),
            monitor_settings: Arc::new(RwLock::new(persistent.monitor_settings)),
            font_outline_style: Arc::new(RwLock::new(persistent.font_outline_style)),
            tag_render_mode: Arc::new(RwLock::new(persistent.tag_render_mode)),
        }
    }

    pub fn store_persistent(&self) {
        let settings = PersistedOptions {
            monitor_settings: self.monitor_settings.read().clone(),
            font_outline_style: *self.font_outline_style.read(),
            tag_render_mode: *self.tag_render_mode.read(),
        };
        Self::store_options_file(&settings);
    }

    fn load_options_file() -> PersistedOptions {
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_draw") {
            if !proj_dirs.config_dir().exists() && fs::create_dir_all(proj_dirs.config_dir()).is_err() {
                log::error!("Can't create configuration directory {:?}", proj_dirs.config_dir());
                return PersistedOptions::default();
            }

            let options_file = proj_dirs.config_dir().join(Self::FILE_NAME);
            if options_file.exists() {
                match fs::read_to_string(options_file) {
                    Ok(txt) => {
                        if let Ok(result) = toml::from_str(&txt) {
                            return result;
                        }
                    }
                    Err(err) => log::error!("Error reading options file: {}", err),
                }
            }
        }

        PersistedOptions::default()
    }

    fn store_options_file(options: &PersistedOptions) {
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_draw") {
            let file_name = proj_dirs.config_dir().join(Self::FILE_NAME);
            match toml::to_string_pretty(options) {
                Ok(text) => {
                    if let Err(err) = fs::write(file_name, text) {
                        log::error!("Error writing options file: {}", err);
                    }
                }
                Err(err) => log::error!("Error serializing options file: {}", err),
            }
        }
    }

    pub fn config_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "GitHub", "icy_draw").map(|p| p.config_dir().to_path_buf())
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
