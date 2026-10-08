use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    sync::Arc,
};

use i18n_embed_fl::fl;
use icy_engine::{EditableScreen, Size, TextScreen};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    qwk::{MessageInfo, QwkPackage},
    threading::{self, Row},
    Res, LANGUAGE_LOADER,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigateDirection {
    Up,
    Down,
    First,
    Last,
    PageUp,
    PageDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pane {
    Conferences,
    #[default]
    Messages,
    Content,
}

impl Pane {
    pub fn cycle(self, forward: bool) -> Self {
        match (self, forward) {
            (Self::Content, true) | (Self::Messages, false) => Self::Conferences,
            (Self::Conferences, true) | (Self::Content, false) => Self::Messages,
            (Self::Messages, true) | (Self::Conferences, false) => Self::Content,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ViewMode {
    #[default]
    List,
    Threads,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageColumn {
    From,
    Date,
    Subject,
    Lines,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConferenceColumn {
    Area,
    Name,
    Count,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    pub fn toggled(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }

    pub fn arrow(self) -> &'static str {
        match self {
            Self::Ascending => "\u{25b2}",
            Self::Descending => "\u{25bc}",
        }
    }

    fn apply(self, order: Ordering) -> Ordering {
        match self {
            Self::Ascending => order,
            Self::Descending => order.reverse(),
        }
    }
}

pub struct ConferenceRow {
    pub number: Option<u16>,
    pub name: String,
    pub count: usize,
}

/// Which parts of a message the search looks at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchFields {
    pub from: bool,
    pub to: bool,
    pub subject: bool,
    /// The message text, searched in the background.
    pub text: bool,
}

impl Default for SearchFields {
    fn default() -> Self {
        Self {
            from: true,
            to: true,
            subject: true,
            text: true,
        }
    }
}

impl SearchFields {
    pub fn all(self) -> bool {
        self.from && self.to && self.subject && self.text
    }

    pub fn any(self) -> bool {
        self.from || self.to || self.subject || self.text
    }
}

pub struct Reader {
    pub package: Option<Arc<QwkPackage>>,
    pub selected_conference: Option<u16>,
    pub selected_message: Option<usize>,
    pub filter: String,
    /// Only messages addressed to this user name (case-insensitive), for the personal mailbox.
    pub personal: Option<String>,
    /// Read flag per package index.
    read: Vec<bool>,
    /// Star per package index.
    starred: Vec<bool>,
    pub unread_only: bool,
    /// Only starred messages, for the starred mailbox.
    pub starred_only: bool,
    pub view_mode: ViewMode,
    pub message_sort: (MessageColumn, SortDirection),
    pub conference_sort: (ConferenceColumn, SortDirection),
    pub conferences: Vec<ConferenceRow>,
    /// Visible rows: `all_messages` without the replies of collapsed threads.
    pub messages: Vec<Row>,
    /// Every message matching the current folder and filters, in list or thread order.
    all_messages: Vec<Row>,
    /// Package indices of threads whose replies are hidden.
    collapsed: HashSet<usize>,
    /// Position in `messages` per package index, `u32::MAX` when filtered out or collapsed.
    positions: Vec<u32>,
    /// Position in `all_messages` per package index, `u32::MAX` when filtered out.
    all_positions: Vec<u32>,
    /// Unread messages in `all_messages`.
    unread: usize,
    /// Package index per `(conference, message number)`.
    numbers: HashMap<(u16, u32), usize>,
    /// Parts of the messages the filter searches.
    pub search_fields: SearchFields,
    /// Lowercased `from`, `to` and `subject` per package index, built on the first search.
    search: Vec<[String; 3]>,
    /// Body matches supplied by a background search, keyed by its normalized query.
    body_matches: Option<(String, HashSet<usize>)>,
}

impl Default for Reader {
    fn default() -> Self {
        Self {
            package: None,
            selected_conference: None,
            selected_message: None,
            filter: String::new(),
            personal: None,
            read: Vec::new(),
            starred: Vec::new(),
            unread_only: false,
            starred_only: false,
            view_mode: ViewMode::List,
            message_sort: (MessageColumn::Date, SortDirection::Ascending),
            conference_sort: (ConferenceColumn::Area, SortDirection::Ascending),
            conferences: Vec::new(),
            messages: Vec::new(),
            all_messages: Vec::new(),
            collapsed: HashSet::new(),
            positions: Vec::new(),
            all_positions: Vec::new(),
            unread: 0,
            numbers: HashMap::new(),
            search_fields: SearchFields::default(),
            search: Vec::new(),
            body_matches: None,
        }
    }
}

impl Reader {
    pub fn set_package(&mut self, package: Arc<QwkPackage>) {
        self.package = Some(package);
        self.selected_conference = None;
        self.selected_message = None;
        self.filter.clear();
        self.personal = None;
        self.read.clear();
        self.starred.clear();
        self.unread_only = false;
        self.starred_only = false;
        self.numbers.clear();
        self.search.clear();
        self.body_matches = None;
        self.collapsed.clear();
        self.rebuild_conferences();
        self.rebuild_messages();
    }

    pub fn rebuild_conferences(&mut self) {
        let Some(package) = &self.package else {
            self.conferences.clear();
            return;
        };
        self.conferences = package
            .conferences()
            .into_iter()
            .map(|(number, name, count)| ConferenceRow {
                number: Some(number),
                name,
                count,
            })
            .collect();
        let (column, direction) = self.conference_sort;
        self.conferences.sort_by(|left, right| {
            direction.apply(match column {
                ConferenceColumn::Area => left.number.cmp(&right.number),
                ConferenceColumn::Name => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
                ConferenceColumn::Count => left.count.cmp(&right.count),
            })
        });
        self.conferences.insert(
            0,
            ConferenceRow {
                number: None,
                name: fl!(LANGUAGE_LOADER, "packet-all-conferences"),
                count: package.message_count(),
            },
        );
    }

    pub fn rebuild_messages(&mut self) {
        let Some(package) = &self.package else {
            self.messages.clear();
            self.all_messages.clear();
            self.selected_message = None;
            return;
        };
        let package = package.clone();
        self.read.resize(package.infos.len(), false);
        self.starred.resize(package.infos.len(), false);
        let needle = self.filter.trim().to_lowercase();
        let cached_threads = (self.view_mode == ViewMode::Threads
            && self.selected_conference.is_none()
            && self.personal.is_none()
            && !self.unread_only
            && !self.starred_only
            && needle.is_empty())
        .then(|| package.cached_threads())
        .flatten();
        if let Some(rows) = cached_threads {
            self.all_messages = rows.to_vec();
        } else {
            if !needle.is_empty() && self.search.len() != package.infos.len() {
                self.search = package
                    .infos
                    .par_iter()
                    .with_min_len(1024)
                    .map(|info| [info.from.to_lowercase(), info.to.to_lowercase(), info.subject.to_lowercase()])
                    .collect();
            }
            let personal = self.personal.as_ref().map(|name| name.trim().to_string());
            let fields = self.search_fields;
            let body_matches = self
                .body_matches
                .as_ref()
                .filter(|(query, _)| fields.text && *query == needle)
                .map(|(_, matches)| matches);
            let search = &self.search;
            let matches = |index: usize| {
                let [from, to, subject] = &search[index];
                (fields.from && from.contains(&needle))
                    || (fields.to && to.contains(&needle))
                    || (fields.subject && subject.contains(&needle))
                    || body_matches.is_some_and(|matches| matches.contains(&index))
            };
            let mut infos: Vec<_> = package
                .infos
                .par_iter()
                .with_min_len(1024)
                .filter(|info| {
                    self.selected_conference.is_none_or(|number| info.conference == number)
                        && personal.as_ref().is_none_or(|name| package.matches_personal(&info.to, name))
                        && (!self.unread_only || !self.read[info.index])
                        && (!self.starred_only || self.starred[info.index])
                        && (needle.is_empty() || matches(info.index))
                })
                .collect();
            self.all_messages = match self.view_mode {
                ViewMode::Threads => threading::build_threads(&infos),
                ViewMode::List => {
                    let (column, direction) = self.message_sort;
                    match column {
                        // Lowercase each key once instead of in every comparison.
                        MessageColumn::From | MessageColumn::Subject => {
                            let text = |info: &MessageInfo| {
                                if column == MessageColumn::From {
                                    info.from.to_lowercase()
                                } else {
                                    info.subject.to_lowercase()
                                }
                            };
                            match direction {
                                SortDirection::Ascending => infos.sort_by_cached_key(|info| (text(info), info.index)),
                                SortDirection::Descending => infos.sort_by_cached_key(|info| (std::cmp::Reverse(text(info)), info.index)),
                            }
                        }
                        MessageColumn::Date => infos.sort_unstable_by(|left, right| {
                            direction
                                .apply(left.date.cmp(&right.date).then(left.number.cmp(&right.number)))
                                .then(left.index.cmp(&right.index))
                        }),
                        MessageColumn::Lines => {
                            infos.sort_unstable_by(|left, right| direction.apply(left.lines.cmp(&right.lines)).then(left.index.cmp(&right.index)));
                        }
                    }
                    infos.iter().map(|info| Row::flat(info.index)).collect()
                }
            };
        }
        self.all_positions.clear();
        self.all_positions.resize(package.infos.len(), u32::MAX);
        self.unread = 0;
        for (position, row) in self.all_messages.iter().enumerate() {
            self.all_positions[row.index] = position as u32;
            self.unread += usize::from(!self.read[row.index]);
        }
        match self.selected_message {
            // A selected reply stays selected, even when its thread was collapsed meanwhile.
            Some(index) if self.contains(index) => {
                self.expand_to(index);
            }
            _ => self.selected_message = self.all_messages.first().map(|row| row.index),
        }
        self.apply_collapsed();
    }

    /// Rebuilds the visible rows from `all_messages`, skipping the replies of collapsed threads.
    fn apply_collapsed(&mut self) {
        let len = self.all_positions.len();
        self.positions.clear();
        self.positions.resize(len, u32::MAX);
        self.messages.clear();
        let mut position = 0;
        while let Some(row) = self.all_messages.get(position).copied() {
            self.positions[row.index] = self.messages.len() as u32;
            self.messages.push(row);
            position += 1;
            if row.descendants > 0 && self.collapsed.contains(&row.index) {
                position += row.descendants as usize;
            }
        }
    }

    /// Clears the collapsed flag of every thread level above `index`. Returns whether any changed.
    fn expand_to(&mut self, index: usize) -> bool {
        let Some(mut position) = self.all_position(index) else {
            return false;
        };
        let mut depth = self.all_messages[position].depth;
        let mut changed = false;
        while depth > 0 && position > 0 {
            position -= 1;
            let row = self.all_messages[position];
            if row.depth < depth {
                changed |= self.collapsed.remove(&row.index);
                depth = row.depth;
            }
        }
        changed
    }

    pub fn select_conference(&mut self, conference: Option<u16>) {
        self.selected_conference = conference;
        self.selected_message = None;
        self.rebuild_messages();
    }

    pub fn select_message(&mut self, index: usize) {
        if self.contains(index) {
            if self.expand_to(index) {
                self.apply_collapsed();
            }
            self.selected_message = Some(index);
        }
    }

    pub fn set_body_matches(&mut self, query: String, matches: HashSet<usize>) {
        self.body_matches = Some((query, matches));
        self.rebuild_messages();
    }

    /// Whether the message with this package index is in the current list, possibly inside a collapsed thread.
    pub fn contains(&self, index: usize) -> bool {
        self.all_position(index).is_some()
    }

    /// Every message in the current list, including the replies of collapsed threads.
    pub fn all_messages(&self) -> &[Row] {
        &self.all_messages
    }

    fn position(&self, index: usize) -> Option<usize> {
        self.positions
            .get(index)
            .filter(|position| **position != u32::MAX)
            .map(|position| *position as usize)
    }

    fn all_position(&self, index: usize) -> Option<usize> {
        self.all_positions
            .get(index)
            .filter(|position| **position != u32::MAX)
            .map(|position| *position as usize)
    }

    /// Whether the replies of this thread are hidden.
    pub fn is_collapsed(&self, index: usize) -> bool {
        self.collapsed.contains(&index)
    }

    /// Hides or shows the replies below a message. Returns whether anything changed.
    ///
    /// Collapsing a thread that contains the selection moves the selection to the collapsed message.
    pub fn set_collapsed(&mut self, index: usize, collapsed: bool) -> bool {
        let Some(position) = self.all_position(index) else {
            return false;
        };
        let row = self.all_messages[position];
        if row.descendants == 0 || self.collapsed.contains(&index) == collapsed {
            return false;
        }
        if collapsed {
            self.collapsed.insert(index);
            let replies = position + 1..=position + row.descendants as usize;
            if self
                .selected_message
                .and_then(|selected| self.all_position(selected))
                .is_some_and(|selected| replies.contains(&selected))
            {
                self.selected_message = Some(index);
            }
        } else {
            self.collapsed.remove(&index);
        }
        self.apply_collapsed();
        true
    }

    pub fn toggle_collapsed(&mut self, index: usize) -> bool {
        self.set_collapsed(index, !self.is_collapsed(index))
    }

    /// Every message of the thread `index` belongs to, the thread's first message first. Only the
    /// thread view knows threads; elsewhere this is just the message itself.
    pub fn thread_of(&self, index: usize) -> Vec<usize> {
        let Some(position) = self.all_position(index).filter(|_| self.view_mode == ViewMode::Threads) else {
            return vec![index];
        };
        let root = self.all_messages[..=position].iter().rposition(|row| row.depth == 0).unwrap_or(position);
        let end = root + self.all_messages[root].descendants as usize;
        self.all_messages[root..=end].iter().map(|row| row.index).collect()
    }

    /// Collapses every thread to its first message, or expands them all. A selected reply moves to its
    /// thread's first message. Returns whether anything changed.
    pub fn set_all_collapsed(&mut self, collapsed: bool) -> bool {
        let before = self.collapsed.len();
        if collapsed {
            let roots: Vec<usize> = self
                .all_messages
                .iter()
                .filter(|row| row.depth == 0 && row.descendants > 0)
                .map(|row| row.index)
                .collect();
            if let Some(root) = self.selected_message.and_then(|selected| self.thread_of(selected).first().copied()) {
                self.selected_message = Some(root);
            }
            self.collapsed.clear();
            self.collapsed.extend(roots);
        } else {
            self.collapsed.clear();
        }
        let changed = before != self.collapsed.len() || collapsed;
        self.apply_collapsed();
        changed
    }

    /// Unread replies below a message, at any depth.
    pub fn unread_replies(&self, index: usize) -> usize {
        self.all_position(index).map_or(0, |position| {
            let row = self.all_messages[position];
            self.all_messages[position + 1..=position + row.descendants as usize]
                .iter()
                .filter(|reply| !self.is_read(reply.index))
                .count()
        })
    }

    /// Left arrow in a tree: collapses the selected thread, or moves to its parent.
    pub fn collapse_or_parent(&mut self) -> bool {
        let Some(position) = self.selected_position() else {
            return false;
        };
        let row = self.messages[position];
        if row.descendants > 0 && !self.is_collapsed(row.index) {
            return self.set_collapsed(row.index, true);
        }
        match self.messages[..position].iter().rev().find(|parent| parent.depth < row.depth) {
            Some(parent) => {
                self.selected_message = Some(parent.index);
                true
            }
            None => false,
        }
    }

    /// Right arrow in a tree: expands the selected thread, or moves to its first reply.
    pub fn expand_or_child(&mut self) -> bool {
        let Some(position) = self.selected_position() else {
            return false;
        };
        let row = self.messages[position];
        if row.descendants == 0 {
            return false;
        }
        if self.is_collapsed(row.index) {
            return self.set_collapsed(row.index, false);
        }
        self.selected_message = self.messages.get(position + 1).map(|child| child.index);
        true
    }

    /// First unread message after `after` (or from the start), looking into collapsed threads too.
    pub fn next_unread(&self, after: Option<usize>) -> Option<usize> {
        let start = after.and_then(|index| self.all_position(index)).map_or(0, |position| position + 1);
        self.all_messages[start.min(self.all_messages.len())..]
            .iter()
            .find(|row| !self.is_read(row.index))
            .map(|row| row.index)
    }

    pub fn is_read(&self, index: usize) -> bool {
        self.read.get(index).copied().unwrap_or(false)
    }

    /// Updates one read mark. Returns whether it changed.
    pub fn set_read(&mut self, index: usize, read: bool) -> bool {
        let Some(flag) = self.read.get_mut(index) else {
            return false;
        };
        if *flag == read {
            return false;
        }
        *flag = read;
        if self.contains(index) {
            if read {
                self.unread -= 1;
            } else {
                self.unread += 1;
            }
        }
        true
    }

    /// Replaces all read marks by the given package indices.
    pub fn set_read_marks(&mut self, indices: impl IntoIterator<Item = usize>) {
        let len = self.package.as_ref().map_or(0, |package| package.infos.len());
        self.read.clear();
        self.read.resize(len, false);
        for index in indices {
            if let Some(flag) = self.read.get_mut(index) {
                *flag = true;
            }
        }
        self.unread = self.all_messages.iter().filter(|row| !self.read[row.index]).count();
    }

    pub fn is_starred(&self, index: usize) -> bool {
        self.starred.get(index).copied().unwrap_or(false)
    }

    /// Updates one star; the starred mailbox keeps showing an unstarred message until it is rebuilt,
    /// so a mistaken click can be undone in place. Returns whether it changed.
    pub fn set_starred(&mut self, index: usize, starred: bool) -> bool {
        match self.starred.get_mut(index) {
            Some(flag) if *flag != starred => {
                *flag = starred;
                true
            }
            _ => false,
        }
    }

    /// Replaces all stars by the given package indices.
    pub fn set_stars(&mut self, indices: impl IntoIterator<Item = usize>) {
        let len = self.package.as_ref().map_or(0, |package| package.infos.len());
        self.starred.clear();
        self.starred.resize(len, false);
        for index in indices {
            if let Some(flag) = self.starred.get_mut(index) {
                *flag = true;
            }
        }
        if self.starred_only {
            self.rebuild_messages();
        }
    }

    /// Unread messages in the current list.
    pub fn unread_count(&self) -> usize {
        self.unread
    }

    /// Package index of a message by its conference and number.
    pub fn find(&mut self, conference: u16, number: u32) -> Option<usize> {
        let package = self.package.as_ref()?;
        if self.numbers.is_empty() {
            self.numbers = package.infos.iter().map(|info| ((info.conference, info.number), info.index)).collect();
        }
        self.numbers.get(&(conference, number)).copied()
    }

    pub fn sort_messages(&mut self, column: MessageColumn) {
        if self.view_mode == ViewMode::Threads {
            return;
        }
        self.message_sort = (
            column,
            if self.message_sort.0 == column {
                self.message_sort.1.toggled()
            } else {
                SortDirection::Ascending
            },
        );
        self.rebuild_messages();
    }

    pub fn sort_conferences(&mut self, column: ConferenceColumn) {
        self.conference_sort = (
            column,
            if self.conference_sort.0 == column {
                self.conference_sort.1.toggled()
            } else {
                SortDirection::Ascending
            },
        );
        self.rebuild_conferences();
    }

    pub fn selected_position(&self) -> Option<usize> {
        self.position(self.selected_message?)
    }

    pub fn conference_position(&self) -> usize {
        self.conferences.iter().position(|row| row.number == self.selected_conference).unwrap_or(0)
    }

    pub fn navigate(&mut self, pane: Pane, direction: NavigateDirection) {
        match pane {
            Pane::Conferences if !self.conferences.is_empty() => {
                let position = step(self.conference_position(), direction, self.conferences.len());
                self.select_conference(self.conferences[position].number);
            }
            Pane::Messages if !self.messages.is_empty() => {
                let position = step(self.selected_position().unwrap_or(0), direction, self.messages.len());
                self.selected_message = Some(self.messages[position].index);
            }
            _ => {}
        }
    }

    pub fn selected_screen(&self) -> Res<TextScreen> {
        match (&self.package, self.selected_message) {
            (Some(package), Some(index)) => render_body(&package.get_message(index)?.text),
            _ => render_body(&[]),
        }
    }
}

pub fn step(current: usize, direction: NavigateDirection, len: usize) -> usize {
    let last = len.saturating_sub(1);
    match direction {
        NavigateDirection::Up => current.saturating_sub(1),
        NavigateDirection::Down => current.saturating_add(1).min(last),
        NavigateDirection::First => 0,
        NavigateDirection::Last => last,
        NavigateDirection::PageUp => current.saturating_sub(10),
        NavigateDirection::PageDown => current.saturating_add(10).min(last),
    }
}

pub fn render_body(data: &[u8]) -> Res<TextScreen> {
    render_with(data, &mut icy_parser_core::AnsiParser::new(), 80)
}

/// Columns of [`render_body_wide`]; longer lines still wrap.
pub const WIDE_COLUMNS: i32 = 256;

/// Renders a message without wrapping lines at 80 columns, for reading it as flowing text.
pub fn render_body_wide(data: &[u8]) -> Res<TextScreen> {
    render_with(data, &mut icy_parser_core::AnsiParser::new(), WIDE_COLUMNS)
}

/// Renders a bulletin, news or new files screen. These stop at the DOS end-of-file mark and may use
/// PCBoard `@X` colours.
pub fn render_file(data: &[u8]) -> Res<TextScreen> {
    render_file_page(data, 0)
}

/// Render only one page of a packet file to keep very large new-files lists manageable.
pub const FILE_PAGE_LINES: usize = 2048;

pub fn render_file_page(data: &[u8], page: usize) -> Res<TextScreen> {
    render_file_page_width(data, page, 80)
}

/// [`render_file_page`] without wrapping lines at 80 columns, like [`render_body_wide`].
pub fn render_file_page_wide(data: &[u8], page: usize) -> Res<TextScreen> {
    render_file_page_width(data, page, WIDE_COLUMNS)
}

fn render_file_page_width(data: &[u8], page: usize, width: i32) -> Res<TextScreen> {
    let data = data.split(|byte| *byte == 0x1A).next().unwrap_or_default();
    let offset = page.saturating_mul(FILE_PAGE_LINES);
    let page_data: Vec<u8> = data
        .split_inclusive(|byte| *byte == b'\n')
        .skip(offset)
        .take(FILE_PAGE_LINES)
        .flat_map(|line| line.iter().copied())
        .collect();
    let data = page_data.as_slice();
    let pcboard = data
        .windows(4)
        .any(|code| code[0] == b'@' && code[1].eq_ignore_ascii_case(&b'x') && code[2].is_ascii_hexdigit() && code[3].is_ascii_hexdigit());
    if pcboard {
        render_with(data, &mut icy_parser_core::PcBoardParser::new(), width)
    } else {
        render_with(data, &mut icy_parser_core::AnsiParser::new(), width)
    }
}

fn render_with(data: &[u8], parser: &mut dyn icy_parser_core::CommandParser, width: i32) -> Res<TextScreen> {
    let mut normalized = Vec::with_capacity(data.len() + data.len() / 8);
    let mut previous = 0;
    for byte in data {
        if *byte == b'\n' && previous != b'\r' {
            normalized.push(b'\r');
        }
        normalized.push(*byte);
        previous = *byte;
    }
    let height = normalized.iter().filter(|byte| **byte == b'\n').count().max(24) + 1;
    let mut screen = TextScreen::new(Size::new(width, height as i32));
    screen.terminal_state_mut().is_terminal_buffer = false;
    icy_engine::load_with_parser(&mut screen, parser, &normalized, true, -1)?;
    screen.update_hyperlinks();
    screen.caret_mut().visible = false;
    Ok(screen)
}

/// Searches CP437 message text without ANSI commands or display-width wrapping.
pub fn body_contains(data: &[u8], needle: &str) -> bool {
    body_search_text(data).contains(needle)
}

/// Query-independent, lowercased text for the packet's background search cache.
pub fn body_search_text(data: &[u8]) -> String {
    use icy_parser_core::{AnsiParser, CommandParser, CommandSink, ErrorLevel, ParseError, TerminalCommand};

    struct Text {
        plain: String,
    }

    impl CommandSink for Text {
        fn print(&mut self, text: &[u8]) {
            self.plain.extend(text.iter().map(|&byte| crate::editor::cp437_char(byte)));
        }

        fn emit(&mut self, command: TerminalCommand) {
            match command {
                TerminalCommand::LineFeed => self.plain.push('\n'),
                TerminalCommand::Tab => self.plain.push('\t'),
                _ => {}
            }
        }

        fn report_error(&mut self, error: ParseError, level: ErrorLevel) {
            // Like the reader renderer, keep the parser's recovered text despite malformed formatting.
            match level {
                ErrorLevel::Error => log::error!("Message search parser error: {error:?}"),
                ErrorLevel::Warning => log::warn!("Message search parser warning: {error:?}"),
                ErrorLevel::Info => log::info!("Message search parser info: {error:?}"),
            }
        }
    }

    let mut text = Text {
        plain: String::with_capacity(data.len()),
    };
    AnsiParser::new().parse(data, &mut text);
    text.plain.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::{Position, Screen, TextPane};

    fn loaded() -> (crate::qwk::tests::TempDir, Reader) {
        let (dir, package) = crate::qwk::tests::load();
        let mut reader = Reader::default();
        reader.set_package(Arc::new(package));
        (dir, reader)
    }

    fn thread_reader(package: QwkPackage) -> Reader {
        let mut reader = Reader {
            view_mode: ViewMode::Threads,
            ..Default::default()
        };
        reader.set_package(Arc::new(package));
        reader
    }

    fn assert_same_thread_view(cached: &Reader, fresh: &Reader) {
        assert_eq!(cached.all_messages, fresh.all_messages);
        assert_eq!(cached.messages, fresh.messages);
        assert_eq!(cached.selected_message, fresh.selected_message);
        assert_eq!(cached.unread_count(), fresh.unread_count());
        assert_eq!(cached.positions, fresh.positions);
        assert_eq!(cached.all_positions, fresh.all_positions);
    }

    #[test]
    fn cached_threads_match_fresh_rows_with_collapsing_and_read_marks() {
        let (dir, package) = crate::qwk::tests::load();
        assert!(package.cached_threads().is_none(), "uncached mutable fixtures must build their own threads");
        let path = dir.path().join("TEST.QWK");
        let cache = crate::qwk::ExtractionCache::new(dir.path().join("packet-extractions"), 30);
        for _ in 0..2 {
            let package = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
            let expected = threading::build_threads(&package.infos.iter().collect::<Vec<_>>());
            assert_eq!(package.cached_threads().unwrap(), expected);
            let mut cached = thread_reader(package);
            let mut fresh = thread_reader(QwkPackage::load_from_file(&path).unwrap());
            assert_same_thread_view(&cached, &fresh);
            for reader in [&mut cached, &mut fresh] {
                reader.set_read_marks([0, 2]);
                reader.select_message(3);
                reader.set_collapsed(2, true);
                reader.rebuild_messages();
            }
            assert_same_thread_view(&cached, &fresh);
            assert_eq!(cached.selected_message, Some(2));
            assert_eq!(cached.unread_count(), 2, "hidden unread replies still count");
            assert_eq!(cached.unread_replies(2), 1);
            assert_eq!(cached.next_unread(Some(2)), Some(3));
            assert!(!cached.messages.iter().any(|row| row.index == 3));
            for reader in [&mut cached, &mut fresh] {
                reader.select_message(3);
                reader.rebuild_messages();
            }
            assert_same_thread_view(&cached, &fresh);
            assert!(!cached.is_collapsed(2), "selecting a hidden reply still unfolds its cached thread");
        }
    }

    #[test]
    fn cached_threads_do_not_bypass_folder_or_search_filters() {
        let (dir, _) = crate::qwk::tests::load();
        let path = dir.path().join("TEST.QWK");
        let cache = crate::qwk::ExtractionCache::new(dir.path().join("packet-extractions"), 30);
        let cached_package = QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        for case in 0..8 {
            let mut cached = thread_reader(cached_package.clone());
            let mut fresh = thread_reader(QwkPackage::load_from_file(&path).unwrap());
            for reader in [&mut cached, &mut fresh] {
                reader.set_read_marks([0, 2]);
                reader.set_stars([1, 3]);
                match case {
                    0 => reader.selected_conference = Some(2),
                    1 => reader.personal = Some("nobody".into()),
                    2 => reader.unread_only = true,
                    3 => reader.starred_only = true,
                    4 => reader.filter = "coffee".into(),
                    5 => {
                        reader.filter = "line 4".into();
                        reader.search_fields = SearchFields {
                            from: false,
                            to: false,
                            subject: false,
                            text: true,
                        };
                        reader.set_body_matches("line 4".into(), HashSet::from([2]));
                    }
                    6 => {
                        reader.selected_conference = Some(2);
                        reader.personal = Some("all".into());
                        reader.unread_only = true;
                        reader.starred_only = true;
                        reader.filter = "dave".into();
                    }
                    _ => reader.selected_conference = Some(0),
                }
                reader.rebuild_messages();
            }
            assert_same_thread_view(&cached, &fresh);
            assert!(cached.messages.len() < cached_package.infos.len(), "case {case} must restrict the cached tree");
            cached.selected_conference = None;
            cached.personal = None;
            cached.unread_only = false;
            cached.starred_only = false;
            cached.filter.clear();
            cached.rebuild_messages();
            assert_eq!(cached.all_messages, cached_package.cached_threads().unwrap());
            assert_eq!(cached.unread_count(), 2, "returning to all messages preserves read marks");
        }
    }

    #[test]
    fn body_search_decodes_cp437_and_ignores_ansi_formatting() {
        assert!(body_contains(b"Hello \x1b[31mWOR\x1b[0mLD\nGr\x81\xE1e", "hello world"));
        assert!(body_contains(b"Gr\x81\xE1e", "grüße"));
        assert!(!body_contains(b"\x1b[31mhello\x1b[0m", "31m"));
        assert!(!body_contains(b"hello", "missing"));
        assert!(body_contains(format!("{}needle", "x".repeat(79)).as_bytes(), "xneedle"));
        assert!(body_contains(b"one\r\ntwo", "one\ntwo"));
        assert!(!body_contains(b"\x1b]0;hidden title\x07hello", "hidden"));
        assert!(body_contains(b"hello |07world", "|07world"), "literal pipe text is searchable");
    }

    #[test]
    fn body_search_continues_after_malformed_ansi_colors() {
        for sequence in ["\x1b[48m", "\x1b[38m", "\x1b[48;5m", "\x1b[48;2;255m", "\x1b[999m"] {
            let body = format!("before {sequence}NEEDLE\x1b[0m after");
            assert!(body_contains(body.as_bytes(), "before needle after"), "{sequence:?}");
            assert!(!body_contains(body.as_bytes(), "missing"), "{sequence:?}");
        }
    }

    #[test]
    fn body_results_combine_with_headers_and_existing_filters() {
        let (_dir, mut reader) = loaded();
        reader.filter = "coffee".into();
        reader.set_body_matches("coffee".into(), HashSet::from([2, 3]));
        assert_eq!(reader.messages.len(), 4, "header and body hits are combined");
        reader.select_conference(Some(2));
        assert_eq!(reader.messages.iter().map(|row| row.index).collect::<Vec<_>>(), [2, 3]);
        reader.set_read(2, true);
        reader.unread_only = true;
        reader.rebuild_messages();
        assert_eq!(reader.selected_message, Some(3));
        reader.view_mode = ViewMode::Threads;
        reader.rebuild_messages();
        assert_eq!(reader.messages.len(), 1);
        reader.personal = Some("nobody".into());
        reader.rebuild_messages();
        assert!(reader.messages.is_empty());
        reader.personal = None;
        reader.filter = "missing".into();
        reader.rebuild_messages();
        assert!(reader.messages.is_empty(), "body hits only apply to their own query");
        let package = reader.package.clone().unwrap();
        reader.set_package(package);
        reader.filter = "coffee".into();
        reader.rebuild_messages();
        assert_eq!(reader.messages.len(), 2, "changing packets clears body hits");
    }

    #[test]
    fn package_selection_filter_and_sort_keep_valid_selection() {
        let (_dir, mut reader) = loaded();
        assert_eq!(reader.conferences.len(), 3);
        assert_eq!(reader.messages.len(), 4);
        assert_eq!(reader.selected_message, Some(0));
        reader.select_conference(Some(2));
        assert_eq!(reader.messages.len(), 2);
        assert_eq!(reader.selected_message, Some(2));
        reader.filter = "DAVE".into();
        reader.rebuild_messages();
        assert_eq!(reader.selected_message, Some(3));
        reader.filter = "not present".into();
        reader.rebuild_messages();
        assert!(reader.messages.is_empty());
        assert!(reader.selected_message.is_none());
        reader.filter.clear();
        reader.select_conference(None);
        reader.sort_messages(MessageColumn::From);
        assert_eq!(reader.messages.iter().map(|row| row.index).collect::<Vec<_>>(), [0, 1, 2, 3]);
        reader.sort_messages(MessageColumn::From);
        assert_eq!(reader.messages.iter().map(|row| row.index).collect::<Vec<_>>(), [3, 2, 1, 0]);
        assert_eq!(reader.selected_message, Some(0));
        reader.sort_conferences(ConferenceColumn::Count);
        assert_eq!(reader.conferences[0].number, None);
    }

    #[test]
    fn personal_and_unread_filters_combine_with_conferences() {
        let (_dir, mut reader) = loaded();
        Arc::make_mut(reader.package.as_mut().unwrap()).infos[2].to = "Reader ".into();
        reader.personal = Some("READER".into());
        reader.rebuild_messages();
        assert_eq!(reader.messages.iter().map(|row| row.index).collect::<Vec<_>>(), [2]);
        reader.personal = None;
        reader.set_read_marks([0, 2]);
        reader.unread_only = true;
        reader.rebuild_messages();
        assert_eq!(reader.messages.iter().map(|row| row.index).collect::<Vec<_>>(), [1, 3]);
        reader.select_conference(Some(2));
        assert_eq!(reader.messages.iter().map(|row| row.index).collect::<Vec<_>>(), [3]);
    }

    #[test]
    fn threads_are_marked_and_folded_as_a_whole() {
        let (_dir, mut reader) = loaded();
        reader.view_mode = ViewMode::Threads;
        reader.rebuild_messages();
        let roots: Vec<usize> = reader.messages.iter().filter(|row| row.depth == 0).map(|row| row.index).collect();
        let (root, reply) = reader
            .messages
            .iter()
            .find(|row| row.depth > 0)
            .map(|reply| {
                let root = reader.thread_of(reply.index)[0];
                (root, reply.index)
            })
            .unwrap();
        assert_eq!(reader.thread_of(reply), reader.thread_of(root), "a reply belongs to its root's thread");
        assert!(reader.thread_of(root).contains(&reply));

        reader.select_message(reply);
        assert!(reader.set_all_collapsed(true));
        assert_eq!(reader.selected_message, Some(root), "the selection moves up to the collapsed thread");
        assert_eq!(reader.messages.len(), roots.len(), "only the threads' first messages stay");
        reader.set_all_collapsed(false);
        assert!(reader.messages.iter().any(|row| row.index == reply));

        reader.view_mode = ViewMode::List;
        reader.rebuild_messages();
        assert_eq!(reader.thread_of(reply), [reply], "the flat list has no threads");
    }

    #[test]
    fn search_can_be_limited_to_some_fields() {
        let (_dir, mut reader) = loaded();
        let package = Arc::make_mut(reader.package.as_mut().unwrap());
        package.infos[0].from = "Needle".into();
        package.infos[1].to = "needle".into();
        package.infos[2].subject = "A needle".into();
        reader.filter = "needle".into();
        reader.set_body_matches("needle".into(), HashSet::from([3]));
        let found = |reader: &mut Reader| {
            reader.rebuild_messages();
            let mut rows: Vec<_> = reader.messages.iter().map(|row| row.index).collect();
            rows.sort_unstable();
            rows
        };
        assert_eq!(found(&mut reader), [0, 1, 2, 3]);
        reader.search_fields = SearchFields {
            from: false,
            to: false,
            subject: true,
            text: false,
        };
        assert_eq!(found(&mut reader), [2]);
        reader.search_fields.from = true;
        reader.search_fields.text = true;
        assert_eq!(found(&mut reader), [0, 2, 3]);
    }

    #[test]
    fn starred_filter_keeps_unstarred_messages_until_rebuilt() {
        let (_dir, mut reader) = loaded();
        reader.starred_only = true;
        reader.set_stars([1, 3]);
        assert_eq!(reader.messages.iter().map(|row| row.index).collect::<Vec<_>>(), [1, 3]);
        assert!(reader.set_starred(3, false));
        assert!(!reader.set_starred(3, false));
        assert_eq!(reader.messages.len(), 2, "unstarring in place allows undo");
        reader.rebuild_messages();
        assert_eq!(reader.messages.iter().map(|row| row.index).collect::<Vec<_>>(), [1]);
        assert!(reader.is_starred(1) && !reader.is_starred(3));
    }

    #[test]
    fn thread_order_is_not_overridden_by_column_sort() {
        let (_dir, mut reader) = loaded();
        reader.view_mode = ViewMode::Threads;
        reader.rebuild_messages();
        let before: Vec<_> = reader.messages.iter().map(|row| (row.index, row.depth)).collect();
        assert!(before.iter().any(|(_, depth)| *depth > 0));
        reader.sort_messages(MessageColumn::From);
        assert_eq!(before, reader.messages.iter().map(|row| (row.index, row.depth)).collect::<Vec<_>>());
        assert_eq!(reader.messages.len(), 4);
    }

    #[test]
    fn collapsed_threads_hide_replies_but_keep_them_reachable() {
        let (_dir, mut reader) = loaded();
        reader.view_mode = ViewMode::Threads;
        reader.rebuild_messages();
        let visible = |reader: &Reader| reader.messages.iter().map(|row| row.index).collect::<Vec<_>>();
        assert_eq!(visible(&reader), [2, 3, 0, 1]);

        reader.select_message(3);
        assert!(reader.set_collapsed(2, true));
        assert_eq!(visible(&reader), [2, 0, 1]);
        assert_eq!(reader.selected_message, Some(2), "a hidden selection moves to its thread");
        assert!(reader.contains(3));
        assert_eq!(reader.all_messages().len(), 4);
        assert_eq!(reader.unread_count(), 4, "hidden replies still count");
        assert_eq!(reader.unread_replies(2), 1);
        assert!(!reader.set_collapsed(3, true), "leaves cannot collapse");

        assert_eq!(reader.next_unread(Some(2)), Some(3), "unread replies are found inside collapsed threads");
        reader.select_message(3);
        assert!(!reader.is_collapsed(2), "selecting a hidden reply expands its thread");
        assert_eq!(visible(&reader), [2, 3, 0, 1]);

        // Left: to parent, then collapse; Right: expand, then to the first reply.
        assert!(reader.collapse_or_parent());
        assert_eq!(reader.selected_message, Some(2));
        assert!(reader.collapse_or_parent());
        assert!(reader.is_collapsed(2));
        assert!(!reader.collapse_or_parent(), "a collapsed root has nowhere to go");
        assert!(reader.expand_or_child());
        assert!(!reader.is_collapsed(2));
        assert!(reader.expand_or_child());
        assert_eq!(reader.selected_message, Some(3));
        assert!(!reader.expand_or_child());

        reader.set_collapsed(0, true);
        reader.select_message(1);
        reader.set_collapsed(0, true);
        reader.filter = "coffee".into();
        reader.rebuild_messages();
        assert_eq!(visible(&reader), [0], "collapsed state survives filtering");
        reader.view_mode = ViewMode::List;
        reader.rebuild_messages();
        assert_eq!(visible(&reader), [0, 1], "the flat list ignores collapsed threads");
    }

    #[test]
    fn conference_zero_is_distinct_from_all() {
        let (_dir, mut reader) = loaded();
        Arc::make_mut(reader.package.as_mut().unwrap()).infos[0].conference = 0;
        reader.rebuild_conferences();
        reader.select_conference(Some(0));
        assert_eq!(reader.messages.len(), 1);
        reader.select_conference(None);
        assert_eq!(reader.messages.len(), 4);
    }

    #[test]
    fn navigation_and_focus_match_the_original() {
        let (_dir, mut reader) = loaded();
        reader.navigate(Pane::Messages, NavigateDirection::Last);
        assert_eq!(reader.selected_message, Some(3));
        reader.navigate(Pane::Messages, NavigateDirection::PageUp);
        assert_eq!(reader.selected_message, Some(0));
        reader.navigate(Pane::Conferences, NavigateDirection::Last);
        assert_eq!(reader.selected_conference, Some(2));
        assert_eq!(reader.selected_message, Some(2));
        assert_eq!(Pane::Conferences.cycle(true), Pane::Messages);
        assert_eq!(Pane::Messages.cycle(true), Pane::Content);
        assert_eq!(Pane::Content.cycle(true), Pane::Conferences);
        assert_eq!(Pane::Conferences.cycle(false), Pane::Content);
        assert_eq!(step(0, NavigateDirection::Down, 0), 0);
    }

    #[test]
    fn ansi_and_line_endings_render_without_a_caret() {
        let screen = render_body(b"\x1b[31mRED\nNEXT\r\nLAST").unwrap();
        assert_eq!(screen.char_at(Position::new(0, 0)).ch, 'R');
        assert_eq!(screen.char_at(Position::new(0, 1)).ch, 'N');
        assert_eq!(screen.char_at(Position::new(0, 2)).ch, 'L');
        assert_eq!(screen.char_at(Position::new(0, 0)).attribute.foreground(), 4);
        assert!(!screen.caret().visible);
    }

    #[test]
    fn full_width_ansi_rows_do_not_add_blank_lines() {
        let mut body = vec![b'A'; 40];
        body.extend_from_slice(b"\n\x1b[A\x1b[40C");
        body.extend(std::iter::repeat_n(b'B', 40));
        body.extend_from_slice(b"\nNEXT");

        let screen = render_body(&body).unwrap();
        assert!((0..40).all(|column| screen.char_at(Position::new(column, 0)).ch == 'A'));
        assert!((40..80).all(|column| screen.char_at(Position::new(column, 0)).ch == 'B'));
        assert_eq!(screen.char_at(Position::new(0, 1)).ch, 'N');
        assert_eq!(screen.char_at(Position::new(0, 2)).ch, ' ');
    }

    #[test]
    fn packet_files_translate_pcboard_colors_and_stop_at_end_of_file() {
        let screen = render_file(b"@X0EDEMO@X07 me@home.net\r\n\x1aSAUCE").unwrap();
        assert_eq!(screen.char_at(Position::new(0, 0)).ch, 'D');
        assert_eq!(screen.char_at(Position::new(0, 0)).attribute.foreground(), 14);
        assert_eq!(screen.char_at(Position::new(5, 0)).attribute.foreground(), 7);
        assert_eq!(screen.char_at(Position::new(0, 1)).ch, ' ', "text after ^Z is not shown");
        let plain = render_file(b"mail me@home.net or you@there.org").unwrap();
        let line: String = (0..33).map(|x| plain.char_at(Position::new(x, 0)).ch).collect();
        assert_eq!(line, "mail me@home.net or you@there.org", "without @X codes, @ stays literal");
    }

    #[test]
    fn packet_file_pages_render_only_the_selected_lines() {
        let mut data = b"first\n".to_vec();
        data.extend(b"filler\n".repeat(FILE_PAGE_LINES - 1));
        data.extend_from_slice(b"second\n");
        let first = render_file_page(&data, 0).unwrap();
        let second = render_file_page(&data, 1).unwrap();
        assert_eq!(first.char_at(Position::new(0, 0)).ch, 'f');
        assert_eq!(second.char_at(Position::new(0, 0)).ch, 's');
        assert_eq!(second.height(), 25);
    }
}
