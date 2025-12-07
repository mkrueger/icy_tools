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
    toml_loader::load_commands_from_str(COMMON_COMMANDS_TOML).expect("Failed to parse embedded commands_common.toml")
}

/// Command definitions for common commands
///
/// Each command is a `LazyLock<CommandDef>` that lazily loads:
/// - Hotkeys from the embedded TOML
/// - Translations from the LANGUAGE_LOADER
pub mod cmd {
    use crate::define_commands;

    const TOML: &str = include_str!("../../data/commands_common.toml");

    define_commands! {
        loader: crate::LANGUAGE_LOADER,
        commands: TOML,

        // File
        FILE_NEW = "file.new",
        FILE_OPEN = "file.open",
        FILE_SAVE = "file.save",
        FILE_SAVE_AS = "file.save_as",
        FILE_EXPORT = "file.export",
        FILE_CLOSE = "file.close",

        // Edit
        EDIT_UNDO = "edit.undo",
        EDIT_REDO = "edit.redo",
        EDIT_CUT = "edit.cut",
        EDIT_COPY = "edit.copy",
        EDIT_PASTE = "edit.paste",
        EDIT_DELETE = "edit.delete",
        EDIT_SELECT_ALL = "edit.select_all",

        // View
        VIEW_ZOOM_IN = "view.zoom_in",
        VIEW_ZOOM_OUT = "view.zoom_out",
        VIEW_ZOOM_RESET = "view.zoom_reset",
        VIEW_ZOOM_FIT = "view.zoom_fit",
        VIEW_FULLSCREEN = "view.fullscreen",

        // Window
        WINDOW_NEW = "window.new",
        WINDOW_CLOSE = "window.close",

        // Focus Navigation
        FOCUS_NEXT = "focus.next",
        FOCUS_PREVIOUS = "focus.previous",

        // Navigation
        NAV_BACK = "nav.back",
        NAV_FORWARD = "nav.forward",
        NAV_UP = "nav.up",

        // Help
        HELP_SHOW = "help.show",
        HELP_ABOUT = "help.about",

        // Settings
        SETTINGS_OPEN = "settings.open",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{KeyCode, Modifiers, macros::CommandId};

    #[test]
    fn test_common_commands_created() {
        let set = create_common_commands();

        // Should have all the common commands
        assert!(set.get(cmd::FILE_OPEN.command_id()).is_some());
        assert!(set.get(cmd::EDIT_COPY.command_id()).is_some());
        assert!(set.get(cmd::VIEW_ZOOM_IN.command_id()).is_some());
        assert!(set.get(cmd::HELP_SHOW.command_id()).is_some());
    }

    #[test]
    fn test_zoom_shortcuts() {
        let set = create_common_commands();

        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(set.match_key(KeyCode::Plus, Modifiers::CTRL), Some(cmd::VIEW_ZOOM_IN.command_id()));
            assert_eq!(set.match_key(KeyCode::Equals, Modifiers::CTRL), Some(cmd::VIEW_ZOOM_IN.command_id()));
            assert_eq!(set.match_key(KeyCode::Minus, Modifiers::CTRL), Some(cmd::VIEW_ZOOM_OUT.command_id()));
            assert_eq!(set.match_key(KeyCode::Num0, Modifiers::CTRL), Some(cmd::VIEW_ZOOM_RESET.command_id()));
        }
    }

    #[test]
    fn test_copy_paste_shortcuts() {
        let set = create_common_commands();

        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(set.match_key(KeyCode::C, Modifiers::CTRL), Some(cmd::EDIT_COPY.command_id()));
            assert_eq!(set.match_key(KeyCode::V, Modifiers::CTRL), Some(cmd::EDIT_PASTE.command_id()));
        }
    }

    #[test]
    fn test_f1_help() {
        let set = create_common_commands();

        // F1 should work on all platforms
        assert_eq!(set.match_key(KeyCode::F1, Modifiers::NONE), Some(cmd::HELP_SHOW.command_id()));
    }

    #[test]
    fn test_fullscreen_shortcuts() {
        let set = create_common_commands();

        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(set.match_key(KeyCode::F11, Modifiers::NONE), Some(cmd::VIEW_FULLSCREEN.command_id()));
            assert_eq!(set.match_key(KeyCode::Enter, Modifiers::ALT), Some(cmd::VIEW_FULLSCREEN.command_id()));
        }
    }

    #[test]
    fn test_hotkey_display() {
        let set = create_common_commands();

        let copy_cmd = set.get(cmd::EDIT_COPY.command_id()).unwrap();

        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(copy_cmd.primary_hotkey_display(), Some("Ctrl+C".to_string()));
        }
    }

    #[test]
    fn test_cmd_static_hotkey_display() {
        // Test that cmd:: statics directly have hotkeys (not via CommandSet lookup)
        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(
                cmd::EDIT_COPY.primary_hotkey_display(),
                Some("Ctrl+C".to_string()),
                "EDIT_COPY should have Ctrl+C"
            );
            assert_eq!(
                cmd::VIEW_ZOOM_IN.primary_hotkey_display(),
                Some("Ctrl++".to_string()),
                "VIEW_ZOOM_IN should have Ctrl++"
            );
            assert_eq!(cmd::HELP_SHOW.primary_hotkey_display(), Some("F1".to_string()), "HELP_SHOW should have F1");
        }
    }
}
