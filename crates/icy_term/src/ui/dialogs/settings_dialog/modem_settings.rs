use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length,
    widget::{Space, button, column, container, pick_list, row, scrollable, svg, text, text_input, tooltip},
};
use icy_engine_gui::{
    SECTION_PADDING, TEXT_SIZE_NORMAL, section_header,
    settings::effect_box,
    ui::{DIALOG_SPACING, TEXT_SIZE_SMALL, left_label_small, secondary_button_style},
};

use crate::ui::{
    select_bps_dialog::STANDARD_RATES,
    settings_dialog::{CharSizeOption, FlowControlOption, ParityOption, SettingsDialogState, SettingsMsg, StopBitsOption},
};

const ADD_SVG: &[u8] = include_bytes!("../../../../data/icons/add.svg");
const DELETE_SVG: &[u8] = include_bytes!("../../../../data/icons/delete.svg");

impl SettingsDialogState {
    pub fn modem_settings_content<'a>(&self) -> Element<'a, crate::ui::Message> {
        let modems = self.temp_options.lock().modems.clone();
        let selected_index = self.selected_modem_index;

        // Modem list with better styling
        let mut modem_list: iced::widget::Column<'_, crate::ui::Message> = column![].spacing(2);
        for (idx, modem) in modems.iter().enumerate() {
            let is_selected = idx == selected_index;
            let modem_name = modem.name.clone();
            let modem_button = button(container(text(modem_name).size(TEXT_SIZE_NORMAL)).width(Length::Fill).padding([8, 12]))
                .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::SelectModem(idx)))
                .width(Length::Fill)
                .style(move |theme: &iced::Theme, status| {
                    use iced::widget::button::{Status, Style};
                    let palette = theme.extended_palette();
                    let base = if is_selected {
                        Style {
                            background: Some(iced::Background::Color(palette.primary.weak.color)),
                            text_color: palette.primary.weak.text,
                            border: Border::default().rounded(6.0),
                            shadow: Default::default(),
                            snap: false,
                        }
                    } else {
                        Style {
                            background: Some(iced::Background::Color(Color::TRANSPARENT)),
                            text_color: palette.background.base.text,
                            border: Border::default().rounded(6.0),
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

        // Icon buttons for Add/Remove
        let add_icon = svg(svg::Handle::from_memory(ADD_SVG)).width(Length::Fixed(18.0)).height(Length::Fixed(18.0));
        let delete_icon = svg(svg::Handle::from_memory(DELETE_SVG)).width(Length::Fixed(18.0)).height(Length::Fixed(18.0));

        let add_button = tooltip(
            button(add_icon)
                .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::AddModem))
                .padding(6)
                .style(secondary_button_style),
            container(text(fl!(crate::LANGUAGE_LOADER, "settings-modem-add-tooltip")).size(TEXT_SIZE_SMALL))
                .style(container::rounded_box)
                .padding(8),
            tooltip::Position::Top,
        )
        .gap(8);

        let remove_button = if !modems.is_empty() {
            tooltip(
                button(delete_icon)
                    .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::RemoveModem(selected_index)))
                    .padding(6)
                    .style(secondary_button_style),
                container(text(fl!(crate::LANGUAGE_LOADER, "settings-modem-remove-tooltip")).size(TEXT_SIZE_SMALL))
                    .style(container::rounded_box)
                    .padding(8),
                tooltip::Position::Top,
            )
            .gap(8)
        } else {
            tooltip(
                button(svg(svg::Handle::from_memory(DELETE_SVG)).width(Length::Fixed(18.0)).height(Length::Fixed(18.0)))
                    .padding(6)
                    .style(secondary_button_style),
                container(text(fl!(crate::LANGUAGE_LOADER, "settings-modem-remove-tooltip")).size(TEXT_SIZE_SMALL))
                    .style(container::rounded_box)
                    .padding(8),
                tooltip::Position::Top,
            )
            .gap(8)
        };

        let list_container = container(scrollable(modem_list).direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default())))
            .height(Length::Fill)
            .width(Length::Fill)
            .style(|theme: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(theme.extended_palette().background.weak.color)),
                border: Border {
                    color: theme.extended_palette().background.strong.color,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            });

        let left_panel = container(column![
            section_header(fl!(crate::LANGUAGE_LOADER, "settings-modem-list-section")),
            list_container,
            Space::new().height(DIALOG_SPACING),
            row![add_button, Space::new().width(DIALOG_SPACING), remove_button, Space::new().width(Length::Fill),].align_y(Alignment::Center),
        ])
        .width(Length::Fixed(200.0))
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
                    // Device section
                    section_header(fl!(crate::LANGUAGE_LOADER, "settings-modem-device-section")),
                    effect_box(
                        column![
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-modem-name")),
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
                                    .size(TEXT_SIZE_NORMAL),
                            ]
                            .spacing(DIALOG_SPACING)
                            .align_y(Alignment::Center),
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-modem-device")),
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
                                    .size(TEXT_SIZE_NORMAL),
                            ]
                            .spacing(DIALOG_SPACING)
                            .align_y(Alignment::Center),
                        ]
                        .spacing(DIALOG_SPACING)
                        .into()
                    ),
                    Space::new().height(DIALOG_SPACING * 2.0),
                    // Serial Port section
                    section_header(fl!(crate::LANGUAGE_LOADER, "settings-modem-serial-section")),
                    effect_box(
                        column![
                            // Baud Rate
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-modem-baud_rate")),
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
                                    .size(TEXT_SIZE_NORMAL),
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
                                .text_size(TEXT_SIZE_NORMAL),
                                Space::new().width(Length::Fill),
                            ]
                            .spacing(DIALOG_SPACING)
                            .align_y(Alignment::Center),
                            // Data Format (combined row like in serial dialog)
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-modem-format")),
                                pick_list(&CharSizeOption::ALL[..], Some(CharSizeOption::from(modem.format.char_size)), {
                                    let temp_options_arc = self.temp_options.clone();
                                    move |value| {
                                        let mut new_options = temp_options_arc.lock().clone();
                                        if let Some(m) = new_options.modems.get_mut(selected_index) {
                                            m.format.char_size = value.into();
                                        }
                                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                    }
                                })
                                .width(Length::Fixed(60.0))
                                .text_size(TEXT_SIZE_NORMAL),
                                text("-").size(TEXT_SIZE_NORMAL),
                                pick_list(&ParityOption::ALL[..], Some(ParityOption::from(modem.format.parity)), {
                                    let temp_options_arc = self.temp_options.clone();
                                    move |value| {
                                        let mut new_options = temp_options_arc.lock().clone();
                                        if let Some(m) = new_options.modems.get_mut(selected_index) {
                                            m.format.parity = value.into();
                                        }
                                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                    }
                                })
                                .width(Length::Fixed(70.0))
                                .text_size(TEXT_SIZE_NORMAL),
                                text("-").size(TEXT_SIZE_NORMAL),
                                pick_list(&StopBitsOption::ALL[..], Some(StopBitsOption::from(modem.format.stop_bits)), {
                                    let temp_options_arc = self.temp_options.clone();
                                    move |value| {
                                        let mut new_options = temp_options_arc.lock().clone();
                                        if let Some(m) = new_options.modems.get_mut(selected_index) {
                                            m.format.stop_bits = value.into();
                                        }
                                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                    }
                                })
                                .width(Length::Fixed(50.0))
                                .text_size(TEXT_SIZE_NORMAL),
                            ]
                            .spacing(DIALOG_SPACING)
                            .align_y(Alignment::Center),
                            // Flow control
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-modem-flow_control")),
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
                                .text_size(TEXT_SIZE_NORMAL),
                            ]
                            .spacing(DIALOG_SPACING)
                            .align_y(Alignment::Center),
                        ]
                        .spacing(DIALOG_SPACING)
                        .into()
                    ),
                    Space::new().height(DIALOG_SPACING * 2.0),
                    // Modem Commands section
                    section_header(fl!(crate::LANGUAGE_LOADER, "settings-modem-commands-section")),
                    effect_box(
                        column![
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-modem-init_command")),
                                text_input("ATZ^M", &modem.init_command)
                                    .on_input({
                                        let temp_options_arc = self.temp_options.clone();
                                        move |value| {
                                            let mut new_options = temp_options_arc.lock().clone();
                                            if let Some(m) = new_options.modems.get_mut(selected_index) {
                                                m.init_command = value;
                                            }
                                            crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                        }
                                    })
                                    .width(Length::Fill)
                                    .size(TEXT_SIZE_NORMAL),
                            ]
                            .spacing(DIALOG_SPACING)
                            .align_y(Alignment::Center),
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-modem-dial_prefix")),
                                text_input("ATDT", &modem.dial_prefix)
                                    .on_input({
                                        let temp_options_arc = self.temp_options.clone();
                                        move |value| {
                                            let mut new_options = temp_options_arc.lock().clone();
                                            if let Some(m) = new_options.modems.get_mut(selected_index) {
                                                m.dial_prefix = value;
                                            }
                                            crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                        }
                                    })
                                    .width(Length::Fill)
                                    .size(TEXT_SIZE_NORMAL),
                            ]
                            .spacing(DIALOG_SPACING)
                            .align_y(Alignment::Center),
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-modem-dial_suffix")),
                                text_input("^M", &modem.dial_suffix)
                                    .on_input({
                                        let temp_options_arc = self.temp_options.clone();
                                        move |value| {
                                            let mut new_options = temp_options_arc.lock().clone();
                                            if let Some(m) = new_options.modems.get_mut(selected_index) {
                                                m.dial_suffix = value;
                                            }
                                            crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                        }
                                    })
                                    .width(Length::Fill)
                                    .size(TEXT_SIZE_NORMAL),
                            ]
                            .spacing(DIALOG_SPACING)
                            .align_y(Alignment::Center),
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-modem-hangup_command")),
                                text_input("+++ATH0^M", &modem.hangup_command)
                                    .on_input({
                                        let temp_options_arc = self.temp_options.clone();
                                        move |value| {
                                            let mut new_options = temp_options_arc.lock().clone();
                                            if let Some(m) = new_options.modems.get_mut(selected_index) {
                                                m.hangup_command = value;
                                            }
                                            crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                                        }
                                    })
                                    .width(Length::Fill)
                                    .size(TEXT_SIZE_NORMAL),
                            ]
                            .spacing(DIALOG_SPACING)
                            .align_y(Alignment::Center),
                        ]
                        .spacing(DIALOG_SPACING)
                        .into()
                    ),
                ]
                .padding(SECTION_PADDING)
                .spacing(DIALOG_SPACING),
            )
            .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default()))
            .into()
        } else {
            // Empty state with better visual design
            container(
                column![
                    text("📡").size(48),
                    Space::new().height(DIALOG_SPACING),
                    text(fl!(crate::LANGUAGE_LOADER, "settings-modem-nothing_selected"))
                        .size(TEXT_SIZE_NORMAL)
                        .style(|theme: &iced::Theme| text::Style {
                            color: Some(theme.extended_palette().background.strong.text),
                        }),
                    Space::new().height(4),
                    text(fl!(crate::LANGUAGE_LOADER, "settings-modem-no-selection-hint"))
                        .size(TEXT_SIZE_SMALL)
                        .style(|theme: &iced::Theme| text::Style {
                            color: Some(theme.extended_palette().background.strong.text.scale_alpha(0.7)),
                        }),
                ]
                .align_x(Alignment::Center),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
        };

        // Right panel with vertical separator
        let right_panel = container(modem_settings).width(Length::Fill).height(Length::Fill);

        // Main layout with separator
        row![left_panel, right_panel,].into()
    }
}
