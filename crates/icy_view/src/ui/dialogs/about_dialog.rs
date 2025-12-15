//! About dialog for icy_view - uses the shared dialog from icy_engine_gui

pub use icy_engine_gui::ui::{AboutDialogMessage, AboutDialogWrapper};

use crate::VERSION;

// Include the about ANSI file at compile time
pub const ABOUT_ANSI: &[u8] = include_bytes!("../../../data/about.icy");

/// Create an about dialog for icy_view
///
/// # Example
/// ```ignore
/// dialog_stack.push(about_dialog(
///     Message::AboutDialog,
///     |msg| match msg { Message::AboutDialog(m) => Some(m), _ => None },
/// ));
/// ```
pub fn about_dialog<M, F, E>(on_message: F, extract_message: E) -> AboutDialogWrapper<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(AboutDialogMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&AboutDialogMessage> + Clone + 'static,
{
    let build_date = option_env!("ICY_BUILD_DATE").map(String::from);
    icy_engine_gui::ui::about_dialog(ABOUT_ANSI, &VERSION, build_date, on_message, extract_message)
}
