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
                fl!(LANGUAGE_LOADER, "help-category-navigation"),
                vec![
                    HelpShortcut::new("Enter", fl!(LANGUAGE_LOADER, "help-action-open"), fl!(LANGUAGE_LOADER, "help-desc-open")),
                    HelpShortcut::new(
                        "Backspace",
                        fl!(LANGUAGE_LOADER, "help-action-parent"),
                        fl!(LANGUAGE_LOADER, "help-desc-parent"),
                    ),
                    HelpShortcut::new(
                        if is_macos() { "⌥ ↑".to_string() } else { "Alt+↑".to_string() },
                        fl!(LANGUAGE_LOADER, "help-action-parent"),
                        fl!(LANGUAGE_LOADER, "help-desc-parent"),
                    ),
                    HelpShortcut::new(
                        if is_macos() { "⌥ ←".to_string() } else { "Alt+←".to_string() },
                        fl!(LANGUAGE_LOADER, "help-action-back"),
                        fl!(LANGUAGE_LOADER, "help-desc-back"),
                    ),
                    HelpShortcut::new(
                        if is_macos() { "⌥ →".to_string() } else { "Alt+→".to_string() },
                        fl!(LANGUAGE_LOADER, "help-action-forward"),
                        fl!(LANGUAGE_LOADER, "help-desc-forward"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} F"),
                        fl!(LANGUAGE_LOADER, "help-action-filter"),
                        fl!(LANGUAGE_LOADER, "help-desc-filter"),
                    ),
                ],
            ),
            HelpCategory::new(
                "📺",
                fl!(LANGUAGE_LOADER, "help-category-display"),
                vec![
                    HelpShortcut::new(
                        "Space",
                        fl!(LANGUAGE_LOADER, "help-action-auto-scroll"),
                        fl!(LANGUAGE_LOADER, "help-desc-auto-scroll"),
                    ),
                    HelpShortcut::new(
                        "F2 / ⇧F2",
                        fl!(LANGUAGE_LOADER, "help-action-scroll-speed"),
                        fl!(LANGUAGE_LOADER, "help-desc-scroll-speed"),
                    ),
                    HelpShortcut::new(
                        "F3 / ⇧F3",
                        fl!(LANGUAGE_LOADER, "help-action-baud-rate"),
                        fl!(LANGUAGE_LOADER, "help-desc-baud-rate"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} F3"),
                        fl!(LANGUAGE_LOADER, "help-action-baud-off"),
                        fl!(LANGUAGE_LOADER, "help-desc-baud-off"),
                    ),
                    HelpShortcut::new("F4", fl!(LANGUAGE_LOADER, "help-action-sauce"), fl!(LANGUAGE_LOADER, "help-desc-sauce")),
                ],
            ),
            HelpCategory::new(
                "🔍",
                fl!(LANGUAGE_LOADER, "help-category-zoom"),
                vec![
                    HelpShortcut::new(
                        if is_macos() { "⌘ ++".to_string() } else { "Ctrl+ ++".to_string() },
                        fl!(LANGUAGE_LOADER, "help-action-zoom-in"),
                        fl!(LANGUAGE_LOADER, "help-desc-zoom-in"),
                    ),
                    HelpShortcut::new(
                        if is_macos() { "⌘ -".to_string() } else { "Ctrl+-".to_string() },
                        fl!(LANGUAGE_LOADER, "help-action-zoom-out"),
                        fl!(LANGUAGE_LOADER, "help-desc-zoom-out"),
                    ),
                    HelpShortcut::new(
                        if is_macos() { "⌘ 0".to_string() } else { "Ctrl+0".to_string() },
                        fl!(LANGUAGE_LOADER, "help-action-zoom-reset"),
                        fl!(LANGUAGE_LOADER, "help-desc-zoom-reset"),
                    ),
                    HelpShortcut::new(
                        if is_macos() { "⌘ ⌫".to_string() } else { "Ctrl+⌫".to_string() },
                        fl!(LANGUAGE_LOADER, "help-action-zoom-fit"),
                        fl!(LANGUAGE_LOADER, "help-desc-zoom-fit"),
                    ),
                ],
            ),
            HelpCategory::new(
                "⚙️",
                fl!(LANGUAGE_LOADER, "help-category-tools"),
                vec![
                    HelpShortcut::new(
                        format!("{mod_symbol} I"),
                        fl!(LANGUAGE_LOADER, "help-action-export"),
                        fl!(LANGUAGE_LOADER, "help-desc-export"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} C"),
                        fl!(LANGUAGE_LOADER, "help-action-copy"),
                        fl!(LANGUAGE_LOADER, "help-desc-copy"),
                    ),
                    HelpShortcut::new(
                        "F5-F8",
                        fl!(LANGUAGE_LOADER, "help-action-external"),
                        fl!(LANGUAGE_LOADER, "help-desc-external"),
                    ),
                    HelpShortcut::new(
                        "F11",
                        fl!(LANGUAGE_LOADER, "help-action-fullscreen"),
                        fl!(LANGUAGE_LOADER, "help-desc-fullscreen"),
                    ),
                    HelpShortcut::new("F1", fl!(LANGUAGE_LOADER, "help-action-help"), fl!(LANGUAGE_LOADER, "help-desc-help")),
                    HelpShortcut::new(
                        if is_macos() { "⌥ A".to_string() } else { "Alt+A".to_string() },
                        fl!(LANGUAGE_LOADER, "help-action-about"),
                        fl!(LANGUAGE_LOADER, "help-desc-about"),
                    ),
                ],
            ),
        ];

        HelpDialogConfig::new(fl!(LANGUAGE_LOADER, "help-title"), fl!(LANGUAGE_LOADER, "help-subtitle")).with_categories(categories)
    }
}
