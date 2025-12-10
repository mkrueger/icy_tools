//! Help dialog for icy_term

use i18n_embed_fl::fl;
use icy_engine_gui::commands::CommandDef;
use icy_engine_gui::ui::{HelpDialogMessage, HelpDialogState, HelpDialogWrapper};

use crate::LANGUAGE_LOADER;
use crate::commands::cmd;

/// Get all commands for the help dialog
fn get_help_commands() -> Vec<CommandDef> {
    vec![
        // Connection commands
        cmd::CONNECTION_DIALING_DIRECTORY.clone(),
        cmd::CONNECTION_SERIAL.clone(),
        cmd::CONNECTION_HANGUP.clone(),
        cmd::APP_QUIT.clone(),
        // Login commands
        cmd::LOGIN_SEND_ALL.clone(),
        cmd::LOGIN_SEND_USER.clone(),
        cmd::LOGIN_SEND_PASSWORD.clone(),
        // Transfer commands
        cmd::TRANSFER_UPLOAD.clone(),
        cmd::TRANSFER_DOWNLOAD.clone(),
        // Window commands
        cmd::WINDOW_CLOSE.clone(),
        cmd::WINDOW_NEW.clone(),
        cmd::VIEW_FULLSCREEN.clone(),
        // Terminal commands
        cmd::TERMINAL_CLEAR.clone(),
        cmd::TERMINAL_SCROLLBACK.clone(),
        cmd::CAPTURE_EXPORT.clone(),
        cmd::CAPTURE_START.clone(),
        // Tools commands
        cmd::SCRIPT_RUN.clone(),
        cmd::TERMINAL_FIND.clone(),
        cmd::APP_SETTINGS.clone(),
        cmd::APP_ABOUT.clone(),
        cmd::HELP_SHOW.clone(),
        // Edit commands
        cmd::EDIT_COPY.clone(),
        cmd::EDIT_PASTE.clone(),
        // Zoom commands
        cmd::VIEW_ZOOM_IN.clone(),
        cmd::VIEW_ZOOM_OUT.clone(),
        cmd::VIEW_ZOOM_RESET.clone(),
        cmd::VIEW_ZOOM_FIT.clone(),
    ]
}

/// Create a help dialog for use with DialogStack
///
/// # Example
/// ```ignore
/// dialog_stack.push(help_dialog(
///     Message::HelpDialog,
///     |msg| match msg { Message::HelpDialog(m) => Some(m), _ => None },
/// ));
/// ```
pub fn help_dialog<M, F, E>(on_message: F, extract_message: E) -> HelpDialogWrapper<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(HelpDialogMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&HelpDialogMessage> + Clone + 'static,
{
    let state = HelpDialogState::new(fl!(LANGUAGE_LOADER, "help-title"), fl!(LANGUAGE_LOADER, "help-subtitle"))
        .with_commands(get_help_commands())
        .with_category_translator(|category_key| {
            let translation_key = format!("cmd-category-{}", category_key);
            icy_engine_gui::LANGUAGE_LOADER.get(&translation_key)
        });

    HelpDialogWrapper::new(state, on_message, extract_message)
}
