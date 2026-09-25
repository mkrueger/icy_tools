use eframe::egui::{self, Key, Modifiers};
use icy_engine_gui::egui::{appearance, shortcuts::keycaps};

use super::{
    app::{draft_title, AfterDiscard, Folder, MailApp, Modal},
    chrome::shortcut,
};

impl MailApp {
    pub fn modals(&mut self, context: &egui::Context) {
        if let Some(error) = self.error.clone() {
            let response = appearance::MessageBox::new("mail-error", appearance::MessageKind::Error, "Icy Mail", error)
                .copyable()
                .buttons([appearance::DialogButton::primary(appearance::labels::close(), ()).cancels()])
                .show(context);
            if response.action.is_some() || response.dismissed {
                self.error = None;
            }
            return;
        }
        let Some(modal) = &self.modal else {
            return;
        };
        match modal {
            Modal::Shortcuts => {
                if self.shortcuts(context) {
                    self.modal = None;
                }
            }
            Modal::PacketInfo => {
                if self.packet_info(context) {
                    self.modal = None;
                }
            }
            Modal::DeleteDraft(id) => {
                let id = *id;
                let title = self
                    .drafts
                    .as_ref()
                    .and_then(|store| store.drafts().iter().find(|draft| draft.id == id))
                    .map(draft_title)
                    .unwrap_or_default();
                let response = appearance::MessageBox::new(
                    "mail-delete-draft",
                    appearance::MessageKind::Question,
                    "Delete this draft?",
                    format!("\u{201c}{title}\u{201d} will be removed from the outbox. This cannot be undone."),
                )
                .buttons([
                    appearance::DialogButton::cancel("Keep", false),
                    appearance::DialogButton::destructive("Delete", true),
                ])
                .show(context);
                if let Some(delete) = response.action.or(response.dismissed.then_some(false)) {
                    self.modal = None;
                    if delete {
                        self.delete_draft(context, id);
                    }
                }
            }
            Modal::Discard(after) => {
                let after = *after;
                let message = if after == AfterDiscard::Quit {
                    "The message you are writing has not been saved. Close the window anyway?"
                } else {
                    "Your changes to this message will be lost."
                };
                let response = appearance::MessageBox::new("mail-discard", appearance::MessageKind::Warning, "Discard this message?", message)
                    .buttons([
                        appearance::DialogButton::cancel("Keep Editing", false),
                        appearance::DialogButton::destructive("Discard", true),
                    ])
                    .show(context);
                if let Some(discard) = response.action.or(response.dismissed.then_some(false)) {
                    self.modal = None;
                    if discard {
                        match after {
                            AfterDiscard::Close => self.composer = None,
                            AfterDiscard::Quit => self.discard_and_close(context),
                        }
                    }
                }
            }
            Modal::ExportProblems(problems) => {
                let list: String = problems.iter().map(|problem| format!("\u{2022} {problem}\n")).collect();
                let response = appearance::MessageBox::new(
                    "mail-export-problems",
                    appearance::MessageKind::Warning,
                    "Some messages need attention",
                    format!("Fix these messages in the outbox before exporting the reply packet:\n\n{}", list.trim_end()),
                )
                .buttons([appearance::DialogButton::primary("Show Outbox", ()).cancels()])
                .show(context);
                if response.action.is_some() || response.dismissed {
                    self.modal = None;
                    self.select_folder(Folder::Drafts);
                }
            }
        }
    }

    fn shortcuts(&self, context: &egui::Context) -> bool {
        let command = |key| shortcut(context, Modifiers::COMMAND, key);
        let command_shift = |key| shortcut(context, Modifiers::COMMAND | Modifiers::SHIFT, key);
        let mut groups: Vec<(&str, Vec<(String, &str)>)> = vec![
            (
                "Reading",
                vec![
                    ("\u{2191}+\u{2193}".into(), "Previous or next entry"),
                    ("Tab".into(), "Next pane"),
                    ("Enter".into(), "Open the selected folder or message"),
                    ("Space".into(), "Page down, then next unread message"),
                    ("N".into(), "Next unread message"),
                    ("M".into(), "Mark as read or unread"),
                    (shortcut(context, Modifiers::SHIFT, Key::C), "Mark the folder as read"),
                    (command(Key::F), "Search messages"),
                    (command(Key::T), "Switch between list and threads"),
                ],
            ),
            (
                "Writing",
                vec![
                    (command(Key::N), "New message"),
                    (command(Key::R), "Reply"),
                    (command(Key::L), "Forward"),
                    (command(Key::S), "Save the draft"),
                    ("Esc".into(), "Cancel writing"),
                    ("Del".into(), "Delete the selected draft"),
                    (command_shift(Key::E), "Export the reply packet"),
                ],
            ),
            ("Packets", vec![(command(Key::O), "Open a packet"), ("F5".into(), "Reload the packet")]),
            (
                "Windows",
                vec![
                    (command_shift(Key::N), "New window"),
                    (command(Key::W), "Close the window"),
                    ("F1".into(), "Keyboard shortcuts"),
                ],
            ),
        ];
        if self.composer.is_some() {
            groups[0] = (
                "Message editor",
                vec![
                    (command(Key::K), "Text color, recolors a selection"),
                    (command(Key::G), "Character table, pick with the keyboard"),
                    (command(Key::Q), "Quote panel"),
                    (command(Key::F), "Find text"),
                    ("F3".into(), "Find next"),
                    (command(Key::D), "Delete the line"),
                    (command(Key::Z), "Undo"),
                    (command(Key::Y), "Redo"),
                    (command(Key::A), "Select all, or quote the rest"),
                    ("Ins".into(), "Insert or overwrite"),
                    ("Esc".into(), "Close a panel or the selection"),
                ],
            );
            groups.remove(2);
        }
        let response = appearance::Dialog::new("mail-shortcuts")
            .title("Keyboard Shortcuts")
            .size(appearance::DialogSize::Medium)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    for (title, entries) in &groups {
                        appearance::compact_group(ui, title, |ui| {
                            for (keys, description) in entries {
                                ui.horizontal(|ui| {
                                    ui.allocate_ui_with_layout(egui::vec2(150.0, 22.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                        ui.set_min_width(150.0);
                                        ui.spacing_mut().item_spacing.x = 3.0;
                                        if keys.contains("\u{2191}") {
                                            keycaps(ui, "\u{2191}");
                                            keycaps(ui, "\u{2193}");
                                        } else {
                                            keycaps(ui, keys);
                                        }
                                    });
                                    ui.label(*description);
                                });
                            }
                        });
                    }
                });
                dialog.buttons([appearance::DialogButton::primary(appearance::labels::close(), ()).cancels()]);
            });
        response.action.is_some() || response.dismissed
    }

    fn packet_info(&self, context: &egui::Context) -> bool {
        let Some(package) = &self.reader.package else {
            return true;
        };
        let control = &package.control_file;
        let text = |value: &bstr::BString| value.to_string().trim().to_string();
        let response = appearance::Dialog::new("mail-packet-info")
            .title(text(&control.bbs_name))
            .subtitle("Packet information")
            .size(appearance::DialogSize::Medium)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    appearance::compact_group(ui, "Board", |ui| {
                        for (label, value) in [
                            ("Location", text(&control.bbs_city_and_state)),
                            ("Phone", text(&control.bbs_phone_number)),
                            ("Sysop", text(&control.bbs_sysop_name)),
                            ("BBS ID", text(&control.bbs_id)),
                        ] {
                            if !value.is_empty() {
                                appearance::value_row(ui, label, &value);
                            }
                        }
                    });
                    appearance::compact_group(ui, "Packet", |ui| {
                        appearance::value_row(ui, "User", &self.user_name());
                        appearance::value_row(ui, "Created", &text(&control.creation_time));
                        appearance::value_row(ui, "Messages", &package.message_count().to_string());
                        appearance::value_row(ui, "Unread", &self.counts.unread.to_string());
                        appearance::value_row(ui, "Conferences", &package.conferences().len().to_string());
                        appearance::value_row(ui, "Outbox", &self.draft_count().to_string());
                        if let Some(path) = &self.path {
                            appearance::value_row(ui, "File", &path.display().to_string());
                        }
                    });
                });
                dialog.buttons([appearance::DialogButton::primary(appearance::labels::close(), ()).cancels()]);
            });
        response.action.is_some() || response.dismissed
    }
}
