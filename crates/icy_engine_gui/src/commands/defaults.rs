//! Default commands shared between icy_view and icy_term
//!
//! These are common commands that both applications use.
//! Each app can extend this set with app-specific commands.
//!
//! Commands are loaded from `data/commands_common.toml`.

use super::{CommandSet, toml_loader};

/// The embedded default commands TOML
const COMMON_COMMANDS_TOML: &str = include_str!("../../data/commands_common.toml");

/// Create the default command set with commands shared across all icy_* apps
///
/// Loads commands from the embedded `commands_common.toml` file.
pub fn create_common_commands() -> CommandSet {
    toml_loader::load_commands_from_str(COMMON_COMMANDS_TOML)
        .expect("Failed to parse embedded commands_common.toml")
}

/// Command IDs for common commands (for type-safe access)
pub mod cmd {
    // File
    pub const FILE_OPEN: &str = "file.open";
    pub const FILE_EXPORT: &str = "file.export";
    pub const FILE_CLOSE: &str = "file.close";

    // Edit
    pub const EDIT_COPY: &str = "edit.copy";
    pub const EDIT_PASTE: &str = "edit.paste";
    pub const EDIT_SELECT_ALL: &str = "edit.select_all";

    // View
    pub const VIEW_ZOOM_IN: &str = "view.zoom_in";
    pub const VIEW_ZOOM_OUT: &str = "view.zoom_out";
    pub const VIEW_ZOOM_RESET: &str = "view.zoom_reset";
    pub const VIEW_ZOOM_FIT: &str = "view.zoom_fit";
    pub const VIEW_FULLSCREEN: &str = "view.fullscreen";

    // Window
    pub const WINDOW_NEW: &str = "window.new";
    pub const WINDOW_CLOSE: &str = "window.close";

    // Help
    pub const HELP_SHOW: &str = "help.show";
    pub const HELP_ABOUT: &str = "help.about";

    // Settings
    pub const SETTINGS_OPEN: &str = "settings.open";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{KeyCode, Modifiers};

    #[test]
    fn test_common_commands_created() {
        let set = create_common_commands();
        
        // Should have all the common commands
        assert!(set.get(cmd::FILE_OPEN).is_some());
        assert!(set.get(cmd::EDIT_COPY).is_some());
        assert!(set.get(cmd::VIEW_ZOOM_IN).is_some());
        assert!(set.get(cmd::HELP_SHOW).is_some());
    }

    #[test]
    fn test_zoom_shortcuts() {
        let set = create_common_commands();

        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(set.match_key(KeyCode::Plus, Modifiers::CTRL), Some(cmd::VIEW_ZOOM_IN));
            assert_eq!(set.match_key(KeyCode::Equals, Modifiers::CTRL), Some(cmd::VIEW_ZOOM_IN));
            assert_eq!(set.match_key(KeyCode::Minus, Modifiers::CTRL), Some(cmd::VIEW_ZOOM_OUT));
            assert_eq!(set.match_key(KeyCode::Num0, Modifiers::CTRL), Some(cmd::VIEW_ZOOM_RESET));
        }
    }

    #[test]
    fn test_copy_paste_shortcuts() {
        let set = create_common_commands();

        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(set.match_key(KeyCode::C, Modifiers::CTRL), Some(cmd::EDIT_COPY));
            assert_eq!(set.match_key(KeyCode::V, Modifiers::CTRL), Some(cmd::EDIT_PASTE));
        }
    }

    #[test]
    fn test_f1_help() {
        let set = create_common_commands();
        
        // F1 should work on all platforms
        assert_eq!(set.match_key(KeyCode::F1, Modifiers::NONE), Some(cmd::HELP_SHOW));
    }

    #[test]
    fn test_fullscreen_shortcuts() {
        let set = create_common_commands();

        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(set.match_key(KeyCode::F11, Modifiers::NONE), Some(cmd::VIEW_FULLSCREEN));
            assert_eq!(set.match_key(KeyCode::Enter, Modifiers::ALT), Some(cmd::VIEW_FULLSCREEN));
        }
    }

    #[test]
    fn test_hotkey_display() {
        let set = create_common_commands();
        
        let copy_cmd = set.get(cmd::EDIT_COPY).unwrap();
        
        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(copy_cmd.primary_hotkey_display(), Some("Ctrl+C".to_string()));
        }
    }
}
