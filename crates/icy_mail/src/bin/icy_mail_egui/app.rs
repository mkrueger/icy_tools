use std::{collections::HashMap, path::PathBuf, sync::Arc};

use eframe::egui::{self, Key, Vec2};
use icy_engine::{BufferType, Position, Selection, Size, TextScreen};
use icy_engine_gui::{egui::screen::ScreenView, MonitorSettings, ScalingMode};
use icy_mail::{
    drafts::{Compose, Draft, DraftStore},
    reader::{NavigateDirection, Pane, Reader, ViewMode},
    state::{ReadState, RecentPackets},
};
use parking_lot::Mutex;

use super::{
    composer::Composer,
    loading::{Event, Loader},
    widgets::{Icons, ROW_HEIGHT},
};

/// What the sidebar has selected and the list shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Folder {
    All,
    Personal,
    Drafts,
    Conference(u16),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoticeKind {
    Info,
    Success,
    Warning,
}

/// Transient status bar message.
pub struct Notice {
    pub text: String,
    pub kind: NoticeKind,
    pub until: f64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AfterDiscard {
    /// Leave the composer.
    Close,
    /// Close the window.
    Quit,
}

pub enum Modal {
    Shortcuts,
    PacketInfo,
    DeleteDraft(u64),
    Discard(AfterDiscard),
    ExportProblems(Vec<String>),
}

/// Unread counts for the sidebar, refreshed when the packet or the read marks change.
#[derive(Default)]
pub struct Counts {
    pub unread: usize,
    pub personal: (usize, usize),
    pub conferences: HashMap<u16, usize>,
}

pub struct MailApp {
    pub reader: Reader,
    pub screen: ScreenView,
    pub settings: MonitorSettings,
    pub focus: Pane,
    pub loader: Loader,
    pub path: Option<PathBuf>,
    pub loading: Option<PathBuf>,
    pub body_loading: bool,
    pub error: Option<String>,
    pub drafts: Option<DraftStore>,
    pub read_state: Option<ReadState>,
    pub recent: Option<RecentPackets>,
    /// Keeps drafts, read marks and the recent list here instead of the user's data directory.
    pub storage: Option<PathBuf>,
    pub folder: Folder,
    pub selected_draft: Option<u64>,
    pub composer: Option<Composer>,
    pub modal: Option<Modal>,
    pub notice: Option<Notice>,
    pub counts: Counts,
    pub icons: Icons,
    pub rendered: Option<usize>,
    pub selection_anchor: Option<Position>,
    pub reveal_sidebar: bool,
    pub reveal_message: bool,
    pub message_view: Vec2,
    pub content_rect: egui::Rect,
    pub new_window: bool,
    pub closed: bool,
    children: Vec<(egui::ViewportId, Arc<Mutex<MailApp>>)>,
}

impl MailApp {
    pub fn new(context: &egui::Context) -> Self {
        Self::create(context, None)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_storage(context: &egui::Context, storage: PathBuf) -> Self {
        Self::create(context, Some(storage))
    }

    fn create(_context: &egui::Context, storage: Option<PathBuf>) -> Self {
        let settings = MonitorSettings {
            scaling_mode: ScalingMode::FitWidth,
            ..Default::default()
        };
        let recent = match &storage {
            Some(directory) => RecentPackets::open_in(directory),
            None => RecentPackets::open(),
        }
        .ok();
        Self {
            reader: Reader::default(),
            screen: ScreenView::new(TextScreen::new(Size::new(80, 25))),
            settings,
            focus: Pane::Messages,
            loader: Loader::default(),
            path: None,
            loading: None,
            body_loading: false,
            error: None,
            drafts: None,
            read_state: None,
            recent,
            storage,
            folder: Folder::All,
            selected_draft: None,
            composer: None,
            modal: None,
            notice: None,
            counts: Counts::default(),
            icons: Icons::default(),
            rendered: None,
            selection_anchor: None,
            reveal_sidebar: false,
            reveal_message: false,
            message_view: Vec2::ZERO,
            content_rect: egui::Rect::NOTHING,
            new_window: false,
            closed: false,
            children: Vec::new(),
        }
    }

    pub fn open(&mut self, path: PathBuf, context: &egui::Context) {
        if self.composer.is_some() {
            self.notify(context, NoticeKind::Warning, "Save or discard the message you are writing first");
            return;
        }
        self.loading = Some(path.clone());
        self.error = None;
        self.body_loading = false;
        self.loader.package(path, context);
    }

    pub fn reload(&mut self, context: &egui::Context) {
        if let Some(path) = self.path.clone() {
            self.open(path, context);
        }
    }

    pub fn notify(&mut self, context: &egui::Context, kind: NoticeKind, text: impl Into<String>) {
        let until = context.input(|input| input.time) + if kind == NoticeKind::Warning { 8.0 } else { 4.0 };
        self.notice = Some(Notice {
            text: text.into(),
            kind,
            until,
        });
    }

    pub fn poll(&mut self, context: &egui::Context) {
        while let Ok(event) = self.loader.receiver.try_recv() {
            match event {
                Event::Package(generation, path, result) if generation == self.loader.package_generation => {
                    self.loading = None;
                    match result {
                        Ok(package) => self.loaded(context, path, package),
                        Err(error) => {
                            self.error = Some(format!("{}\n{error}", path.display()));
                            self.rendered = None;
                            if !path.exists() {
                                if let Some(recent) = &mut self.recent {
                                    let _ = recent.remove(&path);
                                }
                            }
                        }
                    }
                }
                Event::Body(generation, result) if generation == self.loader.body_generation => {
                    self.body_loading = false;
                    match result {
                        Ok(screen) => {
                            self.screen = ScreenView::new(screen);
                            if self.folder != Folder::Drafts {
                                if let Some(index) = self.rendered {
                                    if !self.reader.read.contains(&index) {
                                        self.set_read(context, &[index], true);
                                    }
                                }
                            }
                        }
                        Err(error) => {
                            self.screen = ScreenView::new(TextScreen::new(Size::new(80, 25)));
                            self.error = Some(error);
                        }
                    }
                }
                Event::Picked(path) => {
                    self.loader.picking = false;
                    if let Some(path) = path {
                        self.open(path, context);
                    }
                }
                Event::Exported(result) => {
                    self.loader.export_picking = false;
                    if let Some((path, result)) = result {
                        match result {
                            Ok(()) => {
                                let file = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                                self.notify(context, NoticeKind::Success, format!("Reply packet saved as {file} - upload it to the BBS"));
                            }
                            Err(error) => self.error = Some(format!("Unable to export {}:\n{error}", path.display())),
                        }
                    }
                }
                _ => {}
            }
        }
        self.sync_body(context);
    }

    fn loaded(&mut self, context: &egui::Context, path: PathBuf, package: Arc<icy_mail::qwk::QwkPackage>) {
        let drafts = match &self.storage {
            Some(directory) => DraftStore::open_in(&path, &package, directory),
            None => DraftStore::open(&path, &package),
        };
        let drafts = match drafts {
            Ok(drafts) => drafts,
            Err(error) => {
                self.error = Some(format!("Unable to load the drafts for {}:\n{error}", path.display()));
                return;
            }
        };
        let read_state = match &self.storage {
            Some(directory) => ReadState::open_in(&path, &package, directory),
            None => ReadState::open(&path, &package),
        };
        let reload = self.path.as_ref() == Some(&path);
        let previous = (self.folder, self.reader.selected_message, self.reader.view_mode, self.reader.message_sort);
        self.reader.set_package(package.clone());
        match read_state {
            Ok(state) => {
                self.reader.read = state.indices(&package);
                self.read_state = Some(state);
            }
            Err(error) => {
                self.read_state = None;
                self.notify(context, NoticeKind::Warning, format!("Read marks are unavailable: {error}"));
            }
        }
        self.drafts = Some(drafts);
        self.path = Some(path.clone());
        self.rendered = None;
        self.refresh_counts();
        if reload {
            self.reader.view_mode = previous.2;
            self.reader.message_sort = previous.3;
            self.select_folder(previous.0);
            if let Some(index) = previous.1 {
                self.reader.select_message(index);
            }
        } else {
            self.folder = Folder::All;
            self.selected_draft = None;
            self.select_folder(Folder::All);
            self.focus = Pane::Messages;
        }
        self.reveal_sidebar = true;
        self.reveal_message = true;
        if let Some(recent) = &mut self.recent {
            if let Err(error) = recent.add(&path) {
                log::warn!("unable to update the recent packet list: {error}");
            }
        }
        context.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "{} - Icy Mail",
            path.file_name().unwrap_or_default().to_string_lossy()
        )));
    }

    fn sync_body(&mut self, context: &egui::Context) {
        if self.loading.is_some() || self.folder == Folder::Drafts || self.rendered == self.reader.selected_message {
            return;
        }
        self.rendered = self.reader.selected_message;
        self.selection_anchor = None;
        self.reveal_message = true;
        if let (Some(package), Some(index)) = (&self.reader.package, self.reader.selected_message) {
            self.body_loading = true;
            self.loader.body(package.clone(), index, context);
        } else {
            self.loader.body_generation = self.loader.body_generation.wrapping_add(1);
            self.body_loading = false;
            self.screen = ScreenView::new(TextScreen::new(Size::new(80, 25)));
        }
    }

    pub fn user_name(&self) -> String {
        self.reader
            .package
            .as_ref()
            .map(|package| package.control_file.qmail_user_name.to_string().trim().to_string())
            .unwrap_or_default()
    }

    pub fn bbs_name(&self) -> String {
        self.reader
            .package
            .as_ref()
            .map(|package| package.control_file.bbs_name.to_string().trim().to_string())
            .unwrap_or_default()
    }

    pub fn refresh_counts(&mut self) {
        let mut counts = Counts::default();
        let user = self.user_name();
        if let Some(package) = &self.reader.package {
            for info in &package.infos {
                let unread = !self.reader.read.contains(&info.index);
                let personal = !user.is_empty() && info.to.trim().eq_ignore_ascii_case(&user);
                if personal {
                    counts.personal.1 += 1;
                }
                if unread {
                    counts.unread += 1;
                    *counts.conferences.entry(info.conference).or_default() += 1;
                    if personal {
                        counts.personal.0 += 1;
                    }
                }
            }
        }
        self.counts = counts;
    }

    /// Sidebar entries in display order.
    pub fn folders(&self) -> Vec<Folder> {
        if self.reader.package.is_none() {
            return Vec::new();
        }
        let mut folders = vec![Folder::All];
        if !self.user_name().is_empty() {
            folders.push(Folder::Personal);
        }
        folders.push(Folder::Drafts);
        folders.extend(self.reader.conferences.iter().filter_map(|row| row.number.map(Folder::Conference)));
        folders
    }

    pub fn select_folder(&mut self, folder: Folder) {
        self.folder = folder;
        self.reveal_message = true;
        self.reveal_sidebar = true;
        if folder == Folder::Drafts {
            let drafts = self.drafts.as_ref().map(DraftStore::drafts).unwrap_or_default();
            if !drafts.iter().any(|draft| Some(draft.id) == self.selected_draft) {
                self.selected_draft = drafts.first().map(|draft| draft.id);
            }
            return;
        }
        self.reader.personal = (folder == Folder::Personal).then(|| self.user_name());
        self.reader.select_conference(match folder {
            Folder::Conference(number) => Some(number),
            _ => None,
        });
        if let Some(row) = self.reader.messages.iter().find(|row| !self.reader.read.contains(&row.index)) {
            self.reader.selected_message = Some(row.index);
        }
    }

    pub fn folder_name(&self, folder: Folder) -> String {
        match folder {
            Folder::All => "All Messages".into(),
            Folder::Personal => "Personal".into(),
            Folder::Drafts => "Outbox".into(),
            Folder::Conference(number) => self
                .reader
                .conferences
                .iter()
                .find(|row| row.number == Some(number))
                .map_or_else(|| format!("Conference {number}"), |row| row.name.clone()),
        }
    }

    pub fn draft_count(&self) -> usize {
        self.drafts.as_ref().map_or(0, |store| store.drafts().len())
    }

    pub fn selected_info(&self) -> Option<&icy_mail::qwk::MessageInfo> {
        let package = self.reader.package.as_ref()?;
        package.infos.get(self.reader.selected_message?)
    }

    /// Whether a message action (reply, forward, mark) applies to the list selection.
    pub fn message_selected(&self) -> bool {
        self.folder != Folder::Drafts && self.reader.selected_message.is_some() && self.composer.is_none()
    }

    pub fn set_read(&mut self, context: &egui::Context, indices: &[usize], read: bool) {
        let Some(package) = self.reader.package.clone() else {
            return;
        };
        let infos: Vec<_> = indices.iter().filter_map(|index| package.infos.get(*index)).collect();
        if let Some(state) = &mut self.read_state {
            if let Err(error) = state.set(infos.iter().copied(), read) {
                self.notify(context, NoticeKind::Warning, format!("Unable to save read marks: {error}"));
            }
        }
        for info in infos {
            if read {
                self.reader.read.insert(info.index);
            } else {
                self.reader.read.remove(&info.index);
            }
        }
        self.refresh_counts();
    }

    pub fn toggle_read(&mut self, context: &egui::Context) {
        if let Some(index) = self.reader.selected_message.filter(|_| self.message_selected()) {
            let read = !self.reader.read.contains(&index);
            self.set_read(context, &[index], read);
            if !read {
                // Keep the message unread instead of marking it again as soon as it is shown.
                self.rendered = self.reader.selected_message;
            }
        }
    }

    pub fn mark_folder_read(&mut self, context: &egui::Context) {
        if self.folder == Folder::Drafts {
            return;
        }
        let indices: Vec<_> = self.reader.messages.iter().map(|row| row.index).collect();
        let count = indices.iter().filter(|index| !self.reader.read.contains(index)).count();
        self.set_read(context, &indices, true);
        if count > 0 {
            self.notify(context, NoticeKind::Info, format!("Marked {count} messages as read"));
        }
    }

    /// Selects the next unread message, continuing in the following conferences.
    pub fn next_unread(&mut self, context: &egui::Context) {
        if self.reader.package.is_none() {
            return;
        }
        if self.folder != Folder::Drafts {
            let start = self.reader.selected_position().map_or(0, |position| position + 1);
            if let Some(row) = self.reader.messages[start.min(self.reader.messages.len())..]
                .iter()
                .find(|row| !self.reader.read.contains(&row.index))
            {
                self.reader.selected_message = Some(row.index);
                self.reveal_message = true;
                return;
            }
        }
        let folders = self.folders();
        let current = folders.iter().position(|folder| *folder == self.folder).unwrap_or(0);
        let next = folders.iter().skip(current + 1).find_map(|folder| match folder {
            Folder::Conference(number) if self.counts.conferences.get(number).copied().unwrap_or(0) > 0 => Some(*folder),
            _ => None,
        });
        match next {
            Some(folder) => {
                self.select_folder(folder);
                let name = self.folder_name(folder);
                self.notify(context, NoticeKind::Info, format!("Continuing in {name}"));
            }
            None => self.notify(context, NoticeKind::Info, "No more unread messages"),
        }
    }

    pub fn set_mode(&mut self, mode: ViewMode) {
        self.reader.view_mode = mode;
        self.reader.rebuild_messages();
        self.reveal_message = true;
    }

    pub fn set_unread_only(&mut self, unread_only: bool) {
        self.reader.unread_only = unread_only;
        self.reader.rebuild_messages();
        self.reveal_message = true;
    }

    pub fn filter_changed(&mut self) {
        self.reader.rebuild_messages();
        self.reveal_message = true;
    }

    pub fn new_draft(&mut self, context: &egui::Context) {
        let Some(package) = self.reader.package.clone() else {
            return;
        };
        let choices = conference_choices(&package);
        let conference = match self.folder {
            Folder::Conference(number) => Some(number),
            _ => self.selected_info().map(|info| info.conference),
        }
        .filter(|number| choices.iter().any(|(choice, _)| choice == number))
        .or_else(|| choices.first().map(|(number, _)| *number));
        let Some(conference) = conference else {
            self.error = Some("This packet has no conference to post a message in.".into());
            return;
        };
        if let Some(store) = &self.drafts {
            match store.prepare(&package, Compose::New { conference }) {
                Ok(mut draft) => {
                    draft.to = "ALL".into();
                    self.start_composer(context, Composer::new(draft, false, None));
                }
                Err(error) => self.error = Some(format!("Unable to start a new message: {error}")),
            }
        }
    }

    pub fn reply(&mut self, context: &egui::Context, forward: bool) {
        if !self.message_selected() {
            return;
        }
        let (Some(package), Some(index)) = (self.reader.package.clone(), self.reader.selected_message) else {
            return;
        };
        let Some(info) = package.infos.get(index) else {
            self.error = Some("The selected message is no longer available.".into());
            return;
        };
        let message = match package.get_message(index) {
            Ok(message) => message,
            Err(error) => {
                self.error = Some(format!("Unable to read the original message: {error}"));
                return;
            }
        };
        let Some(store) = &self.drafts else {
            return;
        };
        let mut draft = match store.prepare(&package, if forward { Compose::Forward { index } } else { Compose::Reply { index } }) {
            Ok(draft) => draft,
            Err(error) => {
                self.error = Some(format!("Unable to start the reply: {error}"));
                return;
            }
        };
        let body = decode_cp437(&message.text);
        let quoted: String = body.trim_end().lines().map(|line| format!("> {line}\n")).collect();
        draft.body = if forward {
            format!(
                "\n\n--- Forwarded message ---\nFrom: {}\nTo: {}\nDate: {}\nSubject: {}\n\n{quoted}",
                info.from, info.to, info.date_str, info.subject
            )
        } else {
            format!("\n\nOn {} {} wrote:\n{quoted}", info.date_str, info.from)
        };
        let origin = format!("{} \u{00b7} {} \u{00b7} #{}", info.from, info.subject, info.number);
        self.start_composer(context, Composer::new(draft, false, Some(origin)));
    }

    pub fn edit_draft(&mut self, context: &egui::Context, id: u64) {
        let draft = self
            .drafts
            .as_ref()
            .and_then(|store| store.drafts().iter().find(|draft| draft.id == id))
            .cloned();
        if let Some(draft) = draft {
            let origin = self.origin(&draft);
            self.start_composer(context, Composer::new(draft, true, origin));
        } else {
            self.error = Some("The draft is no longer available.".into());
        }
    }

    /// The message a reply refers to, for the composer heading.
    fn origin(&self, draft: &Draft) -> Option<String> {
        if draft.ref_number == 0 {
            return None;
        }
        let package = self.reader.package.as_ref()?;
        let info = package
            .infos
            .iter()
            .find(|info| info.number == draft.ref_number && info.conference == draft.conference)?;
        Some(format!("{} \u{00b7} {} \u{00b7} #{}", info.from, info.subject, info.number))
    }

    fn start_composer(&mut self, context: &egui::Context, composer: Composer) {
        self.composer = Some(composer);
        self.notice = None;
        context.memory_mut(|memory| memory.surrender_focus(egui::Id::new("mail-search")));
    }

    pub fn delete_draft(&mut self, context: &egui::Context, id: u64) {
        let Some(store) = &self.drafts else {
            return;
        };
        let mut next = store.clone();
        let position = next.drafts().iter().position(|draft| draft.id == id);
        match next.delete(id) {
            Ok(()) => {
                if self.selected_draft == Some(id) {
                    self.selected_draft = position
                        .and_then(|position| next.drafts().get(position).or_else(|| next.drafts().last()))
                        .map(|draft| draft.id);
                }
                self.drafts = Some(next);
                if self.composer.as_ref().is_some_and(|composer| composer.draft.id == id) {
                    self.composer = None;
                }
                self.notify(context, NoticeKind::Info, "Draft deleted");
            }
            Err(error) => self.error = Some(format!("Unable to delete the draft: {error}")),
        }
    }

    pub fn export(&mut self, context: &egui::Context) {
        let (Some(path), Some(store)) = (&self.path, &self.drafts) else {
            return;
        };
        if store.drafts().is_empty() {
            self.notify(context, NoticeKind::Info, "There are no replies to export");
            return;
        }
        let mut problems = Vec::new();
        for draft in store.drafts() {
            if let Some(issue) = store.issues(draft).first() {
                problems.push(format!("{}: {}", draft_title(draft), issue.message));
            }
        }
        if !problems.is_empty() {
            self.modal = Some(Modal::ExportProblems(problems));
            self.select_folder(Folder::Drafts);
            return;
        }
        self.loader.pick_export(store.default_export_path(path), store.clone(), context);
    }

    pub fn set_focus(&mut self, pane: Pane, context: &egui::Context) {
        self.focus = pane;
        context.memory_mut(|memory| memory.request_focus(egui::Id::new(format!("mail-pane-{pane:?}"))));
    }

    /// Whether leaving now would lose typed text, in this window or one it opened.
    fn unsaved(&self) -> bool {
        self.composer.as_ref().is_some_and(Composer::dirty) || self.children.iter().any(|(_, child)| child.lock().unsaved())
    }

    pub fn close(&mut self, context: &egui::Context) {
        if self.unsaved() {
            self.modal = Some(Modal::Discard(AfterDiscard::Quit));
            return;
        }
        self.closed = true;
        context.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    pub fn discard_and_close(&mut self, context: &egui::Context) {
        self.composer = None;
        for (_, child) in &self.children {
            child.lock().composer = None;
        }
        self.closed = true;
        context.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    pub fn show(&mut self, context: &egui::Context) {
        if !context.will_discard() {
            self.poll(context);
        }
        let time = context.input(|input| input.time);
        if let Some(notice) = &self.notice {
            if notice.until <= time {
                self.notice = None;
            } else {
                context.request_repaint_after(std::time::Duration::from_secs_f64(notice.until - time));
            }
        }
        let blocked = self.error.is_some() || self.modal.is_some();
        if !blocked && !context.will_discard() {
            if self.composer.is_some() {
                self.composer_keys(context);
            } else {
                self.keys(context);
                let dropped = context.input(|input| input.raw.dropped_files.iter().find_map(|file| file.path.clone()));
                if let Some(path) = dropped {
                    self.open(path, context);
                }
            }
        }
        let composing = self.composer.is_some();
        egui::TopBottomPanel::top("toolbar")
            .frame(egui::Frame::side_top_panel(&context.style()).inner_margin(egui::Margin::symmetric(8, 6)))
            .show(context, |ui| {
                ui.add_enabled_ui(!blocked && !composing, |ui| self.toolbar(ui));
            });
        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::side_top_panel(&context.style()).inner_margin(egui::Margin::symmetric(8, 2)))
            .exact_height(26.0)
            .show(context, |ui| {
                ui.add_enabled_ui(!blocked, |ui| self.status_bar(ui));
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(context.style().visuals.panel_fill))
            .show(context, |ui| {
                ui.add_enabled_ui(!blocked && self.loading.is_none(), |ui| {
                    if self.reader.package.is_none() {
                        self.welcome(ui);
                    } else if composing {
                        self.composer_view(ui);
                    } else if ui.available_width() < 760.0 || ui.available_height() < 300.0 {
                        self.narrow(ui);
                    } else {
                        egui::SidePanel::left("sidebar")
                            .default_width(240.0)
                            .width_range(180.0..=380.0)
                            .frame(egui::Frame::new().fill(sidebar_fill(ui.visuals())))
                            .show_inside(ui, |ui| {
                                self.sidebar(ui);
                            });
                        egui::TopBottomPanel::top("messages")
                            .default_height(260.0)
                            .height_range(120.0..=(ui.available_height() - 140.0).max(120.0))
                            .resizable(true)
                            .frame(egui::Frame::new())
                            .show_inside(ui, |ui| {
                                self.list(ui);
                            });
                        egui::CentralPanel::default().frame(egui::Frame::new()).show_inside(ui, |ui| self.content(ui));
                    }
                });
            });
        if !blocked && !context.will_discard() {
            self.sync_body(context);
        }
        self.modals(context);
        self.windows(context);
    }

    fn narrow(&mut self, ui: &mut egui::Ui) {
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            for (pane, label) in [(Pane::Conferences, "Folders"), (Pane::Messages, "Messages"), (Pane::Content, "Message")] {
                if super::widgets::pill(ui, self.focus == pane, label).clicked() {
                    self.set_focus(pane, ui.ctx());
                }
            }
        });
        ui.add_space(3.0);
        let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), stroke);
        match self.focus {
            Pane::Conferences => {
                if self.sidebar(ui) {
                    self.set_focus(Pane::Messages, ui.ctx());
                }
            }
            Pane::Messages => {
                // A tap opens the message; the list stays reachable through the pane switcher.
                if self.list(ui) && self.folder != Folder::Drafts {
                    self.set_focus(Pane::Content, ui.ctx());
                }
            }
            Pane::Content => self.content(ui),
        }
    }

    pub fn cell(&self, position: egui::Pos2) -> Option<Position> {
        let info = self.screen.terminal.render_info.read();
        let (column, row) = info.screen_to_cell(position.x, position.y)?;
        Some(Position::new(
            column + (self.screen.terminal.scroll_x() / info.font_width.max(1.0)) as i32,
            row + (self.screen.terminal.scroll_y() / info.font_height.max(1.0)) as i32,
        ))
    }

    pub fn copy(&self, context: &egui::Context) {
        if !self.body_loading {
            if let Some(text) = self.screen.terminal.screen.lock().copy_text() {
                context.copy_text(text);
            }
        }
    }

    pub fn copy_message(&mut self, context: &egui::Context) {
        if self.body_loading || self.reader.selected_message.is_none() || self.folder == Folder::Drafts {
            return;
        }
        {
            let mut screen = self.screen.terminal.screen.lock();
            let old = screen.selection();
            let mut selection = Selection::new((0, 0));
            selection.lead = (screen.width() - 1, screen.height() - 1).into();
            let _ = screen.set_selection(selection);
            if let Some(text) = screen.copy_text() {
                context.copy_text(text.trim_end().to_string());
            }
            if let Some(selection) = old {
                let _ = screen.set_selection(selection);
            } else {
                let _ = screen.clear_selection();
            }
        }
        self.notify(context, NoticeKind::Info, "Message copied to the clipboard");
    }

    fn scroll_content(&mut self, direction: NavigateDirection) {
        let mut offset = self.screen.offset;
        let page = (self.content_rect.height() - ROW_HEIGHT).max(ROW_HEIGHT);
        offset.y = match direction {
            NavigateDirection::Up => offset.y - ROW_HEIGHT,
            NavigateDirection::Down => offset.y + ROW_HEIGHT,
            NavigateDirection::PageUp => offset.y - page,
            NavigateDirection::PageDown => offset.y + page,
            NavigateDirection::First => 0.0,
            NavigateDirection::Last => self.screen.max_offset.y,
        };
        self.screen.scroll_to = Some(offset.max(Vec2::ZERO));
    }

    fn navigate_list(&mut self, pane: Pane, direction: NavigateDirection) {
        match pane {
            Pane::Conferences => {
                let folders = self.folders();
                if let Some(current) = folders.iter().position(|folder| *folder == self.folder) {
                    let next = icy_mail::reader::step(current, direction, folders.len());
                    if next != current {
                        self.select_folder(folders[next]);
                    }
                }
            }
            Pane::Messages if self.folder == Folder::Drafts => {
                let Some(store) = &self.drafts else {
                    return;
                };
                let drafts = store.drafts();
                if drafts.is_empty() {
                    return;
                }
                let current = drafts.iter().position(|draft| Some(draft.id) == self.selected_draft).unwrap_or(0);
                self.selected_draft = Some(drafts[icy_mail::reader::step(current, direction, drafts.len())].id);
                self.reveal_message = true;
            }
            _ => {
                self.reader.navigate(Pane::Messages, direction);
                self.reveal_message = true;
            }
        }
    }

    fn keys(&mut self, context: &egui::Context) {
        if key(context, Key::F1, false, false) {
            self.modal = Some(Modal::Shortcuts);
            return;
        }
        if key(context, Key::O, true, false) {
            self.loader.pick(context);
        }
        if key(context, Key::N, true, true) {
            self.new_window = true;
        }
        if key(context, Key::W, true, false) {
            self.close(context);
        }
        if self.reader.package.is_none() {
            return;
        }
        if key(context, Key::N, true, false) {
            self.new_draft(context);
        }
        if key(context, Key::R, true, false) {
            self.reply(context, false);
        }
        if key(context, Key::L, true, false) {
            self.reply(context, true);
        }
        if key(context, Key::E, true, true) {
            self.export(context);
        }
        if key(context, Key::T, true, false) {
            self.set_mode(if self.reader.view_mode == ViewMode::List {
                ViewMode::Threads
            } else {
                ViewMode::List
            });
        }
        if key(context, Key::F, true, false) {
            context.memory_mut(|memory| memory.request_focus(egui::Id::new("mail-search")));
        }
        if key(context, Key::F5, false, false) {
            self.reload(context);
        }
        if self.composer.is_some() || self.loading.is_some() || egui::Popup::is_any_open(context) {
            return;
        }
        if context.memory(|memory| memory.focused() == Some(egui::Id::new("mail-search"))) {
            if key(context, Key::Escape, false, false) {
                self.reader.filter.clear();
                self.filter_changed();
                self.set_focus(Pane::Messages, context);
            }
            if key(context, Key::Enter, false, false) || key(context, Key::ArrowDown, false, false) {
                self.set_focus(Pane::Messages, context);
            }
            return;
        }
        for (pressed, direction) in [
            (Key::ArrowUp, NavigateDirection::Up),
            (Key::ArrowDown, NavigateDirection::Down),
            (Key::Home, NavigateDirection::First),
            (Key::End, NavigateDirection::Last),
            (Key::PageUp, NavigateDirection::PageUp),
            (Key::PageDown, NavigateDirection::PageDown),
        ] {
            if key(context, pressed, false, false) {
                if self.focus == Pane::Content && self.folder != Folder::Drafts {
                    self.scroll_content(direction);
                } else {
                    self.navigate_list(self.focus, direction);
                }
            }
        }
        if key(context, Key::Space, false, false) && self.folder != Folder::Drafts {
            if self.screen.offset.y + 1.0 < self.screen.max_offset.y {
                self.scroll_content(NavigateDirection::PageDown);
            } else {
                self.next_unread(context);
            }
        }
        if key(context, Key::N, false, false) {
            self.next_unread(context);
        }
        if key(context, Key::M, false, false) {
            self.toggle_read(context);
        }
        if key(context, Key::C, false, true) {
            self.mark_folder_read(context);
        }
        if key(context, Key::Tab, false, false) {
            self.set_focus(self.focus.cycle(true), context);
        }
        if key(context, Key::Tab, false, true) {
            self.set_focus(self.focus.cycle(false), context);
        }
        if self.folder == Folder::Drafts && self.focus != Pane::Conferences {
            if let Some(id) = self.selected_draft {
                if key(context, Key::Enter, false, false) {
                    self.edit_draft(context, id);
                    return;
                }
                if key(context, Key::Delete, false, false) || key(context, Key::Backspace, false, false) {
                    self.modal = Some(Modal::DeleteDraft(id));
                    return;
                }
            }
        }
        if key(context, Key::Enter, false, false) && self.focus != Pane::Content {
            self.set_focus(self.focus.cycle(true), context);
        }
        if key(context, Key::Escape, false, false) && !self.reader.filter.is_empty() {
            self.reader.filter.clear();
            self.filter_changed();
        }
        if self.focus == Pane::Content {
            let copy_event = context.input(|input| input.events.iter().any(|event| matches!(event, egui::Event::Copy)));
            if key(context, Key::C, true, false) || copy_event {
                self.copy(context);
            }
            if key(context, Key::A, true, false) {
                let mut screen = self.screen.terminal.screen.lock();
                let mut selection = Selection::new((0, 0));
                selection.lead = (screen.width() - 1, screen.height() - 1).into();
                let _ = screen.set_selection(selection);
            }
        }
    }

    fn windows(&mut self, context: &egui::Context) {
        if self.new_window && !context.will_discard() {
            self.new_window = false;
            let mut child = Self::create(context, self.storage.clone());
            child.settings = self.settings.clone();
            let id = egui::ViewportId::from_hash_of(("mail-window", child.screen.shader_state.instance_id));
            self.children.push((id, Arc::new(Mutex::new(child))));
        }
        self.children.retain(|(_, child)| !child.lock().closed);
        for (id, child) in &self.children {
            let child = child.clone();
            let parent = context.viewport_id();
            context.show_viewport_deferred(*id, super::viewport(), move |context, _| {
                let mut child = child.lock();
                child.show(context);
                if child.closed {
                    context.request_repaint_of(parent);
                }
            });
        }
        if context.input(|input| input.viewport().close_requested()) && !self.closed {
            if self.unsaved() {
                context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.modal = Some(Modal::Discard(AfterDiscard::Quit));
            } else {
                self.closed = true;
            }
        }
        if self.closed {
            for (id, child) in &self.children {
                child.lock().closed = true;
                context.send_viewport_cmd_to(*id, egui::ViewportCommand::Close);
            }
        }
    }
}

pub fn sidebar_fill(visuals: &egui::Visuals) -> egui::Color32 {
    if visuals.dark_mode {
        visuals.panel_fill.lerp_to_gamma(egui::Color32::BLACK, 0.18)
    } else {
        visuals.panel_fill.lerp_to_gamma(visuals.widgets.inactive.bg_fill, 0.35)
    }
}

pub fn draft_title(draft: &Draft) -> String {
    if draft.subject.trim().is_empty() {
        "(no subject)".into()
    } else {
        draft.subject.clone()
    }
}

pub fn decode_cp437(bytes: &[u8]) -> String {
    bytes
        .iter()
        .filter_map(|byte| match *byte {
            b'\n' => Some('\n'),
            b'\t' => Some('\t'),
            0..=31 | 127 => None,
            byte => Some(BufferType::CP437.convert_to_unicode(byte as char)),
        })
        .collect::<String>()
        .trim_end()
        .to_owned()
}

pub fn conference_choices(package: &icy_mail::qwk::QwkPackage) -> Vec<(u16, String)> {
    let mut conferences: std::collections::BTreeMap<_, _> = package
        .control_file
        .conferences
        .iter()
        .map(|conference| (conference.number, decode_cp437(&conference.name)))
        .collect();
    for (number, name, _) in package.conferences() {
        conferences.entry(number).or_insert(name);
    }
    conferences.into_iter().collect()
}

pub fn key(context: &egui::Context, key: Key, command: bool, shift: bool) -> bool {
    context.input_mut(|input| {
        let mut found = false;
        input.events.retain(|event| {
            let matched = matches!(event, egui::Event::Key { key: pressed, pressed: true, modifiers, .. }
                if *pressed == key && modifiers.command == command && modifiers.shift == shift && !modifiers.alt
                    && (command || (!modifiers.ctrl && !modifiers.mac_cmd)));
            found |= matched;
            !matched
        });
        found
    })
}

impl eframe::App for MailApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.show(context);
    }
}
