//! Keyboard shortcuts driven by the shared command table both frontends use.

use eframe::egui;
use icy_engine_gui::commands::{CommandDef, CommandSet, KeyCode};
use std::sync::LazyLock;

/// The merged set, not the individual statics: `icy_term` overrides some shared
/// defaults (for example Copy becomes Ctrl+Shift+C) only in the merged table.
static COMMANDS: LazyLock<CommandSet> = LazyLock::new(icy_term::commands::create_icy_term_commands);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    DialingDirectory,
    Hangup,
    Serial,
    Upload,
    Download,
    SendLogin,
    SendUser,
    SendPassword,
    ClearScreen,
    Scrollback,
    Find,
    ToggleMouse,
    Capture,
    ExportScreen,
    RunScript,
    Settings,
    Quit,
    About,
    Help,
    Fullscreen,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ZoomFit,
    NewWindow,
    CloseWindow,
    Copy,
    Paste,
}

/// Ordered most specific first so that shared prefixes cannot shadow each other.
fn bindings() -> Vec<(Action, &'static str)> {
    vec![
        (Action::Copy, "edit.copy"),
        (Action::Paste, "edit.paste"),
        (Action::NewWindow, "window.new"),
        (Action::CloseWindow, "window.close"),
        (Action::DialingDirectory, "connection.dialing_directory"),
        (Action::Hangup, "connection.hangup"),
        (Action::Serial, "connection.serial"),
        (Action::Upload, "transfer.upload"),
        (Action::Download, "transfer.download"),
        (Action::SendLogin, "login.send_all"),
        (Action::SendUser, "login.send_user"),
        (Action::SendPassword, "login.send_password"),
        (Action::ClearScreen, "terminal.clear"),
        (Action::Scrollback, "terminal.scrollback"),
        (Action::Find, "terminal.find"),
        (Action::ToggleMouse, "terminal.toggle_mouse"),
        (Action::Capture, "capture.start"),
        (Action::ExportScreen, "capture.export"),
        (Action::RunScript, "script.run"),
        (Action::Settings, "app.settings"),
        (Action::Quit, "app.quit"),
        (Action::About, "app.about"),
        (Action::Help, "help.show"),
        (Action::Fullscreen, "view.fullscreen"),
        (Action::ZoomIn, "view.zoom_in"),
        (Action::ZoomOut, "view.zoom_out"),
        (Action::ZoomReset, "view.zoom_reset"),
        (Action::ZoomFit, "view.zoom_fit"),
    ]
}

pub fn command(action: Action) -> &'static CommandDef {
    let id = bindings().into_iter().find(|(candidate, _)| *candidate == action).map(|(_, id)| id).unwrap();
    COMMANDS.get(id).unwrap_or_else(|| panic!("{id} missing from the command table"))
}

pub fn shortcut(action: Action) -> String {
    command(action).primary_hotkey_display().unwrap_or_default()
}

/// Shortcuts that stay available while the terminal owns the keyboard.
/// Mirrors the legacy rule: Alt combinations and help are application level,
/// everything else may be claimed by the terminal key maps first.
fn works_while_connected(action: Action) -> bool {
    if matches!(action, Action::CloseWindow) {
        return false;
    }
    if matches!(action, Action::Help) {
        return true;
    }
    command(action)
        .active_hotkeys()
        .iter()
        .all(|hotkey| hotkey.modifiers.alt || hotkey.modifiers.cmd)
}

fn key(code: KeyCode) -> Option<egui::Key> {
    use egui::Key;
    Some(match code {
        KeyCode::A => Key::A,
        KeyCode::B => Key::B,
        KeyCode::C => Key::C,
        KeyCode::D => Key::D,
        KeyCode::E => Key::E,
        KeyCode::F => Key::F,
        KeyCode::G => Key::G,
        KeyCode::H => Key::H,
        KeyCode::I => Key::I,
        KeyCode::J => Key::J,
        KeyCode::K => Key::K,
        KeyCode::L => Key::L,
        KeyCode::M => Key::M,
        KeyCode::N => Key::N,
        KeyCode::O => Key::O,
        KeyCode::P => Key::P,
        KeyCode::Q => Key::Q,
        KeyCode::R => Key::R,
        KeyCode::S => Key::S,
        KeyCode::T => Key::T,
        KeyCode::U => Key::U,
        KeyCode::V => Key::V,
        KeyCode::W => Key::W,
        KeyCode::X => Key::X,
        KeyCode::Y => Key::Y,
        KeyCode::Z => Key::Z,
        KeyCode::Num0 => Key::Num0,
        KeyCode::Num1 => Key::Num1,
        KeyCode::Num2 => Key::Num2,
        KeyCode::Num3 => Key::Num3,
        KeyCode::Num4 => Key::Num4,
        KeyCode::Num5 => Key::Num5,
        KeyCode::Num6 => Key::Num6,
        KeyCode::Num7 => Key::Num7,
        KeyCode::Num8 => Key::Num8,
        KeyCode::Num9 => Key::Num9,
        KeyCode::F1 => Key::F1,
        KeyCode::F2 => Key::F2,
        KeyCode::F3 => Key::F3,
        KeyCode::F4 => Key::F4,
        KeyCode::F5 => Key::F5,
        KeyCode::F6 => Key::F6,
        KeyCode::F7 => Key::F7,
        KeyCode::F8 => Key::F8,
        KeyCode::F9 => Key::F9,
        KeyCode::F10 => Key::F10,
        KeyCode::F11 => Key::F11,
        KeyCode::F12 => Key::F12,
        KeyCode::Escape => Key::Escape,
        KeyCode::Tab => Key::Tab,
        KeyCode::Space => Key::Space,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter => Key::Enter,
        KeyCode::Delete => Key::Delete,
        KeyCode::Insert => Key::Insert,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::ArrowUp => Key::ArrowUp,
        KeyCode::ArrowDown => Key::ArrowDown,
        KeyCode::ArrowLeft => Key::ArrowLeft,
        KeyCode::ArrowRight => Key::ArrowRight,
        KeyCode::Plus => Key::Plus,
        KeyCode::Minus => Key::Minus,
        KeyCode::Equals => Key::Equals,
        KeyCode::BracketLeft => Key::OpenBracket,
        KeyCode::BracketRight => Key::CloseBracket,
        KeyCode::Backslash => Key::Backslash,
        KeyCode::Semicolon => Key::Semicolon,
        KeyCode::Quote => Key::Quote,
        KeyCode::Comma => Key::Comma,
        KeyCode::Period => Key::Period,
        KeyCode::Slash => Key::Slash,
        KeyCode::Backtick => Key::Backtick,
    })
}

/// egui's own matcher ignores additional Shift/Alt, which would let Ctrl+Shift+C
/// trigger a plain Ctrl+C binding, so modifiers are compared exactly here.
fn consume(context: &egui::Context, command: &CommandDef) -> bool {
    let wanted: Vec<_> = command
        .active_hotkeys()
        .iter()
        .filter_map(|hotkey| key(hotkey.key).map(|key| (key, hotkey.modifiers)))
        .collect();
    if wanted.is_empty() {
        return false;
    }
    context.input_mut(|input| {
        let mut hit = false;
        input.events.retain(|event| {
            let matched = matches!(event, egui::Event::Key { key, modifiers, pressed: true, .. }
            if wanted.iter().any(|(wanted_key, wanted_modifiers)| {
                *key == *wanted_key
                    && modifiers.alt == wanted_modifiers.alt
                    && modifiers.shift == wanted_modifiers.shift
                    && modifiers.ctrl == wanted_modifiers.ctrl
                    && modifiers.mac_cmd == wanted_modifiers.cmd
            }));
            hit |= matched;
            !matched
        });
        hit
    })
}

pub fn poll(context: &egui::Context, terminal_has_keyboard: bool) -> Vec<Action> {
    bindings()
        .into_iter()
        .map(|(action, _)| action)
        .filter(|action| !terminal_has_keyboard || works_while_connected(*action))
        .filter(|action| consume(context, command(*action)))
        .collect()
}

/// Shared commands are translated by `icy_engine_gui`, `icy_term` commands by the app itself.
fn translate(key: &str) -> Option<String> {
    if icy_term::LANGUAGE_LOADER.has(key) {
        return Some(icy_term::LANGUAGE_LOADER.get(key));
    }
    if icy_engine_gui::LANGUAGE_LOADER.has(key) {
        return Some(icy_engine_gui::LANGUAGE_LOADER.get(key));
    }
    None
}

/// Grouped for the shortcut overview, following the legacy help dialog.
pub fn help_entries() -> Vec<(String, Vec<(String, String)>)> {
    let mut categories: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for (action, _) in bindings() {
        let command = command(action);
        let Some(shortcut) = command.primary_hotkey_display() else {
            continue;
        };
        let name = if command.label_action.is_empty() {
            translate(&command.fluent_action_key()).unwrap_or_else(|| command.id.clone())
        } else {
            command.label_action.clone()
        };
        let category = command
            .fluent_category_key()
            .and_then(|key| translate(&key))
            .unwrap_or_else(|| tr!("egui-session"));
        match categories.iter_mut().find(|(existing, _)| *existing == category) {
            Some((_, entries)) => entries.push((name, shortcut)),
            None => categories.push((category, vec![(name, shortcut)])),
        }
    }
    categories
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn poll_with(events: Vec<egui::Event>, terminal_has_keyboard: bool) -> Vec<Action> {
        let context = egui::Context::default();
        let mut seen = Vec::new();
        let _ = context.run(egui::RawInput { events, ..Default::default() }, |context| {
            seen = poll(context, terminal_has_keyboard)
        });
        seen
    }

    #[test]
    fn shortcuts_match_the_shared_command_table() {
        let commands = icy_term::commands::create_icy_term_commands();
        for (action, id) in bindings() {
            let shared = commands.get(id).unwrap_or_else(|| panic!("{action:?} missing from the command table"));
            assert_eq!(
                shared.primary_hotkey_display(),
                command(action).primary_hotkey_display(),
                "{action:?} differs from the shared definition"
            );
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(shortcut(Action::DialingDirectory), "Alt+D");
            assert_eq!(shortcut(Action::Hangup), "Alt+H");
            assert_eq!(shortcut(Action::Upload), "Alt+PageUp");
            assert_eq!(shortcut(Action::Copy), "Ctrl+Shift+C");
            assert_eq!(shortcut(Action::Help), "F1");
        }
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn modifiers_are_matched_exactly_and_consumed() {
        assert_eq!(
            poll_with(vec![press(egui::Key::D, egui::Modifiers::ALT)], false),
            vec![Action::DialingDirectory]
        );
        assert!(poll_with(vec![press(egui::Key::D, egui::Modifiers::ALT | egui::Modifiers::SHIFT)], false).is_empty());
        assert!(poll_with(vec![press(egui::Key::D, egui::Modifiers::NONE)], false).is_empty());
        assert_eq!(
            poll_with(vec![press(egui::Key::C, egui::Modifiers::CTRL | egui::Modifiers::SHIFT)], false),
            vec![Action::Copy]
        );
        assert!(
            poll_with(vec![press(egui::Key::C, egui::Modifiers::CTRL)], false).is_empty(),
            "Ctrl+C must stay available as a terminal control character"
        );

        let context = egui::Context::default();
        let mut remaining = 1;
        let _ = context.run(
            egui::RawInput {
                events: vec![press(egui::Key::D, egui::Modifiers::ALT)],
                ..Default::default()
            },
            |context| {
                poll(context, false);
                remaining = context.input(|input| input.events.len());
            },
        );
        assert_eq!(remaining, 0, "a handled shortcut must not reach the terminal");
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn connected_terminal_keeps_control_combinations() {
        assert_eq!(poll_with(vec![press(egui::Key::H, egui::Modifiers::ALT)], true), vec![Action::Hangup]);
        assert_eq!(poll_with(vec![press(egui::Key::F1, egui::Modifiers::NONE)], true), vec![Action::Help]);
        assert!(
            poll_with(vec![press(egui::Key::W, egui::Modifiers::CTRL)], true).is_empty(),
            "Ctrl+W must reach the connection as 0x17"
        );
        assert_eq!(poll_with(vec![press(egui::Key::W, egui::Modifiers::CTRL)], false), vec![Action::CloseWindow]);
    }
}
