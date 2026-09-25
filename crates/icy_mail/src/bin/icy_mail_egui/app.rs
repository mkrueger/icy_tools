use std::{path::PathBuf, sync::Arc};

use eframe::egui::{self, Key, Vec2};
use icy_engine::{BufferType, Position, Selection, Size, TextScreen};
use icy_engine_gui::{
    egui::{appearance, screen::ScreenView},
    MonitorSettings, ScalingMode,
};
use icy_mail::{
    drafts::{Compose, Draft, DraftStore},
    reader::{ConferenceColumn, MessageColumn, NavigateDirection, Pane, Reader, ViewMode},
};
use parking_lot::Mutex;

use super::{
    loading::{Event, Loader},
    widgets::{self, Icon, Icons, ROW_HEIGHT},
};

struct Composer {
    draft: Draft,
    original: Draft,
    index: Option<usize>,
    discard: bool,
    delete: bool,
    error: Option<String>,
}

impl Composer {
    fn new(draft: Draft, index: Option<usize>) -> Self {
        Self {
            original: draft.clone(),
            draft,
            index,
            discard: false,
            delete: false,
            error: None,
        }
    }
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
    composer: Option<Composer>,
    notice: Option<String>,
    icons: Icons,
    rendered: Option<usize>,
    selection_anchor: Option<Position>,
    reveal_conference: bool,
    reveal_message: bool,
    conference_view: Vec2,
    message_view: Vec2,
    pub content_rect: egui::Rect,
    new_window: bool,
    pub closed: bool,
    children: Vec<(egui::ViewportId, Arc<Mutex<MailApp>>)>,
}

impl MailApp {
    pub fn new(_context: &egui::Context) -> Self {
        let mut settings = MonitorSettings::default();
        settings.scaling_mode = ScalingMode::FitWidth;
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
            composer: None,
            notice: None,
            icons: Icons::default(),
            rendered: None,
            selection_anchor: None,
            reveal_conference: false,
            reveal_message: false,
            conference_view: Vec2::ZERO,
            message_view: Vec2::ZERO,
            content_rect: egui::Rect::NOTHING,
            new_window: false,
            closed: false,
            children: Vec::new(),
        }
    }

    pub fn open(&mut self, path: PathBuf, context: &egui::Context) {
        if self.composer.is_some() {
            return;
        }
        self.loading = Some(path.clone());
        self.error = None;
        self.body_loading = false;
        self.loader.package(path, context);
    }

    pub fn poll(&mut self, context: &egui::Context) {
        while let Ok(event) = self.loader.receiver.try_recv() {
            match event {
                Event::Package(generation, path, result) if generation == self.loader.package_generation => {
                    self.loading = None;
                    match result {
                        Ok(package) => match DraftStore::open(&path, &package) {
                            Ok(drafts) => {
                                self.drafts = Some(drafts);
                                self.reader.set_package(package);
                                self.path = Some(path);
                                self.notice = None;
                                self.rendered = None;
                                self.focus = Pane::Messages;
                                self.reveal_conference = true;
                                self.reveal_message = true;
                                context.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                                    "{} - Icy Mail",
                                    self.path.as_ref().unwrap().file_name().unwrap_or_default().to_string_lossy()
                                )));
                            }
                            Err(error) => self.error = Some(format!("Unable to load drafts for {}:\n{error}", path.display())),
                        },
                        Err(error) => {
                            self.error = Some(format!("{}\n{error}", path.display()));
                            self.rendered = None;
                        }
                    }
                }
                Event::Body(generation, result) if generation == self.loader.body_generation => {
                    self.body_loading = false;
                    match result {
                        Ok(screen) => self.screen = ScreenView::new(screen),
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
                            Ok(()) => self.notice = Some(format!("Reply packet exported to {}", path.display())),
                            Err(error) => self.error = Some(format!("Unable to export {}:\n{error}", path.display())),
                        }
                    }
                }
                _ => {}
            }
        }
        self.sync_body(context);
    }

    fn sync_body(&mut self, context: &egui::Context) {
        if self.loading.is_some() || self.rendered == self.reader.selected_message {
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

    pub fn show(&mut self, context: &egui::Context) {
        if !context.will_discard() {
            self.poll(context);
        }
        let blocked = self.error.is_some() || self.composer.is_some();
        if !blocked && !context.will_discard() {
            self.keys(context);
            let dropped = context.input(|input| input.raw.dropped_files.iter().find_map(|file| file.path.clone()));
            if let Some(path) = dropped {
                self.open(path, context);
            }
        }
        egui::TopBottomPanel::top("toolbar").show(context, |ui| {
            ui.add_enabled_ui(!blocked, |ui| self.toolbar(ui));
        });
        egui::TopBottomPanel::bottom("status").exact_height(24.0).show(context, |ui| {
            ui.horizontal(|ui| {
                if let Some(path) = &self.loading {
                    ui.spinner();
                    ui.add(egui::Label::new(format!("Loading {}", path.file_name().unwrap_or_default().to_string_lossy())).truncate());
                } else if let Some(notice) = &self.notice {
                    ui.add(egui::Label::new(notice).truncate());
                } else if let Some(package) = &self.reader.package {
                    ui.add(egui::Label::new(format!("{} / {} messages", self.reader.messages.len(), package.message_count())).truncate());
                    if let Some(drafts) = &self.drafts {
                        ui.separator();
                        ui.label(format!("{} drafts", drafts.drafts().len()));
                    }
                    ui.separator();
                    ui.add(egui::Label::new(package.control_file.bbs_name.to_string()).truncate())
                        .on_hover_text(self.path.as_ref().map_or(String::new(), |path| path.display().to_string()));
                } else {
                    ui.weak("No package open");
                }
            });
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(context.style().visuals.panel_fill))
            .show(context, |ui| {
                ui.add_enabled_ui(!blocked && self.loading.is_none(), |ui| {
                    if ui.available_width() < 760.0 || ui.available_height() < 300.0 {
                        ui.horizontal(|ui| {
                            for (pane, label) in [(Pane::Conferences, "Conferences"), (Pane::Messages, "Messages"), (Pane::Content, "Message")] {
                                if ui.selectable_label(self.focus == pane, label).clicked() {
                                    self.set_focus(pane, context);
                                }
                            }
                        });
                        ui.separator();
                        match self.focus {
                            Pane::Conferences => self.conferences(ui),
                            Pane::Messages => self.messages(ui),
                            Pane::Content => self.content(ui),
                        }
                    } else {
                        egui::SidePanel::left("conferences")
                            .default_width(300.0)
                            .width_range(180.0..=420.0)
                            .frame(egui::Frame::new())
                            .show_inside(ui, |ui| self.conferences(ui));
                        egui::TopBottomPanel::top("messages")
                            .default_height(240.0)
                            .height_range(90.0..=(ui.available_height() - 120.0).max(90.0))
                            .resizable(true)
                            .frame(egui::Frame::new())
                            .show_inside(ui, |ui| self.messages(ui));
                        egui::CentralPanel::default().frame(egui::Frame::new()).show_inside(ui, |ui| self.content(ui));
                    }
                });
            });
        if !blocked && !context.will_discard() {
            self.sync_body(context);
        }
        if let Some(error) = self.error.clone() {
            let response = appearance::MessageBox::new("mail-error", appearance::MessageKind::Error, "Mail Error", error)
                .copyable()
                .buttons([appearance::DialogButton::primary(appearance::labels::close(), ()).cancels()])
                .show(context);
            if response.action.is_some() || response.dismissed {
                self.error = None;
            }
        }
        self.compose_dialog(context);
        self.windows(context);
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let compact = ui.available_width() < 610.0;
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.horizontal(|ui| {
            if self.icons.button(ui, Icon::Open, "Open Package", !self.loader.picking).clicked() {
                self.loader.pick(&context);
            }
            if self
                .icons
                .button(ui, Icon::Refresh, "Refresh Package", self.path.is_some() && self.loading.is_none())
                .clicked()
            {
                self.open(self.path.clone().unwrap(), &context);
            }
            ui.separator();
            if ui.add_enabled(self.drafts.is_some(), egui::Button::new("New")).clicked() {
                self.new_draft();
            }
            if ui.add_enabled(self.reader.selected_message.is_some(), egui::Button::new("Reply")).clicked() {
                self.reply(false);
            }
            if !compact && ui.add_enabled(self.reader.selected_message.is_some(), egui::Button::new("Forward")).clicked() {
                self.reply(true);
            }
            if !compact
                && ui
                    .add_enabled(
                        self.drafts.as_ref().is_some_and(|store| !store.drafts().is_empty()),
                        egui::Button::new("Export REP"),
                    )
                    .clicked()
            {
                self.pick_export(&context);
            }
            ui.separator();
            for (mode, label) in [(ViewMode::List, "List"), (ViewMode::Threads, "Threads")] {
                if ui.selectable_label(self.reader.view_mode == mode, label).clicked() {
                    self.set_mode(mode);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let menu = self.icons.button(ui, Icon::Menu, "Menu", true);
                egui::Popup::menu(&menu).show(|ui| {
                    if ui.add_enabled(self.reader.selected_message.is_some(), egui::Button::new("Forward")).clicked() {
                        self.reply(true);
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.drafts.as_ref().is_some_and(|store| !store.drafts().is_empty()),
                            egui::Button::new("Export REP"),
                        )
                        .clicked()
                    {
                        self.pick_export(&context);
                        ui.close();
                    }
                    if let Some(store) = &self.drafts {
                        let entries: Vec<_> = store
                            .drafts()
                            .iter()
                            .enumerate()
                            .map(|(index, draft)| (index, draft.subject.clone(), draft.to.clone()))
                            .collect();
                        if !entries.is_empty() {
                            ui.separator();
                            ui.label("Drafts (select to edit)");
                            for (index, subject, to) in entries {
                                if ui
                                    .button(format!("{} - {}", if subject.is_empty() { "(no subject)" } else { &subject }, to))
                                    .clicked()
                                {
                                    self.edit_draft(index);
                                    ui.close();
                                }
                            }
                        }
                    }
                    ui.separator();
                    if ui.button("New Window").clicked() {
                        self.new_window = true;
                        ui.close();
                    }
                    if ui.button("Close Window").clicked() {
                        self.close(&context);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Copy Message").clicked() {
                        self.copy_message(&context);
                        ui.close();
                    }
                    ui.separator();
                    ui.label("Appearance");
                    egui::widgets::global_theme_preference_buttons(ui);
                    ui.separator();
                    egui::ComboBox::from_id_salt("mail-zoom")
                        .selected_text(if self.settings.scaling_mode.is_fit_width() { "Fit width" } else { "100%" })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.settings.scaling_mode, ScalingMode::FitWidth, "Fit width");
                            ui.selectable_value(&mut self.settings.scaling_mode, ScalingMode::Manual(1.0), "100%");
                        });
                });
                if !compact {
                    self.filter(ui, 260.0);
                }
            });
        });
        if compact {
            self.filter(ui, ui.available_width());
        }
    }

    fn filter(&mut self, ui: &mut egui::Ui, width: f32) {
        ui.horizontal(|ui| {
            let response = ui.add_sized(
                [width - 32.0, 30.0],
                appearance::text_edit(&mut self.reader.filter)
                    .id(egui::Id::new("mail-filter"))
                    .hint_text("Filter author or subject"),
            );
            if response.changed() {
                self.reader.rebuild_messages();
                self.reveal_message = true;
            }
            if !self.reader.filter.is_empty()
                && ui
                    .add_sized([24.0, 24.0], egui::Button::new("\u{00d7}").frame(false))
                    .on_hover_text("Clear Filter")
                    .clicked()
            {
                self.reader.filter.clear();
                self.reader.rebuild_messages();
                self.reveal_message = true;
                response.request_focus();
            }
        });
    }

    fn set_mode(&mut self, mode: ViewMode) {
        self.reader.view_mode = mode;
        self.reader.rebuild_messages();
        self.reveal_message = true;
    }

    fn new_draft(&mut self) {
        let Some(package) = &self.reader.package else {
            return;
        };
        let conference = self
            .reader
            .selected_conference
            .or_else(|| conference_choices(package).first().map(|(number, _)| *number));
        let Some(conference) = conference else {
            self.error = Some("This packet has no conference available for a new post".into());
            return;
        };
        if let Some(store) = &self.drafts {
            match store.prepare(package, Compose::New { conference }) {
                Ok(mut draft) => {
                    draft.to = "ALL".into();
                    self.composer = Some(Composer::new(draft, None));
                }
                Err(error) => self.error = Some(format!("Unable to start new message: {error}")),
            }
        }
    }

    fn reply(&mut self, forward: bool) {
        let (Some(package), Some(index)) = (&self.reader.package, self.reader.selected_message) else {
            return;
        };
        let Some(info) = package.infos.get(index) else {
            self.error = Some("Selected message is no longer available".into());
            return;
        };
        let message = match package.get_message(index) {
            Ok(message) => message,
            Err(error) => {
                self.error = Some(format!("Unable to read original message: {error}"));
                return;
            }
        };
        let Some(store) = &self.drafts else {
            return;
        };
        let mut draft = match store.prepare(package, if forward { Compose::Forward { index } } else { Compose::Reply { index } }) {
            Ok(draft) => draft,
            Err(error) => {
                self.error = Some(format!("Unable to start reply: {error}"));
                return;
            }
        };
        let body = decode_cp437(&message.text);
        draft.body = format!(
            "\n\nOn {} {} wrote:\n{}",
            info.date_str,
            info.from,
            body.trim_end().lines().map(|line| format!("> {line}\n")).collect::<String>()
        );
        self.composer = Some(Composer::new(draft, None));
    }

    pub(super) fn edit_draft(&mut self, index: usize) {
        if let Some(draft) = self.drafts.as_ref().and_then(|store| store.drafts().get(index)) {
            self.composer = Some(Composer::new(draft.clone(), Some(index)));
        } else {
            self.error = Some("Draft is no longer available".into());
        }
    }

    fn pick_export(&mut self, context: &egui::Context) {
        if let (Some(path), Some(store)) = (&self.path, &self.drafts) {
            if store.drafts().is_empty() {
                return;
            }
            self.loader.pick_export(store.default_export_path(path), store.clone(), context);
        }
    }

    fn compose_dialog(&mut self, context: &egui::Context) {
        let Some(composer) = &mut self.composer else {
            return;
        };
        #[derive(Clone, Copy)]
        enum Action {
            Save,
            Cancel,
            Delete,
            Discard,
            Keep,
        }
        let Some(package) = &self.reader.package else {
            self.error = Some("No packet open for the draft".into());
            return;
        };
        let conferences = conference_choices(package);
        let response = appearance::Dialog::new("mail-compose")
            .title(if composer.index.is_some() { "Edit Draft" } else { "Compose Message" })
            .size(appearance::DialogSize::Large)
            .max_height((context.content_rect().height() - 24.0).max(160.0))
            .show(context, |dialog| {
                dialog.content(|ui| {
                    if composer.delete || composer.discard {
                        ui.label(if composer.delete {
                            "Delete this draft permanently?"
                        } else {
                            "Discard unsaved changes?"
                        });
                        return;
                    }
                    appearance::form_row(ui, "Conference", |ui| {
                        let selected = conferences
                            .iter()
                            .find(|(number, _)| *number == composer.draft.conference)
                            .map(|(_, name)| name.as_str())
                            .unwrap_or("Select conference");
                        egui::ComboBox::from_id_salt("draft-conference").selected_text(selected).show_ui(ui, |ui| {
                            for (number, name) in &conferences {
                                ui.selectable_value(&mut composer.draft.conference, *number, name);
                            }
                        });
                    });
                    for (label, field) in [
                        ("From", &mut composer.draft.from),
                        ("To", &mut composer.draft.to),
                        ("Subject", &mut composer.draft.subject),
                    ] {
                        appearance::form_row(ui, label, |ui| {
                            ui.add(appearance::text_edit(field).desired_width(f32::INFINITY));
                        });
                    }
                    ui.checkbox(&mut composer.draft.private, "Private message");
                    ui.label("Message");
                    ui.add(
                        egui::TextEdit::multiline(&mut composer.draft.body)
                            .desired_width(f32::INFINITY)
                            .desired_rows(12),
                    );
                    if let Some(error) = &composer.error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                });
                let buttons = if composer.delete {
                    vec![
                        appearance::DialogButton::cancel("Keep Draft", Action::Keep),
                        appearance::DialogButton::destructive("Delete", Action::Delete),
                    ]
                } else if composer.discard {
                    vec![
                        appearance::DialogButton::cancel("Keep Editing", Action::Keep),
                        appearance::DialogButton::destructive("Discard", Action::Discard),
                    ]
                } else {
                    let mut buttons = Vec::new();
                    if composer.index.is_some() {
                        buttons.push(appearance::DialogButton::destructive("Delete Draft", Action::Delete).leading());
                    }
                    buttons.push(appearance::DialogButton::cancel("Cancel", Action::Cancel));
                    buttons.push(appearance::DialogButton::primary("Save Draft", Action::Save));
                    buttons
                };
                dialog.buttons(buttons);
            });
        match response.action.or_else(|| response.dismissed.then_some(Action::Cancel)) {
            Some(Action::Save) => {
                if let Some(store) = &self.drafts {
                    let mut next = store.clone();
                    let result = if composer.index.is_some() {
                        next.update(composer.draft.clone())
                    } else {
                        next.insert(composer.draft.clone())
                    };
                    match result {
                        Ok(()) => {
                            self.drafts = Some(next);
                            self.notice = Some("Draft saved".into());
                            self.error = None;
                            self.composer = None;
                        }
                        Err(error) => composer.error = Some(format!("Unable to save draft: {error}")),
                    }
                }
            }
            Some(Action::Delete) if composer.delete => {
                if let (Some(store), Some(index)) = (&self.drafts, composer.index) {
                    let mut next = store.clone();
                    let id = next.drafts()[index].id;
                    match next.delete(id) {
                        Ok(()) => {
                            self.drafts = Some(next);
                            self.notice = Some("Draft deleted".into());
                            self.error = None;
                            self.composer = None;
                        }
                        Err(error) => composer.error = Some(format!("Unable to delete draft: {error}")),
                    }
                }
            }
            Some(Action::Delete) => composer.delete = true,
            Some(Action::Discard) => {
                self.composer = None;
                self.error = None;
            }
            Some(Action::Cancel) if composer.draft != composer.original => composer.discard = true,
            Some(Action::Cancel) => self.composer = None,
            Some(Action::Keep) => {
                composer.discard = false;
                composer.delete = false;
            }
            None => {}
        }
    }

    fn set_focus(&mut self, pane: Pane, context: &egui::Context) {
        self.focus = pane;
        context.memory_mut(|memory| memory.request_focus(egui::Id::new(format!("mail-pane-{pane:?}"))));
    }

    fn conferences(&mut self, ui: &mut egui::Ui) {
        let pane_rect = ui.max_rect();
        ui.spacing_mut().item_spacing.y = 0.0;
        let width = ui.available_width().max(240.0);
        let widths = [52.0, width - 108.0, 56.0];
        let active = match self.reader.conference_sort.0 {
            ConferenceColumn::Area => 0,
            ConferenceColumn::Name => 1,
            ConferenceColumn::Count => 2,
        };
        let mut select = None;
        egui::ScrollArea::horizontal()
            .id_salt("conference-columns")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(width);
                if let Some(column) = widgets::header(
                    ui,
                    &widths,
                    &["Area", "Description", "Msgs"],
                    Some((active, self.reader.conference_sort.1)),
                    true,
                ) {
                    self.reader
                        .sort_conferences([ConferenceColumn::Area, ConferenceColumn::Name, ConferenceColumn::Count][column]);
                    self.reveal_conference = true;
                }
                let mut scroll = egui::ScrollArea::vertical().id_salt("conference-rows").auto_shrink([false, false]);
                if self.reveal_conference {
                    scroll = scroll.vertical_scroll_offset(widgets::reveal(
                        self.reader.conference_position(),
                        self.conference_view.x,
                        self.conference_view.y,
                    ));
                    self.reveal_conference = false;
                }
                let output = scroll.show_rows(ui, ROW_HEIGHT, self.reader.conferences.len(), |ui, range| {
                    for position in range {
                        let entry = &self.reader.conferences[position];
                        let number = entry.number.map_or_else(|| "All".into(), |number| number.to_string());
                        if widgets::row(
                            ui,
                            &widths,
                            &[&number, &entry.name, &entry.count.to_string()],
                            entry.number == self.reader.selected_conference,
                            self.focus == Pane::Conferences,
                            0,
                        )
                        .clicked()
                        {
                            select = Some(entry.number);
                        }
                    }
                });
                self.conference_view = egui::vec2(output.state.offset.y, output.inner_rect.height());
            });
        if let Some(conference) = select {
            self.reader.select_conference(conference);
            self.set_focus(Pane::Conferences, ui.ctx());
            self.reveal_message = true;
        }
        self.pane_border(ui, pane_rect, Pane::Conferences);
    }

    fn messages(&mut self, ui: &mut egui::Ui) {
        let pane_rect = ui.max_rect();
        ui.spacing_mut().item_spacing.y = 0.0;
        let width = ui.available_width().max(580.0);
        let widths = [150.0, 126.0, width - 328.0, 52.0];
        let active = match self.reader.message_sort.0 {
            MessageColumn::From => 0,
            MessageColumn::Date => 1,
            MessageColumn::Subject => 2,
            MessageColumn::Lines => 3,
        };
        let threaded = self.reader.view_mode == ViewMode::Threads;
        let mut select = None;
        egui::ScrollArea::horizontal()
            .id_salt("message-columns")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(width);
                if let Some(column) = widgets::header(
                    ui,
                    &widths,
                    &["Author", "Date", if threaded { "Subject (threaded)" } else { "Subject" }, "Lines"],
                    (!threaded).then_some((active, self.reader.message_sort.1)),
                    !threaded,
                ) {
                    self.reader
                        .sort_messages([MessageColumn::From, MessageColumn::Date, MessageColumn::Subject, MessageColumn::Lines][column]);
                    self.reveal_message = true;
                }
                let mut scroll = egui::ScrollArea::vertical().id_salt("message-rows").auto_shrink([false, false]);
                if self.reveal_message {
                    if let Some(position) = self.reader.selected_position() {
                        scroll = scroll.vertical_scroll_offset(widgets::reveal(position, self.message_view.x, self.message_view.y));
                    }
                    self.reveal_message = false;
                }
                let output = scroll.show_rows(ui, ROW_HEIGHT, self.reader.messages.len(), |ui, range| {
                    let Some(package) = &self.reader.package else {
                        return;
                    };
                    for position in range {
                        let row = &self.reader.messages[position];
                        let info = &package.infos[row.index];
                        let subject = if threaded {
                            format!(
                                "{} {}",
                                if row.depth > 0 {
                                    "\u{21b3}"
                                } else if row.has_children {
                                    "\u{25be}"
                                } else {
                                    " "
                                },
                                info.subject
                            )
                        } else {
                            info.subject.clone()
                        };
                        let response = widgets::row(
                            ui,
                            &widths,
                            &[&info.from, &info.date_str, &subject, &info.lines.to_string()],
                            self.reader.selected_message == Some(row.index),
                            self.focus == Pane::Messages,
                            row.depth,
                        );
                        if response.clicked() {
                            select = Some((row.index, response.double_clicked()));
                        }
                    }
                });
                self.message_view = egui::vec2(output.state.offset.y, output.inner_rect.height());
                if self.reader.messages.is_empty() {
                    ui.painter().text(
                        output.inner_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        if self.reader.package.is_none() {
                            "No package open"
                        } else if self.reader.filter.is_empty() {
                            "No messages in this conference"
                        } else {
                            "No messages match the filter"
                        },
                        egui::FontId::proportional(14.0),
                        ui.visuals().weak_text_color(),
                    );
                }
            });
        if let Some((index, read)) = select {
            self.reader.select_message(index);
            self.set_focus(if read { Pane::Content } else { Pane::Messages }, ui.ctx());
        }
        self.pane_border(ui, pane_rect, Pane::Messages);
    }

    fn pane_border(&self, ui: &egui::Ui, rect: egui::Rect, pane: Pane) {
        let color = if self.focus == pane {
            ui.visuals().hyperlink_color
        } else {
            ui.visuals().widgets.noninteractive.bg_stroke.color
        };
        ui.painter().rect_stroke(rect, 0, egui::Stroke::new(1.0, color), egui::StrokeKind::Inside);
    }

    fn content(&mut self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        let compact_header = ui.available_height() < 200.0;
        let info = self
            .reader
            .package
            .as_ref()
            .and_then(|package| self.reader.selected_message.and_then(|index| package.infos.get(index)))
            .cloned();
        if let Some(info) = info {
            egui::Frame::new().fill(ui.visuals().faint_bg_color).inner_margin(8).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 90.0 - ui.spacing().item_spacing.x * 3.0).max(30.0);
                    ui.allocate_ui_with_layout(egui::vec2(width, 30.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(width);
                        ui.add(egui::Label::new(egui::RichText::new(&info.subject).strong()).truncate())
                            .on_hover_text(&info.subject);
                    });
                    let context = ui.ctx().clone();
                    let mut action = None;
                    if self
                        .icons
                        .button(ui, Icon::Up, "Previous Message", self.reader.selected_position().is_some_and(|pos| pos > 0))
                        .clicked()
                    {
                        action = Some(NavigateDirection::Up);
                    }
                    if self
                        .icons
                        .button(
                            ui,
                            Icon::Down,
                            "Next Message",
                            self.reader.selected_position().is_some_and(|pos| pos + 1 < self.reader.messages.len()),
                        )
                        .clicked()
                    {
                        action = Some(NavigateDirection::Down);
                    }
                    if self.icons.button(ui, Icon::Copy, "Copy Message", !self.body_loading).clicked() {
                        self.copy_message(&context);
                    }
                    if let Some(action) = action {
                        self.reader.navigate(Pane::Messages, action);
                    }
                });
                if let Some(info) = self
                    .reader
                    .package
                    .as_ref()
                    .and_then(|package| self.reader.selected_message.and_then(|index| package.infos.get(index)))
                {
                    if compact_header {
                        let details = format!(
                            "From: {}  To: {}  {}{}",
                            info.from,
                            info.to,
                            info.date_str,
                            if info.private { "  Private" } else { "" }
                        );
                        ui.add(egui::Label::new(egui::RichText::new(&details).monospace().size(12.0)).truncate())
                            .on_hover_text(details);
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;
                            for field in [format!("From: {}", info.from), format!("To: {}", info.to), info.date_str.clone()] {
                                ui.add(egui::Label::new(egui::RichText::new(field).monospace().size(12.0)).wrap());
                            }
                            if info.private {
                                ui.weak("Private");
                            }
                        });
                    }
                }
            });
            if self.body_loading || self.rendered != self.reader.selected_message {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                });
            } else {
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
                response.context_menu(|ui| {
                    if ui.button("Copy").clicked() {
                        self.copy(ui.ctx());
                        ui.close();
                    }
                    if ui.button("Copy Message").clicked() {
                        self.copy_message(ui.ctx());
                        ui.close();
                    }
                });
            }
        } else {
            ui.centered_and_justified(|ui| {
                ui.weak("No message selected");
            });
        }
        self.pane_border(ui, rect, Pane::Content);
    }

    fn cell(&self, position: egui::Pos2) -> Option<Position> {
        let info = self.screen.terminal.render_info.read();
        let (column, row) = info.screen_to_cell(position.x, position.y)?;
        Some(Position::new(
            column + (self.screen.terminal.scroll_x() / info.font_width.max(1.0)) as i32,
            row + (self.screen.terminal.scroll_y() / info.font_height.max(1.0)) as i32,
        ))
    }

    fn copy(&self, context: &egui::Context) {
        if !self.body_loading {
            if let Some(text) = self.screen.terminal.screen.lock().copy_text() {
                context.copy_text(text);
            }
        }
    }

    fn copy_message(&self, context: &egui::Context) {
        if self.body_loading || self.reader.selected_message.is_none() {
            return;
        }
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

    fn keys(&mut self, context: &egui::Context) {
        if key(context, Key::N, true, false) {
            self.new_draft();
        }
        if key(context, Key::R, true, false) {
            self.reply(false);
        }
        if key(context, Key::E, true, true) {
            self.pick_export(context);
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
        if key(context, Key::T, true, false) {
            self.set_mode(if self.reader.view_mode == ViewMode::List {
                ViewMode::Threads
            } else {
                ViewMode::List
            });
        }
        if key(context, Key::F, true, false) {
            context.memory_mut(|memory| memory.request_focus(egui::Id::new("mail-filter")));
        }
        if key(context, Key::F5, false, false) {
            if let Some(path) = self.path.clone() {
                self.open(path, context);
            }
        }
        if self.loading.is_some() || egui::Popup::is_any_open(context) {
            return;
        }
        if context.memory(|memory| memory.focused() == Some(egui::Id::new("mail-filter"))) {
            if key(context, Key::Escape, false, false) {
                self.reader.filter.clear();
                self.reader.rebuild_messages();
                self.set_focus(Pane::Messages, context);
            }
            if key(context, Key::Enter, false, false) {
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
                if self.focus == Pane::Content {
                    let mut offset = self.screen.offset;
                    let page = self.content_rect.height().max(ROW_HEIGHT);
                    offset.y = match direction {
                        NavigateDirection::Up => offset.y - ROW_HEIGHT,
                        NavigateDirection::Down => offset.y + ROW_HEIGHT,
                        NavigateDirection::PageUp => offset.y - page,
                        NavigateDirection::PageDown => offset.y + page,
                        NavigateDirection::First => 0.0,
                        NavigateDirection::Last => self.screen.max_offset.y,
                    };
                    self.screen.scroll_to = Some(offset.max(Vec2::ZERO));
                } else {
                    self.reader.navigate(self.focus, direction);
                    self.reveal_message = true;
                    self.reveal_conference = true;
                }
            }
        }
        if key(context, Key::Tab, false, false) {
            self.set_focus(self.focus.cycle(true), context);
        }
        if key(context, Key::Tab, false, true) {
            self.set_focus(self.focus.cycle(false), context);
        }
        if key(context, Key::Enter, false, false) && self.focus != Pane::Content {
            self.set_focus(self.focus.cycle(true), context);
        }
        if key(context, Key::Escape, false, false) && !self.reader.filter.is_empty() {
            self.reader.filter.clear();
            self.reader.rebuild_messages();
            self.reveal_message = true;
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

    fn close(&mut self, context: &egui::Context) {
        self.closed = true;
        context.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn windows(&mut self, context: &egui::Context) {
        if self.new_window && !context.will_discard() {
            self.new_window = false;
            let mut child = Self::new(context);
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
        if context.input(|input| input.viewport().close_requested()) {
            self.closed = true;
        }
        if self.closed {
            for (id, child) in &self.children {
                child.lock().closed = true;
                context.send_viewport_cmd_to(*id, egui::ViewportCommand::Close);
            }
        }
    }
}

fn decode_cp437(bytes: &[u8]) -> String {
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

fn conference_choices(package: &icy_mail::qwk::QwkPackage) -> Vec<(u16, String)> {
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

fn key(context: &egui::Context, key: Key, command: bool, shift: bool) -> bool {
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
