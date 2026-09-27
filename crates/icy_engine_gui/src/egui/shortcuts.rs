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

/// One row of the keyboard shortcut overview: keys such as `"Ctrl+Shift+N"`, the action and an
/// optional longer description.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShortcutEntry {
    pub keys: String,
    pub action: String,
    pub description: String,
}

impl ShortcutEntry {
    pub fn new(keys: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            keys: keys.into(),
            action: action.into(),
            description: String::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
}

/// A titled section of the keyboard shortcut overview.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShortcutGroup {
    pub title: String,
    pub entries: Vec<ShortcutEntry>,
}

impl ShortcutGroup {
    pub fn new(title: impl Into<String>, entries: Vec<ShortcutEntry>) -> Self {
        Self { title: title.into(), entries }
    }
}

/// Splits `"Ctrl+Shift+N"` into single keys; a trailing `"++"` means the literal plus key and
/// spaces separate alternative chords.
pub fn key_parts(shortcut: &str) -> Vec<String> {
    shortcut
        .split(' ')
        .filter(|chunk| !chunk.is_empty())
        .flat_map(|chunk| {
            if chunk == "+" {
                return vec!["+".to_string()];
            }
            let literal_plus = chunk.ends_with("++");
            let keys = if literal_plus { &chunk[..chunk.len() - 1] } else { chunk };
            let mut parts: Vec<String> = keys.split('+').filter(|part| !part.is_empty()).map(str::to_string).collect();
            if literal_plus {
                parts.push("+".to_string());
            }
            parts
        })
        .collect()
}

fn key_pill(ui: &mut ::egui::Ui, label: &str) {
    let accent = ui.visuals().selection.stroke.color;
    let bright = u32::from(accent.r()) + u32::from(accent.g()) + u32::from(accent.b()) > 380;
    let text = if bright {
        ::egui::Color32::from_rgb(18, 22, 28)
    } else {
        ::egui::Color32::WHITE
    };
    let font = ::egui::FontId::proportional(12.0);
    let width = ui.fonts_mut(|fonts| fonts.layout_no_wrap(label.to_owned(), font.clone(), text).size().x);
    let (rect, _) = ui.allocate_exact_size(::egui::vec2(width + 18.0, 24.0), ::egui::Sense::hover());
    ui.painter().rect_filled(rect, 8.0, accent);
    ui.painter().text(rect.center(), ::egui::Align2::CENTER_CENTER, label, font, text);
}

fn category_band(ui: &mut ::egui::Ui, name: &str) {
    ::egui::Frame::new()
        .fill(ui.visuals().widgets.inactive.bg_fill.gamma_multiply(0.6))
        .corner_radius(6)
        .inner_margin(::egui::Margin::symmetric(12, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(::egui::RichText::new(name).size(15.0).strong());
        });
}

fn shortcut_row(ui: &mut ::egui::Ui, entry: &ShortcutEntry, shaded: bool, compact: bool) {
    let fill = if shaded { ui.visuals().faint_bg_color } else { ::egui::Color32::TRANSPARENT };
    ::egui::Frame::new().fill(fill).inner_margin(::egui::Margin::symmetric(12, 5)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            let keys = if compact { 132.0 } else { 186.0 };
            ui.allocate_ui_with_layout(::egui::vec2(keys, 0.0), ::egui::Layout::left_to_right(::egui::Align::Min), |ui| {
                ui.set_min_width(keys);
                for (index, part) in key_parts(&entry.keys).iter().enumerate() {
                    if index > 0 {
                        ui.label("+");
                    }
                    key_pill(ui, part);
                }
            });
            let has_description = !compact && !entry.description.is_empty();
            let action = if has_description {
                (ui.available_width() * 0.40).min(150.0)
            } else {
                ui.available_width()
            };
            ui.allocate_ui_with_layout(::egui::vec2(action, 0.0), ::egui::Layout::top_down(::egui::Align::Min), |ui| {
                ui.set_min_width(action);
                ui.add(::egui::Label::new(&entry.action).wrap());
            });
            if has_description {
                ui.add(::egui::Label::new(::egui::RichText::new(&entry.description).color(ui.visuals().weak_text_color())).wrap());
            }
        });
    });
}

/// The keyboard shortcut overview shared by all Icy apps; returns true once it was closed.
pub fn shortcuts_dialog(context: &::egui::Context, title: &str, subtitle: &str, groups: &[ShortcutGroup]) -> bool {
    use super::appearance::{labels, Dialog, DialogButton, DialogSize};
    let response = Dialog::new("shortcuts")
        .title(title)
        .icon("\u{2328}")
        .subtitle(subtitle)
        .size(DialogSize::Width(700.0))
        .fixed_height(560.0)
        .show(context, |dialog| {
            let compact = dialog.ui().available_width() < 520.0;
            dialog.content(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for group in groups.iter().filter(|group| !group.entries.is_empty()) {
                    category_band(ui, &group.title);
                    for (index, entry) in group.entries.iter().enumerate() {
                        shortcut_row(ui, entry, index % 2 == 0, compact);
                    }
                    ui.add_space(6.0);
                }
            });
            dialog.buttons([DialogButton::primary(labels::close(), ()).cancels()]);
        });
    response.action.is_some() || response.dismissed
}

#[cfg(test)]
mod tests {
    use super::key_parts;

    #[test]
    fn key_parts_split_chords_and_keep_the_plus_key() {
        assert_eq!(key_parts("Ctrl+Shift+N"), ["Ctrl", "Shift", "N"]);
        assert_eq!(key_parts("Alt++"), ["Alt", "+"]);
        assert_eq!(key_parts("F1"), ["F1"]);
    }
}
