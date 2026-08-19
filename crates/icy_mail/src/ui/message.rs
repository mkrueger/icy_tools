use crate::qwk::QwkPackage;
use icy_engine_gui::TerminalMessage;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigateDirection {
    Up,
    Down,
    First,
    Last,
    PageUp,
    PageDown,
}

/// Which list the keyboard is driving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pane {
    Conferences,
    #[default]
    Messages,
    Content,
}

/// Flat list vs. reply threads - the message list renders one or the other.
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

    /// Arrow shown in the column header.
    pub fn arrow(self) -> &'static str {
        match self {
            Self::Ascending => "\u{25B2}",
            Self::Descending => "\u{25BC}",
        }
    }
}

#[derive(Clone)]
pub enum Message {
    _QuitIcyMail,
    BufferUpdated,
    OpenPackage,
    PackageSelected(PathBuf),
    _LoadingProgress(f32, Option<String>),
    PackageLoaded(Arc<QwkPackage>),
    PackageLoadError(String),

    SelectConference(u16),
    SelectMessage(usize),
    SetViewMode(ViewMode),
    NewMessage,
    Refresh,

    FilterChanged(String),
    ClearFilter,
    SortMessagesBy(MessageColumn),
    SortConferencesBy(ConferenceColumn),

    Navigate(NavigateDirection),
    FocusPane(Pane),
    /// Tab / Shift+Tab between the panes.
    CyclePane {
        forward: bool,
    },
    /// Reports a list's scroll position so navigation can keep the selection in view.
    ListScrolled {
        pane: Pane,
        offset_y: f32,
        height: f32,
    },

    TerminalMessage(TerminalMessage),
    Noop,
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::_QuitIcyMail => write!(f, "QuitIcyMail"),
            Message::BufferUpdated => write!(f, "BufferUpdated"),
            Message::OpenPackage => write!(f, "OpenPackage"),
            Message::PackageSelected(path) => write!(f, "PackageSelected({path:?})"),
            Message::_LoadingProgress(p, msg) => write!(f, "LoadingProgress({p}, {msg:?})"),
            Message::PackageLoaded(_) => write!(f, "PackageLoaded(<package>)"),
            Message::PackageLoadError(e) => write!(f, "PackageLoadError({e})"),
            Message::SelectConference(c) => write!(f, "SelectConference({c})"),
            Message::SelectMessage(m) => write!(f, "SelectMessage({m})"),
            Message::SetViewMode(mode) => write!(f, "SetViewMode({mode:?})"),
            Message::NewMessage => write!(f, "NewMessage"),
            Message::Refresh => write!(f, "Refresh"),
            Message::FilterChanged(s) => write!(f, "FilterChanged({s})"),
            Message::ClearFilter => write!(f, "ClearFilter"),
            Message::SortMessagesBy(c) => write!(f, "SortMessagesBy({c:?})"),
            Message::SortConferencesBy(c) => write!(f, "SortConferencesBy({c:?})"),
            Message::Navigate(dir) => write!(f, "Navigate({dir:?})"),
            Message::FocusPane(pane) => write!(f, "FocusPane({pane:?})"),
            Message::CyclePane { forward } => write!(f, "CyclePane({forward})"),
            Message::ListScrolled { pane, offset_y, height } => write!(f, "ListScrolled({pane:?}, {offset_y}, {height})"),
            Message::TerminalMessage(msg) => write!(f, "TerminalMessage({msg:?})"),
            Message::Noop => write!(f, "Noop"),
        }
    }
}
