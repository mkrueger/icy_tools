use iced::{
    Element, Length,
    widget::{column, container},
};
use icy_engine::{Screen, formats::FileFormat};
use icy_engine_gui::ui::{StateResult, button_row, dialog_area, primary_button, separator};
use icy_engine_gui::version_helper::replace_version_marker;
use icy_engine_gui::{MonitorSettings, Terminal, TerminalView, dialog_wrapper};
use icy_parser_core::MusicOption;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::VERSION;

// Include the about ANSI file at compile time
pub const ABOUT_ANSI: &[u8] = include_bytes!("../../../data/about.icy");

/// Messages for the about dialog
#[derive(Debug, Clone)]
pub enum AboutDialogMessage {
    /// Close the dialog
    Close,
    /// Open a URL link
    OpenLink(String),
}

#[dialog_wrapper(close_on_blur = true)]
pub struct AboutDialogState {
    terminal: Terminal,
}

impl AboutDialogState {
    pub fn new() -> Self {
        Self::with_ansi(ABOUT_ANSI)
    }

    pub fn with_ansi(ansi: &[u8]) -> Self {
        let mut screen;

        match FileFormat::IcyDraw.from_bytes(ansi, Some(icy_engine::formats::LoadData::new(None, Some(MusicOption::Off), None))) {
            Ok(mut loaded_screen) => {
                let build_date = option_env!("ICY_BUILD_DATE").unwrap_or("-").to_string();
                replace_version_marker(&mut loaded_screen.buffer, &VERSION, Some(build_date));
                screen = loaded_screen;
            }
            Err(e) => {
                panic!("Failed to load about ANSI: {}", e);
            }
        }
        screen.caret.visible = false;

        let edit_screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(screen)));
        let terminal = Terminal::new(edit_screen.clone());

        Self { terminal }
    }

    pub fn handle_message(&mut self, message: AboutDialogMessage) -> StateResult<()> {
        match message {
            AboutDialogMessage::Close => StateResult::Close,
            AboutDialogMessage::OpenLink(_) => {
                // Link opening is handled by the app via on_cancel callback
                // We just signal to close after opening the link
                StateResult::None
            }
        }
    }

    pub fn view<'a, Message: Clone + 'static>(&'a self, on_message: impl Fn(AboutDialogMessage) -> Message + 'a + Clone) -> Element<'a, Message> {
        let mut settings = MonitorSettings::neutral();
        settings.use_integer_scaling = false;

        let on_msg = on_message.clone();
        let on_msg_close = on_message.clone();
        let terminal_view = TerminalView::show_with_effects(&self.terminal, settings).map(move |terminal_msg| match terminal_msg {
            icy_engine_gui::Message::OpenLink(url) => on_msg(AboutDialogMessage::OpenLink(url)),
            _ => on_msg_close(AboutDialogMessage::Close),
        });

        let ok_button = primary_button(format!("{}", icy_engine_gui::ButtonType::Ok), Some(on_message(AboutDialogMessage::Close)));

        let buttons = button_row(vec![ok_button.into()]);

        column![container(terminal_view).height(Length::Fill), separator(), dialog_area(buttons),]
            .spacing(0)
            .into()
    }
}

impl Default for AboutDialogState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Builder function for about dialog
// ============================================================================

/// Create an about dialog for use with DialogStack
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
    AboutDialogWrapper::new(AboutDialogState::new(), on_message, extract_message)
}
