use eframe::egui;
use icy_engine_gui::egui::appearance;
use icy_mail::reader::{ConferenceColumn, Pane};

use super::{
    app::{Folder, MailApp, Modal},
    widgets::{self, Badge, Icon},
};

impl MailApp {
    /// Packet header, mailboxes and conferences. Returns whether the user picked a folder.
    pub fn sidebar(&mut self, ui: &mut egui::Ui) -> bool {
        let context = ui.ctx().clone();
        let focused = self.focus == Pane::Conferences;
        let bbs = self.bbs_name();
        let user = self.user_name();
        let header = egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 12,
                right: 8,
                top: 10,
                bottom: 4,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    widgets::avatar(ui, &bbs, 34.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.add(egui::Label::new(appearance::bold(ui, &bbs).size(14.0)).truncate().selectable(false));
                        let detail = if user.is_empty() { "Offline mail".to_string() } else { user.clone() };
                        ui.add(egui::Label::new(egui::RichText::new(detail).weak().size(12.0)).truncate().selectable(false));
                    });
                });
            })
            .response;
        let header = ui.interact(header.rect, ui.id().with("packet-header"), egui::Sense::click());
        if header.hovered() {
            ui.painter()
                .rect_filled(header.rect.shrink(4.0), 6.0, ui.visuals().widgets.hovered.weak_bg_fill.gamma_multiply(0.5));
        }
        if header
            .on_hover_text("Packet information")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            self.modal = Some(Modal::PacketInfo);
        }
        let mut picked = None;
        let reveal = std::mem::take(&mut self.reveal_sidebar);
        egui::ScrollArea::vertical().id_salt("sidebar").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            widgets::section(ui, "Mailboxes");
            let total = self.reader.package.as_ref().map_or(0, |package| package.message_count());
            let mut entries = vec![(
                Folder::All,
                Some(Icon::Inbox),
                "All Messages".to_string(),
                unread_badge(self.counts.unread, total),
                format!("{total} messages, {} unread", self.counts.unread),
            )];
            if !user.is_empty() {
                entries.push((
                    Folder::Personal,
                    Some(Icon::Personal),
                    "Personal".to_string(),
                    if self.counts.personal.0 > 0 {
                        Badge::Unread(self.counts.personal.0)
                    } else {
                        Badge::None
                    },
                    format!("Messages addressed to {user}"),
                ));
            }
            let drafts = self.draft_count();
            let problems = self
                .drafts
                .as_ref()
                .map_or(0, |store| store.drafts().iter().filter(|draft| !store.issues(draft).is_empty()).count());
            entries.push((
                Folder::Drafts,
                Some(Icon::Export),
                "Outbox".to_string(),
                if problems > 0 {
                    Badge::Warning(problems)
                } else if drafts > 0 {
                    Badge::Total(drafts)
                } else {
                    Badge::None
                },
                "Replies and new messages waiting to be exported".to_string(),
            ));
            for (folder, icon, label, badge, tooltip) in entries {
                let image = icon.map(|icon| self.icons.image(&context, icon, 18.0));
                let response = widgets::nav_row(ui, image, &label, badge, self.folder == folder, focused).on_hover_text(tooltip);
                if reveal && self.folder == folder {
                    response.scroll_to_me(None);
                }
                if response.clicked() {
                    picked = Some(folder);
                }
            }
            ui.add_space(6.0);
            widgets::section_with(ui, "Conferences", |ui| {
                let image = self.icons.image(&context, Icon::Sort, 16.0);
                let button = ui
                    .add(egui::Button::image(image).frame_when_inactive(false).image_tint_follows_text_color(true))
                    .on_hover_text("Sort conferences");
                egui::Popup::menu(&button).show(|ui| {
                    for (column, label) in [
                        (ConferenceColumn::Area, "By Number"),
                        (ConferenceColumn::Name, "By Name"),
                        (ConferenceColumn::Count, "By Message Count"),
                    ] {
                        let active = self.reader.conference_sort.0 == column;
                        let text = if active {
                            format!("{label} {}", widgets::arrow(self.reader.conference_sort.1))
                        } else {
                            label.to_string()
                        };
                        if ui.add(egui::Button::selectable(active, text)).clicked() {
                            self.reader.sort_conferences(column);
                            self.reveal_sidebar = true;
                            ui.close();
                        }
                    }
                });
            });
            let rows: Vec<_> = self
                .reader
                .conferences
                .iter()
                .filter_map(|row| row.number.map(|number| (number, row.name.clone(), row.count)))
                .collect();
            for (number, name, count) in rows {
                let unread = self.counts.conferences.get(&number).copied().unwrap_or(0);
                let folder = Folder::Conference(number);
                let response = widgets::nav_row(ui, None, &name, unread_badge(unread, count), self.folder == folder, focused)
                    .on_hover_text(format!("Conference {number}\n{count} messages, {unread} unread"));
                if reveal && self.folder == folder {
                    response.scroll_to_me(None);
                }
                if response.clicked() {
                    picked = Some(folder);
                }
            }
            ui.add_space(8.0);
        });
        if let Some(folder) = picked {
            if folder != self.folder {
                self.select_folder(folder);
            }
            self.set_focus(Pane::Conferences, &context);
        }
        picked.is_some()
    }
}

fn unread_badge(unread: usize, total: usize) -> Badge {
    if unread > 0 {
        Badge::Unread(unread)
    } else {
        Badge::Total(total)
    }
}
