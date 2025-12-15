use icy_engine_gui::MonitorSettings;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use futures::executor::block_on;

const SCROLL_SPEED: [f32; 3] = [80.0, 160.0, 320.0];

/// Sort order for file listing
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub enum SortOrder {
    /// Sort by name (A-Z)
    #[default]
    NameAsc,
    /// Sort by name (Z-A)
    NameDesc,
    /// Sort by size (smallest first)
    SizeAsc,
    /// Sort by size (largest first)
    SizeDesc,
    /// Sort by date (oldest first)
    DateAsc,
    /// Sort by date (newest first)
    DateDesc,
}

impl SortOrder {
    /// Cycle to the next sort order
    pub fn next(&self) -> SortOrder {
        match self {
            SortOrder::NameAsc => SortOrder::NameDesc,
            SortOrder::NameDesc => SortOrder::SizeAsc,
            SortOrder::SizeAsc => SortOrder::SizeDesc,
            SortOrder::SizeDesc => SortOrder::DateAsc,
            SortOrder::DateAsc => SortOrder::DateDesc,
            SortOrder::DateDesc => SortOrder::NameAsc,
        }
    }

    /// Get the icon for this sort order
    pub fn icon(&self) -> &'static str {
        match self {
            SortOrder::NameAsc => "A↓",
            SortOrder::NameDesc => "A↑",
            SortOrder::SizeAsc => "S↓",
            SortOrder::SizeDesc => "S↑",
            SortOrder::DateAsc => "D↓",
            SortOrder::DateDesc => "D↑",
        }
    }

    /// Get the tooltip for this sort order
    pub fn tooltip_key(&self) -> &'static str {
        match self {
            SortOrder::NameAsc => "tooltip-sort-name-asc",
            SortOrder::NameDesc => "tooltip-sort-name-desc",
            SortOrder::SizeAsc => "tooltip-sort-size-asc",
            SortOrder::SizeDesc => "tooltip-sort-size-desc",
            SortOrder::DateAsc => "tooltip-sort-date-asc",
            SortOrder::DateDesc => "tooltip-sort-date-desc",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub enum ViewMode {
    /// List view with file browser on left, preview on right
    #[default]
    List,
    /// Tile/grid view showing thumbnails
    Tiles,
}

impl ViewMode {
    /// Toggle between list and tile view
    pub fn toggle(&self) -> ViewMode {
        match self {
            ViewMode::List => ViewMode::Tiles,
            ViewMode::Tiles => ViewMode::List,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum ScrollSpeed {
    Slow,
    Medium,
    Fast,
}

impl ScrollSpeed {
    pub fn get_speed(&self) -> f32 {
        match self {
            ScrollSpeed::Slow => SCROLL_SPEED[0],
            ScrollSpeed::Medium => SCROLL_SPEED[1],
            ScrollSpeed::Fast => SCROLL_SPEED[2],
        }
    }

    pub(crate) fn _next(&self) -> ScrollSpeed {
        match self {
            ScrollSpeed::Slow => ScrollSpeed::Medium,
            ScrollSpeed::Medium => ScrollSpeed::Fast,
            ScrollSpeed::Fast => ScrollSpeed::Slow,
        }
    }
}

/// Number of external command slots (F5-F8)
pub const EXTERNAL_COMMAND_COUNT: usize = 4;

/// An external command that can be executed on a file
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct ExternalCommand {
    /// Command to execute. Use %F as placeholder for the file path
    pub command: String,
}

impl ExternalCommand {
    pub fn is_configured(&self) -> bool {
        !self.command.is_empty()
    }

    pub fn build_command(&self, file_path: &std::path::Path) -> Option<(String, Vec<String>)> {
        if self.command.is_empty() {
            return None;
        }
        let file_str = file_path.to_string_lossy();

        // Replace %F placeholder in the entire command first
        let expanded_command = self.command.replace("%F", &file_str);

        let parts: Vec<&str> = expanded_command.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }
        let program = parts[0].to_string();
        let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();

        // If no %F placeholder was in original command, append file as last argument
        if !self.command.contains("%F") {
            let mut args = args;
            args.push(file_str.to_string());
            return Some((program, args));
        }
        Some((program, args))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Options {
    pub auto_scroll_enabled: bool,
    pub scroll_speed: ScrollSpeed,
    pub show_settings: bool,
    #[serde(default)]
    pub view_mode: ViewMode,
    #[serde(default)]
    pub sort_order: SortOrder,
    #[serde(default)]
    pub sauce_mode: bool,
    #[serde(default)]
    pub monitor_settings: MonitorSettings,

    #[serde(default)]
    pub external_commands: [ExternalCommand; EXTERNAL_COMMAND_COUNT],

    /// Default path for file export
    #[serde(default)]
    pub export_path: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            show_settings: true,
            auto_scroll_enabled: true,
            scroll_speed: ScrollSpeed::Medium,
            view_mode: ViewMode::List,
            sort_order: SortOrder::default(),
            sauce_mode: false,
            monitor_settings: MonitorSettings::default(),
            external_commands: Default::default(),
            export_path: String::new(),
        }
    }
}

impl Options {
    pub fn load_options() -> Self {
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_view") {
            if !proj_dirs.config_dir().exists() && fs::create_dir_all(proj_dirs.config_dir()).is_err() {
                log::error!("Can't create configuration directory {:?}", proj_dirs.config_dir());
                return Self::default();
            }
            let options_file = proj_dirs.config_dir().join("options.toml");
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
        Self::default()
    }

    pub fn store_options(&self) {
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_view") {
            let file_name = proj_dirs.config_dir().join("options.toml");
            match toml::to_string(self) {
                Ok(text) => {
                    if let Err(err) = fs::write(file_name, text) {
                        log::error!("Error writing options file: {}", err);
                    }
                }
                Err(err) => log::error!("Error writing options file: {}", err),
            }
        }
    }

    /// Returns the configuration directory path
    pub fn get_config_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "GitHub", "icy_view").map(|proj_dirs| proj_dirs.config_dir().to_path_buf())
    }

    /// Returns the log directory path
    pub fn get_log_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "GitHub", "icy_view").map(|proj_dirs| proj_dirs.config_dir().to_path_buf())
    }

    /// Returns the path to the current log file.
    pub fn get_log_file() -> Option<PathBuf> {
        Self::get_log_dir().map(|log_dir| {
            if cfg!(windows) {
                log_dir.join("icy_view_rCURRENT.log")
            } else {
                log_dir.join("icy_view.log")
            }
        })
    }

    /// Reset monitor settings to defaults
    pub fn reset_monitor_settings(&mut self) {
        self.monitor_settings = MonitorSettings::default();
    }

    /// Returns the export path, falling back to default if not set
    pub fn export_path(&self) -> PathBuf {
        if self.export_path.is_empty() {
            Self::default_export_directory()
        } else {
            PathBuf::from(&self.export_path)
        }
    }

    /// Returns the default export directory (user's documents folder)
    pub fn default_export_directory() -> PathBuf {
        if let Some(user_dirs) = directories::UserDirs::new() {
            if let Some(doc_dir) = user_dirs.document_dir() {
                return doc_dir.to_path_buf();
            }
        }
        PathBuf::from(".")
    }

    /// Prepare a file for external command execution.
    /// For local files, returns the path directly.
    /// For virtual files (archives, web), copies to temp directory and returns temp path.
    pub fn prepare_file_for_external(item: &dyn crate::Item) -> Option<PathBuf> {
        // Check if it's a local file with a real path
        if let Some(full_path) = item.get_full_path() {
            let path = PathBuf::from(full_path);
            if path.exists() {
                return Some(path);
            }
        }

        // Virtual file - need to copy to temp directory
        // For virtual files we need to clone the item to read data
        let item_clone = item.clone_box();
        // Use block_on for this synchronous context - this is acceptable as external
        // command execution already involves blocking operations
        let data = block_on(item_clone.read_data()).ok()?;
        let file_name = PathBuf::from(&item.get_file_path()).file_name()?.to_string_lossy().to_string();

        // Create session-specific temp directory
        let temp_dir = std::env::temp_dir().join(format!("icy_view_{}", std::process::id()));
        if !temp_dir.exists() {
            std::fs::create_dir_all(&temp_dir).ok()?;
        }

        let temp_path = temp_dir.join(&file_name);
        std::fs::write(&temp_path, &data).ok()?;

        Some(temp_path)
    }

    /// Clean up session temp directory on exit
    pub fn cleanup_session_temp() {
        let temp_dir = std::env::temp_dir().join(format!("icy_view_{}", std::process::id()));
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
    }
}
