use i18n_embed_fl::fl;
use iced::{
    widget::{button, checkbox, column, container, row, scrollable, svg, text, text_input, tooltip, Space},
    Alignment, Border, Color, Element, Length,
};
use icy_engine_gui::{
    section_header,
    settings::effect_box,
    ui::{left_label_small, secondary_button_style, DIALOG_SPACING, TEXT_SIZE_SMALL},
    SECTION_PADDING, TEXT_SIZE_NORMAL,
};
use icy_net::modem::ModemCommand;

use crate::ui::settings_dialog::{modem_command_input_generic, SettingsDialogMessage, SettingsDialogState};

const ADD_SVG: &[u8] = include_bytes!("../../../../data/icons/add.svg");
const DELETE_SVG: &[u8] = include_bytes!("../../../../data/icons/delete.svg");

impl SettingsDialogState {
    pub fn protocol_settings_content_generic<'a, M: Clone + 'static>(
        &self,
        on_message: impl Fn(SettingsDialogMessage) -> M + Clone + 'static,
    ) -> Element<'a, M> {
        let protocols = self.temp_options.lock().transfer_protocols.clone();
        let selected_index = self.selected_protocol_index;

        // Protocol list with styling similar to modem settings
        let mut protocol_list: iced::widget::Column<'_, M> = column![].spacing(2);
        for (idx, protocol) in protocols.iter().enumerate() {
            let is_selected = idx == selected_index;
            let protocol_name = protocol.get_name();
            let is_enabled = protocol.enabled;
            let on_msg = on_message.clone();

            let protocol_button = button(container(text(protocol_name).size(TEXT_SIZE_NORMAL)).width(Length::Fill).padding([8, 12]))
                .on_press(on_msg(SettingsDialogMessage::SelectProtocol(idx)))
                .width(Length::Fill)
                .style(move |theme: &iced::Theme, status| {
                    use iced::widget::button::{Status, Style};
                    let palette = theme.extended_palette();
                    let text_color = if is_enabled {
                        if is_selected {
                            palette.primary.weak.text
                        } else {
                            palette.background.base.text
                        }
                    } else {
                        palette.background.strong.text.scale_alpha(0.5)
                    };

                    let base = if is_selected {
                        Style {
                            background: Some(iced::Background::Color(palette.primary.weak.color)),
                            text_color,
                            border: Border::default().rounded(6.0),
                            shadow: Default::default(),
                            snap: false,
                        }
                    } else {
                        Style {
                            background: Some(iced::Background::Color(Color::TRANSPARENT)),
                            text_color,
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
            protocol_list = protocol_list.push(protocol_button);
        }

        // Icon buttons for Add/Remove
        let add_icon = svg(svg::Handle::from_memory(ADD_SVG)).width(Length::Fixed(18.0)).height(Length::Fixed(18.0));
        let delete_icon = svg(svg::Handle::from_memory(DELETE_SVG)).width(Length::Fixed(18.0)).height(Length::Fixed(18.0));

        let on_msg = on_message.clone();
        let add_button = tooltip(
            button(add_icon)
                .on_press(on_msg(SettingsDialogMessage::AddProtocol))
                .padding(6)
                .style(secondary_button_style),
            container(text(fl!(crate::LANGUAGE_LOADER, "settings-protocol-add-tooltip")).size(TEXT_SIZE_SMALL))
                .style(container::rounded_box)
                .padding(8),
            tooltip::Position::Top,
        )
        .gap(8);

        let can_remove = !protocols.is_empty() && protocols.get(selected_index).map_or(false, |p| !p.is_internal());
        let on_msg = on_message.clone();
        let remove_button = if can_remove {
            tooltip(
                button(delete_icon)
                    .on_press(on_msg(SettingsDialogMessage::RemoveProtocol(selected_index)))
                    .padding(6)
                    .style(secondary_button_style),
                container(text(fl!(crate::LANGUAGE_LOADER, "settings-protocol-remove-tooltip")).size(TEXT_SIZE_SMALL))
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
                container(text(fl!(crate::LANGUAGE_LOADER, "settings-protocol-remove-tooltip")).size(TEXT_SIZE_SMALL))
                    .style(container::rounded_box)
                    .padding(8),
                tooltip::Position::Top,
            )
            .gap(8)
        };

        // Text-based buttons for Move Up/Down
        let can_move_up = selected_index > 0;
        let on_msg = on_message.clone();
        let move_up_button = if can_move_up {
            tooltip(
                button(text("▲").size(TEXT_SIZE_NORMAL))
                    .on_press(on_msg(SettingsDialogMessage::MoveProtocolUp(selected_index)))
                    .padding(6)
                    .style(secondary_button_style),
                container(text(fl!(crate::LANGUAGE_LOADER, "settings-protocol-move-up-tooltip")).size(TEXT_SIZE_SMALL))
                    .style(container::rounded_box)
                    .padding(8),
                tooltip::Position::Top,
            )
            .gap(8)
        } else {
            tooltip(
                button(text("▲").size(TEXT_SIZE_NORMAL)).padding(6).style(secondary_button_style),
                container(text(fl!(crate::LANGUAGE_LOADER, "settings-protocol-move-up-tooltip")).size(TEXT_SIZE_SMALL))
                    .style(container::rounded_box)
                    .padding(8),
                tooltip::Position::Top,
            )
            .gap(8)
        };

        let can_move_down = selected_index < protocols.len().saturating_sub(1);
        let on_msg = on_message.clone();
        let move_down_button = if can_move_down {
            tooltip(
                button(text("▼").size(TEXT_SIZE_NORMAL))
                    .on_press(on_msg(SettingsDialogMessage::MoveProtocolDown(selected_index)))
                    .padding(6)
                    .style(secondary_button_style),
                container(text(fl!(crate::LANGUAGE_LOADER, "settings-protocol-move-down-tooltip")).size(TEXT_SIZE_SMALL))
                    .style(container::rounded_box)
                    .padding(8),
                tooltip::Position::Top,
            )
            .gap(8)
        } else {
            tooltip(
                button(text("▼").size(TEXT_SIZE_NORMAL)).padding(6).style(secondary_button_style),
                container(text(fl!(crate::LANGUAGE_LOADER, "settings-protocol-move-down-tooltip")).size(TEXT_SIZE_SMALL))
                    .style(container::rounded_box)
                    .padding(8),
                tooltip::Position::Top,
            )
            .gap(8)
        };

        let list_container = container(scrollable(protocol_list).direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default())))
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
            section_header(fl!(crate::LANGUAGE_LOADER, "settings-protocol-list-section")),
            list_container,
            Space::new().height(DIALOG_SPACING),
            row![
                add_button,
                Space::new().width(DIALOG_SPACING),
                remove_button,
                Space::new().width(DIALOG_SPACING * 2.0),
                move_up_button,
                Space::new().width(DIALOG_SPACING),
                move_down_button,
                Space::new().width(Length::Fill),
            ]
            .align_y(Alignment::Center),
        ])
        .width(Length::Fixed(200.0))
        .height(Length::Fill);

        // Get the selected protocol and clone its data
        let selected_protocol = protocols.get(selected_index).cloned();

        let protocol_settings: Element<'_, M> = if let Some(protocol) = selected_protocol {
            let is_internal = protocol.is_internal();

            // Use cloned values directly
            let protocol_id = protocol.id.clone();
            let protocol_name = protocol.name.clone();
            let protocol_description = protocol.description.clone();
            let protocol_enabled = protocol.enabled;
            let protocol_batch = protocol.batch;
            let protocol_auto_transfer = protocol.auto_transfer;
            let protocol_send_command = protocol.send_command.clone();
            let protocol_recv_command = protocol.recv_command.clone();
            let protocol_download_signature = protocol.download_signature.clone();
            let protocol_upload_signature = protocol.upload_signature.clone();

            // Clone temp_options for closures
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

            // Build ID row based on whether protocol is internal
            let id_row: Element<'_, M> = if is_internal {
                row![
                    left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-protocol-id")),
                    text(protocol_id.clone()).size(TEXT_SIZE_NORMAL),
                ]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center)
                .into()
            } else {
                row![
                    left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-protocol-id")),
                    text_input("", &protocol_id)
                        .on_input(move |value| {
                            let mut new_options = temp_opts1.lock().clone();
                            if let Some(p) = new_options.transfer_protocols.get_mut(selected_index) {
                                p.id = value;
                            }
                            on_msg1(SettingsDialogMessage::UpdateOptions(new_options))
                        })
                        .width(Length::Fill)
                        .size(TEXT_SIZE_NORMAL),
                ]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center)
                .into()
            };

            // Build name row (only for external protocols)
            let name_row: Element<'_, M> = if !is_internal {
                row![
                    left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-protocol-name")),
                    text_input("", &protocol_name)
                        .on_input(move |value| {
                            let mut new_options = temp_opts2.lock().clone();
                            if let Some(p) = new_options.transfer_protocols.get_mut(selected_index) {
                                p.name = value;
                            }
                            on_msg2(SettingsDialogMessage::UpdateOptions(new_options))
                        })
                        .width(Length::Fill)
                        .size(TEXT_SIZE_NORMAL),
                ]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center)
                .into()
            } else {
                Space::new().height(0).into()
            };

            // Build description row (only for external protocols)
            let description_row: Element<'_, M> = if !is_internal {
                row![
                    left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-protocol-description")),
                    text_input("", &protocol_description)
                        .on_input(move |value| {
                            let mut new_options = temp_opts3.lock().clone();
                            if let Some(p) = new_options.transfer_protocols.get_mut(selected_index) {
                                p.description = value;
                            }
                            on_msg3(SettingsDialogMessage::UpdateOptions(new_options))
                        })
                        .width(Length::Fill)
                        .size(TEXT_SIZE_NORMAL),
                ]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center)
                .into()
            } else {
                Space::new().height(0).into()
            };

            // Build internal protocol hint
            let internal_hint: Element<'_, M> = if is_internal {
                text(fl!(crate::LANGUAGE_LOADER, "settings-protocol-internal-hint"))
                    .size(TEXT_SIZE_SMALL)
                    .style(|theme: &iced::Theme| text::Style {
                        color: Some(theme.extended_palette().background.strong.text.scale_alpha(0.7)),
                    })
                    .into()
            } else {
                Space::new().height(0).into()
            };

            scrollable(
                column![
                    // General section
                    section_header(fl!(crate::LANGUAGE_LOADER, "settings-protocol-general-section")),
                    effect_box(
                        column![
                            // Enabled checkbox
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-protocol-enabled")),
                                checkbox(protocol_enabled)
                                    .on_toggle(move |value| {
                                        let mut new_options = temp_opts4.lock().clone();
                                        if let Some(p) = new_options.transfer_protocols.get_mut(selected_index) {
                                            p.enabled = value;
                                        }
                                        on_msg4(SettingsDialogMessage::UpdateOptions(new_options))
                                    })
                                    .size(18),
                            ]
                            .spacing(DIALOG_SPACING)
                            .align_y(Alignment::Center),
                            id_row,
                            name_row,
                            description_row,
                            // Batch transfer checkbox
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-protocol-batch")),
                                checkbox(protocol_batch)
                                    .on_toggle(move |value| {
                                        let mut new_options = temp_opts5.lock().clone();
                                        if let Some(p) = new_options.transfer_protocols.get_mut(selected_index) {
                                            p.batch = value;
                                        }
                                        on_msg5(SettingsDialogMessage::UpdateOptions(new_options))
                                    })
                                    .size(18),
                            ]
                            .spacing(DIALOG_SPACING)
                            .align_y(Alignment::Center),
                        ]
                        .spacing(DIALOG_SPACING)
                        .into()
                    ),
                    Space::new().height(DIALOG_SPACING * 2.0),
                    // Commands section
                    section_header(fl!(crate::LANGUAGE_LOADER, "settings-protocol-commands-section")),
                    effect_box(
                        column![
                            internal_hint,
                            // Send command
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-protocol-send-command")),
                                text_input("", &protocol_send_command)
                                    .on_input(move |value| {
                                        let mut new_options = temp_opts6.lock().clone();
                                        if let Some(p) = new_options.transfer_protocols.get_mut(selected_index) {
                                            p.send_command = value;
                                        }
                                        on_msg6(SettingsDialogMessage::UpdateOptions(new_options))
                                    })
                                    .width(Length::Fill)
                                    .size(TEXT_SIZE_NORMAL),
                            ]
                            .spacing(DIALOG_SPACING)
                            .align_y(Alignment::Center),
                            // Receive command
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-protocol-recv-command")),
                                text_input("", &protocol_recv_command)
                                    .on_input(move |value| {
                                        let mut new_options = temp_opts7.lock().clone();
                                        if let Some(p) = new_options.transfer_protocols.get_mut(selected_index) {
                                            p.recv_command = value;
                                        }
                                        on_msg7(SettingsDialogMessage::UpdateOptions(new_options))
                                    })
                                    .width(Length::Fill)
                                    .size(TEXT_SIZE_NORMAL),
                            ]
                            .spacing(DIALOG_SPACING)
                            .align_y(Alignment::Center),
                            // Auto transfer checkbox
                            row![
                                left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-protocol-auto-transfer")),
                                checkbox(protocol_auto_transfer)
                                    .on_toggle(move |value| {
                                        let mut new_options = temp_opts8.lock().clone();
                                        if let Some(p) = new_options.transfer_protocols.get_mut(selected_index) {
                                            p.auto_transfer = value;
                                        }
                                        on_msg8(SettingsDialogMessage::UpdateOptions(new_options))
                                    })
                                    .size(18),
                            ]
                            .spacing(DIALOG_SPACING)
                            .align_y(Alignment::Center),
                            // Download signature - only enabled when auto_transfer is true
                            if protocol_auto_transfer {
                                modem_command_input_generic(
                                    fl!(crate::LANGUAGE_LOADER, "settings-protocol-download-signature"),
                                    "",
                                    &protocol_download_signature,
                                    move |value| {
                                        let mut new_options = temp_opts9.lock().clone();
                                        if let Some(p) = new_options.transfer_protocols.get_mut(selected_index) {
                                            if let Ok(cmd) = value.parse::<ModemCommand>() {
                                                p.download_signature = cmd;
                                            }
                                        }
                                        on_msg9(SettingsDialogMessage::UpdateOptions(new_options))
                                    },
                                )
                            } else {
                                row![
                                    left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-protocol-download-signature")),
                                    text_input("", &protocol_download_signature.to_string())
                                        .width(Length::Fill)
                                        .size(TEXT_SIZE_NORMAL),
                                ]
                                .spacing(DIALOG_SPACING)
                                .align_y(Alignment::Center)
                                .into()
                            },
                            // Upload signature - only enabled when auto_transfer is true
                            if protocol_auto_transfer {
                                modem_command_input_generic(
                                    fl!(crate::LANGUAGE_LOADER, "settings-protocol-upload-signature"),
                                    "",
                                    &protocol_upload_signature,
                                    move |value| {
                                        let mut new_options = temp_opts10.lock().clone();
                                        if let Some(p) = new_options.transfer_protocols.get_mut(selected_index) {
                                            if let Ok(cmd) = value.parse::<ModemCommand>() {
                                                p.upload_signature = cmd;
                                            }
                                        }
                                        on_msg10(SettingsDialogMessage::UpdateOptions(new_options))
                                    },
                                )
                            } else {
                                row![
                                    left_label_small(fl!(crate::LANGUAGE_LOADER, "settings-protocol-upload-signature")),
                                    text_input("", &protocol_upload_signature.to_string())
                                        .width(Length::Fill)
                                        .size(TEXT_SIZE_NORMAL),
                                ]
                                .spacing(DIALOG_SPACING)
                                .align_y(Alignment::Center)
                                .into()
                            },
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
            // Empty state
            container(
                column![
                    text("📦").size(48),
                    Space::new().height(DIALOG_SPACING),
                    text(fl!(crate::LANGUAGE_LOADER, "settings-protocol-nothing-selected"))
                        .size(TEXT_SIZE_NORMAL)
                        .style(|theme: &iced::Theme| text::Style {
                            color: Some(theme.extended_palette().background.strong.text),
                        }),
                    Space::new().height(4),
                    text(fl!(crate::LANGUAGE_LOADER, "settings-protocol-no-selection-hint"))
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

        // Right panel
        let right_panel = container(protocol_settings).width(Length::Fill).height(Length::Fill);

        // Main layout
        row![left_panel, right_panel,].into()
    }
}
