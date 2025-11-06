use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length, Padding, Theme,
    widget::{Space, button, column, container, row, scrollable, text},
};

pub struct HelpDialog;

impl HelpDialog {
    pub fn new() -> Self {
        Self
    }

    pub fn view<'a>(&'a self, terminal_content: Element<'a, crate::ui::Message>) -> Element<'a, crate::ui::Message> {
        let overlay = self.create_modal_content();
        crate::ui::modal(
            terminal_content,
            overlay,
            crate::ui::Message::CloseDialog(Box::new(crate::ui::MainWindowMode::ShowTerminal)),
        )
    }

    fn create_modal_content(&self) -> Element<'_, crate::ui::Message> {
        let is_mac = cfg!(target_os = "macos");
        let mod_symbol = if is_mac { "⌘" } else { "Alt" };

        #[derive(Clone)]
        struct Shortcut {
            keys: String,
            action: String,
            desc: String,
        }

        #[derive(Clone)]
        struct Category {
            name: String,
            icon: &'static str,
            shortcuts: Vec<Shortcut>,
        }

        let categories: Vec<Category> = vec![
            Category {
                icon: "🔌",
                name: fl!(crate::LANGUAGE_LOADER, "help-category-connection"),
                shortcuts: vec![
                    Shortcut {
                        keys: format!("{mod_symbol} D"),
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-dialing-directory"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-dialing-directory"),
                    },
                    Shortcut {
                        keys: format!("{mod_symbol} H"),
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-disconnect"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-disconnect"),
                    },
                    Shortcut {
                        keys: format!("{mod_symbol} X"),
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-exit"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-exit"),
                    },
                ],
            },
            Category {
                icon: "🔐",
                name: fl!(crate::LANGUAGE_LOADER, "help-category-authentication"),
                shortcuts: vec![
                    Shortcut {
                        keys: format!("{mod_symbol} L"),
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-auto-login"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-auto-login"),
                    },
                    Shortcut {
                        keys: format!("{mod_symbol} N"),
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-send-username"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-send-username"),
                    },
                    Shortcut {
                        keys: format!("{mod_symbol} S"),
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-send-password"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-send-password"),
                    },
                ],
            },
            Category {
                icon: "📁",
                name: fl!(crate::LANGUAGE_LOADER, "help-category-file-transfer"),
                shortcuts: vec![
                    Shortcut {
                        keys: format!("{mod_symbol} PgUp"),
                        action: fl!(crate::LANGUAGE_LOADER, "terminal-upload"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-upload"),
                    },
                    Shortcut {
                        keys: format!("{mod_symbol} PgDn"),
                        action: fl!(crate::LANGUAGE_LOADER, "terminal-download"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-download"),
                    },
                ],
            },
            Category {
                icon: "🪟",
                name: fl!(crate::LANGUAGE_LOADER, "help-category-windows"),
                shortcuts: vec![
                    Shortcut {
                        keys: format!("{mod_symbol} W"),
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-close-window"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-close-window"),
                    },
                    Shortcut {
                        keys: if is_mac { "⌘ N".to_string() } else { "Ctrl+Shift+N".to_string() },
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-new-window"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-new-window"),
                    },
                    Shortcut {
                        keys: format!("{mod_symbol} 1-0"),
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-switch-window"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-switch-window"),
                    },
                    Shortcut {
                        keys: format!("{mod_symbol} ↵"),
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-fullscreen"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-fullscreen"),
                    },
                ],
            },
            Category {
                icon: "📺",
                name: fl!(crate::LANGUAGE_LOADER, "help-category-display"),
                shortcuts: vec![
                    Shortcut {
                        keys: format!("{mod_symbol} C"),
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-clear-screen"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-clear-screen"),
                    },
                    Shortcut {
                        keys: format!("{mod_symbol} I"),
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-capture-screen"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-capture-screen"),
                    },
                    Shortcut {
                        keys: format!("{mod_symbol} P"),
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-capture-session"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-capture-session"),
                    },
                    Shortcut {
                        keys: format!("{mod_symbol} F"),
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-find"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-find"),
                    },
                ],
            },
            Category {
                icon: "⚙️",
                name: fl!(crate::LANGUAGE_LOADER, "help-category-tools"),
                shortcuts: vec![
                    Shortcut {
                        keys: format!("{mod_symbol} O"),
                        action: fl!(crate::LANGUAGE_LOADER, "menu-item-settings"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-settings"),
                    },
                    Shortcut {
                        keys: format!("{mod_symbol} A"),
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-about"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-about"),
                    },
                    Shortcut {
                        keys: "F1".to_string(),
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-help"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-help"),
                    },
                ],
            },
            Category {
                icon: "✂️",
                name: fl!(crate::LANGUAGE_LOADER, "help-category-editing"),
                shortcuts: vec![
                    Shortcut {
                        keys: if is_mac { "⌘ C".to_string() } else { "Ctrl+C".to_string() },
                        action: fl!(crate::LANGUAGE_LOADER, "terminal-menu-copy"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-copy"),
                    },
                    Shortcut {
                        keys: if is_mac { "⌘ V".to_string() } else { "Ctrl+V".to_string() },
                        action: fl!(crate::LANGUAGE_LOADER, "terminal-menu-paste"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-paste"),
                    },
                    Shortcut {
                        keys: fl!(crate::LANGUAGE_LOADER, "help-key-middle-click"),
                        action: fl!(crate::LANGUAGE_LOADER, "help-action-smart-paste"),
                        desc: fl!(crate::LANGUAGE_LOADER, "help-desc-smart-paste"),
                    },
                ],
            },
        ];

        fn pill(content: &str) -> Element<'static, crate::ui::Message> {
            container(
                text(content.to_owned())
                    .size(12)
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..iced::Font::default()
                    })
                    .style(|theme: &Theme| text::Style {
                        color: Some(theme.palette().text),
                        ..Default::default()
                    }),
            )
            .padding(Padding::from([5, 12]))
            .style(|theme: &Theme| container::Style {
                background: Some(theme.palette().primary.into()),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            })
            .into()
        }

        fn key_group(keys: &str) -> Element<'static, crate::ui::Message> {
            let parts = keys
                .split(' ')
                .flat_map(|chunk| {
                    if chunk.contains('+') {
                        chunk.split('+').collect::<Vec<_>>()
                    } else {
                        vec![chunk]
                    }
                })
                .collect::<Vec<_>>();

            let mut r = row![].spacing(8).align_y(Alignment::Center);
            for (i, p) in parts.iter().enumerate() {
                r = r.push(pill(p));
                if i + 1 < parts.len() {
                    r = r.push(text("+").size(12).style(|theme: &Theme| text::Style {
                        color: Some(theme.extended_palette().background.base.text),
                        ..Default::default()
                    }));
                }
            }
            r.into()
        }

        fn category_header(icon: &str, name: &str) -> container::Container<'static, crate::ui::Message> {
            container(
                row![
                    text(icon.to_owned()).size(16),
                    Space::new().width(8),
                    text(name.to_owned()).size(16).style(|theme: &Theme| text::Style {
                        color: Some(theme.palette().text),
                        ..Default::default()
                    }),
                ]
                .align_y(Alignment::Center),
            )
            .padding(Padding::from([10, 24]))
            .style(|t: &Theme| container::Style {
                background: Some(
                    Color::from_rgba(
                        t.extended_palette().background.weak.color.r,
                        t.extended_palette().background.weak.color.g,
                        t.extended_palette().background.weak.color.b,
                        0.3,
                    )
                    .into(),
                ),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            })
        }

        // Title block
        let title = container(
            row![
                text("⌨").size(24),
                Space::new().width(10),
                column![
                    text(fl!(crate::LANGUAGE_LOADER, "help-title")).size(22).style(|theme: &Theme| text::Style {
                        color: Some(theme.palette().text),
                        ..Default::default()
                    }),
                    text(fl!(crate::LANGUAGE_LOADER, "help-subtitle")).size(12).style(|theme: &Theme| text::Style {
                        color: Some(theme.extended_palette().background.base.text),
                        ..Default::default()
                    }),
                ]
                .spacing(2)
            ]
            .align_y(Alignment::Center),
        )
        .padding(Padding {
            top: 16.0,
            right: 30.0,
            bottom: 8.0,
            left: 30.0,
        });

        // Build scrollable content
        let mut content = column![].spacing(0);

        for (cat_index, cat) in categories.iter().enumerate() {
            let header = category_header(cat.icon, &cat.name.clone()); // Clone the name
            content = content.push(header.width(Length::Fill));

            for (row_index, sc) in cat.shortcuts.iter().enumerate() {
                let shaded = (cat_index + row_index) % 2 == 0;
                let shortcut_row = container(
                    row![
                        container(key_group(&sc.keys.clone())).width(Length::Fixed(220.0)), // Clone keys
                        Space::new().width(16),
                        container(text(sc.action.clone()).size(13).style(|theme: &Theme| text::Style {
                            // Clone action
                            color: Some(theme.palette().text),
                            ..Default::default()
                        }),)
                        .width(Length::Fixed(140.0)),
                        Space::new().width(12),
                        text(sc.desc.clone()) // Clone desc
                            .size(12)
                            .style(|theme: &Theme| text::Style {
                                color: Some(theme.extended_palette().background.base.text),
                                ..Default::default()
                            })
                            .width(Length::Fill),
                    ]
                    .align_y(Alignment::Center),
                )
                .padding(Padding::from([7, 30]))
                .width(Length::Fill)
                .style(move |_theme: &Theme| container::Style {
                    background: if shaded { Some(Color::from_rgba(0.0, 0.0, 0.0, 0.015).into()) } else { None },
                    ..Default::default()
                });

                content = content.push(shortcut_row);
            }

            content = content.push(container(Space::new().height(4)).width(Length::Fill).style(|_theme: &Theme| container::Style {
                background: None,
                ..Default::default()
            }));
        }

        // Footer
        let footer = container(
            row![
                Space::new().width(Length::Fill),
                button(text(fl!(crate::LANGUAGE_LOADER, "dialog-close_button")).size(13))
                    .padding(Padding::from([5, 20]))
                    .on_press(crate::ui::Message::CloseDialog(Box::new(crate::ui::MainWindowMode::ShowTerminal)))
                    .style(|theme: &Theme, status| {
                        let palette = theme.extended_palette();
                        match status {
                            button::Status::Active => button::Style {
                                background: Some(palette.primary.base.color.into()),
                                text_color: palette.primary.base.text,
                                border: Border {
                                    color: Color::TRANSPARENT,
                                    width: 0.0,
                                    radius: 5.0.into(),
                                },
                                ..Default::default()
                            },
                            button::Status::Hovered => button::Style {
                                background: Some(palette.primary.strong.color.into()),
                                text_color: palette.primary.strong.text,
                                border: Border {
                                    color: Color::TRANSPARENT,
                                    width: 0.0,
                                    radius: 5.0.into(),
                                },
                                ..Default::default()
                            },
                            button::Status::Pressed => button::Style {
                                background: Some(palette.primary.base.color.into()),
                                text_color: palette.primary.base.text,
                                border: Border {
                                    color: palette.primary.strong.color,
                                    width: 0.0,
                                    radius: 5.0.into(),
                                },
                                ..Default::default()
                            },
                            _ => button::Style::default(),
                        }
                    }),
            ]
            .align_y(Alignment::Center),
        )
        .padding(Padding::from([12, 30]))
        .width(Length::Fill);

        // Main dialog container
        let dialog = container(
            column![
                title,
                container(scrollable(container(content).padding(Padding::from([0, 0])).width(Length::Fill),).height(Length::Fill),)
                    .height(Length::FillPortion(1))
                    .padding(Padding::from([0, 0])),
                container(Space::new().height(1)).width(Length::Fill).style(|theme: &Theme| container::Style {
                    background: Some(theme.extended_palette().background.weak.color.into()),
                    ..Default::default()
                }),
                footer,
            ]
            .spacing(0),
        )
        .width(Length::Fixed(780.0))
        .height(Length::Fixed(500.0))
        .style(|theme: &Theme| container::Style {
            background: Some(iced::Background::Color(theme.extended_palette().background.base.color)),
            border: Border {
                color: theme.extended_palette().background.strong.color,
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                offset: iced::Vector::new(0.0, 6.0),
                blur_radius: 20.0,
            },
            ..Default::default()
        });

        container(dialog)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}
