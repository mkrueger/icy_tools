//! Help dialog for icy_view

use i18n_embed_fl::fl;
use iced::Element;
use icy_engine_gui::commands::CommandDef;
use icy_engine_gui::ui::{HelpDialogConfig, help_dialog_content, modal};

use crate::LANGUAGE_LOADER;
use crate::commands::cmd;

pub struct HelpDialog;

impl HelpDialog {
    pub fn new() -> Self {
        Self
    }

    pub fn view<'a, Message: Clone + 'static>(&'a self, background: Element<'a, Message>, close_msg: Message) -> Element<'a, Message> {
        let config = self.build_config();
        let content = help_dialog_content(&config, close_msg.clone());
        modal(background, content, close_msg)
    }

    fn build_config(&self) -> HelpDialogConfig {
        // Collect all commands for the help dialog
        let commands: Vec<CommandDef> = vec![
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
        ];

        HelpDialogConfig::new(fl!(LANGUAGE_LOADER, "help-title"), fl!(LANGUAGE_LOADER, "help-subtitle"))
            .with_commands(commands)
            .with_category_translator(|category_key| {
                // Use icy_engine_gui's LANGUAGE_LOADER for category translations
                let translation_key = format!("cmd-category-{}", category_key);
                icy_engine_gui::LANGUAGE_LOADER.get(&translation_key)
            })
    }
}
