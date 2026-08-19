use std::sync::Arc;

use crate::qwk::QwkPackage;
use crate::ui::threading::{self, Row};
use crate::ui::{ConferenceColumn, Message, MessageColumn, NavigateDirection, Pane, SortDirection, ViewMode};
use icy_engine::{EditableScreen, Screen, Size, TextScreen};
use icy_engine_gui::{MonitorSettings, Terminal};
use icy_ui::widget::{button, column, container, operation, progress_bar, text, Space};
use icy_ui::{window, Alignment, Element, Length, Task, Theme};
use parking_lot::Mutex;

/// Row height of both lists; keeping it fixed lets keyboard navigation scroll to a selection.
pub const ROW_HEIGHT: f32 = 20.0;

#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub enum MainWindowMode {
    #[default]
    ShowWelcomeScreen,
    LoadingPackage,
    ShowMailReader,
}

/// A conference entry as shown in the left list.
pub struct ConferenceRow {
    pub number: u16,
    pub name: String,
    pub count: usize,
}

pub struct MainWindow {
    _id: window::Id,
    mode: MainWindowMode,
    pub package: Option<Arc<QwkPackage>>,
    loading_progress: f32,
    loading_message: String,

    pub selected_conference: u16,
    pub selected_message: Option<usize>,
    pub focus: Pane,
    pub view_mode: ViewMode,

    pub filter: String,
    pub message_sort: (MessageColumn, SortDirection),
    pub conference_sort: (ConferenceColumn, SortDirection),

    /// Conferences after sorting; index 0 is the synthetic "All" entry.
    conference_rows: Vec<ConferenceRow>,
    /// Messages after filtering, sorting and (optionally) threading.
    message_rows: Vec<Row>,

    /// Bumped whenever `message_rows` is recomputed, so the virtualized list drops its cache.
    list_generation: u64,

    pub conference_scroll: icy_ui::widget::Id,
    pub message_scroll: icy_ui::widget::Id,
    pub filter_input: icy_ui::widget::Id,

    /// Last reported scroll offset / viewport height per list, used to scroll a selection
    /// into view only when it actually left the visible area.
    conference_view: (f32, f32),
    message_view: (f32, f32),

    pub terminal: Terminal,
    pub monitor_settings: Arc<MonitorSettings>,
}

impl MainWindow {
    pub fn new(id: window::Id, mode: MainWindowMode) -> Self {
        Self {
            _id: id,
            mode,
            package: None,
            loading_progress: 0.0,
            loading_message: String::new(),
            selected_conference: 0,
            selected_message: None,
            focus: Pane::Messages,
            view_mode: ViewMode::List,
            filter: String::new(),
            message_sort: (MessageColumn::Date, SortDirection::Ascending),
            conference_sort: (ConferenceColumn::Area, SortDirection::Ascending),
            conference_rows: Vec::new(),
            message_rows: Vec::new(),
            list_generation: 0,
            conference_scroll: icy_ui::widget::Id::unique(),
            message_scroll: icy_ui::widget::Id::unique(),
            filter_input: icy_ui::widget::Id::unique(),
            conference_view: (0.0, 0.0),
            message_view: (0.0, 0.0),
            terminal: empty_terminal(),
            monitor_settings: Arc::new(MonitorSettings::default()),
        }
    }

    pub fn conference_rows(&self) -> &[ConferenceRow] {
        &self.conference_rows
    }

    pub fn message_rows(&self) -> &[Row] {
        &self.message_rows
    }

    /// Changes whenever the visible rows would render differently, invalidating the row cache.
    pub fn message_list_cache_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.list_generation.hash(&mut hasher);
        self.message_rows.len().hash(&mut hasher);
        self.selected_message.hash(&mut hasher);
        (self.focus == Pane::Messages).hash(&mut hasher);
        hasher.finish()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::_QuitIcyMail => icy_ui::exit(),

            Message::OpenPackage => Task::perform(
                async {
                    let file_dialog = rfd::AsyncFileDialog::new()
                        .set_title("Open Mail Package")
                        .add_filter("Mail Packages", &["zip", "qwk", "rep"])
                        .add_filter("All Files", &["*"]);

                    file_dialog.pick_file().await
                },
                |file| {
                    if let Some(file) = file {
                        Message::PackageSelected(file.path().to_path_buf())
                    } else {
                        Message::Noop
                    }
                },
            ),

            Message::PackageSelected(path) => {
                self.mode = MainWindowMode::LoadingPackage;
                self.loading_progress = 0.0;
                self.loading_message = format!("Loading {}", path.file_name().unwrap_or_default().to_string_lossy());

                Task::perform(
                    async move { tokio::task::spawn_blocking(move || QwkPackage::load_from_file(path).map(Arc::new)).await },
                    |result| match result {
                        Ok(Ok(package)) => Message::PackageLoaded(package),
                        Ok(Err(e)) => Message::PackageLoadError(format!("Failed to load package: {e}")),
                        Err(e) => Message::PackageLoadError(format!("Thread error: {e}")),
                    },
                )
            }

            Message::PackageLoaded(package) => {
                self.package = Some(package);
                self.mode = MainWindowMode::ShowMailReader;
                self.loading_progress = 1.0;
                self.selected_conference = 0;
                self.selected_message = None;
                self.filter.clear();
                self.focus = Pane::Messages;
                self.rebuild_conferences();
                self.rebuild_messages();
                self.select_first_message()
            }

            Message::PackageLoadError(_error) => {
                self.mode = MainWindowMode::ShowWelcomeScreen;
                self.loading_progress = 0.0;
                self.loading_message.clear();
                Task::none()
            }

            Message::SelectConference(conf) => {
                self.selected_conference = conf;
                self.focus = Pane::Conferences;
                self.rebuild_messages();
                Task::batch([self.scroll_conferences_to_selection(), self.select_first_message()])
            }

            Message::SelectMessage(index) => {
                self.selected_message = Some(index);
                self.focus = Pane::Messages;
                self.load_selected_message();
                Task::batch([self.scroll_messages_to_selection(), self.terminal.scroll_to_content(Some(0.0), Some(0.0))])
            }

            Message::SetViewMode(mode) => {
                self.view_mode = mode;
                self.rebuild_messages();
                self.scroll_messages_to_selection()
            }

            Message::FilterChanged(filter) => {
                self.filter = filter;
                self.rebuild_messages();
                // Keep the current message if it survived the filter, otherwise take the first hit.
                if self.selected_row_position().is_some() {
                    self.scroll_messages_to_selection()
                } else {
                    self.select_first_message()
                }
            }

            Message::ClearFilter => {
                self.filter.clear();
                self.rebuild_messages();
                let focus: Task<()> = operation::focus(self.filter_input.clone());
                Task::batch([focus.discard(), self.scroll_messages_to_selection()])
            }

            Message::SortMessagesBy(col) => {
                self.message_sort = if self.message_sort.0 == col {
                    (col, self.message_sort.1.toggled())
                } else {
                    (col, SortDirection::Ascending)
                };
                self.rebuild_messages();
                self.scroll_messages_to_selection()
            }

            Message::SortConferencesBy(col) => {
                self.conference_sort = if self.conference_sort.0 == col {
                    (col, self.conference_sort.1.toggled())
                } else {
                    (col, SortDirection::Ascending)
                };
                self.rebuild_conferences();
                self.scroll_conferences_to_selection()
            }

            Message::FocusPane(pane) => {
                self.focus = pane;
                Task::none()
            }

            Message::CyclePane { forward } => {
                self.focus = match (self.focus, forward) {
                    (Pane::Conferences, true) => Pane::Messages,
                    (Pane::Messages, true) => Pane::Content,
                    (Pane::Content, true) => Pane::Conferences,
                    (Pane::Conferences, false) => Pane::Content,
                    (Pane::Messages, false) => Pane::Conferences,
                    (Pane::Content, false) => Pane::Messages,
                };
                Task::none()
            }

            Message::Navigate(direction) => match self.focus {
                Pane::Conferences => self.navigate_conferences(direction),
                Pane::Messages => self.navigate_messages(direction),
                Pane::Content => self.navigate_content(direction),
            },

            Message::ListScrolled { pane, offset_y, height } => {
                match pane {
                    Pane::Conferences => self.conference_view = (offset_y, height),
                    _ => self.message_view = (offset_y, height),
                }
                Task::none()
            }

            Message::Refresh => {
                self.rebuild_conferences();
                self.rebuild_messages();
                if self.selected_row_position().is_some() {
                    self.scroll_messages_to_selection()
                } else {
                    self.select_first_message()
                }
            }

            Message::TerminalMessage(_) => Task::none(),

            _ => Task::none(),
        }
    }

    // ----- derived state -------------------------------------------------------------------

    fn rebuild_conferences(&mut self) {
        let _timer = crate::perf::Timer::new("rebuild_conferences");
        let Some(package) = &self.package else {
            self.conference_rows.clear();
            return;
        };

        let mut rows: Vec<ConferenceRow> = package
            .conferences()
            .into_iter()
            .map(|(number, name, count)| ConferenceRow { number, name, count })
            .collect();

        let (column, direction) = self.conference_sort;
        rows.sort_by(|a, b| {
            let ordering = match column {
                ConferenceColumn::Area => a.number.cmp(&b.number),
                ConferenceColumn::Name => a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()),
                ConferenceColumn::Count => a.count.cmp(&b.count),
            };
            match direction {
                SortDirection::Ascending => ordering,
                SortDirection::Descending => ordering.reverse(),
            }
        });

        // "All" is a view over every conference and always stays on top.
        rows.insert(
            0,
            ConferenceRow {
                number: 0,
                name: "All Conferences".to_string(),
                count: package.infos.len(),
            },
        );
        self.conference_rows = rows;
    }

    fn rebuild_messages(&mut self) {
        let _timer = crate::perf::Timer::with("rebuild_messages", format!("mode {:?}", self.view_mode));
        self.list_generation = self.list_generation.wrapping_add(1);
        let Some(package) = &self.package else {
            self.message_rows.clear();
            return;
        };

        let needle = self.filter.trim().to_ascii_lowercase();
        let mut infos: Vec<&crate::qwk::MessageInfo> = package
            .infos
            .iter()
            .filter(|info| self.selected_conference == 0 || info.conference == self.selected_conference)
            .filter(|info| {
                needle.is_empty()
                    || info.from.to_ascii_lowercase().contains(&needle)
                    || info.to.to_ascii_lowercase().contains(&needle)
                    || info.subject.to_ascii_lowercase().contains(&needle)
            })
            .collect();

        match self.view_mode {
            ViewMode::Threads => {
                // Threading defines its own order, so column sorting does not apply here.
                self.message_rows = threading::build_threads(&infos);
            }
            ViewMode::List => {
                let (column, direction) = self.message_sort;
                infos.sort_by(|a, b| {
                    let ordering = match column {
                        MessageColumn::From => a.from.to_ascii_lowercase().cmp(&b.from.to_ascii_lowercase()),
                        MessageColumn::Date => a.date.cmp(&b.date).then(a.number.cmp(&b.number)),
                        MessageColumn::Subject => a.subject.to_ascii_lowercase().cmp(&b.subject.to_ascii_lowercase()),
                        MessageColumn::Lines => a.lines.cmp(&b.lines),
                    };
                    match direction {
                        SortDirection::Ascending => ordering,
                        SortDirection::Descending => ordering.reverse(),
                    }
                });
                self.message_rows = infos
                    .iter()
                    .map(|info| Row {
                        index: info.index,
                        depth: 0,
                        has_children: false,
                    })
                    .collect();
            }
        }
    }

    fn selected_row_position(&self) -> Option<usize> {
        let selected = self.selected_message?;
        self.message_rows.iter().position(|row| row.index == selected)
    }

    fn selected_conference_position(&self) -> usize {
        self.conference_rows.iter().position(|row| row.number == self.selected_conference).unwrap_or(0)
    }

    // ----- navigation ----------------------------------------------------------------------

    fn navigate_conferences(&mut self, direction: NavigateDirection) -> Task<Message> {
        if self.conference_rows.is_empty() {
            return Task::none();
        }
        let position = step(self.selected_conference_position(), direction, self.conference_rows.len());
        self.selected_conference = self.conference_rows[position].number;
        self.rebuild_messages();
        Task::batch([self.scroll_conferences_to_selection(), self.select_first_message()])
    }

    fn navigate_messages(&mut self, direction: NavigateDirection) -> Task<Message> {
        if self.message_rows.is_empty() {
            return Task::none();
        }
        let position = match self.selected_row_position() {
            Some(current) => step(current, direction, self.message_rows.len()),
            None => 0,
        };
        self.selected_message = Some(self.message_rows[position].index);
        self.load_selected_message();
        Task::batch([self.scroll_messages_to_selection(), self.terminal.scroll_to_content(Some(0.0), Some(0.0))])
    }

    fn navigate_content(&mut self, direction: NavigateDirection) -> Task<Message> {
        // The reading pane scrolls; Home/End jump to the message boundaries.
        let state = self.terminal.scroll_state();
        let page = state.viewport_height_px.max(ROW_HEIGHT);
        let line = ROW_HEIGHT;
        let target = match direction {
            NavigateDirection::Up => state.scroll_y - line,
            NavigateDirection::Down => state.scroll_y + line,
            NavigateDirection::PageUp => state.scroll_y - page,
            NavigateDirection::PageDown => state.scroll_y + page,
            NavigateDirection::First => 0.0,
            NavigateDirection::Last => f32::MAX,
        };
        self.terminal.scroll_to_content(None, Some(target.max(0.0)))
    }

    fn select_first_message(&mut self) -> Task<Message> {
        self.selected_message = self.message_rows.first().map(|row| row.index);
        if self.selected_message.is_some() {
            self.load_selected_message();
        } else {
            self.terminal = empty_terminal();
        }
        Task::batch([self.scroll_messages_to_selection(), self.terminal.scroll_to_content(Some(0.0), Some(0.0))])
    }

    fn scroll_messages_to_selection(&self) -> Task<Message> {
        let Some(position) = self.selected_row_position() else {
            return Task::none();
        };
        scroll_into_view(self.message_scroll.clone(), position, self.message_view)
    }

    fn scroll_conferences_to_selection(&self) -> Task<Message> {
        scroll_into_view(self.conference_scroll.clone(), self.selected_conference_position(), self.conference_view)
    }

    // ----- message rendering ---------------------------------------------------------------

    fn load_selected_message(&mut self) {
        let Some(index) = self.selected_message else {
            self.terminal = empty_terminal();
            return;
        };
        let Some(package) = &self.package else { return };
        let Ok(message) = package.get_message(index) else {
            self.terminal = empty_terminal();
            return;
        };
        let text = message.text.clone();
        self.load_message_to_screen(&text);
    }

    /// Renders the message body through the ANSI parser into a terminal screen.
    fn load_message_to_screen(&mut self, data: &[u8]) {
        let _timer = crate::perf::Timer::with("load_message_to_screen", format!("{} bytes", data.len()));
        use icy_engine::load_with_parser;
        use icy_parser_core::AnsiParser;

        // QWK stores bare LF line ends; the ANSI parser needs the CR to return to column 0.
        let mut normalized = Vec::with_capacity(data.len() + data.len() / 8);
        for byte in data {
            if *byte == b'\n' {
                normalized.push(b'\r');
            }
            normalized.push(*byte);
        }

        let height = normalized.iter().filter(|b| **b == b'\n').count().max(24) + 1;
        let mut text_screen = TextScreen::new(Size::new(80, height as i32));
        text_screen.terminal_state_mut().is_terminal_buffer = false;

        let mut parser = AnsiParser::new();
        let _ = load_with_parser(&mut text_screen, &mut parser, &normalized, true, -1);

        let screen: Box<dyn Screen> = Box::new(text_screen);
        self.terminal = Terminal::new(Arc::new(Mutex::new(screen)));
        self.terminal.set_fit_terminal_height_to_bounds(false);
    }

    // ----- view ----------------------------------------------------------------------------

    pub fn view(&self) -> Element<'_, Message> {
        match &self.mode {
            MainWindowMode::ShowWelcomeScreen => {
                let content = column![
                    Space::new().height(Length::Fill),
                    text("Welcome to IcyMail").size(32),
                    Space::new().height(20),
                    text("Open a mail package to get started").size(16),
                    Space::new().height(30),
                    button(text("Open Package").size(16)).on_press(Message::OpenPackage).padding([12, 24]),
                    Space::new().height(Length::Fill),
                ]
                .align_x(Alignment::Center)
                .width(Length::Fill);

                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into()
            }

            MainWindowMode::LoadingPackage => {
                let content = column![
                    Space::new().height(Length::Fill),
                    text("Loading Package").size(24),
                    Space::new().height(20),
                    text(&self.loading_message).size(14),
                    Space::new().height(20),
                    progress_bar(0.0..=1.0, self.loading_progress),
                    Space::new().height(Length::Fill),
                ]
                .align_x(Alignment::Center)
                .width(Length::Fill);

                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into()
            }

            MainWindowMode::ShowMailReader => {
                crate::perf::count_frame(self.message_rows.len());
                let _timer = crate::perf::Timer::new("view::mail_reader");
                self.mail_reader_view()
            }
        }
    }

    pub fn handle_event(&self, event: &icy_ui::Event, captured: bool) -> Option<Message> {
        use icy_ui::keyboard::{key::Named, Event as KeyEvent, Key};

        let icy_ui::Event::Keyboard(KeyEvent::KeyPressed { key, modifiers, .. }) = event else {
            return None;
        };

        // Ctrl+T flips the view mode even while the filter box has focus.
        if modifiers.command() && matches!(key, Key::Character(c) if c.as_str() == "t") {
            return Some(Message::SetViewMode(match self.view_mode {
                ViewMode::List => ViewMode::Threads,
                ViewMode::Threads => ViewMode::List,
            }));
        }

        // Anything the focused widget already used (typing in the filter box) is not navigation.
        if captured {
            return None;
        }

        match key {
            Key::Named(Named::ArrowUp) => Some(Message::Navigate(NavigateDirection::Up)),
            Key::Named(Named::ArrowDown) => Some(Message::Navigate(NavigateDirection::Down)),
            Key::Named(Named::Home) => Some(Message::Navigate(NavigateDirection::First)),
            Key::Named(Named::End) => Some(Message::Navigate(NavigateDirection::Last)),
            Key::Named(Named::PageUp) => Some(Message::Navigate(NavigateDirection::PageUp)),
            Key::Named(Named::PageDown) => Some(Message::Navigate(NavigateDirection::PageDown)),
            Key::Named(Named::Tab) => Some(Message::CyclePane { forward: !modifiers.shift() }),
            Key::Named(Named::Enter) if self.focus == Pane::Conferences => Some(Message::FocusPane(Pane::Messages)),
            Key::Named(Named::Enter) if self.focus == Pane::Messages => Some(Message::FocusPane(Pane::Content)),
            Key::Named(Named::Escape) if !self.filter.is_empty() => Some(Message::ClearFilter),
            _ => None,
        }
    }

    pub fn theme(&self) -> Theme {
        Theme::dark()
    }
}

/// Moves `current` by `direction` inside a list of `len` entries.
fn step(current: usize, direction: NavigateDirection, len: usize) -> usize {
    let last = len.saturating_sub(1);
    match direction {
        NavigateDirection::Up => current.saturating_sub(1),
        NavigateDirection::Down => (current + 1).min(last),
        NavigateDirection::First => 0,
        NavigateDirection::Last => last,
        NavigateDirection::PageUp => current.saturating_sub(10),
        NavigateDirection::PageDown => (current + 10).min(last),
    }
}

/// Scrolls just far enough to reveal `position`, so arrow keys do not yank the list around.
fn scroll_into_view(id: icy_ui::widget::Id, position: usize, view: (f32, f32)) -> Task<Message> {
    let Some(target) = scroll_target(position, view) else {
        return Task::none();
    };

    operation::scroll_to(id, operation::AbsoluteOffset { x: None, y: Some(target) })
}

/// New scroll offset needed to reveal `position`, or `None` when it is already visible.
fn scroll_target(position: usize, (offset, height): (f32, f32)) -> Option<f32> {
    let top = position as f32 * ROW_HEIGHT;
    let bottom = top + ROW_HEIGHT;

    let target = if height <= 0.0 {
        // Viewport size not reported yet - fall back to putting the row on top.
        top
    } else if top < offset {
        top
    } else if bottom > offset + height {
        bottom - height
    } else {
        return None;
    };

    Some(target.max(0.0))
}

fn empty_terminal() -> Terminal {
    let screen: Box<dyn Screen> = Box::new(TextScreen::new(Size::new(80, 25)));
    let mut terminal = Terminal::new(Arc::new(Mutex::new(screen)));
    terminal.set_fit_terminal_height_to_bounds(false);
    terminal
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{ConferenceColumn, MessageColumn};

    #[test]
    fn step_clamps_at_both_ends() {
        assert_eq!(step(0, NavigateDirection::Up, 5), 0);
        assert_eq!(step(4, NavigateDirection::Down, 5), 4);
        assert_eq!(step(2, NavigateDirection::Up, 5), 1);
        assert_eq!(step(2, NavigateDirection::Down, 5), 3);
    }

    #[test]
    fn step_jumps_to_first_and_last() {
        assert_eq!(step(3, NavigateDirection::First, 5), 0);
        assert_eq!(step(1, NavigateDirection::Last, 5), 4);
    }

    #[test]
    fn step_pages_by_ten() {
        assert_eq!(step(25, NavigateDirection::PageUp, 100), 15);
        assert_eq!(step(25, NavigateDirection::PageDown, 100), 35);
        assert_eq!(step(95, NavigateDirection::PageDown, 100), 99);
    }

    #[test]
    fn step_on_empty_list_stays_at_zero() {
        assert_eq!(step(0, NavigateDirection::Down, 0), 0);
        assert_eq!(step(0, NavigateDirection::Last, 0), 0);
    }

    #[test]
    fn visible_rows_do_not_scroll_the_list() {
        // Rows 0..9 are visible with a 200px viewport at offset 0.
        assert_eq!(scroll_target(0, (0.0, 200.0)), None);
        assert_eq!(scroll_target(9, (0.0, 200.0)), None);
    }

    #[test]
    fn scrolls_by_one_row_when_stepping_past_an_edge() {
        // Row 10 sits just below the viewport, so it scrolls exactly one row.
        assert_eq!(scroll_target(10, (0.0, 200.0)), Some(ROW_HEIGHT));
        // Row 4 sits just above a viewport starting at row 5.
        assert_eq!(scroll_target(4, (5.0 * ROW_HEIGHT, 200.0)), Some(4.0 * ROW_HEIGHT));
    }

    #[test]
    fn scroll_target_never_goes_negative() {
        assert_eq!(scroll_target(0, (0.0, 10_000.0)), None);
        assert!(scroll_target(0, (50.0, 200.0)).unwrap() >= 0.0);
    }

    #[test]
    fn unknown_viewport_falls_back_to_top_alignment() {
        assert_eq!(scroll_target(7, (0.0, 0.0)), Some(7.0 * ROW_HEIGHT));
    }

    /// A window with the synthetic test packet already loaded.
    fn loaded() -> (crate::qwk::tests::TempDir, MainWindow) {
        let (dir, package) = crate::qwk::tests::load();
        let mut window = MainWindow::new(window::Id::unique(), MainWindowMode::ShowWelcomeScreen);
        let _ = window.update(Message::PackageLoaded(Arc::new(package)));
        (dir, window)
    }

    #[test]
    fn loading_selects_the_first_message_and_lists_all_conferences() {
        let (_dir, window) = loaded();
        // "All" plus the two populated conferences.
        assert_eq!(window.conference_rows().len(), 3);
        assert_eq!(window.conference_rows()[0].count, 4);
        assert_eq!(window.message_rows().len(), 4);
        assert!(window.selected_message.is_some());
    }

    #[test]
    fn selecting_a_conference_filters_the_message_list() {
        let (_dir, mut window) = loaded();
        let _ = window.update(Message::SelectConference(2));
        assert_eq!(window.message_rows().len(), 2);
        let package = window.package.clone().unwrap();
        assert!(window.message_rows().iter().all(|row| package.infos[row.index].conference == 2));
    }

    #[test]
    fn filter_matches_author_and_subject_case_insensitively() {
        let (_dir, mut window) = loaded();

        let _ = window.update(Message::FilterChanged("ALICE".to_string()));
        assert_eq!(window.message_rows().len(), 1);

        let _ = window.update(Message::FilterChanged("amiga".to_string()));
        assert_eq!(window.message_rows().len(), 2);

        let _ = window.update(Message::FilterChanged("nothing here".to_string()));
        assert!(window.message_rows().is_empty());
        assert!(window.selected_message.is_none());

        let _ = window.update(Message::ClearFilter);
        assert_eq!(window.message_rows().len(), 4);
    }

    #[test]
    fn sorting_toggles_direction_on_the_same_column() {
        let (_dir, mut window) = loaded();
        let package = window.package.clone().unwrap();

        let _ = window.update(Message::SortMessagesBy(MessageColumn::From));
        let ascending: Vec<&str> = window.message_rows().iter().map(|r| package.infos[r.index].from.as_str()).collect();
        assert_eq!(ascending, vec!["alice", "bob", "carol", "dave"]);

        let _ = window.update(Message::SortMessagesBy(MessageColumn::From));
        let descending: Vec<&str> = window.message_rows().iter().map(|r| package.infos[r.index].from.as_str()).collect();
        assert_eq!(descending, vec!["dave", "carol", "bob", "alice"]);
    }

    #[test]
    fn conference_sorting_keeps_all_on_top() {
        let (_dir, mut window) = loaded();
        let _ = window.update(Message::SortConferencesBy(ConferenceColumn::Count));
        assert_eq!(window.conference_rows()[0].number, 0);
    }

    #[test]
    fn thread_mode_indents_replies_without_losing_messages() {
        let (_dir, mut window) = loaded();
        let _ = window.update(Message::SetViewMode(ViewMode::Threads));

        assert_eq!(window.message_rows().len(), 4);
        assert!(window.message_rows().iter().any(|row| row.depth > 0));

        let _ = window.update(Message::SetViewMode(ViewMode::List));
        assert!(window.message_rows().iter().all(|row| row.depth == 0));
    }

    #[test]
    fn navigation_moves_the_selection_through_the_visible_rows() {
        let (_dir, mut window) = loaded();
        let first = window.selected_message;

        let _ = window.update(Message::Navigate(NavigateDirection::Down));
        assert_ne!(window.selected_message, first);

        let _ = window.update(Message::Navigate(NavigateDirection::Last));
        assert_eq!(window.selected_message, window.message_rows().last().map(|r| r.index));

        let _ = window.update(Message::Navigate(NavigateDirection::First));
        assert_eq!(window.selected_message, window.message_rows().first().map(|r| r.index));
    }

    #[test]
    fn tab_cycles_panes_in_both_directions() {
        let (_dir, mut window) = loaded();
        window.focus = Pane::Conferences;

        let _ = window.update(Message::CyclePane { forward: true });
        assert_eq!(window.focus, Pane::Messages);
        let _ = window.update(Message::CyclePane { forward: true });
        assert_eq!(window.focus, Pane::Content);
        let _ = window.update(Message::CyclePane { forward: true });
        assert_eq!(window.focus, Pane::Conferences);
        let _ = window.update(Message::CyclePane { forward: false });
        assert_eq!(window.focus, Pane::Content);
    }

    #[test]
    fn reader_view_builds_in_every_mode() {
        let (_dir, mut window) = loaded();

        // Exercises the layout code so a bad index or width cannot ship unnoticed.
        let _ = window.view();

        let _ = window.update(Message::SetViewMode(ViewMode::Threads));
        let _ = window.view();

        let _ = window.update(Message::FilterChanged("no match at all".to_string()));
        let _ = window.view();
    }
}
