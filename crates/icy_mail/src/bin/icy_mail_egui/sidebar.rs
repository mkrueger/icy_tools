use std::collections::HashMap;

use eframe::egui;
use i18n_embed_fl::fl;
use icy_engine_gui::egui::appearance;
use icy_mail::{
    reader::{ConferenceColumn, Pane},
    LANGUAGE_LOADER,
};

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
                        let detail = if user.is_empty() {
                            fl!(LANGUAGE_LOADER, "sidebar-offline-mail")
                        } else {
                            user.clone()
                        };
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
            .on_hover_text(fl!(LANGUAGE_LOADER, "sidebar-packet-information"))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            self.modal = Some(Modal::PacketInfo);
        }
        let mut picked = None;
        let reveal = std::mem::take(&mut self.reveal_sidebar);
        egui::ScrollArea::vertical().id_salt("sidebar").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            widgets::section(ui, &fl!(LANGUAGE_LOADER, "sidebar-mailboxes"));
            let total = self.reader.package.as_ref().map_or(0, |package| package.message_count());
            let mut entries = vec![(
                Folder::All,
                Some(Icon::Inbox),
                fl!(LANGUAGE_LOADER, "folder-all"),
                unread_badge(self.counts.unread, total),
                fl!(LANGUAGE_LOADER, "folder-all-tooltip", count = total, unread = self.counts.unread),
            )];
            if !user.is_empty() {
                entries.push((
                    Folder::Personal,
                    Some(Icon::Personal),
                    fl!(LANGUAGE_LOADER, "folder-personal"),
                    if self.counts.personal.0 > 0 {
                        Badge::Unread(self.counts.personal.0)
                    } else {
                        Badge::None
                    },
                    fl!(LANGUAGE_LOADER, "folder-personal-tooltip", user = user.as_str()),
                ));
            }
            entries.push((
                Folder::Starred,
                Some(Icon::Star),
                fl!(LANGUAGE_LOADER, "folder-starred"),
                if self.counts.starred > 0 {
                    Badge::Total(self.counts.starred)
                } else {
                    Badge::None
                },
                fl!(LANGUAGE_LOADER, "folder-starred-tooltip"),
            ));
            let drafts = self.draft_count();
            let problems = self
                .drafts
                .as_ref()
                .map_or(0, |store| store.drafts().iter().filter(|draft| !store.issues(draft).is_empty()).count());
            entries.push((
                Folder::Drafts,
                Some(Icon::Export),
                fl!(LANGUAGE_LOADER, "folder-outbox"),
                if problems > 0 {
                    Badge::Warning(problems)
                } else if drafts > 0 {
                    Badge::Total(drafts)
                } else {
                    Badge::None
                },
                fl!(LANGUAGE_LOADER, "folder-outbox-tooltip"),
            ));
            let files = self.file_count();
            if files > 0 {
                entries.push((
                    Folder::Bulletins,
                    Some(Icon::Bulletins),
                    fl!(LANGUAGE_LOADER, "folder-bulletins"),
                    Badge::Total(files),
                    fl!(LANGUAGE_LOADER, "folder-bulletins-tooltip"),
                ));
            }
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
            widgets::section_with(ui, &fl!(LANGUAGE_LOADER, "sidebar-conferences"), |ui| {
                let only = self.conferences_unread_only;
                let image = self.icons.image(&context, Icon::Unread, 16.0);
                if ui
                    .add(
                        egui::Button::image(image)
                            .selected(only)
                            .frame_when_inactive(only)
                            .image_tint_follows_text_color(true),
                    )
                    .on_hover_text(fl!(LANGUAGE_LOADER, "sidebar-unread-conferences-only"))
                    .clicked()
                {
                    self.conferences_unread_only = !only;
                    self.reveal_sidebar = true;
                }
                let image = self.icons.image(&context, Icon::Sort, 16.0);
                let button = ui
                    .add(egui::Button::image(image).frame_when_inactive(false).image_tint_follows_text_color(true))
                    .on_hover_text(fl!(LANGUAGE_LOADER, "sidebar-sort-conferences"));
                egui::Popup::menu(&button).show(|ui| {
                    for (column, label) in [
                        (ConferenceColumn::Area, fl!(LANGUAGE_LOADER, "sidebar-sort-number")),
                        (ConferenceColumn::Name, fl!(LANGUAGE_LOADER, "sidebar-sort-name")),
                        (ConferenceColumn::Count, fl!(LANGUAGE_LOADER, "sidebar-sort-count")),
                    ] {
                        let active = self.reader.conference_sort.0 == column;
                        let text = if active {
                            format!("{label} {}", widgets::arrow(self.reader.conference_sort.1))
                        } else {
                            label
                        };
                        if ui.add(egui::Button::selectable(active, text)).clicked() {
                            self.reader.sort_conferences(column);
                            self.reveal_sidebar = true;
                            ui.close();
                        }
                    }
                });
            });
            let mut toggle = None;
            let mut shown = 0;
            for node in self.conference_tree() {
                match node {
                    ConferenceNode::Single { number, name, count } => {
                        if self.filtered_out(number) {
                            continue;
                        }
                        shown += 1;
                        if self.conference_row(ui, number, &name, &name, count, 0.0, reveal, focused) {
                            picked = Some(Folder::Conference(number));
                        }
                    }
                    ConferenceNode::Group { network, members } => {
                        let visible: Vec<_> = members.iter().filter(|(number, _, _)| !self.filtered_out(*number)).collect();
                        if visible.is_empty() {
                            continue;
                        }
                        shown += 1;
                        let open = self.is_network_open(&network, &members);
                        let unread: usize = members.iter().map(|(number, _, _)| self.unread_in(*number)).sum();
                        let total: usize = members.iter().map(|(_, _, count)| count).sum();
                        let tooltip = fl!(
                            LANGUAGE_LOADER,
                            "sidebar-network-tooltip",
                            network = network.as_str(),
                            conferences = members.len(),
                            unread = unread
                        );
                        if widgets::group_row(ui, &network, unread_badge(unread, total), open)
                            .on_hover_text(tooltip)
                            .clicked()
                        {
                            toggle = Some((network.to_lowercase(), !open));
                        }
                        if !open {
                            continue;
                        }
                        for (number, area, count) in visible {
                            let name = format!("{network}.{area}");
                            if self.conference_row(ui, *number, area, &name, *count, 16.0, reveal, focused) {
                                picked = Some(Folder::Conference(*number));
                            }
                        }
                    }
                }
            }
            if shown == 0 && self.conferences_unread_only {
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "sidebar-conferences-all-read")).weak().size(12.0));
                });
            }
            if let Some((network, open)) = toggle {
                self.network_open.insert(network, open);
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

impl MailApp {
    /// One conference entry; `label` is the name shown, `name` the full one for the tooltip.
    #[allow(clippy::too_many_arguments)]
    fn conference_row(&mut self, ui: &mut egui::Ui, number: u16, label: &str, name: &str, count: usize, indent: f32, reveal: bool, focused: bool) -> bool {
        let unread = self.unread_in(number);
        let folder = Folder::Conference(number);
        let details = fl!(LANGUAGE_LOADER, "sidebar-conference-tooltip", number = number, count = count, unread = unread);
        let response = widgets::nav_row_indented(ui, None, label, unread_badge(unread, count), self.folder == folder, focused, indent)
            .on_hover_text(format!("{name}\n{details}"));
        if reveal && self.folder == folder {
            response.scroll_to_me(None);
        }
        response.clicked()
    }
}

fn unread_badge(unread: usize, total: usize) -> Badge {
    if unread > 0 {
        Badge::Unread(unread)
    } else {
        Badge::Total(total)
    }
}

/// A sidebar conference, or a network whose conferences are listed under a collapsible heading.
#[derive(Debug, PartialEq, Eq)]
pub enum ConferenceNode {
    Single {
        number: u16,
        name: String,
        count: usize,
    },
    /// `members` hold the number, the name without the network prefix and the message count.
    Group {
        network: String,
        members: Vec<(u16, String, usize)>,
    },
}

/// Splits `fsx.General` or `DOVE.Debate` into network and area name. The network is a short run of
/// letters and digits directly followed by the area name, so "Mr. Smith" or "v1.2 Beta" stay whole.
pub fn network_of(name: &str) -> Option<(&str, &str)> {
    let (network, area) = name.trim().split_once('.')?;
    let network_ok = (1..=16).contains(&network.len()) && network.chars().all(|ch| ch.is_alphanumeric() || ch == '-' || ch == '_');
    let area_ok = area.chars().next().is_some_and(|ch| !ch.is_whitespace() && !ch.is_ascii_digit());
    (network_ok && area_ok).then_some((network, area))
}

/// Groups conferences of networks with at least two conferences, in the order of their first
/// conference; a single network holding every conference stays flat.
pub fn conference_tree(rows: &[(u16, String, usize)]) -> Vec<ConferenceNode> {
    let key = |name: &str| network_of(name).map(|(network, _)| network.to_lowercase());
    let mut sizes: HashMap<String, usize> = HashMap::new();
    for (_, name, _) in rows {
        if let Some(key) = key(name) {
            *sizes.entry(key).or_default() += 1;
        }
    }
    let flat = sizes.values().any(|size| *size == rows.len());
    let mut nodes = Vec::new();
    let mut groups: HashMap<String, usize> = HashMap::new();
    for (number, name, count) in rows {
        let grouped = key(name).filter(|key| !flat && sizes[key] >= 2);
        let (Some(key), Some((network, area))) = (grouped, network_of(name)) else {
            nodes.push(ConferenceNode::Single {
                number: *number,
                name: name.clone(),
                count: *count,
            });
            continue;
        };
        let position = *groups.entry(key).or_insert_with(|| {
            nodes.push(ConferenceNode::Group {
                network: network.to_string(),
                members: Vec::new(),
            });
            nodes.len() - 1
        });
        if let ConferenceNode::Group { members, .. } = &mut nodes[position] {
            members.push((*number, area.to_string(), *count));
        }
    }
    nodes
}

impl MailApp {
    pub fn conference_tree(&self) -> Vec<ConferenceNode> {
        let rows: Vec<_> = self
            .reader
            .conferences
            .iter()
            .filter_map(|row| row.number.map(|number| (number, row.name.clone(), row.count)))
            .collect();
        conference_tree(&rows)
    }

    fn unread_in(&self, number: u16) -> usize {
        self.counts.conferences.get(&number).copied().unwrap_or(0)
    }

    /// Whether the unread filter hides a conference; the open one always stays.
    fn filtered_out(&self, number: u16) -> bool {
        self.conferences_unread_only && self.unread_in(number) == 0 && self.folder != Folder::Conference(number)
    }

    /// Networks open while they hold unread messages or the open conference, unless the user decided otherwise.
    pub fn is_network_open(&self, network: &str, members: &[(u16, String, usize)]) -> bool {
        self.network_open.get(&network.to_lowercase()).copied().unwrap_or_else(|| {
            members
                .iter()
                .any(|(number, _, _)| self.unread_in(*number) > 0 || self.folder == Folder::Conference(*number))
        })
    }

    /// Conferences in sidebar order; `visible` leaves out closed networks and filtered conferences.
    pub fn conference_folders(&self, visible: bool) -> Vec<Folder> {
        let mut folders = Vec::new();
        for node in self.conference_tree() {
            match node {
                ConferenceNode::Single { number, .. } => {
                    if !visible || !self.filtered_out(number) {
                        folders.push(Folder::Conference(number));
                    }
                }
                ConferenceNode::Group { network, members } => {
                    if visible && !self.is_network_open(&network, &members) {
                        continue;
                    }
                    folders.extend(
                        members
                            .iter()
                            .filter(|(number, _, _)| !visible || !self.filtered_out(*number))
                            .map(|(number, _, _)| Folder::Conference(*number)),
                    );
                }
            }
        }
        folders
    }

    /// Opens the network of `number` if the user had closed it, so a selected conference stays visible.
    pub fn reveal_conference(&mut self, number: u16) {
        let name = self.folder_name(Folder::Conference(number));
        if let Some((network, _)) = network_of(&name) {
            let key = network.to_lowercase();
            if self.network_open.get(&key) == Some(&false) {
                self.network_open.insert(key, true);
            }
        }
    }
}
