//! Command definition system with embedded TOML and translation source
//!
//! This module provides `define_commands!` macro that creates `LazyLock<CommandDef>`
//! statics. Each command loads its hotkeys from the embedded TOML and translations
//! from the associated LanguageLoader - all lazily on first access.
//!
//! # Example
//! ```ignore
//! use icy_engine_gui::define_commands;
//!
//! define_commands! {
//!     loader: crate::LANGUAGE_LOADER,
//!     commands: include_str!("../../data/commands.toml"),
//!
//!     FILE_NEW = "file.new",
//!     FILE_SAVE = "file.save",
//! }
//!
//! // Access a command - lazily loads hotkeys and translations
//! println!("{}", FILE_NEW.label_menu);
//! ```

use i18n_embed::fluent::FluentLanguageLoader;

use super::toml_loader::CommandToml;
use super::CommandDef;

/// Parse a TOML string and find the command definition for the given ID
fn find_command_in_toml(toml_str: &str, id: &str) -> Option<CommandDef> {
    #[derive(serde::Deserialize)]
    struct CommandsFile {
        commands: Vec<CommandToml>,
    }

    let file: CommandsFile = toml::from_str(toml_str).ok()?;
    file.commands.into_iter().find(|cmd| cmd.id == id).map(|c| c.into_command_def())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::toml_loader::CommandToml;

    const TEST_TOML: &str = include_str!("../../data/commands_common.toml");

    #[ignore = "fixme"]
    #[test]
    fn test_find_command_in_toml() {
        // First, try to parse the TOML to see if it works at all
        #[derive(serde::Deserialize)]
        struct CommandsFile {
            _commands: Vec<CommandToml>,
        }

        let result: Result<CommandsFile, _> = toml::from_str(TEST_TOML);
        assert!(result.is_ok(), "TOML should parse without errors");

        let cmd = find_command_in_toml(TEST_TOML, "edit.copy");
        assert!(cmd.is_some(), "edit.copy should be found in TOML");

        let cmd = cmd.unwrap();
        assert_eq!(cmd.id, "edit.copy");
        assert_eq!(cmd.hotkeys().len(), 1, "edit.copy should have hotkeys from TOML");
        assert_eq!(cmd.primary_hotkey_display(), Some("Ctrl+C".to_string()));
    }
}

/// Create a CommandDef from TOML, applying translations from the loader
pub fn create_command_def(id: &'static str, toml_str: &'static str, loader: &FluentLanguageLoader) -> CommandDef {
    // Try to find in TOML, otherwise create a basic command
    let mut cmd = find_command_in_toml(toml_str, id).unwrap_or_else(|| CommandDef::new(id));

    // Apply translations
    cmd.translate(|key| loader.get(key));

    cmd
}

/// Macro to define commands with their associated TOML and translation source
///
/// # Example
/// ```ignore
/// define_commands! {
///     loader: crate::LANGUAGE_LOADER,
///     commands: include_str!("../../data/commands_common.toml"),
///
///     FILE_NEW = "file.new",
///     FILE_SAVE = "file.save",
/// }
/// ```
///
/// This creates:
/// - `pub static FILE_NEW: LazyLock<CommandDef>` with hotkeys from TOML and translations from loader
/// - `pub static FILE_SAVE: LazyLock<CommandDef>` with hotkeys from TOML and translations from loader
#[macro_export]
macro_rules! define_commands {
    (
        loader: $loader:expr,
        commands: $toml:expr,
        $( $name:ident = $id:literal ),* $(,)?
    ) => {
        $(
            pub static $name: std::sync::LazyLock<$crate::commands::CommandDef> =
                std::sync::LazyLock::new(|| {
                    $crate::commands::command_ref::create_command_def(
                        $id,
                        $toml,
                        &$loader,
                    )
                });
        )*
    };
}
