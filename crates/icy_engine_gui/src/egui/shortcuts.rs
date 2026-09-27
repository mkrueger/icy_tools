use crate::commands::{CommandDef, KeyCode};

pub fn key(code: KeyCode) -> Option<::egui::Key> {
    match code {
        KeyCode::BracketLeft => Some(::egui::Key::OpenBracket),
        KeyCode::BracketRight => Some(::egui::Key::CloseBracket),
        _ => {
            let name = format!("{code:?}");
            ::egui::Key::from_name(name.strip_prefix("Num").unwrap_or(&name))
        }
    }
}

pub fn consume(context: &::egui::Context, command: &CommandDef) -> bool {
    context.input_mut(|input| {
        let mut hit = false;
        input.events.retain(|event| {
            let matched = matches!(event, ::egui::Event::Key { key: pressed_key, modifiers, pressed: true, .. }
                if command.active_hotkeys().iter().any(|hotkey| key(hotkey.key) == Some(*pressed_key)
                    && modifiers.alt == hotkey.modifiers.alt && modifiers.shift == hotkey.modifiers.shift
                    && modifiers.ctrl == hotkey.modifiers.ctrl && modifiers.mac_cmd == hotkey.modifiers.cmd));
            hit |= matched;
            !matched
        });
        hit
    })
}

pub fn keycaps(ui: &mut ::egui::Ui, shortcut: &str) {
    for part in shortcut.split('+').filter(|part| !part.is_empty()) {
        ::egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .corner_radius(3)
            .inner_margin(::egui::Margin::symmetric(5, 2))
            .show(ui, |ui| {
                ui.small(part);
            });
    }
    if shortcut.ends_with("++") {
        ::egui::Frame::new().fill(ui.visuals().faint_bg_color).inner_margin(3).show(ui, |ui| {
            ui.small("+");
        });
    }
}
