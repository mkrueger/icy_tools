use chrono::Datelike;
use eframe::egui;
use i18n_embed_fl::fl;
use icy_engine_gui::egui::appearance;
use icy_mail::{
    drafts::DraftKind,
    qwk::{PacketFile, PacketFileKind},
    reader::{MessageColumn, Pane, ViewMode},
    threading::Row,
    LANGUAGE_LOADER,
};

use super::{
    app::{draft_title, Folder, MailApp, Modal},
    widgets::{self, Cell, Icon, ROW_HEIGHT},
};

const FLAGS_WIDTH: f32 = 52.0;
/// Message rows also hold the star between the unread dot and the reply and private flags.
const MESSAGE_FLAGS_WIDTH: f32 = 68.0;
/// Below this width the list scrolls sideways instead of squeezing its columns further.
const MIN_LIST_WIDTH: f32 = 420.0;
/// Width of one thread level in the subject column.
const TREE_STEP: f32 = 14.0;
/// Deeper replies share the last level, so long chains do not push the subject out of view.
const TREE_MAX_DEPTH: u16 = 12;

fn tree_indent(depth: u16) -> f32 {
    f32::from(depth.min(TREE_MAX_DEPTH) + 1) * TREE_STEP
}

/// Position of the row a thread reply answers: the closest earlier row one level up.
fn parent_position(rows: &[Row], position: usize) -> Option<usize> {
    let depth = rows[position].depth;
    if depth == 0 {
        return None;
    }
    rows[..position].iter().rposition(|row| row.depth < depth)
}

/// Leading bytes of a reply's subject that repeat its parent's: `Re:` prefixes, and either the whole
/// subject or the shared words before the first difference ("FidoNews 43:39 " of numbered parts).
pub fn repeated_subject_len(subject: &str, parent: &str) -> usize {
    let prefix = icy_mail::qwk::reply_prefix_len(subject);
    let rest = &subject[prefix..];
    let parent = &parent[icy_mail::qwk::reply_prefix_len(parent)..];
    if rest.trim_end().eq_ignore_ascii_case(parent.trim_end()) {
        return subject.len();
    }
    let common = rest
        .char_indices()
        .zip(parent.chars())
        .take_while(|((_, own), other)| own.to_lowercase().eq(other.to_lowercase()))
        .last()
        .map_or(0, |((index, own), _)| index + own.len_utf8());
    let words = rest[..common]
        .char_indices()
        .rev()
        .find(|(_, ch)| ch.is_whitespace())
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    // Short matches such as "A " or "The " are coincidence rather than a shared title.
    prefix + if rest[..words].trim().len() >= 4 { words } else { 0 }
}

/// Draws the connector lines of a thread row and its disclosure triangle.
/// Returns the triangle's rectangle when the row has replies.
fn paint_tree(ui: &egui::Ui, cell: egui::Rect, row: &Row, collapsed: bool, color: egui::Color32) -> Option<egui::Rect> {
    let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(cell));
    let depth = row.depth.min(TREE_MAX_DEPTH);
    let left = cell.left() + 6.0;
    let center = |level: u16| painter.round_to_pixel_center(left + f32::from(level) * TREE_STEP + TREE_STEP / 2.0);
    let (top, bottom) = (cell.top(), cell.bottom());
    let middle = painter.round_to_pixel_center(cell.center().y);
    let stroke = egui::Stroke::new(1.0, color.gamma_multiply(0.6));

    for level in 0..depth.saturating_sub(1) {
        if level < u32::BITS as u16 && row.guides & (1 << level) != 0 {
            painter.vline(center(level), top..=bottom, stroke);
        }
    }
    let twisty = egui::Rect::from_center_size(egui::pos2(center(depth), middle), egui::Vec2::splat(9.0));
    if depth > 0 {
        let x = center(depth - 1);
        painter.vline(x, top..=if row.last { middle } else { bottom }, stroke);
        let end = if row.has_children {
            twisty.left() - 2.0
        } else {
            left + tree_indent(depth) - 3.0
        };
        painter.hline(x..=end, middle, stroke);
    }
    if !row.has_children {
        return None;
    }
    let points = if collapsed {
        vec![
            twisty.left_top() + egui::vec2(1.5, 0.0),
            twisty.right_center(),
            twisty.left_bottom() + egui::vec2(1.5, 0.0),
        ]
    } else {
        painter.vline(center(depth), twisty.bottom() + 1.0..=bottom, stroke);
        vec![
            twisty.left_top() + egui::vec2(0.0, 1.5),
            twisty.right_top() + egui::vec2(0.0, 1.5),
            twisty.center_bottom(),
        ]
    };
    painter.add(egui::Shape::convex_polygon(points, color, egui::Stroke::NONE));
    Some(twisty)
}

impl MailApp {
    /// Folder heading and its messages or drafts. Returns whether the user picked an entry.
    pub fn list(&mut self, ui: &mut egui::Ui) -> bool {
        self.list_title(ui);
        match self.folder {
            Folder::Drafts => self.draft_list(ui),
            Folder::Bulletins => self.file_list(ui),
            _ => self.message_list(ui),
        }
    }

    fn list_title(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let drafts = self.folder == Folder::Drafts;
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
                    } else if self.folder == Folder::Bulletins {
                        fl!(LANGUAGE_LOADER, "status-files", count = (self.file_count() as i64))
                    } else {
                        let unread = self.reader.unread_count();
                        fl!(
                            LANGUAGE_LOADER,
                            "list-message-count-unread",
                            count = (self.reader.all_messages().len() as i64),
                            unread = (unread as i64)
                        )
                    };
                    ui.label(egui::RichText::new(detail).weak().size(12.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.folder == Folder::Drafts {
                            if self.draft_count() > 0 && ui.add(appearance::primary_button(fl!(LANGUAGE_LOADER, "list-export-replies"))).clicked() {
                                self.export(&context);
                            }
                        } else if self.folder.holds_messages() {
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
                if drafts {
                    ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "list-outbox-steps")).weak().size(12.0));
                }
            });
    }

    fn message_list(&mut self, ui: &mut egui::Ui) -> bool {
        let context = ui.ctx().clone();
        ui.spacing_mut().item_spacing.y = 0.0;
        let width = ui.available_width().max(MIN_LIST_WIDTH);
        let from = (width * 0.25).clamp(110.0, 170.0);
        let date = (width * 0.22).clamp(112.0, 136.0);
        let widths = [MESSAGE_FLAGS_WIDTH, from, width - MESSAGE_FLAGS_WIDTH - from - date, date];
        let columns = [None, Some(MessageColumn::From), Some(MessageColumn::Subject), Some(MessageColumn::Date)];
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
        let mut toggle = None;
        let mut star = None;
        let star_color = widgets::star(ui);
        let star_tooltip = fl!(LANGUAGE_LOADER, "list-star-tooltip");
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
                let labels = ["", column_from.as_str(), column_subject.as_str(), column_date.as_str()];
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
                let from_needle = self.search_needle(self.reader.search_fields.from);
                let subject_needle = self.search_needle(self.reader.search_fields.subject);
                let today = today();
                let output = scroll.show_rows(ui, ROW_HEIGHT, self.reader.messages.len(), |ui, range| {
                    let Some(package) = self.reader.package.clone() else {
                        return;
                    };
                    for position in range {
                        let row = self.reader.messages[position];
                        let info = &package.infos[row.index];
                        let unread = !self.reader.is_read(row.index);
                        let selected = self.reader.selected_message == Some(row.index);
                        let date = friendly_date(info.date, &info.date_str, today);
                        let repeated = if threaded {
                            parent_position(&self.reader.messages, position).map_or(0, |parent| {
                                let parent = &package.infos[self.reader.messages[parent].index];
                                repeated_subject_len(info.subject.as_str(), parent.subject.as_str())
                            })
                        } else {
                            0
                        };
                        let collapsed = threaded && row.descendants > 0 && self.reader.is_collapsed(row.index);
                        let hidden_unread = if collapsed { self.reader.unread_replies(row.index) } else { 0 };
                        let replies = row.descendants.to_string();
                        let strong = unread || hidden_unread > 0;
                        let (response, cells, colors) = widgets::row(
                            ui,
                            &widths,
                            &[
                                Cell::new(""),
                                Cell::header(&info.from).strong(strong).highlight(&from_needle),
                                Cell::header(&info.subject)
                                    .strong(strong)
                                    .highlight(&subject_needle)
                                    .indent(if threaded { tree_indent(row.depth) } else { 0.0 })
                                    .dim(repeated)
                                    .badge(collapsed.then_some((replies.as_str(), hidden_unread > 0))),
                                Cell::new(&date).weak(),
                            ],
                            selected,
                            focused,
                        );
                        let mut twisty_clicked = false;
                        if threaded {
                            if let Some(twisty) = paint_tree(ui, cells[2], &row, collapsed, colors.weak) {
                                let hit = ui
                                    .interact(twisty.expand(4.0), ui.id().with(("thread-twisty", row.index)), egui::Sense::click())
                                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                                if hit.clicked() {
                                    toggle = Some(row.index);
                                    twisty_clicked = true;
                                }
                            }
                        }
                        let flags = cells[0];
                        if unread {
                            let dot = egui::Rect::from_center_size(egui::pos2(flags.left() + 10.0, flags.center().y), egui::Vec2::splat(8.0));
                            widgets::unread_dot(ui, dot, if colors.selected { colors.text } else { accent });
                        }
                        let starred = self.reader.is_starred(row.index);
                        let star_rect = egui::Rect::from_center_size(egui::pos2(flags.left() + 27.0, flags.center().y), egui::Vec2::splat(16.0));
                        let star_hit = ui
                            .interact(star_rect.expand(3.0), ui.id().with(("message-star", row.index)), egui::Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(&star_tooltip);
                        if starred {
                            self.icons.paint(ui, Icon::Starred, star_rect, star_color);
                        } else if response.hovered() || star_hit.hovered() {
                            self.icons.paint(ui, Icon::Star, star_rect, colors.weak);
                        }
                        let star_clicked = star_hit.clicked();
                        if star_clicked {
                            star = Some((row.index, !starred));
                        }
                        let mut left = flags.left() + 38.0;
                        if replied.contains(&(info.conference, info.number)) {
                            let rect = egui::Rect::from_min_size(egui::pos2(left, flags.center().y - 7.0), egui::Vec2::splat(14.0));
                            self.icons.paint(ui, Icon::Reply, rect, colors.weak);
                            left += 15.0;
                        }
                        if info.private {
                            let rect = egui::Rect::from_min_size(egui::pos2(left, flags.center().y - 7.0), egui::Vec2::splat(14.0));
                            self.icons.paint(ui, Icon::Lock, rect, colors.weak);
                        }
                        if !twisty_clicked && !star_clicked && (response.clicked() || response.secondary_clicked()) {
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
                            if threaded && ui.button(fl!(LANGUAGE_LOADER, "list-mark-thread-read")).clicked() {
                                action = Some(("thread", index));
                                ui.close();
                            }
                            let label = if starred {
                                fl!(LANGUAGE_LOADER, "list-unstar")
                            } else {
                                fl!(LANGUAGE_LOADER, "list-star")
                            };
                            if ui.button(label).clicked() {
                                action = Some(("star", index));
                                ui.close();
                            }
                        });
                    }
                });
                self.message_view = egui::vec2(output.state.offset.y, output.inner_rect.height());
                if self.reader.messages.is_empty() {
                    let detail = if self.loader.searching {
                        fl!(LANGUAGE_LOADER, "search-bodies-running")
                    } else if !self.reader.filter.trim().is_empty() {
                        fl!(LANGUAGE_LOADER, "list-nothing-matches", query = self.reader.filter.trim())
                    } else if self.reader.unread_only {
                        fl!(LANGUAGE_LOADER, "list-all-read")
                    } else if self.folder == Folder::Personal {
                        let name = self.user_name();
                        fl!(LANGUAGE_LOADER, "list-no-personal", name = name.as_str())
                    } else if self.folder == Folder::Starred {
                        fl!(LANGUAGE_LOADER, "list-no-starred")
                    } else {
                        fl!(LANGUAGE_LOADER, "list-folder-empty")
                    };
                    let icon = if self.folder == Folder::Starred { Icon::Star } else { Icon::Inbox };
                    let image = self.icons.image(&context, icon, 40.0);
                    let rect = output.inner_rect;
                    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                        widgets::empty(ui, image, &fl!(LANGUAGE_LOADER, "list-no-messages"), &detail);
                    });
                }
            });
        if let Some(index) = toggle {
            self.reader.toggle_collapsed(index);
            self.set_focus(Pane::Messages, &context);
        }
        if let Some((index, starred)) = star {
            self.set_starred(&context, index, starred);
        }
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
                "star" => self.set_starred(&context, index, !self.reader.is_starred(index)),
                "thread" => self.mark_thread_read(&context),
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
        let width = ui.available_width().max(MIN_LIST_WIDTH);
        let to = (width * 0.22).clamp(100.0, 160.0);
        let conference = (width * 0.2).clamp(90.0, 150.0);
        let date = (width * 0.22).clamp(112.0, 136.0);
        let widths = [FLAGS_WIDTH, to, width - FLAGS_WIDTH - to - conference - date, conference, date];
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
                let today = today();
                let output = scroll.show_rows(ui, ROW_HEIGHT, store.drafts().len(), |ui, range| {
                    for draft in &store.drafts()[range] {
                        let title = draft_title(draft);
                        let conference = conferences
                            .iter()
                            .find(|(number, _)| *number == draft.conference)
                            .map_or_else(|| draft.conference.to_string(), |(_, name)| name.clone());
                        let written = friendly_qwk_date(&draft.date, today);
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

impl MailApp {
    /// Welcome, news and goodbye screens, bulletins and new files lists of the packet.
    fn file_list(&mut self, ui: &mut egui::Ui) -> bool {
        let context = ui.ctx().clone();
        ui.spacing_mut().item_spacing.y = 0.0;
        let Some(package) = self.reader.package.clone() else {
            return false;
        };
        let width = ui.available_width().max(MIN_LIST_WIDTH);
        let file = (width * 0.3).clamp(120.0, 200.0);
        let widths = [FLAGS_WIDTH, width - FLAGS_WIDTH - file - 56.0, file, 56.0];
        let focused = self.focus == Pane::Messages;
        let mut select = None;
        egui::ScrollArea::horizontal()
            .id_salt("file-columns")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(width);
                let column_title = fl!(LANGUAGE_LOADER, "list-column-title");
                let column_file = fl!(LANGUAGE_LOADER, "list-column-file");
                let column_lines = fl!(LANGUAGE_LOADER, "list-column-lines");
                widgets::header(
                    ui,
                    &widths,
                    &["", column_title.as_str(), column_file.as_str(), column_lines.as_str()],
                    None,
                    false,
                );
                let mut scroll = egui::ScrollArea::vertical().id_salt("file-rows").auto_shrink([false, false]);
                if self.reveal_message {
                    if let Some(position) = self.selected_file {
                        scroll = scroll.vertical_scroll_offset(widgets::reveal(position, self.message_view.x, self.message_view.y));
                    }
                    self.reveal_message = false;
                }
                let output = scroll.show_rows(ui, ROW_HEIGHT, package.files.len(), |ui, range| {
                    for position in range {
                        let file = &package.files[position];
                        let title = file_title(file);
                        let lines = file.lines().to_string();
                        let (response, cells, colors) = widgets::row(
                            ui,
                            &widths,
                            &[Cell::new(""), Cell::new(&title), Cell::new(&file.name).weak(), Cell::new(&lines).weak().right()],
                            self.selected_file == Some(position),
                            focused,
                        );
                        let flags = cells[0];
                        let icon = if file.kind == PacketFileKind::NewFiles { Icon::Open } else { Icon::Bulletins };
                        let rect = egui::Rect::from_min_size(egui::pos2(flags.left() + 6.0, flags.center().y - 8.0), egui::Vec2::splat(16.0));
                        self.icons.paint(ui, icon, rect, colors.weak);
                        if response.clicked() || response.secondary_clicked() {
                            select = Some((position, response.double_clicked()));
                        }
                    }
                });
                self.message_view = egui::vec2(output.state.offset.y, output.inner_rect.height());
            });
        let picked = select.is_some();
        if let Some((position, open)) = select {
            if self.selected_file != Some(position) {
                self.selected_file_page = 0;
            }
            self.selected_file = Some(position);
            self.set_focus(if open { Pane::Content } else { Pane::Messages }, &context);
        }
        picked
    }
}

/// Display name of a packet file; bulletins are numbered by the extension of `BLT-<conference>.<number>`.
pub fn file_title(file: &PacketFile) -> String {
    match file.kind {
        PacketFileKind::Welcome => fl!(LANGUAGE_LOADER, "file-kind-welcome"),
        PacketFileKind::News => fl!(LANGUAGE_LOADER, "file-kind-news"),
        PacketFileKind::Bulletin => match file.name.rsplit_once('.').and_then(|(_, number)| number.parse::<u32>().ok()) {
            Some(number) => fl!(LANGUAGE_LOADER, "file-kind-bulletin-number", number = number),
            None => fl!(LANGUAGE_LOADER, "file-kind-bulletin"),
        },
        PacketFileKind::NewFiles => fl!(LANGUAGE_LOADER, "file-kind-new-files"),
        PacketFileKind::Goodbye => fl!(LANGUAGE_LOADER, "file-kind-goodbye"),
    }
}

/// Formats a QWK `MM-DD-YYHH:MM` header date like message dates (`YYYY-MM-DD HH:MM`).
pub fn display_date(date: &str) -> String {
    chrono::NaiveDateTime::parse_from_str(date, "%m-%d-%y%H:%M").map_or_else(|_| date.to_string(), |date| date.format("%Y-%m-%d %H:%M").to_string())
}

/// Compact list date: "Today 08:00", "Yesterday 08:00", the weekday during the past week and
/// the plain date for anything older (or in the future). Unparsed dates keep their raw `fallback`.
pub fn friendly_date(date: chrono::NaiveDateTime, fallback: &str, today: chrono::NaiveDate) -> String {
    if fallback != date.format("%Y-%m-%d %H:%M").to_string() {
        return fallback.to_string();
    }
    let time = date.format("%H:%M").to_string();
    match (today - date.date()).num_days() {
        0 => fl!(LANGUAGE_LOADER, "list-date-today", time = time),
        1 => fl!(LANGUAGE_LOADER, "list-date-yesterday", time = time),
        2..=6 => fl!(
            LANGUAGE_LOADER,
            "list-date-weekday",
            weekday = i64::from(date.weekday().num_days_from_monday()),
            time = time
        ),
        _ => date.format("%Y-%m-%d").to_string(),
    }
}

/// [`friendly_date`] for a QWK `MM-DD-YYHH:MM` header date, as stored in drafts.
pub fn friendly_qwk_date(date: &str, today: chrono::NaiveDate) -> String {
    match chrono::NaiveDateTime::parse_from_str(date, "%m-%d-%y%H:%M") {
        Ok(parsed) => friendly_date(parsed, &parsed.format("%Y-%m-%d %H:%M").to_string(), today),
        Err(_) => date.to_string(),
    }
}

pub fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}
