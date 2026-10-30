use eframe::egui::{self, Key};
use i18n_embed_fl::fl;
use icy_engine_gui::egui::appearance;
use icy_mail::{
    charset::Charset,
    drafts::{Draft, DraftField, DraftKind},
    editor, LANGUAGE_LOADER,
};

use super::{
    app::{key, AfterDiscard, MailApp, Modal, NoticeKind},
    terminal_editor::TerminalEditor,
    text_editor::TextEditor,
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
    /// The message text of a CP437 draft, edited on the terminal like on a BBS.
    pub editor: TerminalEditor,
    /// The plain-text editor of a UTF-8 draft, with input methods; `None` for CP437 drafts.
    pub text: Option<TextEditor>,
    blue_wave: bool,
}

impl Composer {
    /// `quotes` are the original message's lines for the quote panel, opened right away if `open_quotes`.
    pub fn new(draft: Draft, existing: bool, origin: Option<String>, quotes: Vec<String>, open_quotes: bool, blue_wave: bool) -> Self {
        let utf8 = draft.charset == Charset::Utf8;
        let text = utf8.then(|| TextEditor::new(&draft.body, quotes.clone(), open_quotes));
        let terminal_body = if utf8 { String::new() } else { draft.body.clone() };
        Self {
            editor: if blue_wave {
                TerminalEditor::new_with_format(&terminal_body, quotes, open_quotes && !utf8, true)
            } else {
                TerminalEditor::new(&terminal_body, quotes, open_quotes && !utf8)
            },
            text,
            blue_wave,
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

    pub fn request_focus(&mut self) {
        match &mut self.text {
            Some(text) => text.request_focus(),
            None => self.editor.request_focus(),
        }
    }

    /// Whether Escape closes something inside the editor rather than the composer.
    pub fn captures_escape(&self) -> bool {
        self.text.as_ref().map_or_else(|| self.editor.captures_escape(), TextEditor::captures_escape)
    }

    /// The active editor's status bar; returns whether help was asked for.
    pub fn status_bar(&mut self, ui: &mut egui::Ui) -> bool {
        match &mut self.text {
            Some(text) => text.status_bar(ui),
            None => self.editor.status_bar(ui),
        }
    }

    /// Switches the draft between the CP437 BBS editor and the UTF-8 text editor. UTF-8 has no
    /// ANSI colors, so they are dropped; CP437 is refused while the text has characters it lacks.
    pub fn set_charset(&mut self, charset: Charset) -> Result<(), String> {
        if charset == self.draft.charset {
            return Ok(());
        }
        let quotes = self.text.as_ref().map_or(&self.editor.quotes, |text| &text.quotes).lines.clone();
        if charset == Charset::Utf8 {
            let body = editor::strip_codes(&self.draft.body);
            let mut text = TextEditor::new(&body, quotes, false);
            if body != self.draft.body {
                text.status = Some(fl!(LANGUAGE_LOADER, "composer-colors-removed"));
            }
            self.draft.body = text.text.clone();
            self.text = Some(text);
        } else {
            let body = self.text.as_ref().map_or_else(|| self.draft.body.clone(), |text| text.text.clone());
            if let Some(character) = body.chars().find(|&ch| ch != '\n' && !editor::is_message_char_for(ch, self.blue_wave)) {
                return Err(fl!(LANGUAGE_LOADER, "composer-not-cp437", character = character.to_string()));
            }
            self.editor = TerminalEditor::new_with_format(&body, quotes, false, self.blue_wave);
            self.draft.body = self.editor.editor.to_paragraphs();
            self.text = None;
        }
        self.draft.charset = charset;
        self.request_focus();
        Ok(())
    }

    fn title(&self) -> String {
        match (self.existing, self.draft.kind) {
            (true, _) => fl!(LANGUAGE_LOADER, "composer-title-edit-draft"),
            (false, DraftKind::New) => fl!(LANGUAGE_LOADER, "composer-title-new-message"),
            (false, DraftKind::Reply) => fl!(LANGUAGE_LOADER, "composer-title-reply"),
            (false, DraftKind::Forward) => fl!(LANGUAGE_LOADER, "composer-title-forward-message"),
            (false, DraftKind::Edit) => fl!(LANGUAGE_LOADER, "composer-title-edit-message"),
        }
    }

    /// The header field that should receive the keyboard first, `None` for the message text.
    fn first_field(&self) -> Option<egui::Id> {
        // A message base opened on its own may not know who writes.
        if self.draft.from.trim().is_empty() {
            Some(egui::Id::new("composer-From"))
        } else if self.draft.to.trim().is_empty() {
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
        let editor_escape = self.composer.as_ref().is_some_and(Composer::captures_escape);
        if key(context, Key::F1, false, false) {
            self.modal = Some(Modal::Shortcuts);
        } else if key(context, Key::S, true, false) || key(context, Key::Enter, true, false) {
            self.save_composer(context);
        } else if key(context, Key::W, true, false) {
            self.close(context);
        } else if key(context, Key::T, true, false) {
            self.open_taglines(true);
        } else if key(context, Key::B, true, false) {
            self.open_address_book(true);
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
        let issues = store.issues(&composer.draft);
        if let Some(issue) = issues.first() {
            // The save button is disabled for the same reason; this covers the keyboard shortcut.
            let message = issue.message.clone();
            self.notify(context, NoticeKind::Warning, message);
            return;
        }
        if store.is_message_base() {
            // A message base has no outbox: saving writes the message, as BBS and FidoNet editors do.
            if let Some(path) = self.path.clone() {
                self.loader.write_message(path, store.clone(), composer.draft.clone(), context);
            }
            return;
        }
        let mut next = store.clone();
        let result = if composer.existing {
            next.update(composer.draft.clone())
        } else {
            next.insert(composer.draft.clone())
        };
        match result {
            Ok(()) => {
                let id = composer.draft.id;
                self.drafts = Some(next);
                self.composer = None;
                self.selected_draft = Some(id);
                let count = self.draft_count();
                self.notify(context, NoticeKind::Success, fl!(LANGUAGE_LOADER, "composer-draft-saved-outbox", count = count));
            }
            Err(error) => composer.error = Some(fl!(LANGUAGE_LOADER, "composer-save-error", error = error.to_string())),
        }
    }

    pub fn composer_view(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        if self.reader.package.is_none() {
            return;
        }
        let Some(store) = self.drafts.as_ref() else {
            return;
        };
        let (save, save_tooltip) = if store.is_message_base() {
            (
                fl!(LANGUAGE_LOADER, "composer-save-message"),
                fl!(LANGUAGE_LOADER, "composer-save-message-tooltip"),
            )
        } else {
            (fl!(LANGUAGE_LOADER, "composer-save-draft"), fl!(LANGUAGE_LOADER, "composer-save-draft-tooltip"))
        };
        let writing = self.loader.writing;
        let conferences: Vec<_> = self.choices.iter().filter(|(number, _)| store.can_post(*number)).cloned().collect();
        let Some(composer) = &mut self.composer else {
            return;
        };
        let issues = store.issues(&composer.draft);
        let mut problems: Vec<String> = issues.iter().map(|issue| issue.message.clone()).collect();
        problems.dedup();
        #[derive(Clone, Copy)]
        enum Action {
            Save,
            Cancel,
            Delete,
            AddressBook,
            Taglines,
            RandomTagline,
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
                            DraftKind::New | DraftKind::Edit => Icon::Compose,
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
                            let label = if composer.draft.kind == DraftKind::Forward {
                                fl!(LANGUAGE_LOADER, "composer-forwarding-origin", origin = origin.as_str())
                            } else {
                                fl!(LANGUAGE_LOADER, "composer-replying-origin", origin = origin.as_str())
                            };
                            ui.add(egui::Label::new(egui::RichText::new(label).weak().size(12.0)).truncate());
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let missing = problems.join("\n");
                        if ui
                            .add_enabled(problems.is_empty() && !writing, appearance::primary_button(save.as_str()))
                            .on_hover_text(save_tooltip.as_str())
                            .on_disabled_hover_text(&missing)
                            .clicked()
                        {
                            action = Some(Action::Save);
                        }
                        if ui
                            .button(fl!(LANGUAGE_LOADER, "composer-cancel"))
                            .on_hover_text(fl!(LANGUAGE_LOADER, "composer-cancel-tooltip"))
                            .clicked()
                        {
                            action = Some(Action::Cancel);
                        }
                        if composer.existing && ui.button(fl!(LANGUAGE_LOADER, "composer-delete-draft")).clicked() {
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
                let mut encoding = None;
                let draft = &mut composer.draft;
                if !store.preserves_reply_references() && draft.ref_number != 0 {
                    ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "composer-bluewave-legacy-reply")).weak().small());
                }
                let icons = &mut self.icons;
                row(ui, &fl!(LANGUAGE_LOADER, "composer-conference"), &mut |ui| {
                    let selected = conferences.iter().find(|(number, _)| *number == draft.conference).map_or_else(
                        || fl!(LANGUAGE_LOADER, "composer-conference-number", number = draft.conference),
                        |(number, name)| format!("{name} ({number})"),
                    );
                    // An edited message is changed where it is stored.
                    ui.add_enabled_ui(draft.kind != DraftKind::Edit, |ui| {
                        egui::ComboBox::from_id_salt("composer-conference")
                            .width((ui.available_width() - 140.0).clamp(120.0, 320.0))
                            .selected_text(selected)
                            .show_ui(ui, |ui| {
                                for (number, name) in &conferences {
                                    ui.selectable_value(&mut draft.conference, *number, format!("{name} ({number})"));
                                }
                            });
                    });
                    ui.add_space(12.0);
                    ui.checkbox(&mut draft.private, fl!(LANGUAGE_LOADER, "composer-private"))
                        .on_hover_text(fl!(LANGUAGE_LOADER, "composer-private-tooltip"));
                    ui.add_space(12.0);
                    let utf8 = store.supports_utf8() || draft.charset == Charset::Utf8;
                    egui::ComboBox::from_id_salt("composer-encoding")
                        .selected_text(draft.charset.name())
                        .show_ui(ui, |ui| {
                            for charset in [Charset::Cp437, Charset::Utf8] {
                                let enabled = charset == Charset::Cp437 || utf8;
                                if ui
                                    .add_enabled(enabled, egui::Button::selectable(draft.charset == charset, charset.name()))
                                    .on_disabled_hover_text(fl!(LANGUAGE_LOADER, "composer-utf8-unsupported"))
                                    .clicked()
                                {
                                    encoding = Some(charset);
                                }
                            }
                        })
                        .response
                        .on_hover_text(fl!(LANGUAGE_LOADER, "composer-encoding-tooltip"));
                });
                for (field, label, id_label) in [
                    (DraftField::To, fl!(LANGUAGE_LOADER, "composer-field-to"), "To"),
                    (DraftField::Subject, fl!(LANGUAGE_LOADER, "composer-field-subject"), "Subject"),
                    (DraftField::From, fl!(LANGUAGE_LOADER, "composer-field-from"), "From"),
                ] {
                    let header_limit = store.field_limit_for(field, draft.charset);
                    let value = match field {
                        DraftField::To => &mut draft.to,
                        DraftField::Subject => &mut draft.subject,
                        _ => &mut draft.from,
                    };
                    let invalid = issues.iter().any(|issue| issue.field == field && !value.trim().is_empty());
                    row(ui, &label, &mut |ui| {
                        let counter_width = 44.0 + if field == DraftField::To { 34.0 } else { 0.0 };
                        let width = (ui.available_width() - counter_width - 8.0).max(80.0);
                        let mut edit = appearance::text_edit(value)
                            .id(egui::Id::new(format!("composer-{id_label}")))
                            .desired_width(width);
                        if invalid {
                            edit = edit.text_color(ui.visuals().error_fg_color);
                        }
                        if ui.add(edit).changed() && value.contains('\t') {
                            *value = value.replace('\t', " ");
                        }
                        let length = value.chars().count();
                        let color = if length > header_limit {
                            ui.visuals().error_fg_color
                        } else {
                            ui.visuals().weak_text_color()
                        };
                        ui.label(egui::RichText::new(format!("{length}/{header_limit}")).size(11.5).color(color))
                            .on_hover_text(fl!(LANGUAGE_LOADER, "composer-qwk-header-limit-tooltip", limit = header_limit));
                        if field == DraftField::To
                            && icons
                                .button(ui, Icon::Contacts, &fl!(LANGUAGE_LOADER, "composer-address-book-tooltip"), true)
                                .clicked()
                        {
                            action = Some(Action::AddressBook);
                        }
                    });
                }
                if store.is_netmail(draft.conference) {
                    let invalid = issues
                        .iter()
                        .any(|issue| issue.field == DraftField::Address && !draft.address.trim().is_empty());
                    row(ui, &fl!(LANGUAGE_LOADER, "draft-field-address"), &mut |ui| {
                        let mut edit = appearance::text_edit(&mut draft.address)
                            .id(egui::Id::new("composer-Address"))
                            .hint_text("2:240/1")
                            .desired_width((ui.available_width() - 8.0).clamp(80.0, 220.0));
                        if invalid {
                            edit = edit.text_color(ui.visuals().error_fg_color);
                        }
                        ui.add(edit);
                    });
                }
                if let Some(charset) = encoding {
                    composer.error = composer.set_charset(charset).err();
                }
                ui.add_space(6.0);
                if let Some(error) = &composer.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                    ui.add_space(4.0);
                }
                egui::Panel::bottom("composer-tagline")
                    .frame(egui::Frame::new().inner_margin(egui::Margin { top: 6, ..Default::default() }))
                    .show_separator_line(false)
                    .show_inside(ui, |ui| {
                        egui::CollapsingHeader::new(fl!(LANGUAGE_LOADER, "composer-signature"))
                            .id_salt("composer-signature")
                            .default_open(!composer.draft.signature.is_empty())
                            .show(ui, |ui| {
                                let invalid = issues.iter().any(|issue| issue.field == DraftField::Signature);
                                let mut edit = egui::TextEdit::multiline(&mut composer.draft.signature)
                                    .desired_rows(2)
                                    .desired_width(f32::INFINITY);
                                if invalid {
                                    edit = edit.text_color(ui.visuals().error_fg_color);
                                }
                                ui.add(edit).on_hover_text(fl!(LANGUAGE_LOADER, "composer-signature-hint"));
                            });
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.allocate_ui_with_layout(egui::vec2(label_width, 28.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_min_width(label_width);
                                ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "composer-tagline")).weak());
                            });
                            let buttons = 3.0 * 32.0;
                            let width = (ui.available_width() - buttons).max(60.0);
                            let tagline = &composer.draft.tagline;
                            ui.allocate_ui_with_layout(egui::vec2(width, 28.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_min_width(width);
                                let text = if tagline.is_empty() {
                                    egui::RichText::new(fl!(LANGUAGE_LOADER, "composer-no-tagline")).weak().italics()
                                } else {
                                    egui::RichText::new(format!("... {tagline}")).monospace()
                                };
                                let response = ui.add(egui::Label::new(text).truncate().sense(egui::Sense::click()));
                                if response.on_hover_text(fl!(LANGUAGE_LOADER, "composer-choose-tagline-tooltip")).clicked() {
                                    action = Some(Action::Taglines);
                                }
                            });
                            if self
                                .icons
                                .button(ui, Icon::Tag, &fl!(LANGUAGE_LOADER, "composer-choose-tagline-tooltip"), true)
                                .clicked()
                            {
                                action = Some(Action::Taglines);
                            }
                            let any = self.taglines.as_ref().is_some_and(|taglines| !taglines.lines.is_empty());
                            if self
                                .icons
                                .button(ui, Icon::Shuffle, &fl!(LANGUAGE_LOADER, "composer-random-tagline"), any)
                                .clicked()
                            {
                                action = Some(Action::RandomTagline);
                            }
                            if self
                                .icons
                                .button(ui, Icon::Close, &fl!(LANGUAGE_LOADER, "composer-no-tagline"), !tagline.is_empty())
                                .clicked()
                            {
                                composer.draft.tagline.clear();
                            }
                        });
                    });
                let modern = self.reading_mode == icy_mail::options::ReadingMode::Modern;
                let output = match &mut composer.text {
                    Some(text) => {
                        let font = egui::FontId::monospace(if modern { self.modern_font_size } else { 14.0 });
                        let output = text.show(ui, &mut self.icons, enabled, font);
                        if output.changed {
                            composer.draft.body = text.text.clone();
                        }
                        output
                    }
                    None => {
                        let output = composer.editor.show(ui, &self.settings, &mut self.icons, enabled, modern);
                        if output.changed {
                            composer.draft.body = composer.editor.editor.to_paragraphs();
                        }
                        output
                    }
                };
                help = output.help;
            });
        if composer.focus_pending {
            composer.focus_pending = false;
            match composer.first_field() {
                Some(id) => context.memory_mut(|memory| memory.request_focus(id)),
                None => composer.request_focus(),
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
            Some(Action::AddressBook) => self.open_address_book(true),
            Some(Action::Taglines) => self.open_taglines(true),
            Some(Action::RandomTagline) => {
                let tagline = self.taglines.as_ref().and_then(|taglines| taglines.random()).map(str::to_string);
                if let (Some(tagline), Some(composer)) = (tagline, &mut self.composer) {
                    composer.draft.tagline = tagline;
                }
            }
            None => {}
        }
    }
}
