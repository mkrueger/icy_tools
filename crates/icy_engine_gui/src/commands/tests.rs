//! Integration tests for the command system

use super::*;
use crate::command_set;

/// Test simulating real-world usage
#[test]
fn test_typical_editor_commands() {
    let set = command_set! {
        "new_file" {
            hotkey: ["Ctrl+N"]
            hotkey_mac: ["Cmd+N"]
        },
        "open_file" {
            hotkey: ["Ctrl+O"]
            hotkey_mac: ["Cmd+O"]
        },
        "save" {
            hotkey: ["Ctrl+S"]
            hotkey_mac: ["Cmd+S"]
        },
        "save_as" {
            hotkey: ["Ctrl+Shift+S"]
            hotkey_mac: ["Cmd+Shift+S"]
        },
        "undo" {
            hotkey: ["Ctrl+Z"]
            hotkey_mac: ["Cmd+Z"]
        },
        "redo" {
            hotkey: ["Ctrl+Shift+Z", "Ctrl+Y"]
            hotkey_mac: ["Cmd+Shift+Z"]
        },
        "copy" {
            hotkey: ["Ctrl+C"]
            hotkey_mac: ["Cmd+C"]
        },
        "cut" {
            hotkey: ["Ctrl+X"]
            hotkey_mac: ["Cmd+X"]
        },
        "paste" {
            hotkey: ["Ctrl+V"]
            hotkey_mac: ["Cmd+V"]
        },
        "select_all" {
            hotkey: ["Ctrl+A"]
            hotkey_mac: ["Cmd+A"]
        },
        "zoom_in" {
            hotkey: ["Ctrl++", "Ctrl+="]
            hotkey_mac: ["+", "Cmd+="]
        },
        "zoom_out" {
            hotkey: ["Ctrl+-"]
            hotkey_mac: ["-", "Cmd+-"]
        },
        "zoom_reset" {
            hotkey: ["Ctrl+0"]
            hotkey_mac: ["Cmd+0"]
        },
        "fullscreen" {
            hotkey: ["F11", "Alt+Enter"]
            hotkey_mac: ["Cmd+Ctrl+F"]
        },
        "quit" {
            hotkey: ["Ctrl+Q"]
            hotkey_mac: ["Cmd+Q"]
        },
    };

    assert_eq!(set.len(), 15);

    #[cfg(not(target_os = "macos"))]
    {
        // Test common shortcuts
        assert_eq!(set.match_key(KeyCode::C, Modifiers::CTRL), Some("copy"));
        assert_eq!(set.match_key(KeyCode::V, Modifiers::CTRL), Some("paste"));
        assert_eq!(set.match_key(KeyCode::Z, Modifiers::CTRL), Some("undo"));
        assert_eq!(set.match_key(KeyCode::Z, Modifiers::CTRL_SHIFT), Some("redo"));

        // Redo also works with Ctrl+Y
        assert_eq!(set.match_key(KeyCode::Y, Modifiers::CTRL), Some("redo"));

        // Zoom with multiple bindings
        assert_eq!(set.match_key(KeyCode::Plus, Modifiers::CTRL), Some("zoom_in"));
        assert_eq!(set.match_key(KeyCode::Equals, Modifiers::CTRL), Some("zoom_in"));

        // Fullscreen with multiple bindings
        assert_eq!(set.match_key(KeyCode::F11, Modifiers::NONE), Some("fullscreen"));
        assert_eq!(set.match_key(KeyCode::Enter, Modifiers::ALT), Some("fullscreen"));
    }
}

/// Test building commands programmatically
#[test]
fn test_programmatic_command_building() {
    let mut set = CommandSet::new();

    // Build commands programmatically (useful for dynamic/plugin systems)
    let commands = vec![
        ("dial", vec!["Ctrl+D"], vec!["Cmd+D"]),
        ("hangup", vec!["Ctrl+H"], vec!["Cmd+H"]),
        ("send_break", vec!["Ctrl+B"], vec!["Cmd+B"]),
    ];

    for (id, hotkeys, hotkeys_mac) in commands {
        let mut cmd = CommandDef::new(id);
        cmd = cmd.with_hotkeys(&hotkeys.iter().map(|s| *s).collect::<Vec<_>>());
        cmd = cmd.with_hotkeys_mac(&hotkeys_mac.iter().map(|s| *s).collect::<Vec<_>>());
        set.add(cmd);
    }

    assert_eq!(set.len(), 3);

    #[cfg(not(target_os = "macos"))]
    {
        assert_eq!(set.match_key(KeyCode::D, Modifiers::CTRL), Some("dial"));
        assert_eq!(set.match_key(KeyCode::H, Modifiers::CTRL), Some("hangup"));
    }
}

/// Test merging command sets from different sources
#[test]
fn test_command_set_composition() {
    // Common commands (from icy_engine_gui)
    let common = command_set! {
        "copy" { hotkey: ["Ctrl+C"] hotkey_mac: ["Cmd+C"] },
        "paste" { hotkey: ["Ctrl+V"] hotkey_mac: ["Cmd+V"] },
        "zoom_in" { hotkey: ["Ctrl++"] hotkey_mac: ["+"] },
        "zoom_out" { hotkey: ["Ctrl+-"] hotkey_mac: ["-"] },
    };

    // App-specific commands (from icy_view)
    let view_commands = command_set! {
        "go_parent" { hotkey: ["Backspace", "Alt+Up"] hotkey_mac: ["Cmd+Up"] },
        "thumbnail_mode" { hotkey: ["Ctrl+T"] hotkey_mac: ["Cmd+T"] },
        "next_file" { hotkey: ["Right", "Space"] },
        "prev_file" { hotkey: ["Left"] },
    };

    // Merge them
    let mut combined = common;
    combined.merge(view_commands);

    assert_eq!(combined.len(), 8);

    // Both common and view-specific commands should be accessible
    assert!(combined.get("copy").is_some());
    assert!(combined.get("go_parent").is_some());

    #[cfg(not(target_os = "macos"))]
    {
        assert_eq!(combined.match_key(KeyCode::C, Modifiers::CTRL), Some("copy"));
        assert_eq!(combined.match_key(KeyCode::Backspace, Modifiers::NONE), Some("go_parent"));
        assert_eq!(combined.match_key(KeyCode::ArrowRight, Modifiers::NONE), Some("next_file"));
    }
}

/// Test user override functionality
#[test]
fn test_user_overrides() {
    let mut set = command_set! {
        "copy" { hotkey: ["Ctrl+C"] },
        "paste" { hotkey: ["Ctrl+V"] },
    };

    // User wants to use Ctrl+Shift+C for copy instead
    set.override_hotkeys("copy", &["Ctrl+Shift+C"]);

    #[cfg(not(target_os = "macos"))]
    {
        // Original binding no longer works
        assert!(set.match_key(KeyCode::C, Modifiers::CTRL).is_none());

        // New binding works
        assert_eq!(set.match_key(KeyCode::C, Modifiers::CTRL_SHIFT), Some("copy"));

        // Other commands unaffected
        assert_eq!(set.match_key(KeyCode::V, Modifiers::CTRL), Some("paste"));
    }
}

/// Test hotkey display for menus
#[test]
fn test_hotkey_display_for_menus() {
    let set = command_set! {
        "save" { hotkey: ["Ctrl+S"] hotkey_mac: ["Cmd+S"] },
        "save_as" { hotkey: ["Ctrl+Shift+S"] hotkey_mac: ["Cmd+Shift+S"] },
        "export" { }, // No hotkey
    };

    let save_cmd = set.get("save").unwrap();
    let save_as_cmd = set.get("save_as").unwrap();
    let export_cmd = set.get("export").unwrap();

    #[cfg(not(target_os = "macos"))]
    {
        assert_eq!(save_cmd.primary_hotkey_display(), Some("Ctrl+S".to_string()));
        assert_eq!(save_as_cmd.primary_hotkey_display(), Some("Ctrl+Shift+S".to_string()));
    }

    // No hotkey assigned
    assert!(export_cmd.primary_hotkey_display().is_none());
}

/// Test that the command ID can be used as i18n key
#[test]
fn test_command_id_as_i18n_key() {
    let cmd = CommandDef::new("menu.file.save_as");

    // The ID should be usable directly as a translation key
    // e.g., fl!(loader, &cmd.id) or similar
    assert_eq!(cmd.id, "menu.file.save_as");

    // Dot-separated IDs are common for i18n
    let cmd2 = CommandDef::new("cmd.edit.copy");
    assert_eq!(cmd2.id, "cmd.edit.copy");
}

/// Test that mouse bindings work with the command system
#[test]
fn test_mouse_bindings() {
    // Create commands with mouse bindings programmatically
    let mut set = CommandSet::new();
    set.add(CommandDef::new("nav.back").with_hotkey("Alt+Left").with_mouse("Back"));
    set.add(CommandDef::new("nav.forward").with_hotkey("Alt+Right").with_mouse("Forward"));

    // Test mouse binding matching
    let back_binding = MouseBinding::new(MouseButton::Back, Modifiers::NONE);
    assert_eq!(set.match_mouse_binding(&back_binding), Some("nav.back"));

    let forward_binding = MouseBinding::new(MouseButton::Forward, Modifiers::NONE);
    assert_eq!(set.match_mouse_binding(&forward_binding), Some("nav.forward"));

    // Test that hotkey still works
    let alt_left = Hotkey::new(KeyCode::ArrowLeft, Modifiers::ALT);
    assert_eq!(set.match_hotkey(&alt_left), Some("nav.back"));

    // Test that unbound mouse buttons don't match
    let middle_binding = MouseBinding::new(MouseButton::Middle, Modifiers::NONE);
    assert_eq!(set.match_mouse_binding(&middle_binding), None);
}
