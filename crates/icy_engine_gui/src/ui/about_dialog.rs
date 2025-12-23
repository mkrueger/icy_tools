//! Shared About Dialog for icy_term, icy_view, and icy_draw.
//!
//! This dialog displays an about screen loaded from an .icy file and supports
//! clickable hyperlinks.

use iced::{
    widget::{column, container},
    Element, Length,
};
use icy_engine::{formats::FileFormat, Screen};
use icy_parser_core::MusicOption;
use parking_lot::Mutex;
use semver::Version;
use std::sync::Arc;

use crate::{
    dialog_wrapper,
    ui::{button_row, dialog_area, primary_button, separator, StateResult},
    version_helper::replace_version_marker,
    MonitorSettings, Terminal, TerminalView,
};

/// Messages for the about dialog
#[derive(Debug, Clone)]
pub enum AboutDialogMessage {
    /// No-op message (for ignored events)
    None,
    /// Close the dialog
    Close,
    /// Open a URL link
    OpenLink(String),
}

/// State for the about dialog
#[dialog_wrapper(style = Fullscreen)]
pub struct AboutDialogState {
    terminal: Terminal,
}

impl AboutDialogState {
    /// Create a new about dialog with the given ANSI data and version string.
    ///
    /// # Arguments
    /// * `ansi_data` - Raw bytes of the .icy file to display
    /// * `version` - Version to replace in the ANSI (replaces %VERSION% marker)
    /// * `build_date` - Optional build date to display
    pub fn new(ansi_data: &[u8], version: &Version, build_date: Option<String>) -> Self {
        let mut screen;

        match FileFormat::IcyDraw.from_bytes(ansi_data, Some(icy_engine::formats::LoadData::new(Some(MusicOption::Off), None))) {
            Ok(mut loaded_doc) => {
                replace_version_marker(&mut loaded_doc.screen.buffer, version, build_date);
                screen = loaded_doc.screen;
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

    /// Handle a message from the about dialog
    pub fn handle_message(&mut self, message: AboutDialogMessage) -> StateResult<()> {
        match message {
            AboutDialogMessage::None => StateResult::None,
            AboutDialogMessage::Close => StateResult::Close,
            AboutDialogMessage::OpenLink(_) => {
                // Link opening is handled by the app via the message
                // We just continue (don't close)
                StateResult::None
            }
        }
    }

    /// Render the about dialog view
    pub fn view<'a, Message: Clone + 'static>(&'a self, on_message: impl Fn(AboutDialogMessage) -> Message + 'a + Clone) -> Element<'a, Message> {
        use iced::mouse;
        use std::sync::Arc;

        let mut settings = MonitorSettings::neutral();
        settings.use_integer_scaling = false;
        let settings = Arc::new(settings);

        let screen = self.terminal.screen.clone();
        let terminal = &self.terminal;
        let on_msg = on_message.clone();
        let terminal_view = TerminalView::show_with_effects(&self.terminal, settings, None).map(move |terminal_msg| {
            match terminal_msg {
                crate::TerminalMessage::Press(evt) => {
                    // Check if clicking on a hyperlink
                    if let Some(screen_guard) = screen.try_lock() {
                        if let Some(url) = evt.get_hyperlink(&**screen_guard) {
                            return on_msg(AboutDialogMessage::OpenLink(url));
                        }
                    }
                    // Not on a link - do nothing (don't close)
                    on_msg(AboutDialogMessage::None)
                }
                crate::TerminalMessage::Move(evt) => {
                    // Update cursor for hyperlink hover
                    if let Some(screen_guard) = screen.try_lock() {
                        if let Some(cell) = evt.text_position {
                            let mut is_over_link = false;
                            for hyperlink in screen_guard.hyperlinks() {
                                if screen_guard.is_position_in_range(cell, hyperlink.position, hyperlink.length) {
                                    is_over_link = true;
                                    break;
                                }
                            }
                            let cursor = if is_over_link {
                                Some(mouse::Interaction::Pointer)
                            } else {
                                Some(mouse::Interaction::default())
                            };
                            *terminal.cursor_icon.write() = cursor;
                        }
                    }
                    on_msg(AboutDialogMessage::None)
                }
                _ => on_msg(AboutDialogMessage::None),
            }
        });

        let ok_button = primary_button(format!("{}", crate::ui::ButtonType::Ok), Some(on_message(AboutDialogMessage::Close)));

        let buttons = button_row(vec![ok_button.into()]);

        column![container(terminal_view).height(Length::Fill), separator(), dialog_area(buttons),]
            .spacing(0)
            .into()
    }
}

/// Create an about dialog for use with DialogStack
///
/// # Arguments
/// * `ansi_data` - Raw bytes of the .icy file to display
/// * `version` - Version to replace in the ANSI
/// * `build_date` - Optional build date
/// * `on_message` - Function to wrap AboutDialogMessage into app message
/// * `extract_message` - Function to extract AboutDialogMessage from app message
///
/// # Example
/// ```ignore
/// dialog_stack.push(about_dialog(
///     include_bytes!("data/about.icy"),
///     &VERSION,
///     option_env!("ICY_BUILD_DATE").map(String::from),
///     Message::AboutDialog,
///     |msg| match msg { Message::AboutDialog(m) => Some(m), _ => None },
/// ));
/// ```
pub fn about_dialog<M, F, E>(ansi_data: &[u8], version: &Version, build_date: Option<String>, on_message: F, extract_message: E) -> AboutDialogWrapper<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(AboutDialogMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&AboutDialogMessage> + Clone + 'static,
{
    AboutDialogWrapper::new(AboutDialogState::new(ansi_data, version, build_date), on_message, extract_message)
}
