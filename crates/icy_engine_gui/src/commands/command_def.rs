//! Command Definition
//!
//! Represents a single command with its ID and platform-specific hotkeys.

use super::{Hotkey, MouseBinding};
use serde::{Deserialize, Serialize};

/// A command definition with platform-specific hotkeys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDef {
    /// Unique identifier for the command (also used as i18n key)
    pub id: String,

    /// Category for help dialog grouping (e.g., "navigation", "zoom", "playback")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// Hotkeys for Windows/Linux
    #[serde(default, alias = "hotkey", skip_serializing_if = "Vec::is_empty")]
    hotkeys: Vec<Hotkey>,

    /// Hotkeys for macOS (falls back to `hotkeys` if empty)
    #[serde(default, alias = "hotkey_mac", skip_serializing_if = "Vec::is_empty")]
    hotkeys_mac: Vec<Hotkey>,

    /// Mouse button bindings (platform-independent)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    mouse_bindings: Vec<MouseBinding>,

    // =========================================================================
    // Localized strings (populated by translate() method)
    // =========================================================================
    /// Localized action name for help dialog (e.g., "Zoom In")
    #[serde(skip)]
    pub label_action: String,

    /// Localized description for help dialog (e.g., "Increase the zoom level")
    #[serde(skip)]
    pub label_description: String,

    /// Localized menu label (e.g., "Zoom In…" with ellipsis for dialogs)
    #[serde(skip)]
    pub label_menu: String,
}

impl CommandDef {
    /// Create a new command definition
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            category: None,
            hotkeys: Vec::new(),
            hotkeys_mac: Vec::new(),
            mouse_bindings: Vec::new(),
            label_action: String::new(),
            label_description: String::new(),
            label_menu: String::new(),
        }
    }

    /// Create from string hotkey definitions
    pub fn from_strings(id: impl Into<String>, hotkeys: &[&str], hotkeys_mac: &[&str]) -> Self {
        Self {
            id: id.into(),
            category: None,
            hotkeys: hotkeys.iter().filter_map(|s| Hotkey::parse(s)).collect(),
            hotkeys_mac: hotkeys_mac.iter().filter_map(|s| Hotkey::parse(s)).collect(),
            mouse_bindings: Vec::new(),
            label_action: String::new(),
            label_description: String::new(),
            label_menu: String::new(),
        }
    }

    /// Set the category for help dialog grouping
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Add a hotkey for Windows/Linux
    pub fn with_hotkey(mut self, hotkey: &str) -> Self {
        if let Some(hk) = Hotkey::parse(hotkey) {
            self.hotkeys.push(hk);
        }
        self
    }

    /// Add a hotkey for macOS
    pub fn with_hotkey_mac(mut self, hotkey: &str) -> Self {
        if let Some(hk) = Hotkey::parse(hotkey) {
            self.hotkeys_mac.push(hk);
        }
        self
    }

    /// Add multiple hotkeys for Windows/Linux
    pub fn with_hotkeys(mut self, hotkeys: &[&str]) -> Self {
        for hk in hotkeys {
            if let Some(parsed) = Hotkey::parse(hk) {
                self.hotkeys.push(parsed);
            }
        }
        self
    }

    /// Add multiple hotkeys for macOS
    pub fn with_hotkeys_mac(mut self, hotkeys: &[&str]) -> Self {
        for hk in hotkeys {
            if let Some(parsed) = Hotkey::parse(hk) {
                self.hotkeys_mac.push(parsed);
            }
        }
        self
    }

    /// Get the active hotkeys for the current platform
    pub fn active_hotkeys(&self) -> &[Hotkey] {
        if cfg!(target_os = "macos") && !self.hotkeys_mac.is_empty() {
            &self.hotkeys_mac
        } else {
            &self.hotkeys
        }
    }

    /// Get all hotkeys (Windows/Linux)
    pub fn hotkeys(&self) -> &[Hotkey] {
        &self.hotkeys
    }

    /// Get all hotkeys (macOS)
    pub fn hotkeys_mac(&self) -> &[Hotkey] {
        &self.hotkeys_mac
    }

    /// Get mouse bindings
    pub fn mouse_bindings(&self) -> &[MouseBinding] {
        &self.mouse_bindings
    }

    /// Add a mouse binding
    pub fn with_mouse(mut self, binding: &str) -> Self {
        if let Some(mb) = MouseBinding::parse(binding) {
            self.mouse_bindings.push(mb);
        }
        self
    }

    /// Get the primary hotkey for display (platform-specific)
    pub fn primary_hotkey(&self) -> Option<&Hotkey> {
        self.active_hotkeys().first()
    }

    /// Get the primary hotkey as a display string
    pub fn primary_hotkey_display(&self) -> Option<String> {
        self.primary_hotkey().map(|hk| hk.to_string())
    }

    /// Override hotkeys from user configuration
    pub fn override_hotkeys(&mut self, hotkeys: Vec<Hotkey>) {
        self.hotkeys = hotkeys;
    }

    /// Override macOS hotkeys from user configuration
    pub fn override_hotkeys_mac(&mut self, hotkeys: Vec<Hotkey>) {
        self.hotkeys_mac = hotkeys;
    }

    /// Get the category of this command
    pub fn category(&self) -> Option<&str> {
        self.category.as_deref()
    }

    /// Convert command ID to fluent key prefix
    /// "view.zoom_in" → "cmd-view-zoom_in"
    pub fn fluent_key_prefix(&self) -> String {
        format!("cmd-{}", self.id.replace('.', "-"))
    }

    /// Get the fluent key for the action name
    /// "view.zoom_in" → "cmd-view-zoom_in-action"
    pub fn fluent_action_key(&self) -> String {
        format!("{}-action", self.fluent_key_prefix())
    }

    /// Get the fluent key for the description
    /// "view.zoom_in" → "cmd-view-zoom_in-desc"
    pub fn fluent_desc_key(&self) -> String {
        format!("{}-desc", self.fluent_key_prefix())
    }

    /// Get the fluent key for the category
    /// Category "navigation" → "cmd-category-navigation"
    pub fn fluent_category_key(&self) -> Option<String> {
        self.category.as_ref().map(|cat| format!("cmd-category-{}", cat))
    }

    /// Get the fluent key for the menu label
    /// "view.zoom_in" → "cmd-view-zoom_in-menu"
    pub fn fluent_menu_key(&self) -> String {
        format!("{}-menu", self.fluent_key_prefix())
    }

    /// Translate all label fields using the provided translator function.
    ///
    /// The translator should return the translated string for a fluent key,
    /// or the key itself if no translation is found.
    ///
    /// # Arguments
    /// * `translator` - Function that takes a fluent key and returns the translation
    ///
    /// # Example
    /// ```ignore
    /// cmd.translate(|key| fl!(LANGUAGE_LOADER, key));
    /// ```
    pub fn translate<F>(&mut self, translator: F)
    where
        F: Fn(&str) -> String,
    {
        self.label_action = translator(&self.fluent_action_key());
        self.label_description = translator(&self.fluent_desc_key());

        // Menu label: use action label as fallback (no separate menu translation needed)
        self.label_menu = self.label_action.clone();
    }

    /// Get menu label with hotkey appended (if any)
    /// Returns something like "Open…\tCtrl+O"
    pub fn menu_label_with_hotkey(&self) -> String {
        if let Some(hotkey) = self.primary_hotkey_display() {
            format!("{}\t{}", self.label_menu, hotkey)
        } else {
            self.label_menu.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::KeyCode;

    #[test]
    fn test_new_command() {
        let cmd = CommandDef::new("copy");
        assert_eq!(cmd.id, "copy");
        assert!(cmd.hotkeys.is_empty());
        assert!(cmd.hotkeys_mac.is_empty());
    }

    #[test]
    fn test_with_hotkey() {
        let cmd = CommandDef::new("copy").with_hotkey("Ctrl+C").with_hotkey_mac("Cmd+C");

        assert_eq!(cmd.hotkeys.len(), 1);
        assert_eq!(cmd.hotkeys[0].key, KeyCode::C);
        assert!(cmd.hotkeys[0].modifiers.ctrl);

        assert_eq!(cmd.hotkeys_mac.len(), 1);
        assert_eq!(cmd.hotkeys_mac[0].key, KeyCode::C);
        assert!(cmd.hotkeys_mac[0].modifiers.cmd);
    }

    #[test]
    fn test_multiple_hotkeys() {
        let cmd = CommandDef::new("zoom_in").with_hotkeys(&["Ctrl++", "Ctrl+="]).with_hotkeys_mac(&["+", "Cmd+="]);

        assert_eq!(cmd.hotkeys.len(), 2);
        assert_eq!(cmd.hotkeys_mac.len(), 2);
    }

    #[test]
    fn test_from_strings() {
        let cmd = CommandDef::from_strings("new_window", &["Ctrl+Shift+N"], &["Cmd+N"]);

        assert_eq!(cmd.id, "new_window");
        assert_eq!(cmd.hotkeys.len(), 1);
        assert_eq!(cmd.hotkeys_mac.len(), 1);
    }

    #[test]
    fn test_primary_hotkey() {
        let cmd = CommandDef::new("test").with_hotkey("Ctrl+T");

        let primary = cmd.primary_hotkey().unwrap();
        assert_eq!(primary.key, KeyCode::T);
        assert!(primary.modifiers.ctrl);
    }

    #[test]
    fn test_primary_hotkey_display() {
        let cmd = CommandDef::new("test").with_hotkey("Ctrl+Shift+N");

        assert_eq!(cmd.primary_hotkey_display(), Some("Ctrl+Shift+N".to_string()));
    }

    #[test]
    fn test_active_hotkeys_fallback() {
        // On non-mac, if hotkeys_mac is empty, should fall back to hotkeys
        let cmd = CommandDef::new("test").with_hotkey("Ctrl+T");

        // This test will behave differently on Mac vs other platforms
        let active = cmd.active_hotkeys();
        assert!(!active.is_empty());
    }

    #[test]
    fn test_serde_roundtrip() {
        let cmd = CommandDef::new("edit.copy").with_hotkey("Ctrl+C").with_hotkey_mac("Cmd+C");

        let toml_str = toml::to_string(&cmd).unwrap();

        // Should contain the id and hotkeys
        assert!(toml_str.contains("edit.copy"));
        assert!(toml_str.contains("Ctrl+C"));
        assert!(toml_str.contains("Cmd+C"));

        // Deserialize back
        let parsed: CommandDef = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.id, "edit.copy");
        assert_eq!(parsed.hotkeys().len(), 1);
        assert_eq!(parsed.hotkeys_mac().len(), 1);
    }

    #[test]
    fn test_serde_empty_hotkeys_not_serialized() {
        let cmd = CommandDef::new("help.about");

        let toml_str = toml::to_string(&cmd).unwrap();

        // Empty hotkeys should not appear in output
        assert!(!toml_str.contains("hotkeys"));
        assert!(toml_str.contains("help.about"));
    }

    #[test]
    fn test_serde_multiple_commands() {
        // TOML format: [[commands]] array of tables
        #[derive(serde::Serialize, serde::Deserialize)]
        struct CommandsFile {
            commands: Vec<CommandDef>,
        }

        let file = CommandsFile {
            commands: vec![CommandDef::new("copy").with_hotkey("Ctrl+C"), CommandDef::new("paste").with_hotkey("Ctrl+V")],
        };

        let toml_str = toml::to_string(&file).unwrap();
        let parsed: CommandsFile = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.commands.len(), 2);
        assert_eq!(parsed.commands[0].id, "copy");
        assert_eq!(parsed.commands[1].id, "paste");
    }

    #[test]
    fn test_fluent_keys() {
        let cmd = CommandDef::new("view.zoom_in").with_category("navigation");

        assert_eq!(cmd.fluent_key_prefix(), "cmd-view-zoom_in");
        assert_eq!(cmd.fluent_action_key(), "cmd-view-zoom_in-action");
        assert_eq!(cmd.fluent_desc_key(), "cmd-view-zoom_in-desc");
        assert_eq!(cmd.fluent_category_key(), Some("cmd-category-navigation".to_string()));
    }

    #[test]
    fn test_serde_hotkey_alias() {
        // Test that 'hotkey' (singular) is correctly deserialized as 'hotkeys'
        let toml_str = r#"
id = "edit.copy"
hotkey = ["Ctrl+C"]
hotkey_mac = ["Cmd+C"]
category = "edit"
"#;
        let cmd: CommandDef = toml::from_str(toml_str).unwrap();
        assert_eq!(cmd.id, "edit.copy");
        assert_eq!(cmd.hotkeys().len(), 1, "hotkeys should be populated from 'hotkey' field");
        assert_eq!(cmd.hotkeys_mac().len(), 1, "hotkeys_mac should be populated from 'hotkey_mac' field");
        assert_eq!(cmd.primary_hotkey_display(), Some("Ctrl+C".to_string()));
    }

    #[test]
    fn test_category() {
        let cmd = CommandDef::new("file.save").with_category("file");

        assert_eq!(cmd.category(), Some("file"));

        let cmd_no_cat = CommandDef::new("misc.action");
        assert_eq!(cmd_no_cat.category(), None);
        assert_eq!(cmd_no_cat.fluent_category_key(), None);
    }
}
