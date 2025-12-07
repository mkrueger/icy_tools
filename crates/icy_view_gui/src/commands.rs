//! icy_view specific commands
//!
//! These commands are specific to icy_view and are merged with
//! the common commands from icy_engine_gui.

use icy_engine_gui::commands::{CommandSet, load_commands_from_str};

/// The embedded icy_view specific commands TOML
const ICY_VIEW_COMMANDS_TOML: &str = include_str!("../data/commands_icy_view.toml");

/// Create the icy_view command set by merging common commands with icy_view specific commands
pub fn create_icy_view_commands() -> CommandSet {
    let mut commands = icy_engine_gui::commands::create_common_commands();

    let icy_view_commands = load_commands_from_str(ICY_VIEW_COMMANDS_TOML).expect("Failed to parse embedded commands_icy_view.toml");

    commands.merge(icy_view_commands);
    commands
}

/// Command definitions for icy_view
///
/// Each command is a `LazyLock<CommandDef>` that lazily loads:
/// - Hotkeys from the embedded TOML
/// - Translations from the LANGUAGE_LOADER
pub mod cmd {
    use icy_engine_gui::define_commands;

    // Re-export common commands from icy_engine_gui
    pub use icy_engine_gui::commands::cmd::*;

    const TOML: &str = include_str!("../data/commands_icy_view.toml");

    define_commands! {
        loader: crate::LANGUAGE_LOADER,
        commands: TOML,

        // Dialogs
        DIALOG_SAUCE = "dialog.sauce",
        DIALOG_EXPORT = "dialog.export",
        DIALOG_FILTER = "dialog.filter",

        // Playback
        PLAYBACK_TOGGLE_SCROLL = "playback.toggle_scroll",
        PLAYBACK_SCROLL_SPEED = "playback.scroll_speed",
        PLAYBACK_SCROLL_SPEED_BACK = "playback.scroll_speed_back",
        PLAYBACK_BAUD_RATE = "playback.baud_rate",
        PLAYBACK_BAUD_RATE_BACK = "playback.baud_rate_back",
        PLAYBACK_BAUD_RATE_OFF = "playback.baud_rate_off",

        // External Commands
        EXTERNAL_COMMAND_0 = "external.command_0",
        EXTERNAL_COMMAND_1 = "external.command_1",
        EXTERNAL_COMMAND_2 = "external.command_2",
        EXTERNAL_COMMAND_3 = "external.command_3",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine_gui::commands::macros::CommandId;

    #[test]
    fn test_icy_view_commands_created() {
        let set = create_icy_view_commands();

        // Should have common commands
        assert!(set.get(cmd::FILE_OPEN.command_id()).is_some());
        assert!(set.get(cmd::VIEW_ZOOM_IN.command_id()).is_some());

        // Should have icy_view specific commands
        assert!(set.get(cmd::DIALOG_SAUCE.command_id()).is_some());
        assert!(set.get(cmd::PLAYBACK_BAUD_RATE.command_id()).is_some());
        assert!(set.get(cmd::EXTERNAL_COMMAND_0.command_id()).is_some());
    }
}
