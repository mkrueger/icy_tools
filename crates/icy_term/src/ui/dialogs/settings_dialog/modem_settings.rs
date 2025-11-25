use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length,
    widget::{Space, button, column, container, pick_list, row, scrollable, text, text_input},
};
use icy_engine_gui::{
    SECTION_PADDING, section_header,
    settings::{effect_box, left_label},
    ui::DIALOG_SPACING as INPUT_SPACING,
};

use crate::ui::{
    select_bps_dialog::STANDARD_RATES,
    settings_dialog::{CharSizeOption, FlowControlOption, ParityOption, SettingsDialogState, SettingsMsg, StopBitsOption},
};

impl SettingsDialogState {
    pub fn modem_settings_content<'a>(&self) -> Element<'a, crate::ui::Message> {
        let modems = self.temp_options.lock().modems.clone();
        let selected_index = self.selected_modem_index;

        // Modem list
        let mut modem_list: iced::widget::Column<'_, crate::ui::Message> = column![].spacing(2);
        for (idx, modem) in modems.iter().enumerate() {
            let is_selected = idx == selected_index;
            let modem_button = button(container(text(modem.name.clone()).size(13)).width(Length::Fill).padding([6, 10]))
                .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::SelectModem(idx)))
                .width(Length::Fill)
                .style(move |theme: &iced::Theme, status| {
                    use iced::widget::button::{Status, Style};
                    let palette = theme.extended_palette();
                    let base = if is_selected {
                        Style {
                            background: Some(iced::Background::Color(palette.primary.weak.color)),
                            text_color: palette.primary.weak.text,
                            border: Border::default().rounded(4.0),
                            shadow: Default::default(),
                            snap: false,
                        }
                    } else {
                        Style {
                            background: Some(iced::Background::Color(Color::TRANSPARENT)),
                            text_color: palette.background.base.text,
                            border: Border::default(),
                            shadow: Default::default(),
                            snap: false,
                        }
                    };

                    match status {
                        Status::Active => base,
                        Status::Hovered if !is_selected => Style {
                            background: Some(iced::Background::Color(palette.background.weak.color)),
                            ..base
                        },
                        Status::Pressed => Style {
                            background: Some(iced::Background::Color(palette.primary.strong.color)),
                            text_color: palette.primary.strong.text,
                            ..base
                        },
                        _ => base,
                    }
                });
            modem_list = modem_list.push(modem_button);
        }

        // Add/Remove buttons with icons
        let add_button = button(row![text("+").size(14)].align_y(Alignment::Center))
            .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::AddModem))
            .padding([6, 10]);

        let remove_button: button::Button<'_, crate::ui::Message> = if !modems.is_empty() {
            button(row![text("-").size(14)].align_y(Alignment::Center))
                .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::RemoveModem(selected_index)))
                .padding([6, 10])
        } else {
            button(row![text("-").size(14)].align_y(Alignment::Center)).padding([6, 10])
        };

        let left_panel = container(column![
            section_header(fl!(crate::LANGUAGE_LOADER, "settings-modem-list-section")),
            container(scrollable(modem_list).direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default())))
                .height(Length::Fill)
                .width(Length::Fill)
                .style(|theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(theme.extended_palette().background.weak.color)),
                    border: Border {
                        color: theme.extended_palette().background.strong.color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                }),
            Space::new().height(8.0),
            row![add_button, Space::new().width(8.0), remove_button].width(Length::Fill),
        ])
        .width(Length::Fixed(180.0))
        .height(Length::Fill);

        let modem_settings: Element<'_, crate::ui::Message> = if let Some(modem) = modems.get(selected_index) {
            // Filter only Some(...) rates
            let baud_options: Vec<String> = STANDARD_RATES.iter().filter_map(|r| r.as_ref()).map(|r| r.to_string()).collect();

            let current_baud = modem.baud_rate.to_string();
            let selected_baud = if baud_options.contains(&current_baud) {
                Some(current_baud.clone())
            } else {
                None
            };

            scrollable(
                column![
                    // Basic Settings section
                    section_header(fl!(crate::LANGUAGE_LOADER, "settings-modem-basic-section")),
                    effect_box(
                        column![
                            row![
                                left_label(fl!(crate::LANGUAGE_LOADER, "settings-modem-name")),
                                text_input("", &modem.name)
                                    .on_input({
                                        let temp_options_arc = self.temp_options.clone();
                                        move |value| {
                                            let mut new_options = temp_options_arc.lock().clone();
                                            if let Some(m) = new_options.modems.get_mut(selected_index) {
                                                m.name = value;
                                            }
                                            crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                        }
                                    })
                                    .width(Length::Fill)
                                    .size(14),
                            ]
                            .spacing(12)
                            .align_y(Alignment::Center),
                            row![
                                left_label(fl!(crate::LANGUAGE_LOADER, "settings-modem-device")),
                                text_input("", &modem.device)
                                    .on_input({
                                        let temp_options_arc = self.temp_options.clone();
                                        move |value| {
                                            let mut new_options = temp_options_arc.lock().clone();
                                            if let Some(m) = new_options.modems.get_mut(selected_index) {
                                                m.device = value;
                                            }
                                            crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                        }
                                    })
                                    .width(Length::Fill)
                                    .size(14),
                            ]
                            .spacing(12)
                            .align_y(Alignment::Center),
                            // Baud Rate
                            row![
                                left_label(fl!(crate::LANGUAGE_LOADER, "settings-modem-baud_rate")),
                                text_input("", &modem.baud_rate.to_string())
                                    .on_input({
                                        let temp_options_arc = self.temp_options.clone();
                                        move |value| {
                                            if let Ok(baud) = value.parse::<u32>() {
                                                let mut new_options = temp_options_arc.lock().clone();
                                                if let Some(m) = new_options.modems.get_mut(selected_index) {
                                                    m.baud_rate = baud;
                                                }
                                                crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                            } else {
                                                crate::ui::Message::SettingsDialog(SettingsMsg::Noop)
                                            }
                                        }
                                    })
                                    .width(Length::Fixed(100.0))
                                    .size(14),
                                pick_list(baud_options, selected_baud, {
                                    let temp_options_arc = self.temp_options.clone();
                                    move |value| {
                                        if let Ok(baud) = value.parse::<u32>() {
                                            let mut new_options = temp_options_arc.lock().clone();
                                            if let Some(m) = new_options.modems.get_mut(selected_index) {
                                                m.baud_rate = baud;
                                            }
                                            crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                        } else {
                                            crate::ui::Message::SettingsDialog(SettingsMsg::Noop)
                                        }
                                    }
                                })
                                .placeholder(fl!(crate::LANGUAGE_LOADER, "settings-modem-baud_rate-quick"))
                                .width(Length::Fixed(100.0))
                                .text_size(14),
                                Space::new().width(Length::Fill),
                            ]
                            .spacing(12)
                            .align_y(Alignment::Center),
                        ]
                        .spacing(INPUT_SPACING)
                        .into()
                    ),
                    Space::new().height(24.0),
                    // Connection Settings section
                    section_header(fl!(crate::LANGUAGE_LOADER, "settings-modem-connection-section")),
                    effect_box(
                        column![
                            // Data Format
                            row![
                                left_label(fl!(crate::LANGUAGE_LOADER, "settings-modem-char_size")),
                                pick_list(&CharSizeOption::ALL[..], Some(CharSizeOption::from(modem.char_size)), {
                                    let temp_options_arc = self.temp_options.clone();
                                    move |value| {
                                        let mut new_options = temp_options_arc.lock().clone();
                                        if let Some(m) = new_options.modems.get_mut(selected_index) {
                                            m.char_size = value.into();
                                        }
                                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                    }
                                })
                                .width(Length::Fixed(60.0))
                                .text_size(14),
                                Space::new().width(12.0),
                            ]
                            .spacing(12)
                            .align_y(Alignment::Center),
                            // Data Format
                            row![
                                left_label("Stopbits".to_string()),
                                pick_list(&StopBitsOption::ALL[..], Some(StopBitsOption::from(modem.stop_bits)), {
                                    let temp_options_arc = self.temp_options.clone();
                                    move |value| {
                                        let mut new_options = temp_options_arc.lock().clone();
                                        if let Some(m) = new_options.modems.get_mut(selected_index) {
                                            m.stop_bits = value.into();
                                        }
                                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                    }
                                })
                                .width(Length::Fixed(60.0))
                                .text_size(14),
                                Space::new().width(Length::Fill),
                            ]
                            .spacing(12)
                            .align_y(Alignment::Center),
                            // Parity
                            row![
                                left_label(fl!(crate::LANGUAGE_LOADER, "settings-modem-parity")),
                                pick_list(&ParityOption::ALL[..], Some(ParityOption::from(modem.parity)), {
                                    let temp_options_arc = self.temp_options.clone();
                                    move |value| {
                                        let mut new_options = temp_options_arc.lock().clone();
                                        if let Some(m) = new_options.modems.get_mut(selected_index) {
                                            m.parity = value.into();
                                        }
                                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                    }
                                })
                                .width(Length::Fixed(120.0))
                                .text_size(14),
                                Space::new().width(Length::Fill),
                            ]
                            .spacing(12)
                            .align_y(Alignment::Center),
                            // Flow control
                            row![
                                left_label(fl!(crate::LANGUAGE_LOADER, "settings-modem-flow_control")),
                                pick_list(&FlowControlOption::ALL[..], Some(FlowControlOption::from(modem.flow_control)), {
                                    let temp_options_arc = self.temp_options.clone();
                                    move |value| {
                                        let mut new_options = temp_options_arc.lock().clone();
                                        if let Some(m) = new_options.modems.get_mut(selected_index) {
                                            m.flow_control = value.into();
                                        }
                                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                    }
                                })
                                .width(Length::Fixed(120.0))
                                .text_size(14),
                                Space::new().width(Length::Fill),
                            ]
                            .spacing(12)
                            .align_y(Alignment::Center),
                        ]
                        .spacing(INPUT_SPACING)
                        .into()
                    ),
                    Space::new().height(24.0),
                    // AT section
                    section_header(fl!(crate::LANGUAGE_LOADER, "settings-modem-at-section")),
                    effect_box(
                        column![
                            row![
                                left_label(fl!(crate::LANGUAGE_LOADER, "settings-modem-init_string")),
                                text_input("ATZ", &modem.init_string)
                                    .on_input({
                                        let temp_options_arc = self.temp_options.clone();
                                        move |value| {
                                            let mut new_options = temp_options_arc.lock().clone();
                                            if let Some(m) = new_options.modems.get_mut(selected_index) {
                                                m.init_string = value;
                                            }
                                            crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                        }
                                    })
                                    .width(Length::Fill)
                                    .size(14),
                            ]
                            .spacing(12)
                            .align_y(Alignment::Center),
                            row![
                                left_label(fl!(crate::LANGUAGE_LOADER, "settings-modem-dial_prefix")),
                                text_input("ATDT", &modem.dial_string)
                                    .on_input({
                                        let temp_options_arc = self.temp_options.clone();
                                        move |value| {
                                            let mut new_options = temp_options_arc.lock().clone();
                                            if let Some(m) = new_options.modems.get_mut(selected_index) {
                                                m.dial_string = value;
                                            }
                                            crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                        }
                                    })
                                    .width(Length::Fill)
                                    .size(14),
                            ]
                            .spacing(12)
                            .align_y(Alignment::Center),
                        ]
                        .spacing(INPUT_SPACING)
                        .into()
                    ),
                ]
                .padding(SECTION_PADDING)
                .spacing(4),
            )
            .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default()))
            .into()
        } else {
            column![
                container(
                    column![
                        text("📡").size(32),
                        Space::new().height(8.0),
                        text(fl!(crate::LANGUAGE_LOADER, "settings-modem-nothing_selected")).size(14),
                        Space::new().height(4.0),
                        text(fl!(crate::LANGUAGE_LOADER, "settings-modem-no-selection-hint")).size(12),
                    ]
                    .align_x(Alignment::Center)
                )
                .align_y(Alignment::Center)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .padding(20),
            ]
            .into()
        };

        // Right panel - settings (scrollable)
        let right_panel = container(modem_settings).width(Length::Fill).height(Length::Fill);

        // Main layout with proper pane splitting
        row![left_panel, right_panel,].into()
    }
}
