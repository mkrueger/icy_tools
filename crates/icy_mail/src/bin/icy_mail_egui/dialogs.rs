use eframe::egui::{self, Key, Modifiers};
use i18n_embed_fl::fl;
use icy_engine_gui::egui::{appearance, shortcuts::keycaps};
use icy_mail::LANGUAGE_LOADER;

use super::{
    app::{draft_title, AfterDiscard, Folder, MailApp, Modal},
    chrome::shortcut,
};

impl MailApp {
    pub fn modals(&mut self, context: &egui::Context) {
        if let Some(error) = self.error.clone() {
            let response = appearance::MessageBox::new(
                "mail-error",
                appearance::MessageKind::Error,
                fl!(LANGUAGE_LOADER, "dialog-mail-app-title"),
                error,
            )
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
            Modal::Settings => {
                if self.settings(context) {
                    self.modal = None;
                }
            }
            Modal::Taglines => {
                if self.taglines_dialog(context) {
                    self.modal = None;
                }
            }
            Modal::AddressBook => {
                if self.address_dialog(context) {
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
                    fl!(LANGUAGE_LOADER, "dialog-mail-delete-draft-title"),
                    fl!(LANGUAGE_LOADER, "dialog-mail-delete-draft-message", title = title.as_str()),
                )
                .buttons([
                    appearance::DialogButton::cancel(fl!(LANGUAGE_LOADER, "dialog-mail-delete-draft-keep"), false),
                    appearance::DialogButton::destructive(fl!(LANGUAGE_LOADER, "dialog-mail-delete-draft-delete"), true),
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
                    fl!(LANGUAGE_LOADER, "dialog-mail-discard-quit-message")
                } else {
                    fl!(LANGUAGE_LOADER, "dialog-mail-discard-message")
                };
                let response = appearance::MessageBox::new(
                    "mail-discard",
                    appearance::MessageKind::Warning,
                    fl!(LANGUAGE_LOADER, "dialog-mail-discard-title"),
                    message,
                )
                .buttons([
                    appearance::DialogButton::cancel(fl!(LANGUAGE_LOADER, "dialog-mail-discard-keep-editing"), false),
                    appearance::DialogButton::destructive(fl!(LANGUAGE_LOADER, "dialog-mail-discard-discard"), true),
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
                    fl!(LANGUAGE_LOADER, "dialog-mail-export-problems-title"),
                    fl!(LANGUAGE_LOADER, "dialog-mail-export-problems-message", problems = list.trim_end()),
                )
                .buttons([appearance::DialogButton::primary(fl!(LANGUAGE_LOADER, "dialog-mail-show-outbox"), ()).cancels()])
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
        let mut groups: Vec<(String, Vec<(String, String)>)> = vec![
            (
                fl!(LANGUAGE_LOADER, "shortcuts-section-reading"),
                vec![
                    ("\u{2191}+\u{2193}".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-previous-next-entry")),
                    ("Tab".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-next-pane")),
                    ("Enter".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-open-selected-folder-message")),
                    ("Space".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-page-down-next-unread")),
                    ("N".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-next-unread")),
                    ("M".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-mark-read-unread")),
                    (
                        shortcut(context, Modifiers::SHIFT, Key::C),
                        fl!(LANGUAGE_LOADER, "shortcuts-reading-mark-folder-read"),
                    ),
                    (command(Key::F), fl!(LANGUAGE_LOADER, "shortcuts-reading-search-messages")),
                    (command(Key::T), fl!(LANGUAGE_LOADER, "shortcuts-reading-switch-list-threads")),
                    ("T".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-save-message-tagline")),
                    ("A".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-address-book")),
                    (
                        shortcut(context, Modifiers::SHIFT, Key::A),
                        fl!(LANGUAGE_LOADER, "shortcuts-reading-add-author-address-book"),
                    ),
                ],
            ),
            (
                fl!(LANGUAGE_LOADER, "shortcuts-section-writing"),
                vec![
                    (command(Key::N), fl!(LANGUAGE_LOADER, "shortcuts-writing-new-message")),
                    (command(Key::R), fl!(LANGUAGE_LOADER, "shortcuts-writing-reply")),
                    (command(Key::L), fl!(LANGUAGE_LOADER, "shortcuts-writing-forward")),
                    (command(Key::S), fl!(LANGUAGE_LOADER, "shortcuts-writing-save-draft")),
                    (command(Key::B), fl!(LANGUAGE_LOADER, "shortcuts-writing-pick-recipient-address-book")),
                    (command(Key::T), fl!(LANGUAGE_LOADER, "shortcuts-writing-choose-tagline")),
                    (command_shift(Key::T), fl!(LANGUAGE_LOADER, "shortcuts-writing-edit-tagline-list")),
                    ("Esc".into(), fl!(LANGUAGE_LOADER, "shortcuts-writing-cancel-writing")),
                    ("Del".into(), fl!(LANGUAGE_LOADER, "shortcuts-writing-delete-selected-draft")),
                    (command_shift(Key::E), fl!(LANGUAGE_LOADER, "shortcuts-writing-export-reply-packet")),
                ],
            ),
            (
                fl!(LANGUAGE_LOADER, "shortcuts-section-packets"),
                vec![
                    (command(Key::O), fl!(LANGUAGE_LOADER, "shortcuts-packets-open-packet")),
                    ("F5".into(), fl!(LANGUAGE_LOADER, "shortcuts-packets-reload-packet")),
                ],
            ),
            (
                fl!(LANGUAGE_LOADER, "shortcuts-section-windows"),
                vec![
                    (command_shift(Key::N), fl!(LANGUAGE_LOADER, "shortcuts-windows-new-window")),
                    (command(Key::W), fl!(LANGUAGE_LOADER, "shortcuts-windows-close-window")),
                    (command(Key::Comma), fl!(LANGUAGE_LOADER, "shortcuts-windows-settings")),
                    ("F1".into(), fl!(LANGUAGE_LOADER, "shortcuts-windows-keyboard-shortcuts")),
                ],
            ),
        ];
        if self.composer.is_some() {
            groups[0] = (
                fl!(LANGUAGE_LOADER, "shortcuts-section-message-editor"),
                vec![
                    (command(Key::K), fl!(LANGUAGE_LOADER, "shortcuts-editor-text-color")),
                    (command(Key::G), fl!(LANGUAGE_LOADER, "shortcuts-editor-character-table")),
                    (command(Key::Q), fl!(LANGUAGE_LOADER, "shortcuts-editor-quote-panel")),
                    (command(Key::F), fl!(LANGUAGE_LOADER, "shortcuts-editor-find-text")),
                    ("F3".into(), fl!(LANGUAGE_LOADER, "shortcuts-editor-find-next")),
                    (command(Key::D), fl!(LANGUAGE_LOADER, "shortcuts-editor-delete-line")),
                    (command(Key::Z), fl!(LANGUAGE_LOADER, "shortcuts-editor-undo")),
                    (command(Key::Y), fl!(LANGUAGE_LOADER, "shortcuts-editor-redo")),
                    (command(Key::A), fl!(LANGUAGE_LOADER, "shortcuts-editor-select-all-quote-rest")),
                    ("Ins".into(), fl!(LANGUAGE_LOADER, "shortcuts-editor-insert-overwrite")),
                    ("Esc".into(), fl!(LANGUAGE_LOADER, "shortcuts-editor-close-panel-selection")),
                ],
            );
            groups.remove(2);
        }
        let response = appearance::Dialog::new("mail-shortcuts")
            .title(fl!(LANGUAGE_LOADER, "shortcuts-title"))
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
                                    ui.label(description.as_str());
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
            .subtitle(fl!(LANGUAGE_LOADER, "dialog-mail-packet-info-subtitle"))
            .size(appearance::DialogSize::Medium)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    appearance::compact_group(ui, &fl!(LANGUAGE_LOADER, "dialog-mail-packet-board"), |ui| {
                        for (label, value) in [
                            (fl!(LANGUAGE_LOADER, "dialog-mail-packet-location"), text(&control.bbs_city_and_state)),
                            (fl!(LANGUAGE_LOADER, "dialog-mail-packet-phone"), text(&control.bbs_phone_number)),
                            (fl!(LANGUAGE_LOADER, "dialog-mail-packet-sysop"), text(&control.bbs_sysop_name)),
                            (fl!(LANGUAGE_LOADER, "dialog-mail-packet-bbs-id"), text(&control.bbs_id)),
                        ] {
                            if !value.is_empty() {
                                appearance::value_row(ui, &label, &value);
                            }
                        }
                    });
                    appearance::compact_group(ui, &fl!(LANGUAGE_LOADER, "dialog-mail-packet-packet"), |ui| {
                        appearance::value_row(ui, &fl!(LANGUAGE_LOADER, "dialog-mail-packet-user"), &self.user_name());
                        appearance::value_row(ui, &fl!(LANGUAGE_LOADER, "dialog-mail-packet-created"), &text(&control.creation_time));
                        appearance::value_row(ui, &fl!(LANGUAGE_LOADER, "dialog-mail-packet-messages"), &package.message_count().to_string());
                        appearance::value_row(ui, &fl!(LANGUAGE_LOADER, "dialog-mail-packet-unread"), &self.counts.unread.to_string());
                        appearance::value_row(
                            ui,
                            &fl!(LANGUAGE_LOADER, "dialog-mail-packet-conferences"),
                            &package.conferences().len().to_string(),
                        );
                        appearance::value_row(ui, &fl!(LANGUAGE_LOADER, "dialog-mail-packet-outbox"), &self.draft_count().to_string());
                        if let Some(path) = &self.path {
                            appearance::value_row(ui, &fl!(LANGUAGE_LOADER, "dialog-mail-packet-file"), &path.display().to_string());
                        }
                    });
                });
                dialog.buttons([appearance::DialogButton::primary(appearance::labels::close(), ()).cancels()]);
            });
        response.action.is_some() || response.dismissed
    }
}
