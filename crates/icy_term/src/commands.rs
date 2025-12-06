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

/// Command IDs specific to icy_term
pub mod cmd {
    // Re-export common commands
    pub use icy_engine_gui::commands::cmd::*;

    // Connection
    pub const CONNECTION_DIALING_DIRECTORY: &str = "connection.dialing_directory";
    pub const CONNECTION_HANGUP: &str = "connection.hangup";
    pub const CONNECTION_SERIAL: &str = "connection.serial";

    // File Transfer
    pub const TRANSFER_UPLOAD: &str = "transfer.upload";
    pub const TRANSFER_DOWNLOAD: &str = "transfer.download";

    // Login
    pub const LOGIN_SEND_ALL: &str = "login.send_all";
    pub const LOGIN_SEND_USER: &str = "login.send_user";
    pub const LOGIN_SEND_PASSWORD: &str = "login.send_password";

    // Terminal
    pub const TERMINAL_CLEAR: &str = "terminal.clear";
    pub const TERMINAL_SCROLLBACK: &str = "terminal.scrollback";
    pub const TERMINAL_FIND: &str = "terminal.find";

    // Capture & Export
    pub const CAPTURE_START: &str = "capture.start";
    pub const CAPTURE_EXPORT: &str = "capture.export";

    // Scripting
    pub const SCRIPT_RUN: &str = "script.run";

    // Application
    pub const APP_SETTINGS: &str = "app.settings";
    pub const APP_QUIT: &str = "app.quit";
    pub const APP_ABOUT: &str = "app.about";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icy_term_commands_created() {
        let set = create_icy_term_commands();

        // Should have common commands
        assert!(set.get(cmd::VIEW_ZOOM_IN).is_some());
        assert!(set.get(cmd::EDIT_COPY).is_some());

        // Should have icy_term specific commands
        assert!(set.get(cmd::CONNECTION_DIALING_DIRECTORY).is_some());
        assert!(set.get(cmd::TRANSFER_UPLOAD).is_some());
        assert!(set.get(cmd::TERMINAL_SCROLLBACK).is_some());
    }
}
