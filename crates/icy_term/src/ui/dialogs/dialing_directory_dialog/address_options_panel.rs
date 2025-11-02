use crate::VGA_MODES;
use crate::ui::Message;
use crate::ui::dialing_directory_dialog::{AddressFieldChange, DialingDirectoryMsg};
use i18n_embed_fl::fl;
use iced::widget::{space, tooltip};
use iced::{
    Alignment, Element, Length,
    widget::{Column, Space, button, checkbox, column, container, pick_list, row, rule, scrollable, svg, text, text_input},
};
use icy_engine::ansi::{BaudEmulation, MusicOption};
use icy_net::{ConnectionType, telnet::TerminalEmulation};
use once_cell::sync::Lazy;
use std::fmt;

static COMMENT_PLACEHOLDER: Lazy<String> = Lazy::new(|| fl!(crate::LANGUAGE_LOADER, "dialing_directory-comment-placeholder"));

const VISIBILITY_SVG: &[u8] = include_bytes!("../../../../data/icons/visibility.svg");
const VISIBILITY_OFF_SVG: &[u8] = include_bytes!("../../../../data/icons/visibility_off.svg");

// Wrapper types to implement Display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionTypeWrapper(pub ConnectionType);

impl fmt::Display for ConnectionTypeWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            ConnectionType::Telnet => write!(f, "Telnet"),
            ConnectionType::Raw => write!(f, "Raw"),
            ConnectionType::Modem => write!(f, "Modem"),
            ConnectionType::SSH => write!(f, "SSH"),
            ConnectionType::Websocket => write!(f, "WebSocket"),
            ConnectionType::SecureWebsocket => write!(f, "Secure WebSocket"),
            ConnectionType::Channel => write!(f, "Channel"),
            ConnectionType::Serial => write!(f, "Serial"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalEmulationWrapper(pub TerminalEmulation);

impl fmt::Display for TerminalEmulationWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            TerminalEmulation::Ansi => write!(f, "ANSI"),
            TerminalEmulation::Utf8Ansi => write!(f, "UTF8ANSI"),
            TerminalEmulation::Ascii => write!(f, "ASCII"),
            TerminalEmulation::Avatar => write!(f, "Avatar"),
            TerminalEmulation::PETscii => write!(f, "PETSCII"),
            TerminalEmulation::ATAscii => write!(f, "ATASCII"),
            TerminalEmulation::ViewData => write!(f, "ViewData"),
            TerminalEmulation::Mode7 => write!(f, "Mode 7"),
            TerminalEmulation::AtariST => write!(f, "Atari ST"),
            TerminalEmulation::Rip => write!(f, "RIP"),
            TerminalEmulation::Skypix => write!(f, "SkyPix"),
        }
    }
}

impl super::DialingDirectoryState {
    pub fn create_option_panel(&self, options: &crate::Options) -> Element<'_, Message> {
        let addr = if let Some(addr_idx) = self.selected_bbs {
            self.addresses.addresses[addr_idx].clone()
        } else {
            self.quick_connect_address.clone()
        };
        let is_quick = self.selected_bbs.is_none();
        let id = self.selected_bbs;
        // Header with system name and star button
        let header = {
            let name_input = text_input(&fl!(crate::LANGUAGE_LOADER, "dialing_directory-name-placeholder"), &addr.system_name)
                .on_input(move |s| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::SystemName(s),
                    })
                })
                .padding(6)
                .size(18)
                .width(Length::Fill);

            let star_btn: button::Button<'_, Message> = button(text(if addr.is_favored { "★" } else { "☆" }))
                .on_press(Message::from(DialingDirectoryMsg::ToggleFavorite(if let Some(addr_idx) = self.selected_bbs {
                    addr_idx
                } else {
                    0
                })))
                .padding(4)
                .style(button::text);

            // Info section
            let info_section = {
                let calls = addr.number_of_calls;
                let last_call_text = match addr.last_call {
                    Some(dt) => {
                        let local: chrono::DateTime<chrono::Local> = chrono::DateTime::from(dt);
                        local
                            .format(&fl!(crate::LANGUAGE_LOADER, "dialing_directory-last-call-date-format"))
                            .to_string()
                    }
                    None => fl!(crate::LANGUAGE_LOADER, "dialing_directory-not-called"),
                };

                column![
                    row![
                        container(text(format!("✆ {calls}")).style(|theme: &iced::Theme| iced::widget::text::Style {
                            color: Some(theme.extended_palette().secondary.base.color),
                            ..Default::default()
                        })),
                        Space::new().width(Length::Fill),
                        container(text(last_call_text).style(|theme: &iced::Theme| iced::widget::text::Style {
                            color: Some(theme.extended_palette().secondary.base.color),
                            ..Default::default()
                        })),
                    ]
                    .spacing(20)
                ]
                .spacing(4)
            };

            let mut cols = column![row![name_input, star_btn].spacing(8).align_y(Alignment::Center)].spacing(4);

            if !is_quick {
                cols = cols.push(info_section);
            }

            cols
        };
        // Create a table-like layout with consistent column widths
        let label_width = Length::Fixed(120.0);
        // Server settings
        let server_section = {
            let mut rows = vec![];

            // Address/Modem row - changes based on protocol
            if addr.protocol == ConnectionType::Modem {
                // For modem protocol, show modem picker
                let modem_names: Vec<String> = options.modems.iter().map(|m| m.name.clone()).collect();

                // Check if current address matches any modem name
                let selected_modem = modem_names.iter().position(|name| name == &addr.address);

                let modem_picker: pick_list::PickList<'_, String, Vec<String>, String, Message> =
                    pick_list(modem_names.clone(), selected_modem.map(|idx| modem_names[idx].clone()), move |modem_name| {
                        Message::from(DialingDirectoryMsg::AddressFieldChanged {
                            id,
                            field: AddressFieldChange::Address(modem_name),
                        })
                    })
                    .placeholder("Select Modem")
                    .width(Length::Fill);

                let mut modem_row = row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-modem")))
                        .align_x(Alignment::End)
                        .width(label_width),
                    modem_picker,
                ]
                .spacing(8)
                .align_y(Alignment::Center);

                // Show warning if modem name doesn't match any configured modem
                if selected_modem.is_none() && !addr.address.is_empty() {
                    let err_msg = if modem_names.is_empty() {
                        fl!(crate::LANGUAGE_LOADER, "dialing_directory-no_modem_configured")
                    } else {
                        fl!(crate::LANGUAGE_LOADER, "dialing_directory-invalid_modem")
                    };
                    modem_row = modem_row.push(
                        container(text(format!("⚠ {}", err_msg)).size(12).style(|theme: &iced::Theme| iced::widget::text::Style {
                            color: Some(theme.extended_palette().danger.base.color),
                            ..Default::default()
                        }))
                        .padding([0, 8]),
                    );
                }

                rows.push(modem_row);
            } else {
                // For other protocols, show address input
                let address_field = text_input("", &addr.address)
                    .on_input(move |s| {
                        Message::from(DialingDirectoryMsg::AddressFieldChanged {
                            id,
                            field: AddressFieldChange::Address(s),
                        })
                    })
                    .padding(6)
                    .width(Length::Fill);

                rows.push(
                    row![
                        container(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-address")))
                            .align_x(Alignment::End)
                            .width(label_width),
                        address_field
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                );
            }

            // Protocol picker
            let protocols = vec![
                ConnectionTypeWrapper(ConnectionType::Telnet),
                ConnectionTypeWrapper(ConnectionType::Raw),
                ConnectionTypeWrapper(ConnectionType::Modem),
                ConnectionTypeWrapper(ConnectionType::SSH),
                ConnectionTypeWrapper(ConnectionType::Websocket),
                ConnectionTypeWrapper(ConnectionType::SecureWebsocket),
            ];

            let current_protocol = ConnectionTypeWrapper(addr.protocol);

            let protocol_pick = pick_list(protocols, Some(current_protocol), move |p: ConnectionTypeWrapper| {
                Message::from(DialingDirectoryMsg::AddressFieldChanged {
                    id,
                    field: AddressFieldChange::Protocol(p.0),
                })
            })
            .placeholder(&fl!(crate::LANGUAGE_LOADER, "dialing_directory-protocol"))
            .width(Length::Fixed(150.0));

            let baud_pick = pick_list(BaudEmulation::OPTIONS.to_vec(), Some(addr.baud_emulation), move |b| {
                Message::from(DialingDirectoryMsg::AddressFieldChanged {
                    id,
                    field: AddressFieldChange::Baud(b),
                })
            })
            .placeholder(&fl!(crate::LANGUAGE_LOADER, "dialing_directory-baud-emulation"))
            .width(Length::Fixed(150.0));

            // Protocol & Baud row - conditionally show baud picker
            let mut protocol_row = row![
                container(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-protocol")))
                    .align_x(Alignment::End)
                    .width(label_width),
                protocol_pick,
            ]
            .spacing(8)
            .align_y(Alignment::Center);

            // Only show baud picker if protocol is NOT Modem
            if addr.protocol != ConnectionType::Modem {
                protocol_row = protocol_row.push(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-baud-emulation")));
                protocol_row = protocol_row.push(baud_pick);
            }

            protocol_row = protocol_row.push(Space::new().width(Length::Fill));
            rows.push(protocol_row);

            let terms = vec![
                TerminalEmulationWrapper(TerminalEmulation::Ansi),
                TerminalEmulationWrapper(TerminalEmulation::Utf8Ansi),
                TerminalEmulationWrapper(TerminalEmulation::Ascii),
                TerminalEmulationWrapper(TerminalEmulation::Avatar),
                TerminalEmulationWrapper(TerminalEmulation::PETscii),
                TerminalEmulationWrapper(TerminalEmulation::ATAscii),
                TerminalEmulationWrapper(TerminalEmulation::ViewData),
                TerminalEmulationWrapper(TerminalEmulation::Mode7),
                TerminalEmulationWrapper(TerminalEmulation::AtariST),
                TerminalEmulationWrapper(TerminalEmulation::Rip),
            ];

            let current_terminal = TerminalEmulationWrapper(addr.terminal_type);

            let term_pick = pick_list(terms, Some(current_terminal), move |t: TerminalEmulationWrapper| {
                Message::from(DialingDirectoryMsg::AddressFieldChanged {
                    id,
                    field: AddressFieldChange::Terminal(t.0),
                })
            })
            .placeholder(&fl!(crate::LANGUAGE_LOADER, "dialing_directory-terminal_type"))
            .width(Length::Fixed(150.0));

            let modes = VGA_MODES.to_vec();
            let screen_mode_pick = pick_list(modes, Some(addr.screen_mode), move |sm| {
                Message::from(DialingDirectoryMsg::AddressFieldChanged {
                    id,
                    field: AddressFieldChange::ScreenMode(sm),
                })
            })
            .placeholder(&fl!(crate::LANGUAGE_LOADER, "dialing_directory-screen_mode"))
            .width(Length::Fixed(150.0));

            let music_options = vec![MusicOption::Off, MusicOption::Banana, MusicOption::Conflicting, MusicOption::Both];

            let music_pick = pick_list(music_options, Some(addr.ansi_music), move |m| {
                Message::from(DialingDirectoryMsg::AddressFieldChanged {
                    id,
                    field: AddressFieldChange::Music(m),
                })
            })
            .placeholder(&fl!(crate::LANGUAGE_LOADER, "dialing_directory-music-option"))
            .width(Length::Fixed(150.0));

            let mut column_row = row![
                container(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-terminal_type")))
                    .align_x(Alignment::End)
                    .width(label_width),
                term_pick,
            ]
            .spacing(8)
            .align_y(Alignment::Center);

            if addr.terminal_type == TerminalEmulation::Ansi
                || addr.terminal_type == TerminalEmulation::Utf8Ansi
                || addr.terminal_type == TerminalEmulation::Avatar
                || addr.terminal_type == TerminalEmulation::Ascii
            {
                column_row = column_row.push(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-screen_mode")));
                column_row = column_row.push(screen_mode_pick);
            }

            if addr.terminal_type == TerminalEmulation::Ansi || addr.terminal_type == TerminalEmulation::Utf8Ansi {
                column_row = column_row.push(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-music-option")));
                column_row = column_row.push(music_pick);
            }

            column_row = column_row.push(Space::new().width(Length::Fill));
            rows.push(column_row);

            let mut col = column![].spacing(8);

            for row in rows {
                col = col.push(row);
            }

            col
        };
        // Login settings
        let login_section = {
            let user_field = text_input("", &addr.user_name)
                .on_input(move |s| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::User(s),
                    })
                })
                .padding(6)
                .width(Length::Fill);

            let pw_field: text_input::TextInput<'_, Message> = text_input("", &addr.password)
                .on_input(move |s| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::Password(s),
                    })
                })
                .secure(!self.show_passwords)
                .padding(6)
                .width(Length::Fill);

            let auto_login_field = text_input("", &addr.auto_login)
                .on_input(move |s| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::AutoLogin(s),
                    })
                })
                .padding(6)
                .width(Length::Fill);

            let override_toggle = checkbox(
                fl!(crate::LANGUAGE_LOADER, "dialing_directory-custom-iemsi-login-data"),
                addr.override_iemsi_settings,
            )
            .on_toggle(move |v| {
                Message::from(DialingDirectoryMsg::AddressFieldChanged {
                    id,
                    field: AddressFieldChange::OverrideIemsi(v),
                })
            });

            // Use SVG icon for visibility toggle
            let visibility_icon = if self.show_passwords {
                svg(svg::Handle::from_memory(VISIBILITY_OFF_SVG))
            } else {
                svg(svg::Handle::from_memory(VISIBILITY_SVG))
            }
            .width(Length::Fixed(20.0))
            .height(Length::Fixed(20.0));

            let toggler_pw = button(visibility_icon)
                .on_press(Message::from(DialingDirectoryMsg::ToggleShowPasswords))
                .padding(4)
                .style(button::text);

            let (generate_btn, tooltip_label) = if addr.password.is_empty() {
                (
                    button(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-generate")))
                        .on_press(Message::from(DialingDirectoryMsg::GeneratePassword))
                        .padding(4),
                    fl!(crate::LANGUAGE_LOADER, "dialing_directory-generate-tooltip"),
                )
            } else {
                (
                    button(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-generate")))
                        .padding(4)
                        .style(button::secondary),
                    fl!(crate::LANGUAGE_LOADER, "dialing_directory-generate-disabled-tooltip"),
                )
            };

            let generate_btn: tooltip::Tooltip<'_, Message> = tooltip(
                generate_btn,
                container(text(tooltip_label)).style(container::rounded_box),
                tooltip::Position::Bottom,
            )
            .gap(10)
            .style(container::rounded_box)
            .padding(8);

            let mut col: Column<'_, Message> = column![
                row![text("Login").size(14), rule::horizontal(1)].spacing(8).align_y(Alignment::Center),
                row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-user")))
                        .width(label_width)
                        .align_x(Alignment::End),
                    user_field
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-password")))
                        .width(label_width)
                        .align_x(Alignment::End),
                    pw_field,
                    toggler_pw,
                    generate_btn
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-autologin")))
                        .width(label_width)
                        .align_x(Alignment::End),
                    auto_login_field
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                row![Space::new().width(label_width), override_toggle].spacing(8)
            ]
            .spacing(8);

            if addr.override_iemsi_settings {
                let iemsi_user = text_input("", &addr.iemsi_user)
                    .on_input(move |s| {
                        Message::from(DialingDirectoryMsg::AddressFieldChanged {
                            id,
                            field: AddressFieldChange::IemsiUser(s),
                        })
                    })
                    .padding(6)
                    .width(Length::Fill);

                let iemsi_pw = text_input("", &addr.iemsi_password)
                    .on_input(move |s| {
                        Message::from(DialingDirectoryMsg::AddressFieldChanged {
                            id,
                            field: AddressFieldChange::IemsiPassword(s),
                        })
                    })
                    .secure(!self.show_passwords)
                    .padding(6)
                    .width(Length::Fill);

                col = col
                    .push(
                        row![
                            container(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-user")))
                                .width(label_width)
                                .align_x(Alignment::End),
                            iemsi_user
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    )
                    .push(
                        row![
                            container(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-password")))
                                .width(label_width)
                                .align_x(Alignment::End),
                            iemsi_pw
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    );
            }

            col
        };
        // Comment section
        let comment_section = {
            let comment = addr.comment.clone();
            column![
                row![text("Notes").size(14), rule::horizontal(1)].spacing(8).align_y(Alignment::Center),
                text_input(&COMMENT_PLACEHOLDER, &comment)
                    .on_input(move |s| {
                        Message::from(DialingDirectoryMsg::AddressFieldChanged {
                            id,
                            field: AddressFieldChange::Comment(s),
                        })
                    })
                    .padding(6)
                    .size(14)
                    .width(Length::Fill),
            ]
            .spacing(8)
        };
        let show_quick_connect = self.selected_bbs == None;
        let mut content = column![
            space().height(Length::Fixed(4.0)),
            if show_quick_connect {
                column![container(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-connect-to")).size(20.0))]
            } else {
                header.into()
            },
            Space::new().height(Length::Fixed(4.0)),
        ]
        .spacing(8)
        .width(Length::Fill);
        // Add all sections
        content = content.push(server_section).push(Space::new().height(Length::Fixed(12.0)));
        if !is_quick {
            content = content.push(login_section).push(comment_section)
        } else {
            let mut add_btn = button(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-add-bbs-button")).size(16))
                .on_press(Message::from(DialingDirectoryMsg::AddAddress))
                .padding(8)
                .width(Length::Shrink);

            if self.quick_connect_address.address.is_empty() {
                add_btn = add_btn.style(button::secondary);
            }

            content = content.push(add_btn);
        }
        scrollable(content).into()
    }
}
