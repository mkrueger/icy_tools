//! Help dialog for icy_view

use i18n_embed_fl::fl;
use icy_engine_gui::commands::CommandDef;
use icy_engine_gui::ui::{HelpDialogMessage, HelpDialogState, HelpDialogWrapper};

use crate::LANGUAGE_LOADER;
use crate::commands::cmd;

/// Get all commands for the help dialog
fn get_help_commands() -> Vec<CommandDef> {
    vec![
        // Navigation commands
        cmd::FILE_OPEN.clone(),
        cmd::NAV_UP.clone(),
        cmd::NAV_BACK.clone(),
        cmd::NAV_FORWARD.clone(),
        cmd::DIALOG_FILTER.clone(),
        // Playback commands
        cmd::PLAYBACK_TOGGLE_SCROLL.clone(),
        cmd::PLAYBACK_SCROLL_SPEED.clone(),
        cmd::PLAYBACK_SCROLL_SPEED_BACK.clone(),
        cmd::PLAYBACK_BAUD_RATE.clone(),
        cmd::PLAYBACK_BAUD_RATE_BACK.clone(),
        cmd::PLAYBACK_BAUD_RATE_OFF.clone(),
        cmd::DIALOG_SAUCE.clone(),
        // View commands
        cmd::VIEW_ZOOM_IN.clone(),
        cmd::VIEW_ZOOM_OUT.clone(),
        cmd::VIEW_ZOOM_RESET.clone(),
        cmd::VIEW_ZOOM_FIT.clone(),
        // External/Tools commands
        cmd::DIALOG_EXPORT.clone(),
        cmd::EDIT_COPY.clone(),
        cmd::EXTERNAL_COMMAND_0.clone(),
        cmd::EXTERNAL_COMMAND_1.clone(),
        cmd::EXTERNAL_COMMAND_2.clone(),
        cmd::EXTERNAL_COMMAND_3.clone(),
        cmd::VIEW_FULLSCREEN.clone(),
        cmd::HELP_SHOW.clone(),
        cmd::HELP_ABOUT.clone(),
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