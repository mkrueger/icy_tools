use std::{cmp::Ordering, sync::Arc};

use icy_engine::{EditableScreen, Size, TextScreen};

use crate::{
    qwk::QwkPackage,
    threading::{self, Row},
    Res,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

pub struct Reader {
    pub package: Option<Arc<QwkPackage>>,
    pub selected_conference: Option<u16>,
    pub selected_message: Option<usize>,
    pub filter: String,
    pub view_mode: ViewMode,
    pub message_sort: (MessageColumn, SortDirection),
    pub conference_sort: (ConferenceColumn, SortDirection),
    pub conferences: Vec<ConferenceRow>,
    pub messages: Vec<Row>,
}

impl Default for Reader {
    fn default() -> Self {
        Self {
            package: None,
            selected_conference: None,
            selected_message: None,
            filter: String::new(),
            view_mode: ViewMode::List,
            message_sort: (MessageColumn::Date, SortDirection::Ascending),
            conference_sort: (ConferenceColumn::Area, SortDirection::Ascending),
            conferences: Vec::new(),
            messages: Vec::new(),
        }
    }
}

impl Reader {
    pub fn set_package(&mut self, package: Arc<QwkPackage>) {
        self.package = Some(package);
        self.selected_conference = None;
        self.selected_message = None;
        self.filter.clear();
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
                name: "All Conferences".into(),
                count: package.message_count(),
            },
        );
    }

    pub fn rebuild_messages(&mut self) {
        let Some(package) = &self.package else {
            self.messages.clear();
            self.selected_message = None;
            return;
        };
        let needle = self.filter.trim().to_lowercase();
        let mut infos: Vec<_> = package
            .infos
            .iter()
            .filter(|info| {
                self.selected_conference.is_none_or(|number| info.conference == number)
                    && (needle.is_empty() || [&info.from, &info.to, &info.subject].iter().any(|value| value.to_lowercase().contains(&needle)))
            })
            .collect();
        self.messages = match self.view_mode {
            ViewMode::Threads => threading::build_threads(&infos),
            ViewMode::List => {
                let (column, direction) = self.message_sort;
                infos.sort_by(|left, right| {
                    direction.apply(match column {
                        MessageColumn::From => left.from.to_lowercase().cmp(&right.from.to_lowercase()),
                        MessageColumn::Date => left.date.cmp(&right.date).then(left.number.cmp(&right.number)),
                        MessageColumn::Subject => left.subject.to_lowercase().cmp(&right.subject.to_lowercase()),
                        MessageColumn::Lines => left.lines.cmp(&right.lines),
                    })
                });
                infos
                    .iter()
                    .map(|info| Row {
                        index: info.index,
                        depth: 0,
                        has_children: false,
                    })
                    .collect()
            }
        };
        if self.selected_position().is_none() {
            self.selected_message = self.messages.first().map(|row| row.index);
        }
    }

    pub fn select_conference(&mut self, conference: Option<u16>) {
        self.selected_conference = conference;
        self.selected_message = None;
        self.rebuild_messages();
    }

    pub fn select_message(&mut self, index: usize) {
        if self.messages.iter().any(|row| row.index == index) {
            self.selected_message = Some(index);
        }
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
        self.messages.iter().position(|row| Some(row.index) == self.selected_message)
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
    let mut screen = TextScreen::new(Size::new(80, height as i32));
    screen.terminal_state_mut().is_terminal_buffer = false;
    icy_engine::load_with_parser(&mut screen, &mut icy_parser_core::AnsiParser::new(), &normalized, true, -1)?;
    screen.caret_mut().visible = false;
    Ok(screen)
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
}
