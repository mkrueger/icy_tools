use i18n_embed_fl::fl;
use iced::Element;
use icy_engine_gui::commands::CommandDef;
use icy_engine_gui::ui::{HelpDialogConfig, help_dialog_content, modal};

use crate::commands::cmd;

pub struct HelpDialog;

impl HelpDialog {
    pub fn new() -> Self {
        Self
    }

    pub fn view<'a>(&'a self, terminal_content: Element<'a, crate::ui::Message>) -> Element<'a, crate::ui::Message> {
        let config = self.build_config();
        let close_msg = crate::ui::Message::CloseDialog(Box::new(crate::ui::MainWindowMode::ShowTerminal));
        let content = help_dialog_content(&config, close_msg.clone());
        modal(terminal_content, content, close_msg)
    }

    fn build_config(&self) -> HelpDialogConfig {
        // Collect all commands for the help dialog
        let commands: Vec<CommandDef> = vec![
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
        ];

        HelpDialogConfig::new(fl!(crate::LANGUAGE_LOADER, "help-title"), fl!(crate::LANGUAGE_LOADER, "help-subtitle"))
            .with_commands(commands)
            .with_category_translator(|category_key| {
                // Use icy_engine_gui's LANGUAGE_LOADER for category translations
                // Category keys in TOML are like "connection", "transfer", "terminal"
                // Translation keys are "cmd-category-{key}"
                let translation_key = format!("cmd-category-{}", category_key);
                icy_engine_gui::LANGUAGE_LOADER.get(&translation_key)
            })
    }
}
