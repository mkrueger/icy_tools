//! icy_term specific commands
//!
//! These commands are specific to icy_term and are merged with
//! the common commands from icy_engine_gui.

use icy_engine_gui::commands::{CommandSet, load_commands_from_str};

/// The embedded icy_term specific commands TOML
const ICY_TERM_COMMANDS_TOML: &str = include_str!("../data/commands_icy_term.toml");

/// Create the icy_term command set by merging common commands with icy_term specific commands
pub fn create_icy_term_commands() -> CommandSet {
    let mut commands = icy_engine_gui::commands::create_common_commands();

    let icy_term_commands = load_commands_from_str(ICY_TERM_COMMANDS_TOML).expect("Failed to parse embedded commands_icy_term.toml");

    commands.merge(icy_term_commands);
    commands
}

/// Command definitions for icy_term
///
/// Each command is a `LazyLock<CommandDef>` that lazily loads:
/// - Hotkeys from the embedded TOML
/// - Translations from the LANGUAGE_LOADER
pub mod cmd {
    use icy_engine_gui::define_commands;

    // Re-export common commands from icy_engine_gui
    pub use icy_engine_gui::commands::cmd::*;

    const TOML: &str = include_str!("../data/commands_icy_term.toml");

    define_commands! {
        loader: crate::LANGUAGE_LOADER,
        commands: TOML,

        // Connection
        CONNECTION_DIALING_DIRECTORY = "connection.dialing_directory",
        CONNECTION_HANGUP = "connection.hangup",
        CONNECTION_SERIAL = "connection.serial",

        // File Transfer
        TRANSFER_UPLOAD = "transfer.upload",
        TRANSFER_DOWNLOAD = "transfer.download",

        // Login
        LOGIN_SEND_ALL = "login.send_all",
        LOGIN_SEND_USER = "login.send_user",
        LOGIN_SEND_PASSWORD = "login.send_password",

        // Terminal
        TERMINAL_CLEAR = "terminal.clear",
        TERMINAL_SCROLLBACK = "terminal.scrollback",
        TERMINAL_FIND = "terminal.find",

        // Capture & Export
        CAPTURE_START = "capture.start",
        CAPTURE_EXPORT = "capture.export",

        // Scripting
        SCRIPT_RUN = "script.run",

        // Application
        APP_SETTINGS = "app.settings",
        APP_QUIT = "app.quit",
        APP_ABOUT = "app.about",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine_gui::commands::macros::CommandId;

    #[test]
    fn test_icy_term_commands_created() {
        let set = create_icy_term_commands();

        // Should have common commands
        assert!(set.get(cmd::VIEW_ZOOM_IN.command_id()).is_some());
        assert!(set.get(cmd::EDIT_COPY.command_id()).is_some());

        // Should have icy_term specific commands
        assert!(set.get(cmd::CONNECTION_DIALING_DIRECTORY.command_id()).is_some());
        assert!(set.get(cmd::TRANSFER_UPLOAD.command_id()).is_some());
        assert!(set.get(cmd::TERMINAL_SCROLLBACK.command_id()).is_some());
    }

    #[test]
    fn test_cmd_static_hotkey_display() {
        // Test that cmd:: statics directly have hotkeys (from icy_engine_gui)
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
        }
    }

    #[test]
    fn test_icy_term_cmd_static_hotkey_display() {
        // Test that icy_term specific cmd:: statics have hotkeys
        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(
                cmd::CONNECTION_DIALING_DIRECTORY.primary_hotkey_display(),
                Some("Alt+D".to_string()),
                "CONNECTION_DIALING_DIRECTORY should have Alt+D"
            );
            assert_eq!(
                cmd::TRANSFER_UPLOAD.primary_hotkey_display(),
                Some("Alt+PageUp".to_string()),
                "TRANSFER_UPLOAD should have Alt+PageUp"
            );
            assert_eq!(
                cmd::APP_SETTINGS.primary_hotkey_display(),
                Some("Alt+O".to_string()),
                "APP_SETTINGS should have Alt+O"
            );
        }
    }
}
