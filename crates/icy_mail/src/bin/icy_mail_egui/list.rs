use eframe::egui;
use i18n_embed_fl::fl;
use icy_engine_gui::egui::appearance;
use icy_mail::{
    drafts::DraftKind,
    reader::{MessageColumn, Pane, ViewMode},
    LANGUAGE_LOADER,
};

use super::{
    app::{draft_title, Folder, MailApp, Modal},
    widgets::{self, Cell, Icon, ROW_HEIGHT},
};

const FLAGS_WIDTH: f32 = 52.0;

impl MailApp {
    /// Folder heading and its messages or drafts. Returns whether the user picked an entry.
    pub fn list(&mut self, ui: &mut egui::Ui) -> bool {
        self.list_title(ui);
        if self.folder == Folder::Drafts {
            self.draft_list(ui)
        } else {
            self.message_list(ui)
        }
    }

    fn list_title(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 12,
                right: 8,
                top: 6,
                bottom: 4,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_min_height(26.0);
                    let name = self.folder_name(self.folder);
                    ui.add(egui::Label::new(appearance::bold(ui, name).size(15.0)).truncate().selectable(false));
                    let detail = if self.folder == Folder::Drafts {
                        fl!(LANGUAGE_LOADER, "list-draft-count", count = (self.draft_count() as i64))
                    } else {
                        let unread = self.reader.unread_count();
                        fl!(
                            LANGUAGE_LOADER,
                            "list-message-count-unread",
                            count = (self.reader.messages.len() as i64),
                            unread = (unread as i64)
                        )
                    };
                    ui.label(egui::RichText::new(detail).weak().size(12.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.folder == Folder::Drafts {
                            if self.draft_count() > 0 && ui.add(appearance::primary_button(fl!(LANGUAGE_LOADER, "list-export-replies"))).clicked() {
                                self.export(&context);
                            }
                        } else {
                            let unread_only = self.reader.unread_only;
                            if widgets::pill(ui, unread_only, &fl!(LANGUAGE_LOADER, "list-unread"))
                                .on_hover_text(fl!(LANGUAGE_LOADER, "list-show-only-unread-messages"))
                                .clicked()
                            {
                                self.set_unread_only(!unread_only);
                            }
                        }
                    });
                });
            });
    }

    fn message_list(&mut self, ui: &mut egui::Ui) -> bool {
        let context = ui.ctx().clone();
        ui.spacing_mut().item_spacing.y = 0.0;
        let width = ui.available_width().max(560.0);
        let widths = [FLAGS_WIDTH, 170.0, width - FLAGS_WIDTH - 170.0 - 136.0 - 56.0, 136.0, 56.0];
        let columns = [
            None,
            Some(MessageColumn::From),
            Some(MessageColumn::Subject),
            Some(MessageColumn::Date),
            Some(MessageColumn::Lines),
        ];
        let threaded = self.reader.view_mode == ViewMode::Threads;
        let active = columns.iter().position(|column| *column == Some(self.reader.message_sort.0));
        let focused = self.focus == Pane::Messages;
        let accent = widgets::accent(ui);
        let replied: std::collections::HashSet<(u16, u32)> = self
            .drafts
            .as_ref()
            .map(|store| {
                store
                    .drafts()
                    .iter()
                    .filter(|draft| draft.kind == DraftKind::Reply)
                    .map(|draft| (draft.conference, draft.ref_number))
                    .collect()
            })
            .unwrap_or_default();
        let mut select = None;
        let mut action = None;
        egui::ScrollArea::horizontal()
            .id_salt("message-columns")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(width);
                let column_from = fl!(LANGUAGE_LOADER, "list-column-from");
                let column_subject = if threaded {
                    fl!(LANGUAGE_LOADER, "list-column-subject-threads")
                } else {
                    fl!(LANGUAGE_LOADER, "list-column-subject")
                };
                let column_date = fl!(LANGUAGE_LOADER, "list-column-date");
                let column_lines = fl!(LANGUAGE_LOADER, "list-column-lines");
                let labels = ["", column_from.as_str(), column_subject.as_str(), column_date.as_str(), column_lines.as_str()];
                if let Some(column) = widgets::header(
                    ui,
                    &widths,
                    &labels,
                    active.filter(|_| !threaded).map(|column| (column, self.reader.message_sort.1)),
                    !threaded,
                ) {
                    if let Some(column) = columns[column] {
                        self.reader.sort_messages(column);
                        self.reveal_message = true;
                    }
                }
                let mut scroll = egui::ScrollArea::vertical().id_salt("message-rows").auto_shrink([false, false]);
                if self.reveal_message {
                    if let Some(position) = self.reader.selected_position() {
                        scroll = scroll.vertical_scroll_offset(widgets::reveal(position, self.message_view.x, self.message_view.y));
                    }
                    self.reveal_message = false;
                }
                let needle = self.reader.filter.trim().to_owned();
                let output = scroll.show_rows(ui, ROW_HEIGHT, self.reader.messages.len(), |ui, range| {
                    let Some(package) = self.reader.package.clone() else {
                        return;
                    };
                    for position in range {
                        let row = self.reader.messages[position];
                        let info = &package.infos[row.index];
                        let unread = !self.reader.is_read(row.index);
                        let lines = info.lines.to_string();
                        let selected = self.reader.selected_message == Some(row.index);
                        let (response, cells, colors) = widgets::row(
                            ui,
                            &widths,
                            &[
                                Cell::new(""),
                                Cell::header(&info.from).strong(unread).highlight(&needle),
                                Cell::header(&info.subject)
                                    .prefix(if threaded && row.depth > 0 { "> " } else { "" })
                                    .strong(unread)
                                    .highlight(&needle)
                                    .indent(if threaded { f32::from(row.depth.min(12)) * 16.0 } else { 0.0 }),
                                Cell::new(&info.date_str).weak(),
                                Cell::new(&lines).weak().right(),
                            ],
                            selected,
                            focused,
                        );
                        let flags = cells[0];
                        if unread {
                            let dot = egui::Rect::from_center_size(egui::pos2(flags.left() + 12.0, flags.center().y), egui::Vec2::splat(8.0));
                            widgets::unread_dot(ui, dot, if colors.selected { colors.text } else { accent });
                        }
                        let mut left = flags.left() + 22.0;
                        if replied.contains(&(info.conference, info.number)) {
                            let rect = egui::Rect::from_min_size(egui::pos2(left, flags.center().y - 7.0), egui::Vec2::splat(14.0));
                            self.icons.paint(ui, Icon::Reply, rect, colors.weak);
                            left += 15.0;
                        }
                        if info.private {
                            let rect = egui::Rect::from_min_size(egui::pos2(left, flags.center().y - 7.0), egui::Vec2::splat(14.0));
                            self.icons.paint(ui, Icon::Lock, rect, colors.weak);
                        }
                        if response.clicked() || response.secondary_clicked() {
                            select = Some((row.index, response.double_clicked()));
                        }
                        let index = row.index;
                        response.context_menu(|ui| {
                            if ui.button(fl!(LANGUAGE_LOADER, "list-reply")).clicked() {
                                action = Some(("reply", index));
                                ui.close();
                            }
                            if ui.button(fl!(LANGUAGE_LOADER, "list-forward")).clicked() {
                                action = Some(("forward", index));
                                ui.close();
                            }
                            ui.separator();
                            let label = if unread {
                                fl!(LANGUAGE_LOADER, "list-mark-as-read")
                            } else {
                                fl!(LANGUAGE_LOADER, "list-mark-as-unread")
                            };
                            if ui.button(label).clicked() {
                                action = Some(("toggle", index));
                                ui.close();
                            }
                        });
                    }
                });
                self.message_view = egui::vec2(output.state.offset.y, output.inner_rect.height());
                if self.reader.messages.is_empty() {
                    let detail = if !self.reader.filter.trim().is_empty() {
                        fl!(LANGUAGE_LOADER, "list-nothing-matches", query = self.reader.filter.trim())
                    } else if self.reader.unread_only {
                        fl!(LANGUAGE_LOADER, "list-all-read")
                    } else if self.folder == Folder::Personal {
                        let name = self.user_name();
                        fl!(LANGUAGE_LOADER, "list-no-personal", name = name.as_str())
                    } else {
                        fl!(LANGUAGE_LOADER, "list-folder-empty")
                    };
                    let image = self.icons.image(&context, Icon::Inbox, 40.0);
                    let rect = output.inner_rect;
                    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                        widgets::empty(ui, image, &fl!(LANGUAGE_LOADER, "list-no-messages"), &detail);
                    });
                }
            });
        let picked = select.is_some();
        if let Some((index, open)) = select {
            self.reader.select_message(index);
            self.set_focus(if open { Pane::Content } else { Pane::Messages }, &context);
        }
        if let Some((action, index)) = action {
            self.reader.select_message(index);
            match action {
                "reply" => self.reply(&context, false),
                "forward" => self.reply(&context, true),
                _ => {
                    let read = !self.reader.is_read(index);
                    self.set_read(&context, &[index], read);
                    if !read {
                        self.rendered = self.reader.selected_message;
                    }
                }
            }
        }
        picked
    }

    fn draft_list(&mut self, ui: &mut egui::Ui) -> bool {
        let context = ui.ctx().clone();
        ui.spacing_mut().item_spacing.y = 0.0;
        let Some(store) = self.drafts.clone() else {
            return false;
        };
        let conferences = self.choices.clone();
        let width = ui.available_width().max(560.0);
        let widths = [FLAGS_WIDTH, 160.0, width - FLAGS_WIDTH - 160.0 - 150.0 - 136.0, 150.0, 136.0];
        let focused = self.focus == Pane::Messages;
        let warning = widgets::warning(ui);
        let mut select = None;
        let mut action = None;
        egui::ScrollArea::horizontal()
            .id_salt("draft-columns")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(width);
                let column_to = fl!(LANGUAGE_LOADER, "list-column-to");
                let column_subject = fl!(LANGUAGE_LOADER, "list-column-subject");
                let column_conference = fl!(LANGUAGE_LOADER, "list-column-conference");
                let column_written = fl!(LANGUAGE_LOADER, "list-column-written");
                let labels = [
                    "",
                    column_to.as_str(),
                    column_subject.as_str(),
                    column_conference.as_str(),
                    column_written.as_str(),
                ];
                widgets::header(ui, &widths, &labels, None, false);
                let mut scroll = egui::ScrollArea::vertical().id_salt("draft-rows").auto_shrink([false, false]);
                if self.reveal_message {
                    if let Some(position) = store.drafts().iter().position(|draft| Some(draft.id) == self.selected_draft) {
                        scroll = scroll.vertical_scroll_offset(widgets::reveal(position, self.message_view.x, self.message_view.y));
                    }
                    self.reveal_message = false;
                }
                let output = scroll.show_rows(ui, ROW_HEIGHT, store.drafts().len(), |ui, range| {
                    for draft in &store.drafts()[range] {
                        let title = draft_title(draft);
                        let conference = conferences
                            .iter()
                            .find(|(number, _)| *number == draft.conference)
                            .map_or_else(|| draft.conference.to_string(), |(_, name)| name.clone());
                        let written = display_date(&draft.date);
                        let to = if draft.to.trim().is_empty() {
                            fl!(LANGUAGE_LOADER, "list-no-recipient")
                        } else {
                            draft.to.clone()
                        };
                        let (response, cells, colors) = widgets::row(
                            ui,
                            &widths,
                            &[
                                Cell::new(""),
                                Cell::new(&to),
                                Cell::new(&title),
                                Cell::new(&conference).weak(),
                                Cell::new(&written).weak(),
                            ],
                            self.selected_draft == Some(draft.id),
                            focused,
                        );
                        let flags = cells[0];
                        let icon = match draft.kind {
                            DraftKind::New => Icon::Compose,
                            DraftKind::Reply => Icon::Reply,
                            DraftKind::Forward => Icon::Forward,
                        };
                        let rect = egui::Rect::from_min_size(egui::pos2(flags.left() + 6.0, flags.center().y - 8.0), egui::Vec2::splat(16.0));
                        self.icons.paint(ui, icon, rect, colors.weak);
                        let issues = store.issues(draft);
                        let response = if let Some(issue) = issues.first() {
                            let rect = egui::Rect::from_min_size(egui::pos2(flags.left() + 26.0, flags.center().y - 8.0), egui::Vec2::splat(16.0));
                            self.icons.paint(ui, Icon::Warning, rect, if colors.selected { colors.text } else { warning });
                            response.on_hover_text(fl!(LANGUAGE_LOADER, "list-needs-attention", message = issue.message.as_str()))
                        } else {
                            response
                        };
                        if response.clicked() || response.secondary_clicked() {
                            select = Some((draft.id, response.double_clicked()));
                        }
                        let id = draft.id;
                        response.context_menu(|ui| {
                            if ui.button(fl!(LANGUAGE_LOADER, "list-edit")).clicked() {
                                action = Some((false, id));
                                ui.close();
                            }
                            if ui.button(fl!(LANGUAGE_LOADER, "list-delete-ellipsis")).clicked() {
                                action = Some((true, id));
                                ui.close();
                            }
                        });
                    }
                });
                self.message_view = egui::vec2(output.state.offset.y, output.inner_rect.height());
                if store.drafts().is_empty() {
                    let image = self.icons.image(&context, Icon::Export, 40.0);
                    ui.scope_builder(egui::UiBuilder::new().max_rect(output.inner_rect), |ui| {
                        widgets::empty(
                            ui,
                            image,
                            &fl!(LANGUAGE_LOADER, "list-outbox-empty-title"),
                            &fl!(LANGUAGE_LOADER, "list-outbox-empty-detail"),
                        );
                    });
                }
            });
        let picked = select.is_some();
        if let Some((id, edit)) = select {
            self.selected_draft = Some(id);
            self.set_focus(Pane::Messages, &context);
            if edit {
                self.edit_draft(&context, id);
            }
        }
        match action {
            Some((false, id)) => self.edit_draft(&context, id),
            Some((true, id)) => self.modal = Some(Modal::DeleteDraft(id)),
            None => {}
        }
        picked
    }
}

/// Formats a QWK `MM-DD-YYHH:MM` header date like message dates (`YYYY-MM-DD HH:MM`).
pub fn display_date(date: &str) -> String {
    chrono::NaiveDateTime::parse_from_str(date, "%m-%d-%y%H:%M").map_or_else(|_| date.to_string(), |date| date.format("%Y-%m-%d %H:%M").to_string())
}
