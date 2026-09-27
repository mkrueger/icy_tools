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

#[cfg(test)]
mod tests {
    use icy_engine::{formats::FileFormat, TextPane};
    use icy_engine_gui::version_helper::replace_version_marker;

    use super::{ABOUT_ANSI, VERSION};

    #[test]
    fn about_document_displays_current_version() {
        let mut document = FileFormat::IcyDraw.from_bytes(ABOUT_ANSI, None).unwrap();
        replace_version_marker(&mut document.screen.buffer, &VERSION, None);
        let buffer = &document.screen.buffer;
        let text = (0..buffer.height())
            .map(|y| (0..buffer.width()).map(|x| buffer.char_at((x, y).into()).ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");

        let expected_version = format!("v{}", *VERSION);
        assert!(
            text.contains(&expected_version),
            "about document must display {expected_version}; contents:\n{text}"
        );
    }
}
