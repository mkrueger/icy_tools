use eframe::egui;
use i18n_embed_fl::fl;
use icy_engine::{Selection, Size, TextScreen};
use icy_engine_gui::egui::{appearance, screen::ScreenView};
use icy_mail::{
    drafts::{Draft, DraftKind},
    editor,
    reader::{NavigateDirection, Pane},
    LANGUAGE_LOADER,
};

use super::{
    app::{draft_title, Folder, MailApp, Modal},
    list::display_date,
    widgets::{self, Icon},
};

impl MailApp {
    pub fn content(&mut self, ui: &mut egui::Ui) {
        if self.folder == Folder::Drafts {
            self.draft_preview(ui);
            return;
        }
        let context = ui.ctx().clone();
        let Some(info) = self.selected_info().cloned() else {
            let image = self.icons.image(&context, Icon::Mailbox, 44.0);
            widgets::empty(
                ui,
                image,
                &fl!(LANGUAGE_LOADER, "reader-no-message-selected"),
                &fl!(LANGUAGE_LOADER, "reader-pick-message-detail"),
            );
            return;
        };
        let needle = self.reader.filter.trim().to_owned();
        let compact = ui.available_height() < 220.0;
        let minimal = ui.available_height() < 160.0;
        let conference = self.folder_name(Folder::Conference(info.conference));
        let parent = if info.ref_number == 0 {
            None
        } else {
            self.reader.find(info.conference, info.ref_number)
        };
        let unread = !self.reader.is_read(info.index);
        let mut navigate = None;
        let mut open_parent = false;
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 14,
                right: 8,
                top: if compact { 2 } else { 8 },
                bottom: if compact { 2 } else { 8 },
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let actions = 8.0 * 32.0 + 18.0;
                    let width = (ui.available_width() - actions).max(40.0);
                    ui.allocate_ui_with_layout(egui::vec2(width, 30.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(width);
                        let size = if compact { 14.0 } else { 17.0 };
                        let mut job = widgets::header_job(ui, &info.subject, "", egui::FontId::proportional(size), ui.visuals().strong_text_color(), true);
                        widgets::highlight(&mut job, 0, &needle, ui);
                        ui.add(egui::Label::new(job).truncate()).on_hover_text(info.subject.as_str());
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let position = self.reader.selected_position();
                        if self
                            .icons
                            .button(
                                ui,
                                Icon::Down,
                                &fl!(LANGUAGE_LOADER, "reader-next-message"),
                                position.is_some_and(|position| position + 1 < self.reader.messages.len()),
                            )
                            .clicked()
                        {
                            navigate = Some(NavigateDirection::Down);
                        }
                        if self
                            .icons
                            .button(
                                ui,
                                Icon::Up,
                                &fl!(LANGUAGE_LOADER, "reader-previous-message"),
                                position.is_some_and(|position| position > 0),
                            )
                            .clicked()
                        {
                            navigate = Some(NavigateDirection::Up);
                        }
                        ui.add_space(6.0);
                        if self
                            .icons
                            .button(ui, Icon::Copy, &fl!(LANGUAGE_LOADER, "reader-copy-message-text"), !self.body_loading)
                            .clicked()
                        {
                            self.copy_message(&context);
                        }
                        let tagline = self.message_tagline();
                        let tooltip = tagline.as_ref().map_or_else(
                            || fl!(LANGUAGE_LOADER, "reader-no-tagline"),
                            |tagline| fl!(LANGUAGE_LOADER, "reader-save-tagline", tagline = tagline.as_str()),
                        );
                        if self.icons.button(ui, Icon::Tag, &tooltip, tagline.is_some()).clicked() {
                            self.save_tagline(&context);
                        }
                        if self
                            .icons
                            .button(ui, Icon::PersonAdd, &fl!(LANGUAGE_LOADER, "reader-add-author-address-book"), true)
                            .clicked()
                        {
                            self.add_sender(&context);
                        }
                        ui.add_space(6.0);
                        let (icon, tooltip) = if unread {
                            (Icon::Read, fl!(LANGUAGE_LOADER, "reader-mark-as-read-short"))
                        } else {
                            (Icon::Unread, fl!(LANGUAGE_LOADER, "reader-mark-as-unread-short"))
                        };
                        if self.icons.button(ui, icon, &tooltip, true).clicked() {
                            self.toggle_read(&context);
                        }
                        if self
                            .icons
                            .button(ui, Icon::Forward, &fl!(LANGUAGE_LOADER, "reader-forward-short"), true)
                            .clicked()
                        {
                            self.reply(&context, true);
                        }
                        if self.icons.button(ui, Icon::Reply, &fl!(LANGUAGE_LOADER, "reader-reply-short"), true).clicked() {
                            self.reply(&context, false);
                        }
                    });
                });
                if minimal {
                    return;
                }
                if compact {
                    let color = ui.visuals().weak_text_color();
                    let font = egui::FontId::proportional(12.0);
                    let mut job = egui::text::LayoutJob::default();
                    widgets::append_header(&mut job, ui, &info.from, "", font.clone(), color, false);
                    widgets::highlight(&mut job, 0, &needle, ui);
                    job.append(
                        " \u{2192} ",
                        0.0,
                        egui::TextFormat {
                            font_id: font.clone(),
                            color,
                            ..Default::default()
                        },
                    );
                    let start = job.text.len();
                    widgets::append_header(&mut job, ui, &info.to, "", font.clone(), color, false);
                    widgets::highlight(&mut job, start, &needle, ui);
                    let suffix = format!("  \u{00b7}  {}  \u{00b7}  {conference}", info.date_str);
                    job.append(
                        &suffix,
                        0.0,
                        egui::TextFormat {
                            font_id: font,
                            color,
                            ..Default::default()
                        },
                    );
                    let details = format!("{} \u{2192} {}  \u{00b7}  {}  \u{00b7}  {conference}", info.from, info.to, info.date_str);
                    ui.add(egui::Label::new(job).truncate()).on_hover_text(details);
                    return;
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    widgets::avatar(ui, &info.from, 34.0);
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            let mut from = widgets::header_job(ui, &info.from, "", egui::FontId::proportional(13.5), ui.visuals().strong_text_color(), true);
                            widgets::highlight(&mut from, 0, &needle, ui);
                            ui.label(from);
                            ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "reader-to")).weak().size(12.5));
                            let mut to = widgets::header_job(ui, &info.to, "", egui::FontId::proportional(13.5), ui.visuals().text_color(), false);
                            widgets::highlight(&mut to, 0, &needle, ui);
                            ui.label(to);
                            if info.private {
                                ui.add_space(6.0);
                                appearance::status_badge(ui, &fl!(LANGUAGE_LOADER, "reader-private"), widgets::warning(ui));
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            let weak = |text: String| egui::RichText::new(text).weak().size(12.0);
                            ui.label(weak(format!("{}  \u{00b7}  {conference}  \u{00b7}  #{}", info.date_str, info.number)));
                            if info.ref_number != 0 {
                                ui.label(weak("\u{00b7}".into()));
                                let text = fl!(LANGUAGE_LOADER, "reader-reply-to", number = info.ref_number);
                                if parent.is_some_and(|index| self.reader.contains(index)) {
                                    let link = egui::RichText::new(text.clone()).size(12.0).color(widgets::accent(ui));
                                    if ui
                                        .add(egui::Label::new(link).sense(egui::Sense::click()))
                                        .on_hover_text(fl!(LANGUAGE_LOADER, "reader-show-original-message"))
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        open_parent = true;
                                    }
                                } else {
                                    ui.label(weak(text));
                                }
                            }
                        });
                    });
                });
            });
        let separator = ui.visuals().widgets.noninteractive.bg_stroke;
        let top = ui.cursor().top();
        ui.painter().hline(ui.max_rect().x_range(), top, separator);
        if let Some(direction) = navigate {
            self.reader.navigate(Pane::Messages, direction);
            self.reveal_message = true;
        }
        if open_parent {
            if let Some(index) = parent {
                self.reader.select_message(index);
                self.reveal_message = true;
            }
        }
        if self.body_loading || self.rendered != self.reader.selected_message {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        }
        let response = self.terminal_body(ui);
        response.context_menu(|ui| {
            if ui.button(fl!(LANGUAGE_LOADER, "reader-copy")).clicked() {
                self.copy(ui.ctx());
                ui.close();
            }
            if ui.button(fl!(LANGUAGE_LOADER, "reader-copy-message")).clicked() {
                self.copy_message(ui.ctx());
                ui.close();
            }
            ui.separator();
            if ui.button(fl!(LANGUAGE_LOADER, "reader-reply")).clicked() {
                self.reply(ui.ctx(), false);
                ui.close();
            }
            if ui.button(fl!(LANGUAGE_LOADER, "reader-forward")).clicked() {
                self.reply(ui.ctx(), true);
                ui.close();
            }
        });
    }

    /// The message terminal with focus handling and mouse selection.
    fn terminal_body(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let response = self.screen.show(ui, &self.settings);
        self.content_rect = response.rect;
        if response.clicked() || response.drag_started() {
            self.set_focus(Pane::Content, ui.ctx());
        }
        if response.drag_started_by(egui::PointerButton::Primary) {
            self.selection_anchor = ui.input(|input| input.pointer.press_origin()).and_then(|pos| self.cell(pos));
        }
        if response.dragged_by(egui::PointerButton::Primary) {
            if let (Some(anchor), Some(lead)) = (self.selection_anchor, response.interact_pointer_pos().and_then(|pos| self.cell(pos))) {
                let mut selection = Selection::new(anchor);
                selection.lead = lead;
                if ui.input(|input| input.modifiers.alt) {
                    selection.shape = icy_engine::Shape::Rectangle;
                }
                let _ = self.screen.terminal.screen.lock().set_selection(selection);
            }
        } else if response.clicked() {
            let _ = self.screen.terminal.screen.lock().clear_selection();
        }
        response
    }

    /// Renders the draft into the message terminal, like received messages.
    fn render_draft(&mut self, draft: &Draft) {
        let text = draft.text();
        if self.rendered_draft.as_ref().is_some_and(|(id, body)| *id == draft.id && *body == text) {
            return;
        }
        let screen = icy_mail::reader::render_body(&editor::encode_message(&text)).unwrap_or_else(|_| TextScreen::new(Size::new(80, 25)));
        self.screen = ScreenView::new(screen);
        // A body still loading for the message list must not replace the draft.
        self.loader.body_generation = self.loader.body_generation.wrapping_add(1);
        self.body_loading = false;
        self.rendered = None;
        self.selection_anchor = None;
        self.rendered_draft = Some((draft.id, text));
    }

    fn draft_preview(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let Some(store) = &self.drafts else {
            return;
        };
        let Some(draft) = store.drafts().iter().find(|draft| Some(draft.id) == self.selected_draft).cloned() else {
            let image = self.icons.image(&context, Icon::Drafts, 44.0);
            widgets::empty(
                ui,
                image,
                &fl!(LANGUAGE_LOADER, "reader-no-draft-selected"),
                &fl!(LANGUAGE_LOADER, "reader-no-draft-detail"),
            );
            return;
        };
        let mut issues: Vec<String> = store.issues(&draft).into_iter().map(|issue| issue.message).collect();
        issues.dedup();
        let conference = self.choices.iter().find(|(number, _)| *number == draft.conference).map_or_else(
            || fl!(LANGUAGE_LOADER, "reader-conference-number", number = draft.conference),
            |(_, name)| name.clone(),
        );
        let mut edit = false;
        let mut delete = false;
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 14,
                right: 10,
                top: 8,
                bottom: 8,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let kind = match draft.kind {
                        DraftKind::New => fl!(LANGUAGE_LOADER, "reader-kind-new-message"),
                        DraftKind::Reply => fl!(LANGUAGE_LOADER, "reader-kind-reply"),
                        DraftKind::Forward => fl!(LANGUAGE_LOADER, "reader-kind-forward"),
                    };
                    appearance::chip(ui, &kind, widgets::accent(ui));
                    let width = (ui.available_width() - 150.0).max(40.0);
                    ui.allocate_ui_with_layout(egui::vec2(width, 30.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(width);
                        ui.add(egui::Label::new(appearance::bold(ui, draft_title(&draft)).size(17.0)).truncate());
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        delete = self
                            .icons
                            .button(ui, Icon::Delete, &fl!(LANGUAGE_LOADER, "reader-delete-draft-short"), true)
                            .clicked();
                        edit = ui
                            .add(appearance::primary_button(fl!(LANGUAGE_LOADER, "reader-edit")))
                            .on_hover_text(fl!(LANGUAGE_LOADER, "reader-edit-draft-short"))
                            .clicked();
                    });
                });
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let weak = |text: String| egui::RichText::new(text).weak().size(12.5);
                    ui.label(weak(fl!(LANGUAGE_LOADER, "reader-to-capital")));
                    let recipient = if draft.to.trim().is_empty() {
                        fl!(LANGUAGE_LOADER, "reader-no-recipient")
                    } else {
                        draft.to.clone()
                    };
                    ui.label(recipient);
                    ui.label(weak(fl!(LANGUAGE_LOADER, "reader-from")));
                    ui.label(&draft.from);
                    ui.label(weak(format!("\u{00b7}  {conference}  \u{00b7}  {}", display_date(&draft.date))));
                    if draft.private {
                        ui.add_space(6.0);
                        appearance::status_badge(ui, &fl!(LANGUAGE_LOADER, "reader-private"), widgets::warning(ui));
                    }
                });
                if !issues.is_empty() {
                    ui.add_space(6.0);
                    issue_box(ui, &mut self.icons, &issues);
                }
            });
        let separator = ui.visuals().widgets.noninteractive.bg_stroke;
        ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), separator);
        self.render_draft(&draft);
        let response = self.terminal_body(ui);
        response.context_menu(|ui| {
            if ui.button(fl!(LANGUAGE_LOADER, "reader-copy")).clicked() {
                self.copy(ui.ctx());
                ui.close();
            }
            ui.separator();
            if ui.button(fl!(LANGUAGE_LOADER, "reader-edit")).clicked() {
                edit = true;
                ui.close();
            }
            if ui.button(fl!(LANGUAGE_LOADER, "reader-delete")).clicked() {
                delete = true;
                ui.close();
            }
        });
        if edit {
            self.edit_draft(&context, draft.id);
        }
        if delete {
            self.modal = Some(Modal::DeleteDraft(draft.id));
        }
    }
}

/// Warning panel listing what keeps a draft from being exported.
pub fn issue_box(ui: &mut egui::Ui, icons: &mut widgets::Icons, issues: &[String]) {
    let color = widgets::warning(ui);
    egui::Frame::new()
        .fill(color.gamma_multiply(0.12))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.5)))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal(|ui| {
                ui.add(icons.image(ui.ctx(), Icon::Warning, 16.0).tint(color));
                ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "reader-fix-before-export")).color(color).strong());
            });
            for issue in issues {
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(22.0);
                    ui.label(egui::RichText::new(issue).size(12.5));
                });
            }
        });
}
