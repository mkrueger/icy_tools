use eframe::egui;
use icy_engine::{AutoWrapMode, IceMode, OriginMode, Screen, ScreenMode, TerminalResolution};
use icy_net::telnet::TerminalEmulation;
use icy_parser_core::{BaudEmulation, MusicOption};
use icy_term::{Address, TerminalCommand};

use super::appearance;

pub struct Dialog {
    groups: Vec<(String, Vec<(String, String, String)>)>,
    profile: Address,
    original: (TerminalEmulation, ScreenMode, MusicOption),
    kitty_flags_description: String,
    pub closed: bool,
}

fn yes_no(value: bool) -> String {
    if value {
        tr!("terminal-info-dialog-yes")
    } else {
        tr!("terminal-info-dialog-no")
    }
}

impl Dialog {
    pub fn new(screen: &dyn Screen, mut profile: Address, terminal: TerminalEmulation, baud: BaudEmulation) -> Self {
        let state = screen.terminal_state();
        let caret = screen.caret();
        let mouse = &state.mouse_state;
        profile.terminal_type = terminal;
        profile.screen_mode = icy_term::normalize_screen_mode(terminal, profile.screen_mode);
        profile.ice_mode = screen.ice_mode() == IceMode::Ice;
        profile.baud_emulation = baud;
        profile.set_lf_expand(state.lf_expand);
        profile.mouse_reporting_enabled = mouse.mouse_tracking_enabled;
        let margins = if state.margins_top_bottom().is_none() && state.margins_left_right().is_none() {
            tr!("terminal-info-dialog-not-set")
        } else {
            [state.margins_top_bottom(), state.margins_left_right()]
                .iter()
                .map(|range| range.map_or_else(|| tr!("terminal-info-dialog-not-set"), |(start, end)| format!("{start}..{end}")))
                .collect::<Vec<_>>()
                .join(" / ")
        };
        let groups = vec![
            (
                tr!("terminal-info-dialog-terminal-section"),
                vec![
                    (
                        tr!("terminal-info-dialog-resolution"),
                        format!(
                            "{}x{} ({}x{} px)",
                            screen.width(),
                            screen.height(),
                            screen.resolution().width,
                            screen.resolution().height
                        ),
                    ),
                    (
                        tr!("terminal-info-dialog-font-size"),
                        format!("{}x{}", screen.font_dimensions().width, screen.font_dimensions().height),
                    ),
                    (tr!("terminal-info-dialog-auto-wrap"), yes_no(state.auto_wrap_mode == AutoWrapMode::AutoWrap)),
                    (tr!("terminal-info-dialog-scroll-mode"), format!("{:?}", state.scroll_state)),
                    (tr!("terminal-info-dialog-margins"), margins),
                    (
                        tr!("egui-info-origin"),
                        format!(
                            "{} / {}",
                            yes_no(state.origin_mode == OriginMode::WithinMargins),
                            yes_no(state.dec_left_right_margins())
                        ),
                    ),
                    (tr!("terminal-info-dialog-mouse-tracking"), format!("{:?}", mouse.mouse_mode)),
                    (tr!("terminal-info-dialog-inverse-colors"), yes_no(state.inverse_video)),
                    (tr!("terminal-info-dialog-use-ice-colors"), yes_no(screen.ice_mode() == IceMode::Ice)),
                ],
            ),
            (
                tr!("terminal-info-dialog-caret-section"),
                vec![
                    (
                        tr!("terminal-info-dialog-caret-position"),
                        format!("X: {}, Y: {}", caret.position().x, caret.position().y),
                    ),
                    (tr!("terminal-info-dialog-caret-shape"), format!("{:?}", caret.shape)),
                    (tr!("terminal-info-dialog-caret-visible"), yes_no(caret.visible)),
                    (tr!("terminal-info-dialog-caret-blinking"), yes_no(caret.blinking)),
                    (
                        tr!("terminal-info-dialog-input-mode"),
                        if caret.insert_mode {
                            tr!("egui-info-insert")
                        } else {
                            tr!("egui-info-overwrite")
                        },
                    ),
                ],
            ),
            (
                tr!("egui-info-input-protocols"),
                vec![
                    (tr!("egui-mouse-reporting"), yes_no(mouse.mouse_tracking_enabled)),
                    (tr!("egui-info-mouse-encoding"), format!("{:?}", mouse.extended_mode)),
                    (
                        tr!("egui-info-focus-scroll"),
                        format!("{} / {}", yes_no(mouse.focus_out_event_enabled), yes_no(mouse.alternate_scroll_enabled)),
                    ),
                    (
                        tr!("egui-info-kitty"),
                        format!("0x{:02X} ({})", state.kitty_keyboard.flags(), state.kitty_keyboard.depth()),
                    ),
                    ("Bracketed paste".into(), yes_no(state.bracketed_paste_mode)),
                    (tr!("egui-info-lf"), if state.lf_expand { "CR+LF" } else { "LF" }.into()),
                ],
            ),
            (
                tr!("egui-info-graphics-protocols"),
                vec![
                    (
                        tr!("egui-info-sixel-position"),
                        if state.sixel_at_cursor {
                            tr!("egui-cursor")
                        } else {
                            tr!("egui-info-screen-origin")
                        },
                    ),
                    (
                        tr!("egui-info-sixel-palette"),
                        if state.sixel_shared_palette {
                            tr!("egui-info-shared")
                        } else {
                            tr!("egui-info-private")
                        },
                    ),
                    ("JPEG XL / Audio APC".into(), format!("{} / {}", yes_no(true), yes_no(true))),
                    ("Opus".into(), yes_no(icy_engine_gui::music::audio_apc::supports_format(32, 100))),
                    (
                        tr!("egui-info-audio-channels"),
                        format!("0x{:04X}", icy_engine_gui::music::audio_apc::status().active_mask()),
                    ),
                    (tr!("egui-info-sync-output"), yes_no(state.synchronized_output())),
                ],
            ),
        ];
        // A trailing "(...)" is supplementary detail and stays visually subordinate.
        let groups = groups
            .into_iter()
            .map(|(title, fields): (String, Vec<(String, String)>)| {
                let fields = fields
                    .into_iter()
                    .map(|(label, value)| match value.split_once(" (") {
                        Some((head, note)) if note.ends_with(')') => (label, head.to_owned(), format!("({note}")),
                        _ => (label, value, String::new()),
                    })
                    .collect();
                (title, fields)
            })
            .collect();
        Self {
            groups,
            kitty_flags_description: kitty_flags_description(state.kitty_keyboard.flags()),
            original: (profile.terminal_type, profile.screen_mode, profile.ansi_music),
            profile,
            closed: false,
        }
    }

    pub fn show(&mut self, context: &egui::Context, scrollback: usize) -> Option<TerminalCommand> {
        let mut command = None;
        egui::Modal::new(egui::Id::new("terminal-information"))
            .frame(appearance::dialog_frame(context))
            .show(context, |ui| {
                let height = (context.content_rect().height() - 64.0).clamp(140.0, 680.0);
                ui.set_width((context.content_rect().width() - 48.0).clamp(220.0, 740.0));
                ui.set_min_height(height);
                self.closed |= appearance::dialog_header(ui, &tr!("terminal-menu-info"));
                egui::ScrollArea::vertical()
                    .max_height((height - 104.0).max(0.0))
                    .auto_shrink([false, false])
                    .min_scrolled_height(0.0)
                    .show(ui, |ui| {
                        for pair in self.groups.chunks(2) {
                            if ui.available_width() >= 600.0 {
                                let scope = ui.scope(|ui| {
                                    ui.columns(2, |columns| {
                                        for (column, (title, fields)) in columns.iter_mut().zip(pair) {
                                            self.group(column, title, fields);
                                        }
                                    });
                                });
                                if pair.len() == 2 {
                                    let rect = scope.response.rect;
                                    ui.painter().line_segment(
                                        [egui::pos2(rect.center().x, rect.top()), egui::pos2(rect.center().x, rect.bottom())],
                                        ui.visuals().widgets.noninteractive.bg_stroke,
                                    );
                                }
                            } else {
                                for (title, fields) in pair {
                                    self.group(ui, title, fields);
                                }
                            }
                            ui.add_space(8.0);
                        }
                        ui.separator();
                        appearance::section(ui, &tr!("settings-heading"));
                        ui.group(|ui| self.settings(ui));
                    });
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui.button(tr!("terminal-menu-copy")).clicked() {
                        context.copy_text(
                            self.groups
                                .iter()
                                .map(|(title, fields)| {
                                    format!(
                                        "{title}\n{}",
                                        fields
                                            .iter()
                                            .map(|(name, value, note)| format!("{name}: {value} {note}").trim_end().to_owned())
                                            .collect::<Vec<_>>()
                                            .join("\n")
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n\n"),
                        );
                    }
                    let changed = self.original != (self.profile.terminal_type, self.profile.screen_mode, self.profile.ansi_music);
                    if ui
                        .add_enabled(changed, appearance::primary_button(tr!("terminal-info-dialog-apply-button")))
                        .clicked()
                    {
                        command = Some(TerminalCommand::SetTerminalProfile {
                            profile: self.profile.clone(),
                            scrollback,
                        });
                        self.closed = true;
                    }
                    self.closed |= ui.button(tr!("egui-close")).clicked();
                });
            });
        command
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(ui.available_width());
        appearance::form_row(ui, &tr!("egui-terminal-emulation"), |ui| {
            let previous = self.profile.terminal_type;
            egui::ComboBox::from_id_salt("info-emulation")
                .selected_text(icy_term::fmt_terminal_emulation(&previous))
                .show_ui(ui, |ui| {
                    for value in icy_term::ALL_TERMINALS {
                        ui.selectable_value(&mut self.profile.terminal_type, value, icy_term::fmt_terminal_emulation(&value));
                    }
                });
            if previous != self.profile.terminal_type {
                self.profile.screen_mode = icy_term::normalize_screen_mode(self.profile.terminal_type, ScreenMode::default());
            }
        });
        appearance::form_row(ui, &tr!("dialing_directory-screen_mode"), |ui| {
            egui::ComboBox::from_id_salt("info-screen-mode")
                .selected_text(self.profile.screen_mode.to_string())
                .show_ui(ui, |ui| {
                    let mut modes = match self.profile.screen_mode {
                        ScreenMode::Vga(_, _) => icy_engine::VGA_MODES.to_vec(),
                        ScreenMode::Unicode(_, _) => icy_engine::VGA_MODES
                            .iter()
                            .map(|mode| icy_term::normalize_screen_mode(TerminalEmulation::Utf8Ansi, *mode))
                            .collect(),
                        ScreenMode::Atascii(_) => vec![ScreenMode::Atascii(40), ScreenMode::Atascii(80)],
                        ScreenMode::AtariST(_, igs) => [TerminalResolution::Low, TerminalResolution::Medium, TerminalResolution::High]
                            .map(|resolution| ScreenMode::AtariST(resolution, igs))
                            .to_vec(),
                        mode => vec![mode],
                    };
                    if !modes.contains(&self.profile.screen_mode) {
                        modes.push(self.profile.screen_mode);
                    }
                    for value in modes {
                        ui.selectable_value(&mut self.profile.screen_mode, value, value.to_string());
                    }
                });
        });
        if self.profile.screen_mode.is_custom_vga() {
            if let ScreenMode::Vga(width, height) | ScreenMode::Unicode(width, height) = &mut self.profile.screen_mode {
                appearance::form_row(ui, &tr!("egui-screen-size"), |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.add(egui::DragValue::new(width).range(1..=500).prefix(format!("{}: ", tr!("egui-columns"))));
                        ui.add(egui::DragValue::new(height).range(1..=200).prefix(format!("{}: ", tr!("egui-rows"))));
                    });
                });
            }
        }
        if matches!(self.profile.terminal_type, TerminalEmulation::Ansi | TerminalEmulation::Utf8Ansi) {
            appearance::form_row(ui, &tr!("egui-ansi-music"), |ui| {
                egui::ComboBox::from_id_salt("info-music")
                    .selected_text(self.profile.ansi_music.to_string())
                    .show_ui(ui, |ui| {
                        for value in [MusicOption::Off, MusicOption::Banana, MusicOption::Conflicting, MusicOption::Both] {
                            ui.selectable_value(&mut self.profile.ansi_music, value, value.to_string());
                        }
                    });
            });
        }
    }

    fn group(&self, ui: &mut egui::Ui, title: &str, fields: &[(String, String, String)]) {
        appearance::section(ui, title);
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            for (name, value, note) in fields {
                let descriptions = if name == &tr!("terminal-info-dialog-mouse-tracking") {
                    vec![
                        ("Off", tr!("terminal-info-dialog-mouse-mode-tooltip-off")),
                        ("X10", tr!("terminal-info-dialog-mouse-mode-tooltip-x10")),
                        ("VT200", tr!("terminal-info-dialog-mouse-mode-tooltip-vt200")),
                        ("VT200Highlight", tr!("terminal-info-dialog-mouse-mode-tooltip-vt200highlight")),
                        ("ButtonEvent", tr!("terminal-info-dialog-mouse-mode-tooltip-btnevent")),
                        ("AnyEvent", tr!("terminal-info-dialog-mouse-mode-tooltip-anyevent")),
                    ]
                } else if name == &tr!("terminal-info-dialog-caret-shape") {
                    vec![
                        ("Block", tr!("terminal-info-dialog-shape-tooltip-block")),
                        ("Underline", tr!("terminal-info-dialog-shape-tooltip-underline")),
                        ("Bar", tr!("terminal-info-dialog-shape-tooltip-bar")),
                    ]
                } else {
                    Vec::new()
                };
                let response = ui.scope(|ui| appearance::value_row_with_note(ui, name, value, note)).response;
                if !descriptions.is_empty() || name == &tr!("egui-info-kitty") {
                    ui.interact(response.rect, response.id.with("details"), egui::Sense::hover()).on_hover_ui(|ui| {
                        ui.set_max_width((ui.ctx().content_rect().width() - 48.0).min(420.0));
                        ui.strong(name);
                        ui.separator();
                        if descriptions.is_empty() {
                            ui.add(egui::Label::new(&self.kitty_flags_description).wrap());
                        }
                        for (mode, description) in descriptions {
                            let active = value.eq_ignore_ascii_case(mode);
                            let color = if active { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
                            ui.horizontal_top(|ui| {
                                ui.allocate_ui_with_layout(egui::vec2(120.0, 18.0), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                                    ui.set_min_width(120.0);
                                    ui.label(egui::RichText::new(mode).monospace().color(color));
                                });
                                ui.add(egui::Label::new(egui::RichText::new(description).color(color)).wrap());
                            });
                        }
                    });
                }
            }
        });
    }
}

fn kitty_flags_description(flags: u8) -> String {
    use icy_engine::KittyKeyboardState;
    let active: Vec<_> = [
        (KittyKeyboardState::DISAMBIGUATE, "Disambiguate"),
        (KittyKeyboardState::REPORT_EVENT_TYPES, "EventTypes"),
        (KittyKeyboardState::REPORT_ALTERNATE_KEYS, "AlternateKeys"),
        (KittyKeyboardState::REPORT_ALL_KEYS, "AllKeys"),
        (KittyKeyboardState::REPORT_ASSOCIATED_TEXT, "AssociatedText"),
    ]
    .into_iter()
    .filter_map(|(bit, name)| (flags & bit != 0).then_some(name))
    .collect();
    if active.is_empty() {
        tr!("terminal-info-dialog-not-set")
    } else {
        active.join(" + ")
    }
}
