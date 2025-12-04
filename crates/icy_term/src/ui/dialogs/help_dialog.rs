use i18n_embed_fl::fl;
use iced::Element;
use icy_engine_gui::ui::{HelpCategory, HelpDialogConfig, HelpShortcut, help_dialog_content, is_macos, modal, platform_mod_symbol};

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
        let mod_symbol = platform_mod_symbol();

        let categories: Vec<HelpCategory> = vec![
            HelpCategory::new(
                "🔌",
                fl!(crate::LANGUAGE_LOADER, "help-category-connection"),
                vec![
                    HelpShortcut::new(
                        format!("{mod_symbol} D"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-dialing-directory"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-dialing-directory"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} T"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-open-serial"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-open-serial"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} H"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-disconnect"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-disconnect"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} X"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-exit"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-exit"),
                    ),
                ],
            ),
            HelpCategory::new(
                "🔐",
                fl!(crate::LANGUAGE_LOADER, "help-category-authentication"),
                vec![
                    HelpShortcut::new(
                        format!("{mod_symbol} L"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-auto-login"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-auto-login"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} U"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-send-username"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-send-username"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} S"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-send-password"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-send-password"),
                    ),
                ],
            ),
            HelpCategory::new(
                "📁",
                fl!(crate::LANGUAGE_LOADER, "help-category-file-transfer"),
                vec![
                    HelpShortcut::new(
                        format!("{mod_symbol} PgUp"),
                        fl!(crate::LANGUAGE_LOADER, "terminal-upload"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-upload"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} PgDn"),
                        fl!(crate::LANGUAGE_LOADER, "terminal-download"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-download"),
                    ),
                ],
            ),
            HelpCategory::new(
                "🪟",
                fl!(crate::LANGUAGE_LOADER, "help-category-windows"),
                vec![
                    HelpShortcut::new(
                        format!("{mod_symbol} W"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-close-window"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-close-window"),
                    ),
                    HelpShortcut::new(
                        if is_macos() { "⌘ N".to_string() } else { "Ctrl+Shift+N".to_string() },
                        fl!(crate::LANGUAGE_LOADER, "help-action-new-window"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-new-window"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} 1-0"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-switch-window"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-switch-window"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} ↵"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-fullscreen"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-fullscreen"),
                    ),
                ],
            ),
            HelpCategory::new(
                "📺",
                fl!(crate::LANGUAGE_LOADER, "help-category-display"),
                vec![
                    HelpShortcut::new(
                        if is_macos() { "⌥ C".to_string() } else { format!("{mod_symbol} C") },
                        fl!(crate::LANGUAGE_LOADER, "help-action-clear-screen"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-clear-screen"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} B"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-scrollback"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-scrollback"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} I"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-capture-screen"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-capture-screen"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} P"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-capture-session"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-capture-session"),
                    ),
                ],
            ),
            HelpCategory::new(
                "⚙️",
                fl!(crate::LANGUAGE_LOADER, "help-category-tools"),
                vec![
                    HelpShortcut::new(
                        format!("{mod_symbol} R"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-run-script"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-run-script"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} F"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-find"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-find"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} O"),
                        fl!(crate::LANGUAGE_LOADER, "menu-item-settings"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-settings"),
                    ),
                    HelpShortcut::new(
                        format!("{mod_symbol} A"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-about"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-about"),
                    ),
                    HelpShortcut::new(
                        "F1",
                        fl!(crate::LANGUAGE_LOADER, "help-action-help"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-help"),
                    ),
                ],
            ),
            HelpCategory::new(
                "✂️",
                fl!(crate::LANGUAGE_LOADER, "help-category-editing"),
                vec![
                    HelpShortcut::new(
                        if is_macos() { "⌘ C".to_string() } else { "Ctrl+C".to_string() },
                        fl!(crate::LANGUAGE_LOADER, "terminal-menu-copy"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-copy"),
                    ),
                    HelpShortcut::new(
                        if is_macos() { "⌘ V".to_string() } else { "Ctrl+V".to_string() },
                        fl!(crate::LANGUAGE_LOADER, "terminal-menu-paste"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-paste"),
                    ),
                    HelpShortcut::new(
                        fl!(crate::LANGUAGE_LOADER, "help-key-middle-click"),
                        fl!(crate::LANGUAGE_LOADER, "help-action-smart-paste"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-smart-paste"),
                    ),
                ],
            ),
            HelpCategory::new(
                "🔍",
                fl!(crate::LANGUAGE_LOADER, "help-category-zoom"),
                vec![
                    HelpShortcut::new(
                        if is_macos() { "⌘ ++".to_string() } else { "Ctrl+ ++".to_string() },
                        fl!(crate::LANGUAGE_LOADER, "help-action-zoom-in"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-zoom-in"),
                    ),
                    HelpShortcut::new(
                        if is_macos() { "⌘ -".to_string() } else { "Ctrl+-".to_string() },
                        fl!(crate::LANGUAGE_LOADER, "help-action-zoom-out"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-zoom-out"),
                    ),
                    HelpShortcut::new(
                        if is_macos() { "⌘ 0".to_string() } else { "Ctrl+0".to_string() },
                        fl!(crate::LANGUAGE_LOADER, "help-action-zoom-reset"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-zoom-reset"),
                    ),
                    HelpShortcut::new(
                        if is_macos() { "⌘ ⌫".to_string() } else { "Ctrl+⌫".to_string() },
                        fl!(crate::LANGUAGE_LOADER, "help-action-zoom-auto"),
                        fl!(crate::LANGUAGE_LOADER, "help-desc-zoom-auto"),
                    ),
                ],
            ),
        ];

        HelpDialogConfig::new(fl!(crate::LANGUAGE_LOADER, "help-title"), fl!(crate::LANGUAGE_LOADER, "help-subtitle")).with_categories(categories)
    }
}
