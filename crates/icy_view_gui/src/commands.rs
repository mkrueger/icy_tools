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
    
    let icy_view_commands = load_commands_from_str(ICY_VIEW_COMMANDS_TOML)
        .expect("Failed to parse embedded commands_icy_view.toml");
    
    commands.merge(icy_view_commands);
    commands
}

/// Command IDs specific to icy_view
pub mod cmd {
    // Re-export common commands
    pub use icy_engine_gui::commands::cmd::*;
    
    // icy_view specific dialogs
    pub const DIALOG_SAUCE: &str = "dialog.sauce";
    pub const DIALOG_EXPORT: &str = "dialog.export";
    pub const DIALOG_FILTER: &str = "dialog.filter";

    // Playback
    pub const PLAYBACK_TOGGLE_SCROLL: &str = "playback.toggle_scroll";
    pub const PLAYBACK_SCROLL_SPEED: &str = "playback.scroll_speed";
    pub const PLAYBACK_SCROLL_SPEED_BACK: &str = "playback.scroll_speed_back";
    pub const PLAYBACK_BAUD_RATE: &str = "playback.baud_rate";
    pub const PLAYBACK_BAUD_RATE_BACK: &str = "playback.baud_rate_back";
    pub const PLAYBACK_BAUD_RATE_OFF: &str = "playback.baud_rate_off";

    // External Commands
    pub const EXTERNAL_COMMAND_0: &str = "external.command_0";
    pub const EXTERNAL_COMMAND_1: &str = "external.command_1";
    pub const EXTERNAL_COMMAND_2: &str = "external.command_2";
    pub const EXTERNAL_COMMAND_3: &str = "external.command_3";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icy_view_commands_created() {
        let set = create_icy_view_commands();
        
        // Should have common commands
        assert!(set.get(cmd::FILE_OPEN).is_some());
        assert!(set.get(cmd::VIEW_ZOOM_IN).is_some());
        
        // Should have icy_view specific commands
        assert!(set.get(cmd::DIALOG_SAUCE).is_some());
        assert!(set.get(cmd::PLAYBACK_BAUD_RATE).is_some());
        assert!(set.get(cmd::EXTERNAL_COMMAND_0).is_some());
    }
}
