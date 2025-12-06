//! TOML loader for command definitions
//!
//! Loads command definitions from TOML files.

use std::path::Path;

use super::{CommandDef, CommandSet, Hotkey, MouseBinding};

/// Raw command definition as it appears in TOML
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CommandToml {
    pub id: String,
    #[serde(default)]
    pub hotkey: Vec<String>,
    #[serde(default)]
    pub hotkey_mac: Vec<String>,
    #[serde(default)]
    pub mouse: Vec<String>,
}

/// Container for the TOML file structure
#[derive(Debug, serde::Deserialize)]
pub struct CommandsFile {
    #[serde(default)]
    pub commands: Vec<CommandToml>,
}

impl CommandToml {
    /// Convert to CommandDef
    pub fn into_command_def(self) -> CommandDef {
        let hotkeys: Vec<Hotkey> = self.hotkey
            .iter()
            .filter_map(|s| Hotkey::parse(s))
            .collect();
        
        let hotkeys_mac: Vec<Hotkey> = self.hotkey_mac
            .iter()
            .filter_map(|s| Hotkey::parse(s))
            .collect();

        let mouse_bindings: Vec<MouseBinding> = self.mouse
            .iter()
            .filter_map(|s| MouseBinding::parse(s))
            .collect();

        let mut cmd = CommandDef::new(self.id);
        for hk in hotkeys {
            cmd = cmd.with_hotkey(&hk.to_string());
        }
        for hk in hotkeys_mac {
            cmd = cmd.with_hotkey_mac(&hk.to_string());
        }
        for mb in mouse_bindings {
            cmd = cmd.with_mouse(&mb.to_string());
        }
        cmd
    }
}

impl CommandsFile {
    /// Parse from TOML string
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }

    /// Load from file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, CommandLoadError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| CommandLoadError::Io(e))?;
        Self::from_toml(&content)
            .map_err(|e| CommandLoadError::Parse(e))
    }

    /// Convert to CommandSet
    pub fn into_command_set(self) -> CommandSet {
        let mut set = CommandSet::new();
        for cmd_toml in self.commands {
            set.add(cmd_toml.into_command_def());
        }
        set
    }
}

/// Error type for command loading
#[derive(Debug)]
pub enum CommandLoadError {
    Io(std::io::Error),
    Parse(toml::de::Error),
}

impl std::fmt::Display for CommandLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::Parse(e) => write!(f, "Parse error: {}", e),
        }
    }
}

impl std::error::Error for CommandLoadError {}

/// Load commands from a TOML string
pub fn load_commands_from_str(toml_str: &str) -> Result<CommandSet, CommandLoadError> {
    let file = CommandsFile::from_toml(toml_str)
        .map_err(|e| CommandLoadError::Parse(e))?;
    Ok(file.into_command_set())
}

/// Load commands from a file path
pub fn load_commands_from_file(path: impl AsRef<Path>) -> Result<CommandSet, CommandLoadError> {
    let file = CommandsFile::from_file(path)?;
    Ok(file.into_command_set())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{KeyCode, Modifiers};

    const TEST_TOML: &str = r#"
[[commands]]
id = "test.copy"
hotkey = ["Ctrl+C"]
hotkey_mac = ["Cmd+C"]

[[commands]]
id = "test.zoom"
hotkey = ["Ctrl++", "Ctrl+="]
hotkey_mac = ["+", "="]

[[commands]]
id = "test.nokey"
hotkey = []
"#;

    #[test]
    fn test_parse_toml() {
        let file = CommandsFile::from_toml(TEST_TOML).unwrap();
        assert_eq!(file.commands.len(), 3);
        assert_eq!(file.commands[0].id, "test.copy");
        assert_eq!(file.commands[0].hotkey, vec!["Ctrl+C"]);
        assert_eq!(file.commands[0].hotkey_mac, vec!["Cmd+C"]);
    }

    #[test]
    fn test_into_command_set() {
        let set = load_commands_from_str(TEST_TOML).unwrap();
        
        assert_eq!(set.len(), 3);
        assert!(set.get("test.copy").is_some());
        assert!(set.get("test.zoom").is_some());
        assert!(set.get("test.nokey").is_some());
    }

    #[test]
    fn test_hotkey_matching() {
        let set = load_commands_from_str(TEST_TOML).unwrap();

        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(set.match_key(KeyCode::C, Modifiers::CTRL), Some("test.copy"));
            assert_eq!(set.match_key(KeyCode::Plus, Modifiers::CTRL), Some("test.zoom"));
            assert_eq!(set.match_key(KeyCode::Equals, Modifiers::CTRL), Some("test.zoom"));
        }
    }

    #[test]
    fn test_empty_hotkey() {
        let set = load_commands_from_str(TEST_TOML).unwrap();
        
        let cmd = set.get("test.nokey").unwrap();
        assert!(cmd.primary_hotkey().is_none());
    }

    #[test]
    fn test_load_common_commands() {
        // Test loading the embedded common commands
        let toml_str = include_str!("../../data/commands_common.toml");
        let set = load_commands_from_str(toml_str).unwrap();
        
        // Should have all the common commands
        assert!(set.get("file.open").is_some());
        assert!(set.get("edit.copy").is_some());
        assert!(set.get("view.zoom_in").is_some());
        assert!(set.get("help.show").is_some());
        
        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(set.match_key(KeyCode::C, Modifiers::CTRL), Some("edit.copy"));
            assert_eq!(set.match_key(KeyCode::F1, Modifiers::NONE), Some("help.show"));
        }
    }
}
