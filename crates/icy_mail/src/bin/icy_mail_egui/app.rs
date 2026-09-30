use std::{collections::HashMap, path::PathBuf, sync::Arc};

use bstr::ByteSlice;
use eframe::egui::{self, Key, Vec2};
use i18n_embed_fl::fl;
use icy_engine::{BufferType, Position, Selection, Size, TextScreen};
use icy_engine_gui::{egui::screen::ScreenView, MonitorSettings};
use icy_mail::{
    address_book::AddressBook,
    drafts::{Compose, Draft, DraftStore},
    editor,
    options::{ModernFont, Options, ReadingMode, ReadingPane},
    qwk::MessageInfo,
    reader::{NavigateDirection, Pane, Reader, ViewMode},
    state::{PacketSummary, ReadState, RecentPackets},
    taglines::{self, Taglines},
    text, LANGUAGE_LOADER,
};
use parking_lot::Mutex;

use super::{
    address_dialog::AddressDialog,
    composer::Composer,
    loading::{BodySource, Event, Loader},
    mark_writer::{MarkKind, MarkWriter},
    modern_view::{Document, Item},
    settings::SettingsDialog,
    tagline_dialog::TaglineDialog,
    widgets::{Icons, ROW_HEIGHT},
};

/// Narrowest message list beside the reading pane.
const MIN_SIDE_LIST_WIDTH: f32 = 420.0;
/// Narrowest reading pane beside the list; 80 columns stay readable at this width.
const MIN_SIDE_READER_WIDTH: f32 = 560.0;
/// Area right of the sidebar from which the automatic layout puts the message beside the list.
const AUTO_SIDE_WIDTH: f32 = 1180.0;

/// What the sidebar has selected and the list shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Folder {
    All,
    Personal,
    Starred,
    Drafts,
    /// Welcome, news and goodbye screens, bulletins and new files lists of the packet.
    Bulletins,
    Conference(u16),
}

impl Folder {
    /// Whether the list shows packet messages, so message actions apply.
    pub fn holds_messages(self) -> bool {
        !matches!(self, Self::Drafts | Self::Bulletins)
    }
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
    About,
    Shortcuts,
    PacketInfo,
    Exported(PathBuf),
    DeleteDraft(u64),
    Discard(AfterDiscard),
    ExportProblems(Vec<String>),
    Settings,
    Taglines,
    AddressBook,
}

/// Unread counts for the sidebar, refreshed when the packet or the read marks change.
#[derive(Default)]
pub struct Counts {
    pub unread: usize,
    pub starred: usize,
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
    pub marks: MarkWriter,
    pub recent: Option<RecentPackets>,
    /// Keeps drafts, read marks and the recent list here instead of the user's data directory.
    pub storage: Option<PathBuf>,
    pub folder: Folder,
    pub selected_draft: Option<u64>,
    /// Index into the packet's `files` while the bulletins folder is open.
    pub selected_file: Option<usize>,
    pub selected_file_page: usize,
    file_page_scroll_to_end: bool,
    pub composer: Option<Composer>,
    pub modal: Option<Modal>,
    pub notice: Option<Notice>,
    pub counts: Counts,
    /// Conferences a draft can be posted to, see [`conference_choices`].
    pub choices: Vec<(u16, String)>,
    pub icons: Icons,
    pub rendered: Option<usize>,
    /// The outbox draft (id and text) shown in `screen`.
    pub rendered_draft: Option<(u64, String)>,
    /// The packet file shown in `screen`.
    pub rendered_file: Option<usize>,
    pub rendered_file_page: usize,
    pub selection_anchor: Option<Selection>,
    pub last_reader_click: Option<(egui::Pos2, f64, u8)>,
    pub body_highlights: super::reader_view::BodyHighlights,
    pub reveal_sidebar: bool,
    pub reveal_message: bool,
    pub message_view: Vec2,
    pub content_rect: egui::Rect,
    pub new_window: bool,
    pub closed: bool,
    /// The settings as last saved.
    pub options: Options,
    pub options_save_after: Option<f64>,
    pub settings_dialog: Option<SettingsDialog>,
    /// New messages start with a random tagline.
    pub random_tagline: bool,
    pub reading_pane: ReadingPane,
    pub conferences_unread_only: bool,
    pub reading_mode: ReadingMode,
    pub modern_font: ModernFont,
    pub modern_font_size: f32,
    /// The shown message as text and rendered art for the modern reading mode, built on first use
    /// and keyed by the packet and message they came from.
    pub modern_items: Option<((usize, Document), Vec<Item>)>,
    /// Long quotes the reader unfolded in the shown document, by their first item.
    pub open_quotes: std::collections::HashSet<usize>,
    /// Networks the user opened or closed in the sidebar, by lowercased name; others follow their unread state.
    pub network_open: HashMap<String, bool>,
    pub taglines: Option<Taglines>,
    pub tagline_dialog: Option<TaglineDialog>,
    pub address_book: Option<AddressBook>,
    pub address_dialog: Option<AddressDialog>,
    pub about: Option<icy_engine_gui::egui::about::AboutDialog>,
    pub latest_version: Option<semver::Version>,
    /// The start page summary last stored for the open packet.
    summary: Option<PacketSummary>,
    version_check: Option<std::sync::mpsc::Receiver<semver::Version>>,
    /// The tagline found in a message, keyed by packet and message index.
    tagline_cache: Option<((usize, usize), Option<String>)>,
    children: Vec<(egui::ViewportId, Arc<Mutex<MailApp>>)>,
}

impl MailApp {
    pub fn new(context: &egui::Context) -> Self {
        Self::create(context, None)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_storage(context: &egui::Context, storage: PathBuf) -> Self {
        #[cfg(test)]
        crate::use_english();
        Self::create(context, Some(storage))
    }

    fn create(context: &egui::Context, storage: Option<PathBuf>) -> Self {
        #[cfg(test)]
        icy_engine_gui::system_clipboard::disable();
        let options = storage
            .clone()
            .or_else(|| Options::directory().ok())
            .map(|directory| {
                Options::load_in(&directory).unwrap_or_else(|error| {
                    log::warn!("could not read the settings, using the defaults: {error}");
                    Options::default()
                })
            })
            .unwrap_or_default();
        let recent = match &storage {
            Some(directory) => RecentPackets::open_in(directory),
            None => RecentPackets::open(),
        }
        .ok();
        let taglines = match &storage {
            Some(directory) => Taglines::open_in(directory),
            None => Taglines::open(),
        }
        .inspect_err(|error| log::warn!("could not read the taglines: {error}"))
        .ok();
        let mut app = Self {
            reader: Reader::default(),
            screen: ScreenView::new(TextScreen::new(Size::new(80, 25))),
            settings: options.monitor_settings.clone(),
            focus: Pane::Messages,
            loader: Loader::default(),
            path: None,
            loading: None,
            body_loading: false,
            error: None,
            drafts: None,
            read_state: None,
            marks: MarkWriter::new(),
            recent,
            storage,
            folder: Folder::All,
            selected_draft: None,
            selected_file: None,
            selected_file_page: 0,
            file_page_scroll_to_end: false,
            composer: None,
            modal: None,
            notice: None,
            counts: Counts::default(),
            choices: Vec::new(),
            icons: Icons::default(),
            rendered: None,
            rendered_draft: None,
            rendered_file: None,
            rendered_file_page: 0,
            selection_anchor: None,
            last_reader_click: None,
            body_highlights: super::reader_view::BodyHighlights::default(),
            reveal_sidebar: false,
            reveal_message: false,
            message_view: Vec2::ZERO,
            content_rect: egui::Rect::NOTHING,
            new_window: false,
            closed: false,
            random_tagline: options.random_tagline,
            reading_pane: options.reading_pane,
            conferences_unread_only: options.conferences_unread_only,
            reading_mode: options.reading_mode,
            modern_font: options.modern_font,
            modern_font_size: options.modern_font_size,
            modern_items: None,
            open_quotes: std::collections::HashSet::new(),
            network_open: HashMap::new(),
            options,
            options_save_after: None,
            settings_dialog: None,
            taglines,
            tagline_dialog: None,
            address_book: None,
            address_dialog: None,
            about: None,
            latest_version: None,
            summary: None,
            version_check: None,
            tagline_cache: None,
            children: Vec::new(),
        };
        let options = app.options.clone();
        app.apply_options(context, &options);
        app
    }

    pub fn options_directory(&self) -> Option<PathBuf> {
        self.storage.clone().or_else(|| Options::directory().ok())
    }

    pub fn open(&mut self, path: PathBuf, context: &egui::Context) {
        if self.composer.is_some() {
            self.notify(context, NoticeKind::Warning, fl!(LANGUAGE_LOADER, "notice-finish-writing"));
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
        let failed: Vec<_> = self.marks.errors().collect();
        for (kind, error) in failed {
            self.mark_save_failed(context, kind, error);
        }
        if let Some(receiver) = &self.version_check {
            match receiver.try_recv() {
                Ok(latest) => {
                    self.latest_version = Some(latest);
                    self.version_check = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.version_check = None,
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
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
                            if self.folder == Folder::Bulletins && self.file_page_scroll_to_end {
                                self.screen.scroll_to = Some(egui::vec2(0.0, f32::MAX));
                            }
                            self.file_page_scroll_to_end = false;
                            if self.folder.holds_messages() {
                                if let Some(index) = self.rendered {
                                    if !self.reader.is_read(index) {
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
                Event::Search(generation, query, result)
                    if generation == self.loader.search_generation() && query == self.reader.filter.trim().to_lowercase() =>
                {
                    self.loader.searching = false;
                    match result {
                        Ok(matches) => {
                            self.reader.set_body_matches(query, matches);
                            self.reveal_message = true;
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
                Event::Picked(path) => {
                    self.loader.picking = false;
                    if let Some(path) = path {
                        self.open(path, context);
                    }
                }
                Event::MessageSaved(result) => {
                    self.loader.save_picking = false;
                    match result {
                        Some((path, Ok(()))) => self.notify(
                            context,
                            NoticeKind::Success,
                            fl!(LANGUAGE_LOADER, "notice-message-saved", path = path.display().to_string()),
                        ),
                        Some((path, Err(error))) => {
                            self.error = Some(fl!(
                                LANGUAGE_LOADER,
                                "app-save-message-failed",
                                path = path.display().to_string(),
                                error = error
                            ))
                        }
                        None => {}
                    }
                }
                Event::Exported(result) => {
                    self.loader.export_picking = false;
                    if let Some((path, result)) = result {
                        match result {
                            Ok(()) => {
                                self.modal = Some(Modal::Exported(path));
                            }
                            Err(error) => {
                                self.error = Some(fl!(
                                    LANGUAGE_LOADER,
                                    "app-export-failed",
                                    path = path.display().to_string(),
                                    error = error.to_string()
                                ))
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        self.sync_body(context);
    }

    /// Checks GitHub in the background so startup is never blocked by the network.
    pub fn check_for_updates(&mut self, context: &egui::Context) {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.version_check = Some(receiver);
        let context = context.clone();
        std::thread::spawn(move || {
            if let Some(latest) = icy_engine_gui::release_check::latest_release("mkrueger/icy_tools", "IcyMail") {
                if latest > *icy_mail::VERSION {
                    let _ = sender.send(latest);
                    context.request_repaint();
                }
            }
        });
    }

    fn loaded(&mut self, context: &egui::Context, path: PathBuf, package: Arc<icy_mail::qwk::QwkPackage>) {
        let drafts = match &self.storage {
            Some(directory) => DraftStore::open_in(&path, &package, directory),
            None => DraftStore::open(&path, &package),
        };
        let drafts = match drafts {
            Ok(drafts) => drafts,
            Err(error) => {
                self.error = Some(fl!(
                    LANGUAGE_LOADER,
                    "app-drafts-failed",
                    path = path.display().to_string(),
                    error = error.to_string()
                ));
                return;
            }
        };
        // A reload must see the marks still queued for this packet.
        self.marks.flush();
        let read_state = match &self.storage {
            Some(directory) => ReadState::open_in(&path, &package, directory),
            None => ReadState::open(&path, &package),
        };
        let reload = self.path.as_ref() == Some(&path);
        let previous = (self.folder, self.reader.selected_message, self.reader.view_mode, self.reader.message_sort);
        self.reader.set_package(package.clone());
        self.choices = conference_choices(&package);
        match read_state {
            Ok(state) => {
                self.reader.set_read_marks(state.indices(&package));
                self.reader.set_stars(state.starred_indices(&package));
                self.read_state = Some(state);
            }
            Err(error) => {
                self.read_state = None;
                self.notify(
                    context,
                    NoticeKind::Warning,
                    fl!(LANGUAGE_LOADER, "notice-read-marks-unavailable", error = error.to_string()),
                );
            }
        }
        self.drafts = Some(drafts);
        self.path = Some(path.clone());
        self.rendered = None;
        self.rendered_file = None;
        self.rendered_file_page = 0;
        if !reload {
            self.selected_file = None;
            self.selected_file_page = 0;
        }
        self.refresh_counts();
        if reload {
            self.reader.view_mode = previous.2;
            self.reader.message_sort = previous.3;
            self.select_folder(previous.0);
            if let Some(index) = previous.1 {
                self.reader.select_message(index);
            }
        } else {
            self.network_open.clear();
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
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        context.send_viewport_cmd(egui::ViewportCommand::Title(fl!(LANGUAGE_LOADER, "window-packet-title", name = name.as_str())));
    }

    fn sync_body(&mut self, context: &egui::Context) {
        if self.loading.is_some() || self.folder == Folder::Drafts {
            return;
        }
        if self.folder == Folder::Bulletins {
            let (Some(package), Some(index)) = (&self.reader.package, self.selected_file) else {
                return;
            };
            if self.rendered_file == Some(index) && self.rendered_file_page == self.selected_file_page && self.rendered_draft.is_none() {
                return;
            }
            if self.rendered_file != Some(index) {
                self.file_page_scroll_to_end = false;
            }
            self.rendered = None;
            self.rendered_draft = None;
            self.rendered_file = Some(index);
            self.rendered_file_page = self.selected_file_page;
            self.selection_anchor = None;
            self.last_reader_click = None;
            self.reveal_message = true;
            self.body_loading = true;
            self.loader.body(package.clone(), BodySource::File(index, self.selected_file_page), context);
            return;
        }
        if self.rendered == self.reader.selected_message && self.rendered_draft.is_none() && self.rendered_file.is_none() {
            return;
        }
        self.rendered = self.reader.selected_message;
        self.rendered_draft = None;
        self.rendered_file = None;
        self.file_page_scroll_to_end = false;
        self.selection_anchor = None;
        self.last_reader_click = None;
        self.reveal_message = true;
        if let (Some(package), Some(index)) = (&self.reader.package, self.reader.selected_message) {
            self.body_loading = true;
            self.loader.body(package.clone(), BodySource::Message(index), context);
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
            .map(|package| text::HeaderText::new(package.control_file.qmail_user_name.trim()).to_string())
            .unwrap_or_default()
    }

    /// Keeps the start page's summary of the open packet current: unread and starred messages and drafts.
    fn sync_summary(&mut self) {
        let (Some(path), Some(package)) = (&self.path, &self.reader.package) else {
            return;
        };
        if self.loading.is_some() || self.recent.is_none() {
            return;
        }
        let summary = PacketSummary {
            path: path.clone(),
            bbs_name: self.bbs_name(),
            created: package.control_file.creation_time.to_string().trim().to_string(),
            messages: package.message_count(),
            unread: self.counts.unread,
            starred: self.counts.starred,
            drafts: self.draft_count(),
        };
        if self.summary.as_ref() == Some(&summary) {
            return;
        }
        if let Some(recent) = &mut self.recent {
            if let Err(error) = recent.set_summary(summary.clone()) {
                log::warn!("unable to update the recent packet summary: {error}");
            }
        }
        self.summary = Some(summary);
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
                let unread = !self.reader.is_read(info.index);
                let personal = is_personal(info, &user);
                if self.reader.is_starred(info.index) {
                    counts.starred += 1;
                }
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
        folders.push(Folder::Starred);
        folders.push(Folder::Drafts);
        if self.file_count() > 0 {
            folders.push(Folder::Bulletins);
        }
        folders.extend(self.conference_folders(true));
        folders
    }

    /// Every folder in sidebar order, including conferences hidden in closed networks or by the unread filter.
    pub fn all_folders(&self) -> Vec<Folder> {
        let mut folders = self.folders();
        folders.retain(|folder| !matches!(folder, Folder::Conference(_)));
        folders.extend(self.conference_folders(false));
        folders
    }

    pub fn select_folder(&mut self, mut folder: Folder) {
        let files = self.file_count();
        if folder == Folder::Bulletins && files == 0 {
            folder = Folder::All;
        }
        if folder == Folder::Bulletins && self.selected_file.is_none_or(|index| index >= files) {
            self.selected_file = Some(0);
            self.selected_file_page = 0;
        }
        if folder == Folder::Bulletins {
            if let Some(file) = self.selected_file() {
                self.selected_file_page = self.selected_file_page.min(file.pages().saturating_sub(1));
            }
        }
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
        if folder == Folder::Bulletins {
            return;
        }
        if let Folder::Conference(number) = folder {
            self.reveal_conference(number);
        }
        self.reader.personal = (folder == Folder::Personal).then(|| self.user_name());
        self.reader.starred_only = folder == Folder::Starred;
        self.reader.select_conference(match folder {
            Folder::Conference(number) => Some(number),
            _ => None,
        });
        if let Some(index) = self.reader.next_unread(None) {
            self.reader.select_message(index);
        }
    }

    pub fn folder_name(&self, folder: Folder) -> String {
        match folder {
            Folder::All => fl!(LANGUAGE_LOADER, "folder-all"),
            Folder::Personal => fl!(LANGUAGE_LOADER, "folder-personal"),
            Folder::Starred => fl!(LANGUAGE_LOADER, "folder-starred"),
            Folder::Drafts => fl!(LANGUAGE_LOADER, "folder-outbox"),
            Folder::Bulletins => fl!(LANGUAGE_LOADER, "folder-bulletins"),
            Folder::Conference(number) => self
                .reader
                .conferences
                .iter()
                .find(|row| row.number == Some(number))
                .map_or_else(|| fl!(LANGUAGE_LOADER, "folder-conference", number = number), |row| row.name.clone()),
        }
    }

    pub fn draft_count(&self) -> usize {
        self.drafts.as_ref().map_or(0, |store| store.drafts().len())
    }

    /// Bulletins, news and new files lists in the packet.
    pub fn file_count(&self) -> usize {
        self.reader.package.as_ref().map_or(0, |package| package.files.len())
    }

    pub fn selected_file(&self) -> Option<&icy_mail::qwk::PacketFile> {
        self.reader.package.as_ref()?.files.get(self.selected_file?)
    }

    pub fn selected_info(&self) -> Option<&icy_mail::qwk::MessageInfo> {
        let package = self.reader.package.as_ref()?;
        package.infos.get(self.reader.selected_message?)
    }

    /// Whether a message action (reply, forward, mark) applies to the list selection.
    pub fn message_selected(&self) -> bool {
        self.folder.holds_messages() && self.reader.selected_message.is_some() && self.composer.is_none()
    }

    pub fn set_read(&mut self, context: &egui::Context, indices: &[usize], read: bool) {
        let Some(package) = self.reader.package.clone() else {
            return;
        };
        let infos: Vec<_> = indices.iter().filter_map(|index| package.infos.get(*index)).collect();
        if let Some(state) = &mut self.read_state {
            if state.mark(infos.iter().copied(), read) {
                self.save_marks(context, MarkKind::Read);
            }
        }
        let user = self.user_name();
        for info in infos {
            if !self.reader.set_read(info.index, read) {
                continue;
            }
            let personal = is_personal(info, &user);
            let delta = |count: &mut usize| {
                if read {
                    *count = count.saturating_sub(1);
                } else {
                    *count += 1;
                }
            };
            delta(&mut self.counts.unread);
            delta(self.counts.conferences.entry(info.conference).or_default());
            if personal {
                delta(&mut self.counts.personal.0);
            }
        }
    }

    /// Queues the packet's read marks and stars for saving; the in-memory state keeps the change
    /// when saving fails, so the list does not flip back while the warning is shown.
    fn save_marks(&mut self, context: &egui::Context, kind: MarkKind) {
        let Some(state) = &self.read_state else {
            return;
        };
        let result = state
            .snapshot()
            .map_err(|error| error.to_string())
            .and_then(|snapshot| self.marks.save(snapshot, kind, context));
        if let Err(error) = result {
            self.mark_save_failed(context, kind, error);
        }
    }

    fn mark_save_failed(&mut self, context: &egui::Context, kind: MarkKind, error: String) {
        let text = match kind {
            MarkKind::Read => fl!(LANGUAGE_LOADER, "notice-read-marks-failed", error = error),
            MarkKind::Star => fl!(LANGUAGE_LOADER, "notice-stars-failed", error = error),
        };
        self.notify(context, NoticeKind::Warning, text);
    }

    pub fn toggle_read(&mut self, context: &egui::Context) {
        if let Some(index) = self.reader.selected_message.filter(|_| self.message_selected()) {
            let read = !self.reader.is_read(index);
            self.set_read(context, &[index], read);
            if !read {
                // Keep the message unread instead of marking it again as soon as it is shown.
                self.rendered = self.reader.selected_message;
            }
        }
    }

    /// Stars or unstars a message and saves it with the read marks.
    pub fn set_starred(&mut self, context: &egui::Context, index: usize, starred: bool) {
        let Some(info) = self.reader.package.as_ref().and_then(|package| package.infos.get(index)).cloned() else {
            return;
        };
        if let Some(state) = &mut self.read_state {
            if state.star(&info, starred) {
                self.save_marks(context, MarkKind::Star);
            }
        }
        if self.reader.set_starred(index, starred) {
            if starred {
                self.counts.starred += 1;
            } else {
                self.counts.starred = self.counts.starred.saturating_sub(1);
            }
        }
    }

    pub fn toggle_star(&mut self, context: &egui::Context) {
        if let Some(index) = self.reader.selected_message.filter(|_| self.message_selected()) {
            self.set_starred(context, index, !self.reader.is_starred(index));
        }
    }

    /// Whether thread actions apply: the thread view with a message selected.
    pub fn thread_selected(&self) -> bool {
        self.message_selected() && self.reader.view_mode == ViewMode::Threads
    }

    /// Marks every message of the selected message's thread read.
    pub fn mark_thread_read(&mut self, context: &egui::Context) {
        let Some(index) = self.reader.selected_message.filter(|_| self.thread_selected()) else {
            return;
        };
        let thread = self.reader.thread_of(index);
        let count = thread.iter().filter(|index| !self.reader.is_read(**index)).count();
        self.set_read(context, &thread, true);
        if count > 0 {
            self.notify(context, NoticeKind::Info, fl!(LANGUAGE_LOADER, "notice-marked-read", count = count));
        }
    }

    /// Collapses every thread to its first message, or expands them all.
    pub fn set_all_threads_collapsed(&mut self, collapsed: bool) {
        if self.reader.view_mode == ViewMode::Threads && self.reader.set_all_collapsed(collapsed) {
            self.reveal_message = true;
        }
    }

    pub fn mark_folder_read(&mut self, context: &egui::Context) {
        if !self.folder.holds_messages() {
            return;
        }
        let indices: Vec<_> = self.reader.all_messages().iter().map(|row| row.index).collect();
        let count = indices.iter().filter(|index| !self.reader.is_read(**index)).count();
        self.set_read(context, &indices, true);
        if count > 0 {
            self.notify(context, NoticeKind::Info, fl!(LANGUAGE_LOADER, "notice-marked-read", count = count));
        }
    }

    /// Selects the next unread message, continuing in the following conferences.
    pub fn next_unread(&mut self, context: &egui::Context) {
        if self.reader.package.is_none() {
            return;
        }
        if self.folder.holds_messages() {
            if let Some(index) = self.reader.next_unread(self.reader.selected_message) {
                self.reader.select_message(index);
                self.reveal_message = true;
                self.reveal_next_unread(context);
                return;
            }
        }
        let folders = self.all_folders();
        let current = folders.iter().position(|folder| *folder == self.folder).unwrap_or(0);
        let next = if self.reader.filter.trim().is_empty() {
            folders.iter().skip(current + 1).find_map(|folder| match folder {
                Folder::Conference(number) if self.counts.conferences.get(number).copied().unwrap_or(0) > 0 => Some(*folder),
                _ => None,
            })
        } else {
            None
        };
        match next {
            Some(folder) => {
                self.select_folder(folder);
                self.reveal_next_unread(context);
                let name = self.folder_name(folder);
                self.notify(context, NoticeKind::Info, fl!(LANGUAGE_LOADER, "notice-continuing-in", name = name));
            }
            None => {
                if self.counts.unread > 0 && self.reader.filter.trim().is_empty() {
                    let previous = self.reader.selected_message;
                    self.select_folder(Folder::All);
                    let first = self.reader.next_unread(None);
                    let next = if first == previous { self.reader.next_unread(first) } else { first };
                    if let Some(index) = next {
                        self.reader.select_message(index);
                        self.reveal_message = true;
                        self.reveal_next_unread(context);
                        return;
                    }
                }
                let notice = if self.counts.unread > 0 && !self.reader.filter.trim().is_empty() {
                    fl!(LANGUAGE_LOADER, "notice-no-more-unread-filtered")
                } else {
                    fl!(LANGUAGE_LOADER, "notice-no-more-unread")
                };
                self.notify(context, NoticeKind::Info, notice);
            }
        }
    }

    fn reveal_next_unread(&mut self, context: &egui::Context) {
        if context.content_rect().width() < 760.0 || context.content_rect().height() < 300.0 {
            self.set_focus(Pane::Content, context);
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
        let query = if self.reader.search_fields.text {
            self.reader.filter.trim().to_lowercase()
        } else {
            String::new()
        };
        if self.loader.search_query.as_ref().is_some_and(|searched| *searched != query) {
            self.loader.cancel_search();
        }
        self.reader.rebuild_messages();
        self.reveal_message = true;
    }

    /// The search text for highlighting a field, empty when the search does not look at it.
    pub fn search_needle(&self, searched: bool) -> String {
        if searched {
            self.reader.filter.trim().to_owned()
        } else {
            String::new()
        }
    }

    /// Changes what the search looks at and filters again.
    pub fn set_search_fields(&mut self, fields: icy_mail::reader::SearchFields) {
        self.reader.search_fields = fields;
        self.filter_changed();
    }

    fn search_bodies(&mut self, context: &egui::Context) {
        if self.loading.is_some() || !self.folder.holds_messages() {
            return;
        }
        if let Some(package) = self.reader.package.clone() {
            // Without the text field there is nothing to look for in the bodies.
            let query = if self.reader.search_fields.text {
                self.reader.filter.trim().to_lowercase()
            } else {
                String::new()
            };
            if let Err(error) = self.loader.search(package, query, context) {
                self.error = Some(error);
            }
        }
    }

    pub fn new_draft(&mut self, context: &egui::Context) {
        let Some(package) = self.reader.package.clone() else {
            return;
        };
        let choices = self.choices.clone();
        let conference = match self.folder {
            Folder::Conference(number) => Some(number),
            _ => self.selected_info().map(|info| info.conference),
        }
        .filter(|number| choices.iter().any(|(choice, _)| choice == number))
        .or_else(|| choices.first().map(|(number, _)| *number));
        let Some(conference) = conference else {
            self.error = Some(fl!(LANGUAGE_LOADER, "app-no-conference"));
            return;
        };
        if let Some(store) = &self.drafts {
            match store.prepare(&package, Compose::New { conference }) {
                Ok(mut draft) => {
                    draft.to = "ALL".into();
                    self.start_composer(context, Composer::new(draft, false, None, Vec::new(), false));
                }
                Err(error) => self.error = Some(fl!(LANGUAGE_LOADER, "app-new-message-failed", error = error.to_string())),
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
            self.error = Some(fl!(LANGUAGE_LOADER, "app-message-unavailable"));
            return;
        };
        let message = match package.get_message(index) {
            Ok(message) => message,
            Err(error) => {
                self.error = Some(fl!(LANGUAGE_LOADER, "app-original-failed", error = error.to_string()));
                return;
            }
        };
        let Some(store) = &self.drafts else {
            return;
        };
        let mut draft = match store.prepare(&package, if forward { Compose::Forward { index } } else { Compose::Reply { index } }) {
            Ok(draft) => draft,
            Err(error) => {
                self.error = Some(fl!(LANGUAGE_LOADER, "app-reply-failed", error = error.to_string()));
                return;
            }
        };
        let quotes = quotes(info, &message.text);
        if forward {
            let text = editor::strip_codes(&editor::decode_message(&message.text));
            let quoted: String = text.trim_end().lines().map(|line| format!("> {line}\n")).collect();
            let header = fl!(
                LANGUAGE_LOADER,
                "app-forward-header",
                from = info.from.as_str(),
                to = info.to.as_str(),
                date = info.date_str.as_str(),
                subject = info.subject.as_str()
            );
            draft.body = format!("\n\n{header}\n\n{quoted}");
        }
        let origin = message_origin(info);
        // Like on a BBS, a reply starts empty with the quote panel open to pick lines from.
        self.start_composer(context, Composer::new(draft, false, Some(origin), quotes, !forward));
    }

    pub fn edit_draft(&mut self, context: &egui::Context, id: u64) {
        let draft = self
            .drafts
            .as_ref()
            .and_then(|store| store.drafts().iter().find(|draft| draft.id == id))
            .cloned();
        if let Some(draft) = draft {
            let origin = self.origin(&draft);
            let quotes = self.original_quotes(&draft);
            self.start_composer(context, Composer::new(draft, true, origin, quotes, false));
        } else {
            self.error = Some(fl!(LANGUAGE_LOADER, "app-draft-unavailable"));
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
        Some(message_origin(info))
    }

    /// Quote lines of the message a reply refers to, when it is in the open packet.
    fn original_quotes(&self, draft: &Draft) -> Vec<String> {
        if draft.ref_number == 0 {
            return Vec::new();
        }
        let Some(package) = self.reader.package.as_ref() else {
            return Vec::new();
        };
        package
            .infos
            .iter()
            .position(|info| info.number == draft.ref_number && info.conference == draft.conference)
            .and_then(|index| Some(quotes(&package.infos[index], &package.get_message(index).ok()?.text)))
            .unwrap_or_default()
    }

    fn start_composer(&mut self, context: &egui::Context, mut composer: Composer) {
        if !composer.existing && self.random_tagline {
            if let Some(tagline) = self.taglines.as_ref().and_then(Taglines::random) {
                composer.draft.tagline = tagline.to_string();
                composer.original.tagline = tagline.to_string();
            }
        }
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
                self.notify(context, NoticeKind::Info, fl!(LANGUAGE_LOADER, "notice-draft-deleted"));
            }
            Err(error) => self.error = Some(fl!(LANGUAGE_LOADER, "app-delete-draft-failed", error = error.to_string())),
        }
    }

    pub fn export(&mut self, context: &egui::Context) {
        let (Some(path), Some(store)) = (&self.path, &self.drafts) else {
            return;
        };
        if store.drafts().is_empty() {
            self.notify(context, NoticeKind::Info, fl!(LANGUAGE_LOADER, "notice-nothing-to-export"));
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
        if !blocked && !context.will_discard() {
            self.search_bodies(context);
        }
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
                        if self.reading_pane_right(ui.available_width()) {
                            let width = ui.available_width();
                            egui::SidePanel::left("messages-side")
                                .default_width((width * 0.45).clamp(MIN_SIDE_LIST_WIDTH, 720.0))
                                .width_range(MIN_SIDE_LIST_WIDTH..=(width - MIN_SIDE_READER_WIDTH).max(MIN_SIDE_LIST_WIDTH))
                                .resizable(true)
                                .frame(egui::Frame::new())
                                .show_inside(ui, |ui| {
                                    self.list(ui);
                                });
                        } else {
                            egui::TopBottomPanel::top("messages")
                                .default_height(260.0)
                                .height_range(120.0..=(ui.available_height() - 140.0).max(120.0))
                                .resizable(true)
                                .frame(egui::Frame::new())
                                .show_inside(ui, |ui| {
                                    self.list(ui);
                                });
                        }
                        egui::CentralPanel::default().frame(egui::Frame::new()).show_inside(ui, |ui| self.content(ui));
                    }
                });
            });
        if !blocked && !context.will_discard() {
            self.sync_body(context);
            self.sync_summary();
        }
        self.modals(context);
        if !matches!(self.modal, Some(Modal::Settings)) {
            self.persist_options(context);
        }
        self.windows(context);
    }

    /// Whether the message goes beside the list in an area `width` wide right of the sidebar.
    pub fn reading_pane_right(&self, width: f32) -> bool {
        match self.reading_pane {
            ReadingPane::Automatic => width >= AUTO_SIDE_WIDTH,
            ReadingPane::Right => width >= MIN_SIDE_LIST_WIDTH + MIN_SIDE_READER_WIDTH,
            ReadingPane::Below => false,
        }
    }

    fn narrow(&mut self, ui: &mut egui::Ui) {
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            for (pane, label) in [
                (Pane::Conferences, fl!(LANGUAGE_LOADER, "app-tab-folders")),
                (Pane::Messages, fl!(LANGUAGE_LOADER, "app-tab-messages")),
                (Pane::Content, fl!(LANGUAGE_LOADER, "app-tab-message")),
            ] {
                if super::widgets::pill(ui, self.focus == pane, &label).clicked() {
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
        if info.font_width <= 0.0 || info.font_height <= 0.0 || info.display_scale <= 0.0 {
            return None;
        }
        let (x, y) = info.screen_to_terminal_pixels_unclamped(position.x, position.y);
        let y = if info.scan_lines { y / 2.0 } else { y };
        let screen = self.screen.terminal.screen.lock();
        Some(Position::new(
            (((x + self.screen.terminal.scroll_x()) / info.font_width).floor() as i32).clamp(0, (screen.width() - 1).max(0)),
            (((y + self.screen.terminal.scroll_y()) / info.font_height).floor() as i32).clamp(0, (screen.height() - 1).max(0)),
        ))
    }

    /// The `... tagline` of the selected message, if it has one.
    pub fn message_tagline(&mut self) -> Option<String> {
        if !self.folder.holds_messages() {
            return None;
        }
        let package = self.reader.package.clone()?;
        let index = self.reader.selected_message?;
        let key = (Arc::as_ptr(&package) as usize, index);
        if let Some((cached, tagline)) = &self.tagline_cache {
            if *cached == key {
                return tagline.clone();
            }
        }
        let tagline = package
            .get_message(index)
            .ok()
            .and_then(|message| taglines::find(&editor::strip_codes(&editor::decode_message(&message.text))));
        self.tagline_cache = Some((key, tagline.clone()));
        tagline
    }

    /// Adds the tagline of the selected message to the tagline list, like MultiMail's tagline stealer.
    pub fn save_tagline(&mut self, context: &egui::Context) {
        let Some(tagline) = self.message_tagline() else {
            if self.message_selected() && self.folder.holds_messages() {
                self.notify(context, NoticeKind::Info, fl!(LANGUAGE_LOADER, "notice-no-tagline"));
            }
            return;
        };
        let mut list = match self.tagline_file() {
            Ok(list) => list,
            Err(error) => {
                self.error = Some(fl!(LANGUAGE_LOADER, "app-taglines-read-failed", error = error.to_string()));
                return;
            }
        };
        if list.lines.contains(&tagline) {
            self.notify(context, NoticeKind::Info, fl!(LANGUAGE_LOADER, "notice-tagline-known"));
        } else if list.add(&tagline).is_some() {
            match list.save() {
                Ok(()) => self.notify(
                    context,
                    NoticeKind::Success,
                    fl!(LANGUAGE_LOADER, "notice-tagline-saved", tagline = tagline.as_str()),
                ),
                Err(error) => self.error = Some(fl!(LANGUAGE_LOADER, "app-taglines-save-failed", error = error.to_string())),
            }
        }
        self.taglines = Some(list);
    }

    pub fn copy(&self, context: &egui::Context) {
        if !self.body_loading {
            icy_engine_gui::system_clipboard::copy_selection(context, &**self.screen.terminal.screen.lock());
        }
    }

    /// Saves the shown message's text as it is in the packet (`.ans`), or converted to UTF-8 (`.txt`),
    /// e.g. to look at it in another viewer or to report a display problem.
    pub fn save_message(&mut self, context: &egui::Context, utf8: bool) {
        let Some(index) = self.reader.selected_message.filter(|_| self.message_selected()) else {
            return;
        };
        let Some(package) = self.reader.package.clone() else {
            return;
        };
        match message_file(&package, index, utf8) {
            Ok((name, data)) => self.loader.pick_save_message(name, data, context),
            Err(error) => self.error = Some(error),
        }
    }

    /// Copies the whole message, or the whole bulletin in the bulletins folder.
    pub fn copy_message(&mut self, context: &egui::Context) {
        let file = self.folder == Folder::Bulletins;
        let shown = if file {
            self.selected_file.is_some()
        } else {
            self.folder.holds_messages() && self.reader.selected_message.is_some()
        };
        if self.body_loading || !shown {
            return;
        }
        {
            let mut screen = self.screen.terminal.screen.lock();
            let old = screen.selection();
            let mut selection = Selection::new((0, 0));
            selection.lead = (screen.width() - 1, screen.height() - 1).into();
            let _ = screen.set_selection(selection);
            if let Ok(mut data) = icy_engine_gui::prepare_clipboard_data(&**screen) {
                data.text = data.text.trim_end().to_string();
                icy_engine_gui::system_clipboard::copy_data_or_text(context, &data);
            }
            if let Some(selection) = old {
                let _ = screen.set_selection(selection);
            } else {
                let _ = screen.clear_selection();
            }
        }
        let notice = if file {
            fl!(LANGUAGE_LOADER, "notice-text-copied")
        } else {
            fl!(LANGUAGE_LOADER, "notice-message-copied")
        };
        self.notify(context, NoticeKind::Info, notice);
    }

    pub(super) fn change_file_page(&mut self, forward: bool) -> bool {
        if self.folder != Folder::Bulletins || self.body_loading {
            return false;
        }
        let Some(pages) = self.selected_file().map(|file| file.pages()) else {
            return false;
        };
        let next = if forward {
            self.selected_file_page.checked_add(1).filter(|page| *page < pages)
        } else {
            self.selected_file_page.checked_sub(1)
        };
        let Some(next) = next else {
            return false;
        };
        self.selected_file_page = next;
        self.file_page_scroll_to_end = !forward;
        true
    }

    fn scroll_content(&mut self, direction: NavigateDirection) {
        if self.folder == Folder::Bulletins {
            let forward = matches!(direction, NavigateDirection::Down | NavigateDirection::PageDown);
            let backward = matches!(direction, NavigateDirection::Up | NavigateDirection::PageUp);
            if ((forward && self.screen.offset.y + 1.0 >= self.screen.max_offset.y) || (backward && self.screen.offset.y <= 1.0))
                && self.change_file_page(forward)
            {
                return;
            }
        }
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
            Pane::Messages if self.folder == Folder::Bulletins => {
                let files = self.file_count();
                if files > 0 {
                    self.selected_file = Some(icy_mail::reader::step(self.selected_file.unwrap_or(0), direction, files));
                    self.selected_file_page = 0;
                    self.reveal_message = true;
                }
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
        if key(context, Key::T, true, true) {
            self.open_taglines(false);
            return;
        }
        if key(context, Key::Comma, true, false) {
            self.open_settings(context);
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
        if self.focus == Pane::Content && self.screen.terminal.screen.lock().selection().is_some() && key(context, Key::Escape, false, false) {
            self.selection_anchor = None;
            self.last_reader_click = None;
            if let Err(error) = self.screen.terminal.screen.lock().clear_selection() {
                self.error = Some(error.to_string());
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
        if self.focus == Pane::Messages && self.folder.holds_messages() && self.reader.view_mode == ViewMode::Threads {
            if key(context, Key::ArrowLeft, false, false) && self.reader.collapse_or_parent() {
                self.reveal_message = true;
            }
            if key(context, Key::ArrowRight, false, false) && self.reader.expand_or_child() {
                self.reveal_message = true;
            }
            if key(context, Key::ArrowLeft, false, true) {
                self.set_all_threads_collapsed(true);
            }
            if key(context, Key::ArrowRight, false, true) {
                self.set_all_threads_collapsed(false);
            }
        }
        if key(context, Key::M, false, true) {
            self.mark_thread_read(context);
        }
        if key(context, Key::Space, false, false) && self.folder != Folder::Drafts {
            if self.folder == Folder::Bulletins || self.screen.offset.y + 1.0 < self.screen.max_offset.y {
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
        if key(context, Key::S, false, false) {
            self.toggle_star(context);
        }
        if key(context, Key::T, false, false) {
            self.save_tagline(context);
        }
        if key(context, Key::A, false, false) {
            self.open_address_book(false);
        }
        if key(context, Key::A, false, true) {
            self.add_sender(context);
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
        // The modern reading mode's text keeps its own selection, which egui copies.
        let modern = self.reading_mode == ReadingMode::Modern;
        if self.focus == Pane::Content && !modern {
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
            child.latest_version = self.latest_version.clone();
            let id = egui::ViewportId::from_hash_of(("mail-window", child.screen.shader_state.instance_id));
            self.children.push((id, Arc::new(Mutex::new(child))));
        }
        if let Some(latest) = &self.latest_version {
            for (_, child) in &self.children {
                child.lock().latest_version = Some(latest.clone());
            }
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

/// File name (`<conference>-<number>.ans` or `.txt`) and content of a saved message.
pub fn message_file(package: &icy_mail::qwk::QwkPackage, index: usize, utf8: bool) -> Result<(String, Vec<u8>), String> {
    let message = package.get_message(index).map_err(|error| error.to_string())?;
    let info = &package.infos[index];
    let (data, extension) = if utf8 {
        (icy_mail::text::to_utf8(&message.text).into_bytes(), "txt")
    } else {
        (message.text.to_vec(), "ans")
    };
    Ok((format!("{}-{}.{extension}", info.conference, info.number), data))
}

pub fn sidebar_fill(visuals: &egui::Visuals) -> egui::Color32 {
    if visuals.dark_mode {
        visuals.panel_fill.lerp_to_gamma(egui::Color32::BLACK, 0.18)
    } else {
        visuals.panel_fill.lerp_to_gamma(visuals.widgets.inactive.bg_fill, 0.35)
    }
}

fn is_personal(info: &MessageInfo, user: &str) -> bool {
    !user.is_empty() && info.to.trim().eq_ignore_ascii_case(user)
}

pub fn draft_title(draft: &Draft) -> String {
    if draft.subject.trim().is_empty() {
        fl!(LANGUAGE_LOADER, "app-no-subject")
    } else {
        draft.subject.clone()
    }
}

/// Sender, subject and number of a message, for the composer heading.
fn message_origin(info: &MessageInfo) -> String {
    format!("{} \u{00b7} {} \u{00b7} #{}", info.from, info.subject, info.number)
}

/// The attribution and quoted lines offered in the editor's quote panel.
fn quotes(info: &MessageInfo, text: &[u8]) -> Vec<String> {
    let mut lines = vec![fl!(
        LANGUAGE_LOADER,
        "app-quote-attribution",
        date = info.date_str.as_str(),
        name = info.from.trim()
    )];
    lines.extend(editor::quote_lines(&info.from, &editor::decode_message(text), editor::WRAP_WIDTH));
    lines
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
