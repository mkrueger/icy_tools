use eframe::egui::{self, Key, Modifiers};
use i18n_embed_fl::fl;
use icy_engine_gui::egui::{
    appearance,
    shortcuts::{shortcuts_dialog, ShortcutEntry, ShortcutGroup},
};
use icy_mail::LANGUAGE_LOADER;

use super::{
    app::{draft_title, AfterDiscard, Folder, InfoDialog, MailApp, Modal, PackDialog, TransferDialog},
    chrome::shortcut,
    loading::BaseJob,
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
            Modal::About => {
                let open = self.about.as_mut().is_some_and(|about| about.show(context));
                if !open {
                    self.modal = None;
                }
            }
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
            Modal::Subscriptions => {
                if self.subscriptions_dialog(context) {
                    self.modal = None;
                }
            }
            Modal::Exported(path) => {
                let path = path.clone();
                let response = appearance::MessageBox::new(
                    "mail-exported",
                    appearance::MessageKind::Info,
                    fl!(LANGUAGE_LOADER, "dialog-mail-exported-title"),
                    fl!(LANGUAGE_LOADER, "dialog-mail-exported-message", path = path.display().to_string()),
                )
                .copyable()
                .buttons([
                    appearance::DialogButton::cancel(appearance::labels::close(), false).cancels(),
                    appearance::DialogButton::primary(fl!(LANGUAGE_LOADER, "dialog-mail-open-folder"), true),
                ])
                .show(context);
                if let Some(open_folder) = response.action.or(response.dismissed.then_some(false)) {
                    self.modal = None;
                    if open_folder {
                        let folder = path
                            .parent()
                            .filter(|parent| !parent.as_os_str().is_empty())
                            .unwrap_or_else(|| std::path::Path::new("."));
                        if let Err(error) = open::that(folder) {
                            self.error = Some(fl!(
                                LANGUAGE_LOADER,
                                "dialog-mail-open-folder-error",
                                path = folder.display().to_string(),
                                error = error.to_string()
                            ));
                        }
                    }
                }
            }
            Modal::PackBase(state) => {
                let state = state.clone();
                self.modal = self.pack_dialog(context, state).map(Modal::PackBase);
            }
            Modal::Transfer(state) => {
                let state = state.clone();
                self.modal = self.transfer_dialog(context, state).map(Modal::Transfer);
            }
            Modal::MessageInfo(state) => {
                let state = state.clone();
                self.modal = self.message_info_dialog(context, state).map(Modal::MessageInfo);
            }
            Modal::CheckReport { checks, repaired } => {
                let (checks, repaired) = (checks.clone(), *repaired);
                if self.check_report_dialog(context, &checks, repaired) {
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

    pub fn open_about(&mut self) {
        if self.about.is_none() {
            match icy_engine_gui::egui::about::AboutDialog::new(
                include_bytes!("../../../data/about.icy"),
                &icy_mail::VERSION,
                option_env!("ICY_BUILD_DATE").map(String::from),
            ) {
                Ok(about) => self.about = Some(about),
                Err(error) => {
                    self.error = Some(error);
                    return;
                }
            }
        }
        self.modal = Some(Modal::About);
    }

    fn shortcuts(&self, context: &egui::Context) -> bool {
        let command = |key| shortcut(context, Modifiers::COMMAND, key);
        let command_shift = |key| shortcut(context, Modifiers::COMMAND | Modifiers::SHIFT, key);
        let mut groups: Vec<(String, Vec<(String, String)>)> = vec![
            (
                fl!(LANGUAGE_LOADER, "shortcuts-section-reading"),
                vec![
                    ("\u{2191}+\u{2193}".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-previous-next-entry")),
                    ("\u{2190}+\u{2192}".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-collapse-expand-thread")),
                    (
                        format!(
                            "{}+{}",
                            shortcut(context, Modifiers::SHIFT, Key::ArrowLeft),
                            shortcut(context, Modifiers::SHIFT, Key::ArrowRight)
                        ),
                        fl!(LANGUAGE_LOADER, "shortcuts-reading-collapse-expand-all-threads"),
                    ),
                    ("Tab".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-next-pane")),
                    ("Enter".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-open-selected-folder-message")),
                    ("Space".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-page-down-next-unread")),
                    ("N".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-next-unread")),
                    ("M".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-mark-read-unread")),
                    (
                        shortcut(context, Modifiers::SHIFT, Key::M),
                        fl!(LANGUAGE_LOADER, "shortcuts-reading-mark-thread-read"),
                    ),
                    ("S".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-star")),
                    (
                        shortcut(context, Modifiers::SHIFT, Key::C),
                        fl!(LANGUAGE_LOADER, "shortcuts-reading-mark-folder-read"),
                    ),
                    (command(Key::F), fl!(LANGUAGE_LOADER, "shortcuts-reading-search-messages")),
                    (command(Key::T), fl!(LANGUAGE_LOADER, "shortcuts-reading-switch-list-threads")),
                    ("T".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-save-message-tagline")),
                    ("X".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-show-hidden-lines")),
                    ("A".into(), fl!(LANGUAGE_LOADER, "shortcuts-reading-address-book")),
                    (
                        shortcut(context, Modifiers::SHIFT, Key::A),
                        fl!(LANGUAGE_LOADER, "shortcuts-reading-add-author-address-book"),
                    ),
                ],
            ),
            (fl!(LANGUAGE_LOADER, "shortcuts-section-writing"), {
                // A message base is written to on saving and has no outbox to delete from or export.
                let base = self.posts_to_base();
                let mut writing = vec![
                    (command(Key::N), fl!(LANGUAGE_LOADER, "shortcuts-writing-new-message")),
                    (command(Key::R), fl!(LANGUAGE_LOADER, "shortcuts-writing-reply")),
                    (command(Key::L), fl!(LANGUAGE_LOADER, "shortcuts-writing-forward")),
                    (
                        command(Key::S),
                        if base {
                            fl!(LANGUAGE_LOADER, "shortcuts-writing-save-message")
                        } else {
                            fl!(LANGUAGE_LOADER, "shortcuts-writing-save-draft")
                        },
                    ),
                    (command(Key::B), fl!(LANGUAGE_LOADER, "shortcuts-writing-pick-recipient-address-book")),
                    (command(Key::T), fl!(LANGUAGE_LOADER, "shortcuts-writing-choose-tagline")),
                    (command_shift(Key::T), fl!(LANGUAGE_LOADER, "shortcuts-writing-edit-tagline-list")),
                    ("Esc".into(), fl!(LANGUAGE_LOADER, "shortcuts-writing-cancel-writing")),
                ];
                if base {
                    writing.push((command(Key::E), fl!(LANGUAGE_LOADER, "shortcuts-writing-edit-message")));
                    writing.push(("Del".into(), fl!(LANGUAGE_LOADER, "shortcuts-writing-mark-deleted")));
                    writing.push(("I".into(), fl!(LANGUAGE_LOADER, "shortcuts-writing-message-info")));
                }
                if !base {
                    writing.push(("Del".into(), fl!(LANGUAGE_LOADER, "shortcuts-writing-delete-selected-draft")));
                    writing.push((command_shift(Key::E), fl!(LANGUAGE_LOADER, "shortcuts-writing-export-reply-packet")));
                }
                writing
            }),
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
        let groups: Vec<_> = groups
            .into_iter()
            .map(|(title, entries)| ShortcutGroup::new(title, entries.into_iter().map(|(keys, action)| ShortcutEntry::new(keys, action)).collect()))
            .collect();
        shortcuts_dialog(
            context,
            &fl!(LANGUAGE_LOADER, "shortcuts-title"),
            &fl!(LANGUAGE_LOADER, "shortcuts-subtitle"),
            &groups,
        )
    }

    fn subscriptions_dialog(&mut self, context: &egui::Context) -> bool {
        let supported = self.supports_subscriptions() && self.drafts.is_some();
        let mut changes = Vec::new();
        let response = appearance::Dialog::new("mail-subscriptions")
            .title(fl!(LANGUAGE_LOADER, "subscriptions-title"))
            .subtitle(fl!(LANGUAGE_LOADER, "subscriptions-help"))
            .size(appearance::DialogSize::Medium)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    if !supported {
                        ui.label(fl!(LANGUAGE_LOADER, "subscriptions-unsupported"));
                    } else {
                        let pending: std::collections::BTreeMap<_, _> = self
                            .drafts
                            .as_ref()
                            .map(|store| store.subscriptions())
                            .unwrap_or_default()
                            .into_iter()
                            .collect();
                        ui.label(fl!(LANGUAGE_LOADER, "subscriptions-pending", count = pending.len()));
                        egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                            for (number, name) in &self.choices {
                                let current = pending.get(number).copied();
                                let mut next = current;
                                ui.horizontal(|ui| {
                                    egui::ComboBox::from_id_salt(("mail-subscription", number))
                                        .selected_text(match current {
                                            Some(true) => fl!(LANGUAGE_LOADER, "subscriptions-subscribe"),
                                            Some(false) => fl!(LANGUAGE_LOADER, "subscriptions-unsubscribe"),
                                            None => fl!(LANGUAGE_LOADER, "subscriptions-no-change"),
                                        })
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut next, None, fl!(LANGUAGE_LOADER, "subscriptions-no-change"));
                                            ui.selectable_value(&mut next, Some(true), fl!(LANGUAGE_LOADER, "subscriptions-subscribe"));
                                            ui.selectable_value(&mut next, Some(false), fl!(LANGUAGE_LOADER, "subscriptions-unsubscribe"));
                                        });
                                    ui.label(format!("{number} - {name}"));
                                });
                                if next != current {
                                    changes.push((*number, next));
                                }
                            }
                        });
                    }
                });
                dialog.buttons([appearance::DialogButton::primary(appearance::labels::close(), ()).cancels()]);
            });
        for (conference, subscribe) in changes {
            self.set_subscription(context, conference, subscribe);
        }
        response.action.is_some() || response.dismissed
    }

    /// Asks before packing; returns the dialog while it stays open.
    fn pack_dialog(&mut self, context: &egui::Context, mut state: PackDialog) -> Option<PackDialog> {
        let message = if state.deleted == 0 {
            fl!(LANGUAGE_LOADER, "dialog-pack-base-message-none")
        } else {
            fl!(LANGUAGE_LOADER, "dialog-pack-base-message", count = (state.deleted as i64))
        };
        let response = appearance::Dialog::new("mail-pack-base")
            .title(fl!(LANGUAGE_LOADER, "dialog-pack-base-title"))
            .subtitle(message)
            .size(appearance::DialogSize::Medium)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut state.older, fl!(LANGUAGE_LOADER, "dialog-pack-base-older"));
                        ui.add_enabled(state.older, egui::DragValue::new(&mut state.days).range(1..=36_500));
                        ui.label(fl!(LANGUAGE_LOADER, "dialog-pack-base-days"));
                    });
                    ui.checkbox(&mut state.received_private, fl!(LANGUAGE_LOADER, "dialog-pack-base-received-private"));
                });
                dialog.buttons([
                    appearance::DialogButton::cancel(appearance::labels::cancel(), false).cancels(),
                    appearance::DialogButton::destructive(fl!(LANGUAGE_LOADER, "dialog-pack-base-pack"), true),
                ]);
            });
        match response.action.or(response.dismissed.then_some(false)) {
            Some(true) => {
                self.pack(context, &state);
                None
            }
            Some(false) => None,
            None => Some(state),
        }
    }

    fn transfer_dialog(&mut self, context: &egui::Context, mut state: TransferDialog) -> Option<TransferDialog> {
        let index = self.index_of(state.area, state.number)?;
        let targets = self.transfer_targets(state.area);
        let can_move = self.reader.package.as_ref().is_some_and(|package| package.can_change(index));
        #[derive(Clone, Copy)]
        enum Choice {
            Cancel,
            Move,
            Copy,
        }
        let response = appearance::Dialog::new("mail-transfer")
            .title(fl!(LANGUAGE_LOADER, "dialog-transfer-title"))
            .subtitle(fl!(LANGUAGE_LOADER, "dialog-transfer-help"))
            .size(appearance::DialogSize::Medium)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    if targets.is_empty() {
                        ui.label(fl!(LANGUAGE_LOADER, "dialog-transfer-none"));
                        return;
                    }
                    ui.horizontal(|ui| {
                        ui.label(fl!(LANGUAGE_LOADER, "dialog-transfer-area"));
                        let selected = targets
                            .iter()
                            .find(|(number, _)| Some(*number) == state.target)
                            .map(|(_, name)| name.clone())
                            .unwrap_or_default();
                        egui::ComboBox::from_id_salt("mail-transfer-area")
                            .width(280.0)
                            .selected_text(selected)
                            .show_ui(ui, |ui| {
                                for (number, name) in &targets {
                                    ui.selectable_value(&mut state.target, Some(*number), name);
                                }
                            });
                    });
                });
                let ready = state.target.is_some();
                dialog.buttons([
                    appearance::DialogButton::cancel(appearance::labels::cancel(), Choice::Cancel).cancels(),
                    appearance::DialogButton::secondary(fl!(LANGUAGE_LOADER, "dialog-transfer-copy"), Choice::Copy).enabled(ready),
                    appearance::DialogButton::primary(fl!(LANGUAGE_LOADER, "dialog-transfer-move"), Choice::Move).enabled(ready && can_move),
                ]);
            });
        let Some(choice) = response.action.or(response.dismissed.then_some(Choice::Cancel)) else {
            return Some(state);
        };
        if let (Choice::Move | Choice::Copy, Some(target)) = (choice, state.target) {
            let keep = matches!(choice, Choice::Copy);
            self.base_job(BaseJob::Transfer { index, target, keep }, context);
        }
        None
    }

    fn message_info_dialog(&mut self, context: &egui::Context, mut state: InfoDialog) -> Option<InfoDialog> {
        let index = self.index_of(state.area, state.number)?;
        let writable = self.reader.package.as_ref().is_some_and(|package| package.can_change(index)) && !self.loader.changing;
        let details = &state.details;
        let date = |date: Option<chrono::NaiveDateTime>| date.map_or_else(|| "-".to_owned(), |date| date.format("%Y-%m-%d %H:%M:%S").to_string());
        let number = |number: u32| if number == 0 { "-".to_owned() } else { number.to_string() };
        let response = appearance::Dialog::new("mail-message-info")
            .title(fl!(LANGUAGE_LOADER, "dialog-info-title", number = details.number))
            .subtitle(details.area.clone())
            .size(appearance::DialogSize::Medium)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    egui::ScrollArea::vertical().max_height(440.0).show(ui, |ui| {
                        let rows = |ui: &mut egui::Ui, rows: &[(String, String)]| {
                            egui::Grid::new(ui.id().with("rows")).num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                                for (label, value) in rows {
                                    ui.label(egui::RichText::new(label).weak());
                                    ui.add(egui::Label::new(value).wrap());
                                    ui.end_row();
                                }
                            });
                        };
                        appearance::compact_group(ui, &fl!(LANGUAGE_LOADER, "dialog-info-dates"), |ui| {
                            rows(
                                ui,
                                &[
                                    (fl!(LANGUAGE_LOADER, "dialog-info-written"), date(details.written)),
                                    (fl!(LANGUAGE_LOADER, "dialog-info-received"), date(details.received)),
                                    (fl!(LANGUAGE_LOADER, "dialog-info-processed"), date(details.processed)),
                                ],
                            );
                        });
                        appearance::compact_group(ui, &fl!(LANGUAGE_LOADER, "dialog-info-links"), |ui| {
                            rows(
                                ui,
                                &[
                                    (fl!(LANGUAGE_LOADER, "dialog-info-reply-to"), number(details.reply_to)),
                                    (fl!(LANGUAGE_LOADER, "dialog-info-reply-first"), number(details.reply_first)),
                                    (fl!(LANGUAGE_LOADER, "dialog-info-reply-next"), number(details.reply_next)),
                                ],
                            );
                        });
                        appearance::compact_group(ui, &fl!(LANGUAGE_LOADER, "dialog-info-storage"), |ui| {
                            rows(
                                ui,
                                &[
                                    (fl!(LANGUAGE_LOADER, "dialog-info-times-read"), details.times_read.to_string()),
                                    (fl!(LANGUAGE_LOADER, "dialog-info-text-length"), details.text_len.to_string()),
                                ],
                            );
                        });
                        appearance::compact_group(ui, &fl!(LANGUAGE_LOADER, "dialog-info-attributes"), |ui| {
                            if !writable {
                                ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "dialog-info-read-only")).weak());
                            }
                            ui.add_enabled_ui(writable, |ui| {
                                egui::Grid::new("mail-message-attributes").num_columns(2).spacing([16.0, 2.0]).show(ui, |ui| {
                                    for (position, (bit, name)) in icy_mail::message_base::EDITABLE_ATTRIBUTES.iter().enumerate() {
                                        let mut on = state.attributes & bit != 0;
                                        if ui.checkbox(&mut on, *name).changed() {
                                            state.attributes = if on { state.attributes | bit } else { state.attributes & !bit };
                                        }
                                        if position % 2 == 1 {
                                            ui.end_row();
                                        }
                                    }
                                });
                            });
                        });
                        if !details.fields.is_empty() {
                            appearance::compact_group(ui, &fl!(LANGUAGE_LOADER, "dialog-info-fields"), |ui| {
                                rows(ui, &details.fields);
                            });
                        }
                    });
                });
                let changed = state.attributes != details.attributes;
                dialog.buttons([
                    appearance::DialogButton::cancel(appearance::labels::close(), false).cancels(),
                    appearance::DialogButton::primary(fl!(LANGUAGE_LOADER, "dialog-info-save"), true).enabled(writable && changed),
                ]);
            });
        match response.action.or(response.dismissed.then_some(false)) {
            Some(true) => {
                let set = state.attributes & !state.details.attributes;
                let clear = state.details.attributes & !state.attributes;
                self.base_job(BaseJob::Attributes { index, set, clear }, context);
                None
            }
            Some(false) => None,
            None => Some(state),
        }
    }

    fn check_report_dialog(&mut self, context: &egui::Context, checks: &[icy_mail::message_base::AreaCheck], repaired: bool) -> bool {
        let problems = checks.iter().filter(|check| !check.ok()).count();
        let summary = if repaired {
            fl!(LANGUAGE_LOADER, "dialog-check-repaired")
        } else if problems == 0 {
            fl!(LANGUAGE_LOADER, "dialog-check-ok", count = (checks.len() as i64))
        } else {
            fl!(LANGUAGE_LOADER, "dialog-check-problems", count = (problems as i64))
        };
        let response = appearance::Dialog::new("mail-check-base")
            .title(fl!(LANGUAGE_LOADER, "dialog-check-title"))
            .subtitle(summary)
            .size(appearance::DialogSize::Medium)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                        for check in checks.iter().filter(|check| !check.ok() || check.reclaimable > 0) {
                            appearance::compact_group(ui, &check.area, |ui| {
                                for problem in &check.problems {
                                    ui.add(egui::Label::new(egui::RichText::new(problem).color(ui.visuals().error_fg_color)).wrap());
                                }
                                if check.reclaimable > 0 {
                                    ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "dialog-check-reclaimable", bytes = (check.reclaimable as i64))).weak());
                                }
                            });
                        }
                    });
                });
                let mut buttons = vec![appearance::DialogButton::cancel(appearance::labels::close(), false).cancels()];
                if problems > 0 && !repaired {
                    buttons.push(appearance::DialogButton::destructive(fl!(LANGUAGE_LOADER, "dialog-check-repair"), true));
                }
                dialog.buttons(buttons);
            });
        match response.action.or(response.dismissed.then_some(false)) {
            Some(true) => {
                self.base_job(BaseJob::Repair, context);
                true
            }
            Some(false) => true,
            None => false,
        }
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
