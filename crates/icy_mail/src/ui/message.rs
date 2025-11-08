use crate::qwk::QwkPackage;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum NavigateDirection {
    Up,
    Down,
    First,
    Last,
    PageUp,
    PageDown,
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
    ToggleThreadView,
    NewMessage,
    Refresh,

    NavigateConference(NavigateDirection),
    FocusConferenceList,
    FocusMessageList,

    NavigateMessage(NavigateDirection),
    FocusMessageContent,

    Noop,
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::_QuitIcyMail => write!(f, "QuitIcyMail"),
            Message::BufferUpdated => write!(f, "BufferUpdated"),
            Message::OpenPackage => write!(f, "OpenPackage"),
            Message::PackageSelected(path) => write!(f, "PackageSelected({:?})", path),
            Message::_LoadingProgress(p, msg) => write!(f, "LoadingProgress({}, {:?})", p, msg),
            Message::PackageLoaded(_) => write!(f, "PackageLoaded(<package>)"),
            Message::PackageLoadError(e) => write!(f, "PackageLoadError({})", e),
            Message::SelectConference(c) => write!(f, "SelectConference({})", c),
            Message::SelectMessage(m) => write!(f, "SelectMessage({})", m),
            Message::ToggleThreadView => write!(f, "ToggleThreadView"),
            Message::NewMessage => write!(f, "NewMessage"),
            Message::Refresh => write!(f, "Refresh"),
            Message::Noop => write!(f, "Noop"),
            Message::NavigateConference(dir) => write!(f, "NavigateConference({:?})", dir),
            Message::FocusConferenceList => write!(f, "FocusConferenceList"),
            Message::FocusMessageList => write!(f, "FocusMessageList"),
            Message::NavigateMessage(dir) => write!(f, "NavigateMessage({:?})", dir),
            Message::FocusMessageContent => write!(f, "FocusMessageContent"),
        }
    }
}
