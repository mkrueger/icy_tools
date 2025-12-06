//! Help dialog for icy_view

use i18n_embed_fl::fl;
use iced::Element;
use icy_engine_gui::ui::{HelpCategory, HelpDialogConfig, HelpShortcut, help_dialog_content, is_macos, modal, platform_mod_symbol};

use crate::LANGUAGE_LOADER;

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
        let mod_symbol = platform_mod_symbol();

        let categories: Vec<HelpCategory> = vec![
            HelpCategory::new(
                "📁",
                fl!(LANGUAGE_LOADER, "cmd-category-navigation"),
                vec![
                    HelpShortcut::new(
                        "Enter",
                        fl!(LANGUAGE_LOADER, "cmd-file-open-action"),
                        fl!(LANGUAGE_LOADER, "cmd-file-open-desc"),
                    ),
                    HelpShortcut::new("Backspace", fl!(LANGUAGE_LOADER, "cmd-nav-up-action"), fl!(LANGUAGE_LOADER, "cmd-nav-up-desc")),
                    HelpShortcut::new(
                        if is_macos() { "⌥ ↑".to_string() } else { "Alt+↑".to_string() },
                        fl!(LANGUAGE_LOADER, "cmd-nav-up-action"),
                        fl!(LANGUAGE_LOADER, "cmd-nav-up-desc"),
                    ),
                    HelpShortcut::new(
                        if is_macos() { "⌥ ←".to_string() } else { "Alt+←".to_string() },
                        fl!(LANGUAGE_LOADER, "cmd-nav-back-action"),
                        fl!(LANGUAGE_LOADER, "cmd-nav-back-desc"),
                    ),
                    HelpShortcut::new(
                        if is_macos() { "⌥ →".to_string() } else { "Alt+→".to_string() },
                        fl!(LANGUAGE_LOADER, "cmd-nav-forward-action"),
                        fl!(LANGUAGE_LOADER, "cmd-nav-forward-desc"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} F"),
                        fl!(LANGUAGE_LOADER, "cmd-dialog-filter-action"),
                        fl!(LANGUAGE_LOADER, "cmd-dialog-filter-desc"),
                    ),
                ],
            ),
            HelpCategory::new(
                "📺",
                fl!(LANGUAGE_LOADER, "cmd-category-playback"),
                vec![
                    HelpShortcut::new(
                        "Space",
                        fl!(LANGUAGE_LOADER, "cmd-playback-toggle_scroll-action"),
                        fl!(LANGUAGE_LOADER, "cmd-playback-toggle_scroll-desc"),
                    ),
                    HelpShortcut::new(
                        "F2 / ⇧F2",
                        fl!(LANGUAGE_LOADER, "cmd-playback-scroll_speed-action"),
                        fl!(LANGUAGE_LOADER, "cmd-playback-scroll_speed-desc"),
                    ),
                    HelpShortcut::new(
                        "F3 / ⇧F3",
                        fl!(LANGUAGE_LOADER, "cmd-playback-baud_rate-action"),
                        fl!(LANGUAGE_LOADER, "cmd-playback-baud_rate-desc"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} F3"),
                        fl!(LANGUAGE_LOADER, "cmd-playback-baud_rate_off-action"),
                        fl!(LANGUAGE_LOADER, "cmd-playback-baud_rate_off-desc"),
                    ),
                    HelpShortcut::new(
                        "F4",
                        fl!(LANGUAGE_LOADER, "cmd-dialog-sauce-action"),
                        fl!(LANGUAGE_LOADER, "cmd-dialog-sauce-desc"),
                    ),
                ],
            ),
            HelpCategory::new(
                "🔍",
                fl!(LANGUAGE_LOADER, "cmd-category-view"),
                vec![
                    HelpShortcut::new(
                        if is_macos() { "⌘ ++".to_string() } else { "Ctrl+ ++".to_string() },
                        fl!(LANGUAGE_LOADER, "cmd-view-zoom_in-action"),
                        fl!(LANGUAGE_LOADER, "cmd-view-zoom_in-desc"),
                    ),
                    HelpShortcut::new(
                        if is_macos() { "⌘ -".to_string() } else { "Ctrl+-".to_string() },
                        fl!(LANGUAGE_LOADER, "cmd-view-zoom_out-action"),
                        fl!(LANGUAGE_LOADER, "cmd-view-zoom_out-desc"),
                    ),
                    HelpShortcut::new(
                        if is_macos() { "⌘ 0".to_string() } else { "Ctrl+0".to_string() },
                        fl!(LANGUAGE_LOADER, "cmd-view-zoom_reset-action"),
                        fl!(LANGUAGE_LOADER, "cmd-view-zoom_reset-desc"),
                    ),
                    HelpShortcut::new(
                        if is_macos() { "⌘ ⌫".to_string() } else { "Ctrl+⌫".to_string() },
                        fl!(LANGUAGE_LOADER, "cmd-view-zoom_fit-action"),
                        fl!(LANGUAGE_LOADER, "cmd-view-zoom_fit-desc"),
                    ),
                ],
            ),
            HelpCategory::new(
                "⚙️",
                fl!(LANGUAGE_LOADER, "cmd-category-external"),
                vec![
                    HelpShortcut::new(
                        format!("{mod_symbol} I"),
                        fl!(LANGUAGE_LOADER, "cmd-dialog-export-action"),
                        fl!(LANGUAGE_LOADER, "cmd-dialog-export-desc"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} C"),
                        fl!(LANGUAGE_LOADER, "cmd-edit-copy-action"),
                        fl!(LANGUAGE_LOADER, "cmd-edit-copy-desc"),
                    ),
                    HelpShortcut::new(
                        "F5-F8",
                        fl!(LANGUAGE_LOADER, "cmd-external-command_0-action"),
                        fl!(LANGUAGE_LOADER, "cmd-external-command_0-desc"),
                    ),
                    HelpShortcut::new(
                        "F11",
                        fl!(LANGUAGE_LOADER, "cmd-view-fullscreen-action"),
                        fl!(LANGUAGE_LOADER, "cmd-view-fullscreen-desc"),
                    ),
                    HelpShortcut::new("F1", fl!(LANGUAGE_LOADER, "cmd-help-show-action"), fl!(LANGUAGE_LOADER, "cmd-help-show-desc")),
                    HelpShortcut::new(
                        if is_macos() { "⌥ A".to_string() } else { "Alt+A".to_string() },
                        fl!(LANGUAGE_LOADER, "cmd-help-about-action"),
                        fl!(LANGUAGE_LOADER, "cmd-help-about-desc"),
                    ),
                ],
            ),
        ];

        HelpDialogConfig::new(fl!(LANGUAGE_LOADER, "help-title"), fl!(LANGUAGE_LOADER, "help-subtitle")).with_categories(categories)
    }
}
