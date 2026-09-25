use eframe::egui;
use icy_engine_gui::egui::appearance;
use icy_mail::{
    drafts::DraftKind,
    reader::{MessageColumn, Pane, ViewMode},
    text,
};

use super::{
    app::{conference_choices, draft_title, Folder, MailApp, Modal},
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
                        match self.draft_count() {
                            1 => "1 message".to_string(),
                            count => format!("{count} messages"),
                        }
                    } else {
                        let unread = self.reader.messages.iter().filter(|row| !self.reader.read.contains(&row.index)).count();
                        format!("{} \u{00b7} {unread} unread", self.reader.messages.len())
                    };
                    ui.label(egui::RichText::new(detail).weak().size(12.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.folder == Folder::Drafts {
                            if self.draft_count() > 0 && ui.add(appearance::primary_button("Export Replies\u{2026}")).clicked() {
                                self.export(&context);
                            }
                        } else {
                            let unread_only = self.reader.unread_only;
                            if widgets::pill(ui, unread_only, "Unread").on_hover_text("Show only unread messages").clicked() {
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
                if let Some(column) = widgets::header(
                    ui,
                    &widths,
                    &["", "From", if threaded { "Subject (threads)" } else { "Subject" }, "Date", "Lines"],
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
                let output = scroll.show_rows(ui, ROW_HEIGHT, self.reader.messages.len(), |ui, range| {
                    let Some(package) = self.reader.package.clone() else {
                        return;
                    };
                    for position in range {
                        let row = self.reader.messages[position];
                        let info = &package.infos[row.index];
                        let unread = !self.reader.read.contains(&row.index);
                        let from = text::decode(&info.from);
                        let subject = text::decode(&info.subject);
                        let subject = if threaded && row.depth > 0 { format!("> {subject}").into() } else { subject };
                        let lines = info.lines.to_string();
                        let selected = self.reader.selected_message == Some(row.index);
                        let (response, cells, colors) = widgets::row(
                            ui,
                            &widths,
                            &[
                                Cell::new(""),
                                Cell::new(&from).strong(unread),
                                Cell::new(&subject)
                                    .strong(unread)
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
                            if ui.button("Reply").clicked() {
                                action = Some(("reply", index));
                                ui.close();
                            }
                            if ui.button("Forward").clicked() {
                                action = Some(("forward", index));
                                ui.close();
                            }
                            ui.separator();
                            if ui.button(if unread { "Mark as Read" } else { "Mark as Unread" }).clicked() {
                                action = Some(("toggle", index));
                                ui.close();
                            }
                        });
                    }
                });
                self.message_view = egui::vec2(output.state.offset.y, output.inner_rect.height());
                if self.reader.messages.is_empty() {
                    let detail = if !self.reader.filter.trim().is_empty() {
                        format!("Nothing matches \u{201c}{}\u{201d}", self.reader.filter.trim())
                    } else if self.reader.unread_only {
                        "Everything in this folder has been read".to_string()
                    } else if self.folder == Folder::Personal {
                        format!("No messages are addressed to {}", self.user_name())
                    } else {
                        "This folder is empty".to_string()
                    };
                    let image = self.icons.image(&context, Icon::Inbox, 40.0);
                    let rect = output.inner_rect;
                    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                        widgets::empty(ui, image, "No messages", &detail);
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
                    let read = !self.reader.read.contains(&index);
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
        let package = self.reader.package.clone();
        let conferences = package.as_deref().map(conference_choices).unwrap_or_default();
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
                widgets::header(ui, &widths, &["", "To", "Subject", "Conference", "Written"], None, false);
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
                        let to = if draft.to.trim().is_empty() { "(no recipient)" } else { draft.to.as_str() };
                        let (response, cells, colors) = widgets::row(
                            ui,
                            &widths,
                            &[
                                Cell::new(""),
                                Cell::new(to),
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
                            response.on_hover_text(format!("Needs attention before export: {}", issue.message))
                        } else {
                            response
                        };
                        if response.clicked() || response.secondary_clicked() {
                            select = Some((draft.id, response.double_clicked()));
                        }
                        let id = draft.id;
                        response.context_menu(|ui| {
                            if ui.button("Edit").clicked() {
                                action = Some((false, id));
                                ui.close();
                            }
                            if ui.button("Delete\u{2026}").clicked() {
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
                            "The outbox is empty",
                            "Replies and new messages stay here until you export them as a reply packet.",
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
