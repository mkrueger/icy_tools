use crate::VGA_MODES;
use crate::ui::Message;
use crate::ui::dialing_directory_dialog::{AddressFieldChange, DialingDirectoryMsg};
use i18n_embed_fl::fl;
use iced::Padding;
use iced::widget::tooltip;
use iced::{
    Alignment, Element, Length,
    widget::{Space, button, checkbox, column, container, pick_list, row, scrollable, svg, text, text_input},
};
use iced_engine_gui::settings::{SECTION_SPACING, effect_box, left_label, section_header};
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
            self.addresses.lock().unwrap().addresses[addr_idx].clone()
        } else {
            self.quick_connect_address.clone()
        };
        let is_quick = self.selected_bbs.is_none();
        let id = self.selected_bbs;

        // Header section - explicitly type as Element
        let header: Element<'_, Message> = if !is_quick {
            let name_input = text_input(&fl!(crate::LANGUAGE_LOADER, "dialing_directory-name-placeholder"), &addr.system_name)
                .on_input(move |s| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::SystemName(s),
                    })
                })
                .size(18)
                .width(Length::Fill);

            let star_btn: button::Button<'_, Message> = button(text(if addr.is_favored { "★" } else { "☆" }))
                .on_press(Message::from(DialingDirectoryMsg::ToggleFavorite(if let Some(addr_idx) = self.selected_bbs {
                    addr_idx
                } else {
                    0
                })))
                .style(button::text);

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
                row![name_input, star_btn].spacing(8).align_y(Alignment::Center),
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
                    Space::new().width(8.0),
                ]
                .spacing(20)
            ]
            .spacing(4)
            .into()
        } else {
            container(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-connect-to")).size(20.0)).into()
        };

        // Server Settings Section
        let server_section: Element<'_, Message> = {
            let mut server_content = column![].spacing(12);

            // Address/Modem row
            if addr.protocol == ConnectionType::Modem {
                let modem_names: Vec<String> = options.modems.iter().map(|m| m.name.clone()).collect();
                let selected_modem = modem_names.iter().position(|name| name == &addr.address);

                let modem_picker = pick_list(modem_names.clone(), selected_modem.map(|idx| modem_names[idx].clone()), move |modem_name| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::Address(modem_name),
                    })
                })
                .placeholder("Select Modem")
                .width(Length::Fill)
                .text_size(14);

                let mut modem_row = row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-modem")), modem_picker,]
                    .spacing(12)
                    .align_y(Alignment::Center);

                if selected_modem.is_none() && !addr.address.is_empty() {
                    let err_msg = if modem_names.is_empty() {
                        fl!(crate::LANGUAGE_LOADER, "dialing_directory-no_modem_configured")
                    } else {
                        fl!(crate::LANGUAGE_LOADER, "dialing_directory-invalid_modem")
                    };
                    modem_row = modem_row.push(text(format!("⚠ {}", err_msg)).size(12).style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().danger.base.color),
                        ..Default::default()
                    }));
                }
                server_content = server_content.push(modem_row);
            } else {
                let address_field = text_input("", &addr.address)
                    .on_input(move |s| {
                        Message::from(DialingDirectoryMsg::AddressFieldChanged {
                            id,
                            field: AddressFieldChange::Address(s),
                        })
                    })
                    .padding(6)
                    .size(14)
                    .width(Length::Fill);

                server_content = server_content.push(
                    row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-address")), address_field]
                        .spacing(12)
                        .align_y(Alignment::Center),
                );
            }

            // Protocol row
            let protocols = vec![
                ConnectionTypeWrapper(ConnectionType::Telnet),
                ConnectionTypeWrapper(ConnectionType::Raw),
                ConnectionTypeWrapper(ConnectionType::Modem),
                ConnectionTypeWrapper(ConnectionType::SSH),
                ConnectionTypeWrapper(ConnectionType::Websocket),
                ConnectionTypeWrapper(ConnectionType::SecureWebsocket),
            ];

            let protocol_pick = pick_list(protocols, Some(ConnectionTypeWrapper(addr.protocol)), move |p: ConnectionTypeWrapper| {
                Message::from(DialingDirectoryMsg::AddressFieldChanged {
                    id,
                    field: AddressFieldChange::Protocol(p.0),
                })
            })
            .width(Length::Fixed(180.0))
            .text_size(14);

            server_content = server_content.push(
                row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-protocol")), protocol_pick,]
                    .spacing(12)
                    .align_y(Alignment::Center),
            );
            const COMBO_WIDTH: f32 = 110.0;
            // Baud emulation row (only if not Modem protocol)
            if addr.protocol != ConnectionType::Modem {
                let baud_pick = pick_list(BaudEmulation::OPTIONS.to_vec(), Some(addr.baud_emulation), move |b| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::Baud(b),
                    })
                })
                .width(Length::Fixed(COMBO_WIDTH))
                .text_size(14);

                server_content = server_content.push(
                    row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-baud-emulation")), baud_pick]
                        .spacing(12)
                        .align_y(Alignment::Center),
                );
            }

            // Terminal type row
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

            let term_pick = pick_list(terms, Some(TerminalEmulationWrapper(addr.terminal_type)), move |t: TerminalEmulationWrapper| {
                Message::from(DialingDirectoryMsg::AddressFieldChanged {
                    id,
                    field: AddressFieldChange::Terminal(t.0),
                })
            })
            .width(Length::Fixed(COMBO_WIDTH))
            .text_size(14);

            server_content = server_content.push(
                row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-terminal_type")), term_pick,]
                    .spacing(12)
                    .align_y(Alignment::Center),
            );

            // Screen mode row (only for certain terminal types)
            if addr.terminal_type == TerminalEmulation::Ansi
                || addr.terminal_type == TerminalEmulation::Utf8Ansi
                || addr.terminal_type == TerminalEmulation::Avatar
                || addr.terminal_type == TerminalEmulation::Ascii
            {
                let screen_mode_pick = pick_list(VGA_MODES.to_vec(), Some(addr.screen_mode), move |sm| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::ScreenMode(sm),
                    })
                })
                .width(Length::Fixed(120.0))
                .text_size(14);

                server_content = server_content.push(
                    row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-screen_mode")), screen_mode_pick]
                        .spacing(12)
                        .align_y(Alignment::Center),
                );
            }

            // Music option row (only for ANSI/UTF8ANSI)
            if addr.terminal_type == TerminalEmulation::Ansi || addr.terminal_type == TerminalEmulation::Utf8Ansi {
                let music_pick = pick_list(
                    vec![MusicOption::Off, MusicOption::Banana, MusicOption::Conflicting, MusicOption::Both],
                    Some(addr.ansi_music),
                    move |m| {
                        Message::from(DialingDirectoryMsg::AddressFieldChanged {
                            id,
                            field: AddressFieldChange::Music(m),
                        })
                    },
                )
                .width(Length::Fixed(COMBO_WIDTH))
                .text_size(14);

                server_content = server_content.push(
                    row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-music-option")), music_pick]
                        .spacing(12)
                        .align_y(Alignment::Center),
                );
            }

            effect_box(server_content.into()).into()
        };

        // Login Settings Section (only for non-quick connect)
        let login_section: Option<Element<'_, Message>> = if !is_quick {
            let mut login_content = column![].spacing(12);

            // User field
            let user_field = text_input("", &addr.user_name)
                .on_input(move |s| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::User(s),
                    })
                })
                .padding(6)
                .size(14)
                .width(Length::Fill);

            login_content = login_content.push(
                row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-user")), user_field]
                    .spacing(12)
                    .align_y(Alignment::Center),
            );

            // Password field with visibility toggle
            let pw_field = text_input("", &addr.password)
                .on_input(move |s| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::Password(s),
                    })
                })
                .secure(!self.show_passwords)
                .padding(6)
                .size(14)
                .width(Length::Fill);

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
                    button(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-generate")).size(14))
                        .on_press(Message::from(DialingDirectoryMsg::GeneratePassword))
                        .padding([6, 12]),
                    fl!(crate::LANGUAGE_LOADER, "dialing_directory-generate-tooltip"),
                )
            } else {
                (
                    button(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-generate")).size(14))
                        .padding([6, 12])
                        .style(button::secondary),
                    fl!(crate::LANGUAGE_LOADER, "dialing_directory-generate-disabled-tooltip"),
                )
            };

            let generate_btn = tooltip(
                generate_btn,
                container(text(tooltip_label).size(12)).style(container::rounded_box),
                tooltip::Position::Bottom,
            )
            .gap(10)
            .style(container::rounded_box)
            .padding(8);

            login_content = login_content.push(
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-password")),
                    pw_field,
                    toggler_pw,
                    generate_btn
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            );

            // Auto login field
            let auto_login_field = text_input("", &addr.auto_login)
                .on_input(move |s| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::AutoLogin(s),
                    })
                })
                .padding(6)
                .size(14)
                .width(Length::Fill);

            login_content = login_content.push(
                row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-autologin")), auto_login_field]
                    .spacing(12)
                    .align_y(Alignment::Center),
            );

            // IEMSI override checkbox
            let override_toggle = checkbox("", addr.override_iemsi_settings)
                .on_toggle(move |v| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::OverrideIemsi(v),
                    })
                })
                .size(18);

            login_content = login_content.push(
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-custom-iemsi-login-data")),
                    override_toggle
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            );

            // IEMSI fields (if override is enabled)
            if addr.override_iemsi_settings {
                let iemsi_user = text_input("", &addr.iemsi_user)
                    .on_input(move |s| {
                        Message::from(DialingDirectoryMsg::AddressFieldChanged {
                            id,
                            field: AddressFieldChange::IemsiUser(s),
                        })
                    })
                    .padding(6)
                    .size(14)
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
                    .size(14)
                    .width(Length::Fill);

                login_content = login_content
                    .push(
                        row![
                            left_label(format!("  IEMSI {}", fl!(crate::LANGUAGE_LOADER, "dialing_directory-user"))),
                            iemsi_user
                        ]
                        .spacing(12)
                        .align_y(Alignment::Center),
                    )
                    .push(
                        row![
                            left_label(format!("  IEMSI {}", fl!(crate::LANGUAGE_LOADER, "dialing_directory-password"))),
                            iemsi_pw
                        ]
                        .spacing(12)
                        .align_y(Alignment::Center),
                    );
            }

            Some(effect_box(login_content.into()).into())
        } else {
            None
        };

        // Comment/Notes section (only for non-quick connect)
        let comment_section: Option<Element<'_, Message>> = if !is_quick {
            let comment = text_input(&COMMENT_PLACEHOLDER, &addr.comment)
                .on_input(move |s| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::Comment(s),
                    })
                })
                .padding(6)
                .size(14)
                .width(Length::Fill);

            Some(effect_box(comment.into()).into())
        } else {
            None
        };

        // Main content layout
        let mut content: iced::widget::Column<'_, Message> = column![header, Space::new().height(SECTION_SPACING), server_section,];

        if !is_quick {
            if let Some(login) = login_section {
                content = content
                    .push(Space::new().height(SECTION_SPACING))
                    .push(section_header("Login Settings".to_string()))
                    .push(login);
            }

            if let Some(notes) = comment_section {
                content = content
                    .push(Space::new().height(SECTION_SPACING))
                    .push(section_header("Notes".to_string()))
                    .push(notes);
            }
        } else {
            // Quick connect "Add BBS" button
            let mut add_btn = button(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-add-bbs-button")).size(14))
                .on_press(Message::from(DialingDirectoryMsg::AddAddress))
                .padding([6, 12])
                .width(Length::Shrink);

            if self.quick_connect_address.address.is_empty() {
                add_btn = add_btn.style(button::secondary);
            }

            content = content.push(Space::new().height(24)).push(row![Space::new().width(Length::Fill), add_btn]);
        }

        scrollable(content.padding(Padding {
            top: 12.0,
            bottom: 16.0,
            left: 0.0,
            right: 20.0,
        }))
        .into()
    }
}
