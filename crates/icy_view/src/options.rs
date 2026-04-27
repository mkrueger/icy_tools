use crate::sort_order::SortOrder;
use icy_engine_gui::MonitorSettings;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, path::PathBuf, sync::OnceLock};

use futures::executor::block_on;

/// Global config directory, resolved once at startup.
static CONFIG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Resolve the configuration directory based on CLI args and auto-detection.
///
/// Priority:
/// 1. `--config-dir <PATH>` (explicit override)
/// 2. `--portable` flag → directory containing the executable
/// 3. Auto-detect: if `options.toml` exists next to the executable → use that directory
/// 4. Platform default via `directories::ProjectDirs`
pub fn init_config_dir(portable: bool, config_dir: Option<PathBuf>) {
    let dir = resolve_config_dir(portable, config_dir);
    if let Err(_existing) = CONFIG_DIR.set(dir) {
        log::warn!("Config directory was already initialized");
    }
}

/// Returns the active configuration directory.
pub fn get_config_dir() -> &'static Path {
    CONFIG_DIR.get().expect("CONFIG_DIR not initialized; call init_config_dir() first")
}

fn resolve_config_dir(portable: bool, config_dir: Option<PathBuf>) -> PathBuf {
    // 1. Explicit --config-dir
    if let Some(dir) = config_dir {
        if let Err(err) = fs::create_dir_all(&dir) {
            log::error!("Can't create custom config directory {:?}: {}", dir, err);
        }
        return dir;
    }

    // 2. --portable flag
    if portable {
        if let Some(dir) = exe_dir() {
            if let Err(err) = fs::create_dir_all(&dir) {
                log::error!("Can't create portable config directory {:?}: {}", dir, err);
            }
            return dir;
        }
    }

    // 3. Auto-detect: options.toml next to executable
    if let Some(dir) = exe_dir() {
        if dir.join("options.toml").exists() {
            return dir;
        }
    }

    // 4. Platform default
    if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_view") {
        let dir = proj_dirs.config_dir().to_path_buf();
        if let Err(err) = fs::create_dir_all(&dir) {
            log::error!("Can't create config directory {:?}: {}", dir, err);
        }
        return dir;
    }

    // Last resort: current directory
    PathBuf::from(".")
}

/// Returns the directory containing the current executable, if available.
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.canonicalize().ok()?.parent().map(|p| p.to_path_buf())
}

const SCROLL_SPEED: [f32; 3] = [80.0, 160.0, 320.0];

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

        // Parse the command before replacing `%F` so an unquoted placeholder
        // still expands to a single argument when the file path contains spaces.
        let parts = split_command_line(&self.command)?;
        if parts.is_empty() {
            return None;
        }
        let program = parts[0].replace("%F", &file_str);
        let args: Vec<String> = parts[1..].iter().map(|s| s.replace("%F", &file_str)).collect();

        // If no %F placeholder was in original command, append file as last argument
        if !self.command.contains("%F") {
            let mut args = args;
            args.push(file_str.to_string());
            return Some((program, args));
        }
        Some((program, args))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CommandQuote {
    Single,
    Double,
}

fn split_command_line(command: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut token_started = false;
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        match (ch, quote) {
            ('\'', None) => {
                quote = Some(CommandQuote::Single);
                token_started = true;
            }
            ('\'', Some(CommandQuote::Single)) => {
                quote = None;
            }
            ('"', None) => {
                quote = Some(CommandQuote::Double);
                token_started = true;
            }
            ('"', Some(CommandQuote::Double)) => {
                quote = None;
            }
            (ch, None) if ch.is_whitespace() => {
                if token_started {
                    parts.push(std::mem::take(&mut current));
                    token_started = false;
                }
            }
            ('\\', Some(CommandQuote::Single)) => {
                current.push('\\');
                token_started = true;
            }
            ('\\', _) => {
                if let Some(next) = chars.peek().copied() {
                    if should_unescape(next) {
                        current.push(chars.next()?);
                    } else {
                        current.push('\\');
                    }
                } else {
                    current.push('\\');
                }
                token_started = true;
            }
            _ => {
                current.push(ch);
                token_started = true;
            }
        }
    }

    if quote.is_some() {
        return None;
    }
    if token_started {
        parts.push(current);
    }

    Some(parts)
}

fn should_unescape(ch: char) -> bool {
    ch == '\\' || ch == '\'' || ch == '"' || ch.is_whitespace()
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

#[cfg(test)]
mod tests {
    use super::ExternalCommand;
    use std::path::Path;

    #[test]
    fn external_command_keeps_unquoted_placeholder_path_as_single_arg() {
        let command = ExternalCommand {
            command: "viewer --open %F".to_string(),
        };

        let (program, args) = command.build_command(Path::new("/tmp/art files/demo file.ans")).unwrap();

        assert_eq!(program, "viewer");
        assert_eq!(args, vec!["--open", "/tmp/art files/demo file.ans"]);
    }

    #[test]
    fn external_command_parses_quoted_program_and_arguments() {
        let command = ExternalCommand {
            command: "'my viewer' --title \"ANSI Art\" '--file=%F'".to_string(),
        };

        let (program, args) = command.build_command(Path::new("/tmp/art files/demo file.ans")).unwrap();

        assert_eq!(program, "my viewer");
        assert_eq!(args, vec!["--title", "ANSI Art", "--file=/tmp/art files/demo file.ans"]);
    }

    #[test]
    fn external_command_appends_file_when_placeholder_is_missing() {
        let command = ExternalCommand {
            command: "viewer --fullscreen".to_string(),
        };

        let (program, args) = command.build_command(Path::new("/tmp/art files/demo file.ans")).unwrap();

        assert_eq!(program, "viewer");
        assert_eq!(args, vec!["--fullscreen", "/tmp/art files/demo file.ans"]);
    }

    #[test]
    fn external_command_rejects_unclosed_quotes() {
        let command = ExternalCommand {
            command: "viewer \"unterminated".to_string(),
        };

        assert!(command.build_command(Path::new("/tmp/file.ans")).is_none());
    }
}

impl Options {
    pub fn load_options() -> Self {
        let config_dir = get_config_dir();
        let options_file = config_dir.join("options.toml");
        if options_file.exists() {
            match fs::read_to_string(&options_file) {
                Ok(txt) => {
                    if let Ok(result) = toml::from_str(&txt) {
                        return result;
                    }
                }
                Err(err) => log::error!("Error reading options file: {}", err),
            }
        }
        Self::default()
    }

    pub fn store_options(&self) {
        let config_dir = get_config_dir();
        let file_name = config_dir.join("options.toml");
        match toml::to_string(self) {
            Ok(text) => {
                if let Err(err) = fs::write(file_name, text) {
                    log::error!("Error writing options file: {}", err);
                }
            }
            Err(err) => log::error!("Error writing options file: {}", err),
        }
    }

    /// Returns the log directory path
    pub fn get_log_dir() -> PathBuf {
        get_config_dir().to_path_buf()
    }

    /// Returns the path to the current log file.
    pub fn get_log_file() -> PathBuf {
        let log_dir = Self::get_log_dir();
        if cfg!(windows) {
            log_dir.join("icy_view_rCURRENT.log")
        } else {
            log_dir.join("icy_view.log")
        }
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
    pub fn prepare_file_for_external(item: &dyn crate::items::Item) -> Option<PathBuf> {
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
