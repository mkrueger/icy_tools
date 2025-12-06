//! Command Set
//!
//! A collection of command definitions that can match keyboard events.

use std::collections::HashMap;

use super::{CommandDef, Hotkey, KeyCode, Modifiers};

/// A collection of commands that can match keyboard events
#[derive(Debug, Default)]
pub struct CommandSet {
    /// Commands indexed by their ID
    commands: HashMap<String, CommandDef>,
    
    /// Lookup table: Hotkey -> command ID (for fast matching)
    hotkey_map: HashMap<Hotkey, String>,
}

impl CommandSet {
    /// Create a new empty command set
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a command to the set
    pub fn add(&mut self, command: CommandDef) {
        let id = command.id.clone();
        
        // Update hotkey lookup map
        for hotkey in command.active_hotkeys() {
            self.hotkey_map.insert(*hotkey, id.clone());
        }
        
        self.commands.insert(id, command);
    }

    /// Add multiple commands
    pub fn add_all(&mut self, commands: impl IntoIterator<Item = CommandDef>) {
        for cmd in commands {
            self.add(cmd);
        }
    }

    /// Get a command by ID
    pub fn get(&self, id: &str) -> Option<&CommandDef> {
        self.commands.get(id)
    }

    /// Get a mutable reference to a command by ID
    pub fn get_mut(&mut self, id: &str) -> Option<&mut CommandDef> {
        self.commands.get_mut(id)
    }

    /// Match a key event and return the command ID if found
    pub fn match_key(&self, key: KeyCode, modifiers: Modifiers) -> Option<&str> {
        let hotkey = Hotkey::new(key, modifiers);
        self.hotkey_map.get(&hotkey).map(|s| s.as_str())
    }

    /// Match a hotkey and return the command ID if found
    pub fn match_hotkey(&self, hotkey: &Hotkey) -> Option<&str> {
        self.hotkey_map.get(hotkey).map(|s| s.as_str())
    }

    /// Merge another command set into this one
    /// Commands from `other` will override commands with the same ID
    pub fn merge(&mut self, other: CommandSet) {
        for (_, cmd) in other.commands {
            self.add(cmd);
        }
    }

    /// Rebuild the hotkey lookup map (call after modifying commands)
    pub fn rebuild_hotkey_map(&mut self) {
        self.hotkey_map.clear();
        for (id, cmd) in &self.commands {
            for hotkey in cmd.active_hotkeys() {
                self.hotkey_map.insert(*hotkey, id.clone());
            }
        }
    }

    /// Override a specific command's hotkeys
    pub fn override_hotkeys(&mut self, id: &str, hotkeys: &[&str]) {
        if let Some(cmd) = self.commands.get_mut(id) {
            let parsed: Vec<Hotkey> = hotkeys.iter()
                .filter_map(|s| Hotkey::parse(s))
                .collect();
            cmd.override_hotkeys(parsed);
            self.rebuild_hotkey_map();
        }
    }

    /// Get all command IDs
    pub fn command_ids(&self) -> impl Iterator<Item = &str> {
        self.commands.keys().map(|s| s.as_str())
    }

    /// Get all commands
    pub fn commands(&self) -> impl Iterator<Item = &CommandDef> {
        self.commands.values()
    }

    /// Number of commands in the set
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Check for hotkey conflicts (same hotkey bound to multiple commands)
    pub fn find_conflicts(&self) -> Vec<(Hotkey, Vec<String>)> {
        let mut hotkey_to_commands: HashMap<Hotkey, Vec<String>> = HashMap::new();
        
        for (id, cmd) in &self.commands {
            for hotkey in cmd.active_hotkeys() {
                hotkey_to_commands
                    .entry(*hotkey)
                    .or_default()
                    .push(id.clone());
            }
        }

        hotkey_to_commands
            .into_iter()
            .filter(|(_, ids)| ids.len() > 1)
            .collect()
    }
}

/// Builder macro for creating command sets
#[macro_export]
macro_rules! command_set {
    (
        $(
            $id:literal {
                $( hotkey: [ $($hk:literal),* $(,)? ] )?
                $( hotkey_mac: [ $($hk_mac:literal),* $(,)? ] )?
            }
        ),* $(,)?
    ) => {{
        let mut set = $crate::commands::CommandSet::new();
        $(
            #[allow(unused_mut)]
            let mut cmd = $crate::commands::CommandDef::new($id);
            $(
                cmd = cmd.with_hotkeys(&[$($hk),*]);
            )?
            $(
                cmd = cmd.with_hotkeys_mac(&[$($hk_mac),*]);
            )?
            set.add(cmd);
        )*
        set
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_set() -> CommandSet {
        let mut set = CommandSet::new();
        
        set.add(CommandDef::new("copy")
            .with_hotkey("Ctrl+C")
            .with_hotkey_mac("Cmd+C"));
        
        set.add(CommandDef::new("paste")
            .with_hotkey("Ctrl+V")
            .with_hotkey_mac("Cmd+V"));
        
        set.add(CommandDef::new("zoom_in")
            .with_hotkeys(&["Ctrl++", "Ctrl+="])
            .with_hotkeys_mac(&["+", "Cmd+="]));
        
        set
    }

    #[test]
    fn test_match_key() {
        let set = create_test_set();
        
        // On non-Mac, Ctrl+C should match "copy"
        #[cfg(not(target_os = "macos"))]
        {
            let result = set.match_key(KeyCode::C, Modifiers::CTRL);
            assert_eq!(result, Some("copy"));
        }

        // Ctrl+V should match "paste"
        #[cfg(not(target_os = "macos"))]
        {
            let result = set.match_key(KeyCode::V, Modifiers::CTRL);
            assert_eq!(result, Some("paste"));
        }

        // Unbound key should return None
        let result = set.match_key(KeyCode::X, Modifiers::NONE);
        assert!(result.is_none());
    }

    #[test]
    fn test_multiple_hotkeys() {
        let set = create_test_set();
        
        #[cfg(not(target_os = "macos"))]
        {
            // Both Ctrl++ and Ctrl+= should match "zoom_in"
            assert_eq!(set.match_key(KeyCode::Plus, Modifiers::CTRL), Some("zoom_in"));
            assert_eq!(set.match_key(KeyCode::Equals, Modifiers::CTRL), Some("zoom_in"));
        }
    }

    #[test]
    fn test_get_command() {
        let set = create_test_set();
        
        let copy_cmd = set.get("copy").unwrap();
        assert_eq!(copy_cmd.id, "copy");
        
        assert!(set.get("nonexistent").is_none());
    }

    #[test]
    fn test_merge() {
        let mut set1 = CommandSet::new();
        set1.add(CommandDef::new("cmd1").with_hotkey("Ctrl+1"));
        
        let mut set2 = CommandSet::new();
        set2.add(CommandDef::new("cmd2").with_hotkey("Ctrl+2"));
        
        set1.merge(set2);
        
        assert!(set1.get("cmd1").is_some());
        assert!(set1.get("cmd2").is_some());
        assert_eq!(set1.len(), 2);
    }

    #[test]
    fn test_override_hotkeys() {
        let mut set = create_test_set();
        
        // Override copy's hotkey
        set.override_hotkeys("copy", &["Ctrl+Shift+C"]);
        
        #[cfg(not(target_os = "macos"))]
        {
            // Old hotkey should no longer match
            assert!(set.match_key(KeyCode::C, Modifiers::CTRL).is_none());
            
            // New hotkey should match
            assert_eq!(set.match_key(KeyCode::C, Modifiers::CTRL_SHIFT), Some("copy"));
        }
    }

    #[test]
    fn test_command_set_macro() {
        let set = command_set! {
            "copy" {
                hotkey: ["Ctrl+C"]
                hotkey_mac: ["Cmd+C"]
            },
            "paste" {
                hotkey: ["Ctrl+V"]
                hotkey_mac: ["Cmd+V"]
            },
        };

        assert_eq!(set.len(), 2);
        assert!(set.get("copy").is_some());
        assert!(set.get("paste").is_some());
    }

    #[test]
    fn test_find_conflicts() {
        let mut set = CommandSet::new();
        
        // Create a conflict: two commands with the same hotkey
        set.add(CommandDef::new("cmd1").with_hotkey("Ctrl+C"));
        set.add(CommandDef::new("cmd2").with_hotkey("Ctrl+C"));
        
        let conflicts = set.find_conflicts();
        // Note: Due to HashMap behavior, one command wins in the hotkey_map,
        // but find_conflicts scans all commands
        assert!(!conflicts.is_empty());
    }
}
