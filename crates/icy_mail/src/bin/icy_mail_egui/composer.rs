use eframe::egui::{self, Key};
use icy_engine_gui::egui::appearance;
use icy_mail::drafts::{Draft, DraftField, DraftKind, HEADER_FIELD_LENGTH};

use super::{
    app::{conference_choices, key, AfterDiscard, MailApp, Modal, NoticeKind},
    reader_view::issue_box,
    terminal_editor::TerminalEditor,
    widgets::Icon,
};

/// The message being written, shown in place of the list and reader.
pub struct Composer {
    pub draft: Draft,
    pub original: Draft,
    /// Whether the draft is already stored, so saving updates it instead of adding a new one.
    pub existing: bool,
    /// The message a reply or forward is based on.
    pub origin: Option<String>,
    pub error: Option<String>,
    focus_pending: bool,
    /// The message text, edited on the terminal like on a BBS.
    pub editor: TerminalEditor,
}

impl Composer {
    /// `quotes` are the original message's lines for the quote panel, opened right away if `open_quotes`.
    pub fn new(draft: Draft, existing: bool, origin: Option<String>, quotes: Vec<String>, open_quotes: bool) -> Self {
        Self {
            editor: TerminalEditor::new(&draft.body, quotes, open_quotes),
            original: draft.clone(),
            draft,
            existing,
            origin,
            error: None,
            focus_pending: true,
        }
    }

    pub fn dirty(&self) -> bool {
        self.draft != self.original
    }

    fn title(&self) -> &'static str {
        match (self.existing, self.draft.kind) {
            (true, _) => "Edit Draft",
            (false, DraftKind::New) => "New Message",
            (false, DraftKind::Reply) => "Reply",
            (false, DraftKind::Forward) => "Forward Message",
        }
    }

    /// The header field that should receive the keyboard first, `None` for the message text.
    fn first_field(&self) -> Option<egui::Id> {
        if self.draft.to.trim().is_empty() {
            Some(egui::Id::new("composer-To"))
        } else if self.draft.subject.trim().is_empty() {
            Some(egui::Id::new("composer-Subject"))
        } else {
            None
        }
    }
}

impl MailApp {
    pub fn composer_keys(&mut self, context: &egui::Context) {
        let editor_escape = self.composer.as_ref().is_some_and(|composer| composer.editor.captures_escape());
        if key(context, Key::F1, false, false) {
            self.modal = Some(Modal::Shortcuts);
        } else if key(context, Key::S, true, false) || key(context, Key::Enter, true, false) {
            self.save_composer(context);
        } else if key(context, Key::W, true, false) {
            self.close(context);
        } else if !editor_escape && !egui::Popup::is_any_open(context) && key(context, Key::Escape, false, false) {
            self.cancel_composer();
        }
    }

    pub fn cancel_composer(&mut self) {
        match &self.composer {
            Some(composer) if composer.dirty() => self.modal = Some(Modal::Discard(AfterDiscard::Close)),
            _ => self.composer = None,
        }
    }

    pub fn save_composer(&mut self, context: &egui::Context) {
        let (Some(composer), Some(store)) = (&mut self.composer, &self.drafts) else {
            return;
        };
        let mut next = store.clone();
        let result = if composer.existing {
            next.update(composer.draft.clone())
        } else {
            next.insert(composer.draft.clone())
        };
        match result {
            Ok(()) => {
                let id = composer.draft.id;
                let problems = !next.issues(&composer.draft).is_empty();
                self.drafts = Some(next);
                self.composer = None;
                self.selected_draft = Some(id);
                if problems {
                    self.notify(context, NoticeKind::Warning, "Draft saved - it needs changes before it can be exported");
                } else {
                    let count = self.draft_count();
                    let waiting = if count == 1 { "1 message".to_string() } else { format!("{count} messages") };
                    self.notify(context, NoticeKind::Success, format!("Draft saved - {waiting} in the outbox"));
                }
            }
            Err(error) => composer.error = Some(format!("Unable to save the draft: {error}")),
        }
    }

    pub fn composer_view(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let Some(package) = self.reader.package.clone() else {
            return;
        };
        let conferences = conference_choices(&package);
        let Some(composer) = &mut self.composer else {
            return;
        };
        let issues = self.drafts.as_ref().map(|store| store.issues(&composer.draft)).unwrap_or_default();
        let mut problems: Vec<String> = issues.iter().map(|issue| issue.message.clone()).collect();
        problems.dedup();
        #[derive(Clone, Copy)]
        enum Action {
            Save,
            Cancel,
            Delete,
        }
        let mut action = None;
        let mut help = false;
        let enabled = self.modal.is_none() && self.error.is_none();
        let narrow = ui.available_width() < 560.0;
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(if narrow { 10 } else { 20 }, 12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let image = self.icons.image(
                        &context,
                        match composer.draft.kind {
                            DraftKind::New => Icon::Compose,
                            DraftKind::Reply => Icon::Reply,
                            DraftKind::Forward => Icon::Forward,
                        },
                        22.0,
                    );
                    ui.add(image.tint(ui.visuals().text_color()));
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(appearance::bold(ui, composer.title()).size(18.0));
                        if let Some(origin) = &composer.origin {
                            let prefix = if composer.draft.kind == DraftKind::Forward {
                                "Forwarding"
                            } else {
                                "Replying to"
                            };
                            ui.add(egui::Label::new(egui::RichText::new(format!("{prefix} {origin}")).weak().size(12.0)).truncate());
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(appearance::primary_button("Save Draft"))
                            .on_hover_text("Keep the message in the outbox (Ctrl+S)")
                            .clicked()
                        {
                            action = Some(Action::Save);
                        }
                        if ui.button("Cancel").on_hover_text("Close without saving (Esc)").clicked() {
                            action = Some(Action::Cancel);
                        }
                        if composer.existing && ui.button("Delete Draft").clicked() {
                            action = Some(Action::Delete);
                        }
                    });
                });
                ui.add_space(10.0);
                let label_width = if narrow { 70.0 } else { 90.0 };
                let row = |ui: &mut egui::Ui, label: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
                    ui.horizontal(|ui| {
                        ui.allocate_ui_with_layout(egui::vec2(label_width, 28.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_min_width(label_width);
                            ui.label(egui::RichText::new(label).weak());
                        });
                        add(ui);
                    });
                };
                let draft = &mut composer.draft;
                row(ui, "Conference", &mut |ui| {
                    let selected = conferences
                        .iter()
                        .find(|(number, _)| *number == draft.conference)
                        .map_or_else(|| format!("Conference {}", draft.conference), |(number, name)| format!("{name} ({number})"));
                    egui::ComboBox::from_id_salt("composer-conference")
                        .width((ui.available_width() - 140.0).clamp(120.0, 320.0))
                        .selected_text(selected)
                        .show_ui(ui, |ui| {
                            for (number, name) in &conferences {
                                ui.selectable_value(&mut draft.conference, *number, format!("{name} ({number})"));
                            }
                        });
                    ui.add_space(12.0);
                    ui.checkbox(&mut draft.private, "Private")
                        .on_hover_text("Only the recipient and the sysop can read private messages");
                });
                for (field, label) in [(DraftField::To, "To"), (DraftField::Subject, "Subject"), (DraftField::From, "From")] {
                    let value = match field {
                        DraftField::To => &mut draft.to,
                        DraftField::Subject => &mut draft.subject,
                        _ => &mut draft.from,
                    };
                    let invalid = issues.iter().any(|issue| issue.field == field && !value.trim().is_empty());
                    row(ui, label, &mut |ui| {
                        let counter_width = 44.0;
                        let width = (ui.available_width() - counter_width - 8.0).max(80.0);
                        let mut edit = appearance::text_edit(value).id(egui::Id::new(format!("composer-{label}"))).desired_width(width);
                        if invalid {
                            edit = edit.text_color(ui.visuals().error_fg_color);
                        }
                        if ui.add(edit).changed() && value.contains('\t') {
                            *value = value.replace('\t', " ");
                        }
                        let length = value.chars().count();
                        let color = if length > HEADER_FIELD_LENGTH {
                            ui.visuals().error_fg_color
                        } else {
                            ui.visuals().weak_text_color()
                        };
                        ui.label(egui::RichText::new(format!("{length}/{HEADER_FIELD_LENGTH}")).size(11.5).color(color))
                            .on_hover_text("QWK headers hold up to 25 characters");
                    });
                }
                ui.add_space(6.0);
                if let Some(error) = &composer.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                    ui.add_space(4.0);
                }
                if !problems.is_empty() && (composer.existing || composer.dirty()) {
                    issue_box(ui, &mut self.icons, &problems);
                    ui.add_space(6.0);
                }
                let output = composer.editor.show(ui, &self.settings, &mut self.icons, enabled);
                if output.changed {
                    composer.draft.body = composer.editor.editor.to_body();
                }
                help = output.help;
            });
        if composer.focus_pending {
            composer.focus_pending = false;
            match composer.first_field() {
                Some(id) => context.memory_mut(|memory| memory.request_focus(id)),
                None => composer.editor.request_focus(),
            }
        }
        if help {
            self.modal = Some(Modal::Shortcuts);
        }
        match action {
            Some(Action::Save) => self.save_composer(&context),
            Some(Action::Cancel) => self.cancel_composer(),
            Some(Action::Delete) => {
                if let Some(composer) = &self.composer {
                    self.modal = Some(Modal::DeleteDraft(composer.draft.id));
                }
            }
            None => {}
        }
    }
}
