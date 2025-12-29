use i18n_embed_fl::fl;
use iced::{
    widget::{button, column, container, pick_list, row, scrollable, svg, text, text_input, tooltip, Space},
    Alignment, Border, Color, Element, Length,
};
use icy_engine_gui::{
    section_header,
    settings::effect_box,
    ui::{left_label_small, secondary_button_style, DIALOG_SPACING, TEXT_SIZE_SMALL},
    SECTION_PADDING, TEXT_SIZE_NORMAL,
};
use icy_net::modem::ModemCommand;

use crate::ui::{
    select_bps_dialog::STANDARD_RATES,
    settings_dialog::{
        modem_command_input_generic, CharSizeOption, FlowControlOption, ParityOption, SettingsDialogMessage, SettingsDialogState, StopBitsOption,
    },
};

const ADD_SVG: &[u8] = include_bytes!("../../../../data/icons/add.svg");
const DELETE_SVG: &[u8] = include_bytes!("../../../../data/icons/delete.svg");

impl SettingsDialogState {
    pub fn modem_settings_content_generic<'a, M: Clone + 'static>(&self, on_message: impl Fn(SettingsDialogMessage) -> M + Clone + 'static) -> Element<'a, M> {
        let modems = self.temp_options.lock().modems.clone();
        let selected_index = self.selected_modem_index;

        // Modem list with better styling
        let mut modem_list: iced::widget::Column<'_, M> = column![].spacing(2);
        for (idx, modem) in modems.iter().enumerate() {
            let is_selected = idx == selected_index;
            let modem_name = modem.name.clone();
            let on_msg = on_message.clone();
            let modem_button = button(container(text(modem_name).size(TEXT_SIZE_NORMAL)).width(Length::Fill).padding([8, 12]))
                .on_press(on_msg(SettingsDialogMessage::SelectModem(idx)))
                .width(Length::Fill)
                .style(move |theme: &iced::Theme, status| {
                    use iced::widget::button::{Status, Style};
                    let base = if is_selected {
                        Style {
                            background: Some(iced::Background::Color(theme.accent.selected)),
                            text_color: theme.accent.on,
                            border: Border::default().rounded(6.0),
                            shadow: Default::default(),
                            snap: false,
                        }
                    } else {
                        Style {
                            background: Some(iced::Background::Color(Color::TRANSPARENT)),
                            text_color: theme.background.on,
                            border: Border::default().rounded(6.0),
                            shadow: Default::default(),
                            snap: false,
                        }
                    };

                    match status {
                        Status::Active => base,
                        Status::Hovered if !is_selected => Style {
                            background: Some(iced::Background::Color(theme.secondary.base)),
                            ..base
                        },
                        Status::Pressed => Style {
                            background: Some(iced::Background::Color(theme.accent.hover)),
                            text_color: theme.accent.on,
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

        let on_msg = on_message.clone();
        let add_button = tooltip(
            button(add_icon)
                .on_press(on_msg(SettingsDialogMessage::AddModem))
                .padding(6)
                .style(secondary_button_style),
            container(text(fl!(crate::LANGUAGE_LOADER, "settings-modem-add-tooltip")).size(TEXT_SIZE_SMALL))
                .style(container::rounded_box)
                .padding(8),
            tooltip::Position::Top,
        )
        .gap(8);

        let on_msg = on_message.clone();
        let remove_button = if !modems.is_empty() {
            tooltip(
                button(delete_icon)
                    .on_press(on_msg(SettingsDialogMessage::RemoveModem(selected_index)))
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
                background: Some(iced::Background::Color(theme.secondary.base)),
                border: Border {
                    color: theme.primary.divider,
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

        let modem_settings: Element<'_, M> = if let Some(modem) = modems.get(selected_index) {
            // Filter only Some(...) rates
            let baud_options: Vec<String> = STANDARD_RATES.iter().filter_map(|r| r.as_ref()).map(|r| r.to_string()).collect();

            let current_baud = modem.baud_rate.to_string();
            let selected_baud = if baud_options.contains(&current_baud) {
                Some(current_baud.clone())
            } else {
                None
            };

            // Clone values for closures
            let modem_name = modem.name.clone();
            let modem_device = modem.device.clone();
            let modem_baud_rate = modem.baud_rate;
            let modem_char_size = modem.format.char_size;
            let modem_parity = modem.format.parity;
            let modem_stop_bits = modem.format.stop_bits;
            let modem_flow_control = modem.flow_control;
            let modem_init_command = modem.init_command.clone();
            let modem_dial_prefix = modem.dial_prefix.clone();
            let modem_dial_suffix = modem.dial_suffix.clone();
            let modem_hangup_command = modem.hangup_command.clone();

            // Create closures with on_message clones
            let temp_opts1 = self.temp_options.clone();
            let temp_opts2 = self.temp_options.clone();
            let temp_opts3 = self.temp_options.clone();
            let temp_opts4 = self.temp_options.clone();
            let temp_opts5 = self.temp_options.clone();
            let temp_opts6 = self.temp_options.clone();
            let temp_opts7 = self.temp_options.clone();
            let temp_opts8 = self.temp_options.clone();
            let temp_opts9 = self.temp_options.clone();
            let temp_opts10 = self.temp_options.clone();
            let temp_opts11 = self.temp_options.clone();
            let temp_opts12 = self.temp_options.clone();
            let _temp_opts13 = self.temp_options.clone();

            let on_msg1 = on_message.clone();
            let on_msg2 = on_message.clone();
            let on_msg3 = on_message.clone();
            let on_msg4 = on_message.clone();
            let on_msg5 = on_message.clone();
            let on_msg6 = on_message.clone();
            let on_msg7 = on_message.clone();
            let on_msg8 = on_message.clone();
            let on_msg9 = on_message.clone();
            let on_msg10 = on_message.clone();
            let on_msg11 = on_message.clone();
            let _on_msg12 = on_message.clone();
            let on_msg13 = on_message.clone();
            let _on_msg14 = on_message.clone();
            let on_msg15 = on_message.clone();
            let _on_msg16 = on_message.clone();
            let on_msg17 = on_message.clone();

            scrollable(
                column![
                    // Device section
                    section_header(fl!(crate::LANGUAGE_LOADER, "settings-modem-device-section")),
                    effect_box(
                        column![
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-modem-name")),
                                text_input("", &modem_name)
                                    .on_input(move |value| {
                                        let mut new_options = temp_opts1.lock().clone();
                                        if let Some(m) = new_options.modems.get_mut(selected_index) {
                                            m.name = value;
                                        }
                                        on_msg1(SettingsDialogMessage::UpdateOptions(new_options))
                                    })
                                    .width(Length::Fill)
                                    .size(TEXT_SIZE_NORMAL),
                            ]
                            .spacing(DIALOG_SPACING)
                            .align_y(Alignment::Center),
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-modem-device")),
                                text_input("", &modem_device)
                                    .on_input(move |value| {
                                        let mut new_options = temp_opts2.lock().clone();
                                        if let Some(m) = new_options.modems.get_mut(selected_index) {
                                            m.device = value;
                                        }
                                        on_msg2(SettingsDialogMessage::UpdateOptions(new_options))
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
                                text_input("", &modem_baud_rate.to_string())
                                    .on_input(move |value| {
                                        if let Ok(baud) = value.parse::<u32>() {
                                            let mut new_options = temp_opts3.lock().clone();
                                            if let Some(m) = new_options.modems.get_mut(selected_index) {
                                                m.baud_rate = baud;
                                            }
                                            on_msg3(SettingsDialogMessage::UpdateOptions(new_options))
                                        } else {
                                            on_msg4(SettingsDialogMessage::Noop)
                                        }
                                    })
                                    .width(Length::Fixed(100.0))
                                    .size(TEXT_SIZE_NORMAL),
                                pick_list(baud_options, selected_baud, move |value| {
                                    if let Ok(baud) = value.parse::<u32>() {
                                        let mut new_options = temp_opts4.lock().clone();
                                        if let Some(m) = new_options.modems.get_mut(selected_index) {
                                            m.baud_rate = baud;
                                        }
                                        on_msg5(SettingsDialogMessage::UpdateOptions(new_options))
                                    } else {
                                        on_msg6(SettingsDialogMessage::Noop)
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
                                pick_list(&CharSizeOption::ALL[..], Some(CharSizeOption::from(modem_char_size)), move |value| {
                                    let mut new_options = temp_opts5.lock().clone();
                                    if let Some(m) = new_options.modems.get_mut(selected_index) {
                                        m.format.char_size = value.into();
                                    }
                                    on_msg7(SettingsDialogMessage::UpdateOptions(new_options))
                                })
                                .width(Length::Fixed(60.0))
                                .text_size(TEXT_SIZE_NORMAL),
                                text("-").size(TEXT_SIZE_NORMAL),
                                pick_list(&ParityOption::ALL[..], Some(ParityOption::from(modem_parity)), move |value| {
                                    let mut new_options = temp_opts6.lock().clone();
                                    if let Some(m) = new_options.modems.get_mut(selected_index) {
                                        m.format.parity = value.into();
                                    }
                                    on_msg8(SettingsDialogMessage::UpdateOptions(new_options))
                                })
                                .width(Length::Fixed(70.0))
                                .text_size(TEXT_SIZE_NORMAL),
                                text("-").size(TEXT_SIZE_NORMAL),
                                pick_list(&StopBitsOption::ALL[..], Some(StopBitsOption::from(modem_stop_bits)), move |value| {
                                    let mut new_options = temp_opts7.lock().clone();
                                    if let Some(m) = new_options.modems.get_mut(selected_index) {
                                        m.format.stop_bits = value.into();
                                    }
                                    on_msg9(SettingsDialogMessage::UpdateOptions(new_options))
                                })
                                .width(Length::Fixed(50.0))
                                .text_size(TEXT_SIZE_NORMAL),
                            ]
                            .spacing(DIALOG_SPACING)
                            .align_y(Alignment::Center),
                            // Flow control
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-modem-flow_control")),
                                pick_list(&FlowControlOption::ALL[..], Some(FlowControlOption::from(modem_flow_control)), move |value| {
                                    let mut new_options = temp_opts8.lock().clone();
                                    if let Some(m) = new_options.modems.get_mut(selected_index) {
                                        m.flow_control = value.into();
                                    }
                                    on_msg10(SettingsDialogMessage::UpdateOptions(new_options))
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
                            modem_command_input_generic(fl!(crate::LANGUAGE_LOADER, "settings-modem-init_command"), "ATZ^M", &modem_init_command, {
                                move |value| {
                                    let mut new_options = temp_opts9.lock().clone();
                                    if let Some(m) = new_options.modems.get_mut(selected_index) {
                                        if let Ok(cmd) = value.parse::<ModemCommand>() {
                                            m.init_command = cmd;
                                        }
                                    }
                                    on_msg11(SettingsDialogMessage::UpdateOptions(new_options))
                                }
                            }),
                            modem_command_input_generic(fl!(crate::LANGUAGE_LOADER, "settings-modem-dial_prefix"), "ATDT", &modem_dial_prefix, {
                                move |value| {
                                    let mut new_options = temp_opts10.lock().clone();
                                    if let Some(m) = new_options.modems.get_mut(selected_index) {
                                        if let Ok(cmd) = value.parse::<ModemCommand>() {
                                            m.dial_prefix = cmd;
                                        }
                                    }
                                    on_msg13(SettingsDialogMessage::UpdateOptions(new_options))
                                }
                            }),
                            modem_command_input_generic(fl!(crate::LANGUAGE_LOADER, "settings-modem-dial_suffix"), "^M", &modem_dial_suffix, {
                                move |value| {
                                    let mut new_options = temp_opts11.lock().clone();
                                    if let Some(m) = new_options.modems.get_mut(selected_index) {
                                        if let Ok(cmd) = value.parse::<ModemCommand>() {
                                            m.dial_suffix = cmd;
                                        }
                                    }
                                    on_msg15(SettingsDialogMessage::UpdateOptions(new_options))
                                }
                            }),
                            modem_command_input_generic(
                                fl!(crate::LANGUAGE_LOADER, "settings-modem-hangup_command"),
                                "+++ATH0^M",
                                &modem_hangup_command,
                                {
                                    move |value| {
                                        let mut new_options = temp_opts12.lock().clone();
                                        if let Some(m) = new_options.modems.get_mut(selected_index) {
                                            if let Ok(cmd) = value.parse::<ModemCommand>() {
                                                m.hangup_command = cmd;
                                            }
                                        }
                                        on_msg17(SettingsDialogMessage::UpdateOptions(new_options))
                                    }
                                }
                            ),
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
                        .style(|theme: &iced::Theme| text::Style { color: Some(theme.primary.on) }),
                    Space::new().height(4),
                    text(fl!(crate::LANGUAGE_LOADER, "settings-modem-no-selection-hint"))
                        .size(TEXT_SIZE_SMALL)
                        .style(|theme: &iced::Theme| text::Style {
                            color: Some(theme.primary.on.scale_alpha(0.7)),
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
