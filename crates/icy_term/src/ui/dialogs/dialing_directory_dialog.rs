use crate::ui::{Message, modal};
use crate::util::Rng;
use crate::{ATARI_MODES, Address, AddressBook, ScreenMode, VGA_MODES};
use i18n_embed_fl::fl;
use iced::widget::tooltip;
use iced::{
    Alignment, Element, Length, Task,
    widget::{Column, Space, button, checkbox, column, container, pick_list, row, rule, scrollable, svg, text, text_input},
};
use icy_engine::ansi::{BaudEmulation, MusicOption};
use icy_net::{ConnectionType, telnet::TerminalEmulation};
use once_cell::sync::Lazy;
use std::{fmt, mem::swap};

const VISIBILITY_SVG: &[u8] = include_bytes!("../../../data/icons/visibility.svg");
const VISIBILITY_OFF_SVG: &[u8] = include_bytes!("../../../data/icons/visibility_off.svg");
const DELETE_SVG: &[u8] = include_bytes!("../../../data/icons/delete.svg");

static COMMENT_PLACEHOLDER: Lazy<String> = Lazy::new(|| fl!(crate::LANGUAGE_LOADER, "dialing_directory-comment-placeholder"));

static CONNECT_TOADDRESS_PLACEHOLDER: Lazy<String> = Lazy::new(|| fl!(crate::LANGUAGE_LOADER, "dialing_directory-connect-to-address"));

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialingDirectoryFilter {
    All,
    Favourites,
}

impl Default for DialingDirectoryFilter {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Debug, Clone)]
pub struct DialingDirectoryState {
    pub addresses: AddressBook,
    pub selected_bbs: Option<usize>,
    pub filter_mode: DialingDirectoryFilter,
    pub filter_text: String,
    pub show_passwords: bool,
    pub pending_delete: Option<usize>,
    pub quick_connect_address: Address,
}

impl DialingDirectoryState {
    pub fn new(addresses: AddressBook) -> Self {
        Self {
            addresses,
            selected_bbs: None,
            filter_mode: DialingDirectoryFilter::All,
            filter_text: String::new(),
            show_passwords: false,
            pending_delete: None,
            quick_connect_address: Address::default(),
        }
    }

    pub fn get_address_mut(&mut self, id: Option<usize>) -> &mut Address {
        if let Some(idx) = id {
            if idx < self.addresses.addresses.len() {
                return &mut self.addresses.addresses[idx];
            }
        }
        &mut self.quick_connect_address
    }

    fn filtered(&self) -> Vec<(usize, &Address)> {
        let fav = matches!(self.filter_mode, DialingDirectoryFilter::Favourites);
        let needle = self.filter_text.trim().to_lowercase();

        let mut filtered: Vec<(usize, &Address)> = self
            .addresses
            .addresses
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                if fav && !a.is_favored {
                    return false;
                }
                if needle.is_empty() {
                    return true;
                }
                a.system_name.to_lowercase().contains(&needle) || a.address.to_lowercase().contains(&needle)
            })
            .collect();

        // Sort by: 1) Favorites first, 2) Number of calls (descending)
        filtered.sort_by(|(_, a), (_, b)| {
            // First compare by favorite status (favorites come first)
            match (a.is_favored, b.is_favored) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                // If both are favorites or both are not, sort by number of calls (descending)
                _ => b.number_of_calls.cmp(&a.number_of_calls),
            }
        });

        filtered
    }

    pub fn view(&self) -> Element<'_, Message> {
        let addresses = self.filtered();

        let left_panel: Element<Message> = {
            let filter_input = text_input(&fl!(crate::LANGUAGE_LOADER, "dialing_directory-filter-placeholder"), &self.filter_text)
                .on_input(|s| Message::from(DialingDirectoryMsg::FilterTextChanged(s)))
                .padding(6)
                .size(16);

            let clear_btn: Element<Message> = if self.filter_text.is_empty() {
                Space::new().into()
            } else {
                button(text("×"))
                    .on_press(Message::from(DialingDirectoryMsg::FilterTextChanged(String::new())))
                    .width(Length::Shrink)
                    .into()
            };

            let list_scroll: Element<Message> = {
                let mut col = Column::new();
                let show_quick_connect = self.filter_text.is_empty();

                if show_quick_connect && !self.addresses.addresses.is_empty() {
                    let selected = self.selected_bbs.is_none();
                    let entry = address_row_entry(selected, None, &CONNECT_TOADDRESS_PLACEHOLDER, "", false, u32::MAX, "");
                    col = col.push(entry);
                }

                for (idx, a) in &addresses {
                    let selected = self.selected_bbs == Some(*idx);
                    // Pass the filter text for highlighting
                    let entry = address_row_entry(
                        selected,
                        Some(*idx),
                        &a.system_name,
                        &a.address,
                        a.is_favored,
                        a.number_of_calls as u32,
                        &self.filter_text, // Pass filter text for highlighting
                    );
                    col = col.push(entry);
                }

                scrollable(col.spacing(2))
                    .height(Length::Fill)
                    .width(Length::Fill)
                    .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::new()))
                    .into()
            };

            column![
                row![filter_input, clear_btn].spacing(8).align_y(Alignment::Center),
                Space::new().height(Length::Fixed(8.0)),
                container(list_scroll)
                    .style(|_theme: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.15))),
                        border: iced::Border {
                            color: iced::Color::from_rgba(0.3, 0.3, 0.3, 0.5),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        text_color: None,
                        shadow: Default::default(),
                        snap: false,
                    })
                    .padding(4),
            ]
            .width(Length::Fixed(280.0))
            .spacing(6)
            .into()
        };

        let right_panel: Element<Message> = {
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

            // Server settings
            let server_section = {
                let address_field = text_input("", &addr.address)
                    .on_input(move |s| {
                        Message::from(DialingDirectoryMsg::AddressFieldChanged {
                            id,
                            field: AddressFieldChange::Address(s),
                        })
                    })
                    .padding(6)
                    .width(Length::Fill);

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

                let terms = vec![
                    TerminalEmulationWrapper(TerminalEmulation::Ansi),
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

                // Create a table-like layout with consistent column widths
                let label_width = Length::Fixed(100.0);

                let mut rows = vec![
                    // Address row
                    row![
                        container(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-address")))
                            .align_x(Alignment::End)
                            .width(label_width),
                        address_field
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                    // Protocol & Baud row
                    row![
                        container(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-protocol")))
                            .align_x(Alignment::End)
                            .width(label_width),
                        protocol_pick,
                        text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-baud-emulation")),
                        baud_pick,
                        Space::new().width(Length::Fill)
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                ];

                // Emulation row - add Screen and Music only for ANSI
                if addr.terminal_type == TerminalEmulation::Ansi {
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

                    rows.push(
                        row![
                            container(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-terminal_type")))
                                .align_x(Alignment::End)
                                .width(label_width),
                            term_pick,
                            text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-screen_mode")),
                            screen_mode_pick,
                            text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-music-option")),
                            music_pick,
                            Space::new().width(Length::Fill)
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    );
                } else {
                    // For non-ANSI terminals, check if they need screen mode
                    let needs_screen = match addr.terminal_type {
                        TerminalEmulation::Ascii | TerminalEmulation::Avatar | TerminalEmulation::Rip => {
                            let modes = VGA_MODES.to_vec();
                            Some(modes)
                        }
                        TerminalEmulation::AtariST => Some(ATARI_MODES.to_vec()),
                        TerminalEmulation::PETscii => Some(vec![ScreenMode::Vic]),
                        TerminalEmulation::ATAscii => Some(vec![ScreenMode::Antic]),
                        TerminalEmulation::ViewData | TerminalEmulation::Mode7 => Some(vec![ScreenMode::Videotex]),
                        TerminalEmulation::Skypix => Some(vec![ScreenMode::SkyPix]),
                        _ => None,
                    };

                    if let Some(modes) = needs_screen {
                        let screen_mode_pick = pick_list(modes, Some(addr.screen_mode), move |sm| {
                            Message::from(DialingDirectoryMsg::AddressFieldChanged {
                                id,
                                field: AddressFieldChange::ScreenMode(sm),
                            })
                        })
                        .placeholder(&fl!(crate::LANGUAGE_LOADER, "dialing_directory-screen_mode"))
                        .width(Length::Fixed(150.0));

                        rows.push(
                            row![
                                container(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-terminal_type")))
                                    .align_x(Alignment::End)
                                    .width(label_width),
                                term_pick,
                                text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-screen_mode")),
                                screen_mode_pick,
                                Space::new().width(Length::Fill)
                            ]
                            .spacing(8)
                            .align_y(Alignment::Center),
                        );
                    } else {
                        // Just emulation, no screen mode needed
                        rows.push(
                            row![
                                container(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-terminal_type")))
                                    .align_x(Alignment::End)
                                    .width(label_width),
                                term_pick,
                                Space::new().width(Length::Fill)
                            ]
                            .spacing(8)
                            .align_y(Alignment::Center),
                        );
                    }
                }

                let mut col = column![].spacing(8);

                for row in rows {
                    col = col.push(row);
                }

                col
            };

            // Login settings
            let login_section = {
                let label_width = Length::Fixed(100.0);

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
        };

        // Bottom bar with Delete, Cancel, and Connect buttons
        let bottom_bar: Element<Message> = {
            use iced::widget::tooltip;

            let delete_label = fl!(crate::LANGUAGE_LOADER, "dialing_directory-delete");

            let delete_icon = svg(svg::Handle::from_memory(DELETE_SVG)).width(Length::Fixed(20.0)).height(Length::Fixed(20.0));

            let can_delete = self.selected_bbs.is_some();

            // Base delete button (icon only)
            let delete_button = if can_delete {
                button(delete_icon)
                    .on_press(Message::from(DialingDirectoryMsg::DeleteAddress(self.selected_bbs.unwrap())))
                    .padding(6)
            } else {
                // Disabled style (secondary) but still show icon + tooltip
                button(delete_icon).style(button::secondary).padding(6)
            };

            // Wrap in tooltip with localized text
            let del_btn: tooltip::Tooltip<'_, Message> = tooltip(
                delete_button,
                container(text(delete_label)).style(container::rounded_box),
                tooltip::Position::Right,
            )
            .gap(10)
            .style(container::rounded_box)
            .padding(8);

            let cancel_btn = button(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-cancel-button"))).on_press(Message::from(DialingDirectoryMsg::Cancel));

            let connect_btn = button(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-connect-button")))
                .on_press(Message::from(DialingDirectoryMsg::ConnectSelected))
                .style(button::primary);

            row![del_btn, Space::new().width(Length::Fill), cancel_btn, connect_btn]
                .spacing(12)
                .align_y(Alignment::Center)
                .padding(12)
                .into()
        };

        // Main layout with left panel, right panel, and bottom bar
        let main_content = column![
            row![container(left_panel).padding(8), container(right_panel).padding(8).width(Length::Fill)].height(Length::Fill),
            container(bottom_bar).width(Length::Fill).style(container::bordered_box)
        ];

        // If there's a pending delete, show the confirmation modal
        if let Some(idx) = self.pending_delete {
            let overlay = self.delete_confirmation_modal(idx);
            modal(main_content, overlay, Message::from(DialingDirectoryMsg::Cancel)).into()
        } else {
            main_content.into()
        }
    }

    fn delete_confirmation_modal(&self, idx: usize) -> Element<'_, Message> {
        let system_name = if idx < self.addresses.addresses.len() {
            &self.addresses.addresses[idx].system_name
        } else {
            "Unknown"
        };

        let title = text(fl!(crate::LANGUAGE_LOADER, "delete-bbs-title")).size(22);

        let question = text(fl!(crate::LANGUAGE_LOADER, "delete-bbs-question", system = system_name))
            .wrapping(text::Wrapping::WordOrGlyph)
            .size(16);

        let delete_btn = button(
            text(fl!(crate::LANGUAGE_LOADER, "delete-bbs-delete-button")).style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().danger.base.color),
                ..Default::default()
            }),
        )
        .on_press(Message::from(DialingDirectoryMsg::ConfirmDelete(idx)))
        .style(|theme: &iced::Theme, status| {
            let palette = theme.extended_palette();
            let base = button::Style {
                background: Some(iced::Background::Color(palette.background.base.color)),
                border: iced::Border {
                    color: palette.danger.base.color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                text_color: palette.danger.base.color,
                shadow: Default::default(),
                snap: false,
            };

            match status {
                button::Status::Hovered => button::Style {
                    background: Some(iced::Background::Color(palette.danger.weak.color)),
                    text_color: palette.background.base.color,
                    ..base
                },
                button::Status::Pressed => button::Style {
                    background: Some(iced::Background::Color(palette.danger.strong.color)),
                    text_color: palette.background.base.color,
                    ..base
                },
                _ => base,
            }
        });

        let cancel_btn = button(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-cancel-button")))
            .on_press(Message::from(DialingDirectoryMsg::Cancel))
            .style(button::secondary);

        let modal_content = container(
            column![
                title.width(Length::Fill).align_x(Alignment::Center),
                rule::horizontal(1),
                question.width(Length::Fixed(440.0)),
                Space::new().height(Length::Fixed(24.0)),
                row![Space::new().width(Length::Fill), cancel_btn, delete_btn].spacing(12)
            ]
            .padding(20)
            .spacing(8),
        )
        .width(Length::Fixed(480.0))
        .style(|theme: &iced::Theme| {
            let palette = theme.palette();
            container::Style {
                background: Some(iced::Background::Color(palette.background)),
                border: iced::Border {
                    color: palette.text,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                text_color: Some(palette.text),
                shadow: iced::Shadow {
                    color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
                snap: false,
            }
        });

        container(modal_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    pub(crate) fn update(&mut self, msg: DialingDirectoryMsg) -> Task<Message> {
        println!("DialingDirectoryMsg: {:?}", msg);
        match msg {
            DialingDirectoryMsg::SelectAddress(idx) => {
                self.selected_bbs = idx;
                Task::none()
            }

            DialingDirectoryMsg::ToggleFavorite(idx) => {
                if idx < self.addresses.addresses.len() {
                    self.addresses.addresses[idx].is_favored = !self.addresses.addresses[idx].is_favored;
                }
                Task::none()
            }

            DialingDirectoryMsg::ChangeFilterMode(mode) => {
                self.filter_mode = mode;
                Task::none()
            }

            DialingDirectoryMsg::FilterTextChanged(text) => {
                self.filter_text = text;
                Task::none()
            }

            DialingDirectoryMsg::AddAddress => {
                let mut new_address = self.quick_connect_address.clone();
                self.quick_connect_address = Address::default();
                new_address.system_name = new_address.address.clone();
                self.addresses.addresses.push(new_address);
                self.selected_bbs = Some(self.addresses.addresses.len() - 1);
                Task::none()
            }

            DialingDirectoryMsg::DeleteAddress(idx) => {
                // Instead of deleting immediately, set pending_delete
                self.pending_delete = Some(idx);
                Task::none()
            }

            DialingDirectoryMsg::ConfirmDelete(idx) => {
                // Actually delete the address
                if idx < self.addresses.addresses.len() {
                    self.addresses.addresses.remove(idx);
                    // Adjust selected index if needed
                    if let Some(selected) = self.selected_bbs {
                        if selected == idx {
                            self.selected_bbs = None;
                        } else if selected > idx {
                            self.selected_bbs = Some(selected - 1);
                        }
                    }

                    // Save the address book
                    if let Err(e) = self.addresses.store_phone_book() {
                        eprintln!("Failed to save address book: {}", e);
                    }
                }
                self.pending_delete = None;
                Task::none()
            }

            DialingDirectoryMsg::AddressFieldChanged { id, field } => {
                let addr = self.get_address_mut(id);

                match field {
                    AddressFieldChange::SystemName(name) => {
                        addr.system_name = name;
                    }
                    AddressFieldChange::Address(address) => {
                        addr.address = address;
                    }
                    AddressFieldChange::User(user) => {
                        addr.user_name = user;
                    }
                    AddressFieldChange::Password(password) => {
                        addr.password = password;
                    }
                    AddressFieldChange::AutoLogin(script) => {
                        addr.auto_login = script;
                    }
                    AddressFieldChange::IemsiUser(user) => {
                        addr.iemsi_user = user;
                    }
                    AddressFieldChange::IemsiPassword(password) => {
                        addr.iemsi_password = password;
                    }
                    AddressFieldChange::Protocol(protocol) => {
                        addr.protocol = protocol;
                    }
                    AddressFieldChange::Terminal(terminal) => {
                        addr.terminal_type = terminal;
                        // Reset screen mode when terminal changes
                        addr.screen_mode = match terminal {
                            TerminalEmulation::Ansi | TerminalEmulation::Ascii | TerminalEmulation::Avatar | TerminalEmulation::Rip => ScreenMode::Vga(80, 25),
                            TerminalEmulation::AtariST => ScreenMode::AtariST(40),
                            TerminalEmulation::PETscii => ScreenMode::Vic,
                            TerminalEmulation::ATAscii => ScreenMode::Antic,
                            TerminalEmulation::ViewData | TerminalEmulation::Mode7 => ScreenMode::Videotex,
                            TerminalEmulation::Skypix => ScreenMode::SkyPix,
                        };
                    }
                    AddressFieldChange::ScreenMode(mode) => {
                        addr.screen_mode = mode;
                    }
                    AddressFieldChange::Baud(baud) => {
                        addr.baud_emulation = baud;
                    }
                    AddressFieldChange::Music(music) => {
                        addr.ansi_music = music;
                    }
                    AddressFieldChange::Comment(comment) => {
                        addr.comment = comment;
                    }
                    AddressFieldChange::OverrideIemsi(override_iemsi) => {
                        addr.override_iemsi_settings = override_iemsi;
                    }
                    AddressFieldChange::IsFavored(is_favored) => {
                        addr.is_favored = is_favored;
                    }
                }

                Task::none()
            }

            DialingDirectoryMsg::ToggleShowPasswords => {
                self.show_passwords = !self.show_passwords;
                Task::none()
            }

            DialingDirectoryMsg::ConnectSelected => {
                // Get the selected address
                let addr = if let Some(idx) = self.selected_bbs {
                    if idx < self.addresses.addresses.len() {
                        self.addresses.addresses[idx].clone()
                    } else {
                        return Task::none();
                    }
                } else if !self.addresses.addresses.is_empty() {
                    // Quick connect - use the first address but with potentially modified address field
                    self.addresses.addresses[0].clone()
                } else {
                    return Task::none();
                };

                // Increment call counter for the selected address
                if let Some(idx) = self.selected_bbs {
                    self.addresses.addresses[idx].number_of_calls += 1;
                    self.addresses.addresses[idx].last_call = Some(chrono::Utc::now());
                }

                // Save the address book
                if let Err(e) = self.addresses.store_phone_book() {
                    eprintln!("Failed to save address book: {}", e);
                }

                // Return a task that triggers the connection
                // You'll need to handle this in the parent component
                Task::done(Message::Connect(addr))
            }

            DialingDirectoryMsg::Cancel => {
                // Cancel the delete operation
                if self.pending_delete.is_some() {
                    self.pending_delete = None;
                    return Task::none();
                }

                // Save any changes before closing
                if let Err(e) = self.addresses.store_phone_book() {
                    eprintln!("Failed to save address book: {}", e);
                }

                // Return a task that closes the dialog
                Task::done(Message::CloseDialingDirectory)
            }

            DialingDirectoryMsg::GeneratePassword => {
                // Generate a random password
                let mut rng = Rng::default();
                let mut pw = String::new();
                for _ in 0..16 {
                    pw.push(unsafe { char::from_u32_unchecked(rng.gen_range(b'0'..=b'z')) });
                }
                let addr = self.get_address_mut(self.selected_bbs);
                addr.password = pw;
                Task::none()
            }

            DialingDirectoryMsg::NavigateUp => {
                let addresses = self.filtered();

                if let Some(selected_idx) = self.selected_bbs {
                    // Find current selection in filtered list
                    if let Some((pos, _)) = addresses.iter().enumerate().find(|(_, (idx, _))| *idx == selected_idx) {
                        if pos > 0 {
                            // Select previous item
                            let (new_idx, _) = addresses[pos - 1];
                            self.selected_bbs = Some(new_idx);
                        } else {
                            // At top, move to quick connect if available
                            let show_quick_connect = self.filter_text.is_empty() && matches!(self.filter_mode, DialingDirectoryFilter::All);
                            if show_quick_connect {
                                self.selected_bbs = None;
                            }
                        }
                    }
                } else if !addresses.is_empty() {
                    // No selection (on quick connect), select last item
                    let (idx, _) = addresses[addresses.len() - 1];
                    self.selected_bbs = Some(idx);
                }
                Task::none()
            }

            DialingDirectoryMsg::NavigateDown => {
                let addresses = self.filtered();
                let show_quick_connect = self.filter_text.is_empty() && matches!(self.filter_mode, DialingDirectoryFilter::All);

                if let Some(selected_idx) = self.selected_bbs {
                    // Find current selection in filtered list
                    if let Some((pos, _)) = addresses.iter().enumerate().find(|(_, (idx, _))| *idx == selected_idx) {
                        if pos + 1 < addresses.len() {
                            // Select next item
                            let (new_idx, _) = addresses[pos + 1];
                            self.selected_bbs = Some(new_idx);
                        } else if show_quick_connect {
                            // At bottom, wrap to quick connect
                            self.selected_bbs = None;
                        }
                    }
                } else if !addresses.is_empty() {
                    // Currently on quick connect, select first item
                    let (idx, _) = addresses[0];
                    self.selected_bbs = Some(idx);
                }
                Task::none()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum DialingDirectoryMsg {
    SelectAddress(Option<usize>),
    ToggleFavorite(usize),
    ChangeFilterMode(DialingDirectoryFilter),
    FilterTextChanged(String),
    AddAddress,
    DeleteAddress(usize),
    AddressFieldChanged { id: Option<usize>, field: AddressFieldChange },
    ToggleShowPasswords,
    GeneratePassword,
    ConnectSelected,
    Cancel,
    NavigateUp,
    NavigateDown,
    ConfirmDelete(usize),
}

#[derive(Debug, Clone)]
pub enum AddressFieldChange {
    SystemName(String),
    Address(String),
    User(String),
    Password(String),
    AutoLogin(String),
    IemsiUser(String),
    IemsiPassword(String),
    Protocol(ConnectionType),
    Terminal(TerminalEmulation),
    ScreenMode(ScreenMode),
    Baud(BaudEmulation),
    Music(MusicOption),
    Comment(String),
    OverrideIemsi(bool),
    IsFavored(bool),
}

impl From<DialingDirectoryMsg> for Message {
    fn from(m: DialingDirectoryMsg) -> Self {
        Message::DialingDirectory(m)
    }
}

// Update the address_row_entry function to support highlighting
fn address_row_entry<'a>(
    selected: bool,
    idx: Option<usize>,
    name: &'a str,
    addr: &'a str,
    favored: bool,
    calls: u32,
    search_text: &'a str, // Add search text parameter
) -> Element<'a, Message> {
    fn truncate_text(text: &str, max_chars: usize) -> String {
        if text.chars().count() <= max_chars {
            text.to_string()
        } else {
            let mut result: String = text.chars().take(max_chars - 1).collect();
            result.push('…');
            result
        }
    }

    let star = if favored {
        text("★").size(16).style(|theme: &iced::Theme| iced::widget::text::Style {
            color: Some(theme.extended_palette().warning.base.color),
            ..Default::default()
        })
    } else {
        text("").size(16)
    };

    let truncated_name = truncate_text(name, 28);
    let truncated_addr = truncate_text(addr, 29);

    // Create highlighted text elements - pass owned Strings
    let name_element = if !search_text.is_empty() && truncated_name.to_lowercase().contains(&search_text.to_lowercase()) {
        highlight_name_text(truncated_name, search_text)
    } else {
        text(truncated_name).size(14).font(iced::Font::MONOSPACE).into()
    };

    let addr_element = if !search_text.is_empty() && truncated_addr.to_lowercase().contains(&search_text.to_lowercase()) {
        highlight_addr_text(truncated_addr, search_text)
    } else {
        text(truncated_addr)
            .size(12)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().secondary.weak.color),
                ..Default::default()
            })
            .font(iced::Font::MONOSPACE)
            .into()
    };

    let calls_text = text(if calls == u32::MAX { String::new() } else { format!("✆ {}", calls) }).size(12);

    let content = column![
        row![name_element, Space::new().width(Length::Fill), star].align_y(Alignment::Center),
        row![
            addr_element,
            Space::new().width(Length::Fill),
            container(calls_text).center_y(Length::Shrink).padding([0, 8])
        ]
    ]
    .spacing(2);

    let entry_container: container::Container<'_, Message> = container(content).width(Length::Fill).padding([6, 10]);

    let clickable = button(Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(0)
        .style(|_theme: &iced::Theme, _status| button::Style {
            background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
            border: iced::Border {
                color: iced::Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
            },
            text_color: iced::Color::BLACK,
            shadow: Default::default(),
            snap: false,
        })
        .on_press(Message::from(DialingDirectoryMsg::SelectAddress(idx)));

    let stacked = iced::widget::stack![entry_container, clickable];

    if selected {
        container(stacked)
            .width(Length::Fill)
            .style(|theme: &iced::Theme| {
                let extended = theme.extended_palette();

                let mut border_color = extended.primary.strong.color;
                border_color.a = 0.6;
                swap(&mut border_color.r, &mut border_color.g);

                container::Style {
                    background: Some(iced::Background::Color({
                        let mut c = extended.primary.weak.color;
                        swap(&mut c.r, &mut c.g);
                        c.a = 0.10;
                        c
                    })),
                    border: iced::Border {
                        color: border_color,
                        width: 1.0,
                        radius: 3.0.into(),
                    },
                    text_color: None,
                    shadow: Default::default(),
                    snap: false,
                }
            })
            .into()
    } else {
        container(stacked).width(Length::Fill).into()
    }
}

// Helper function to highlight text in name (larger font) - now takes owned String
fn highlight_name_text<'a>(text_str: String, search: &str) -> Element<'a, Message> {
    if search.is_empty() || !text_str.to_lowercase().contains(&search.to_lowercase()) {
        return text(text_str).size(14).font(iced::Font::MONOSPACE).into();
    }

    let lower_text = text_str.to_lowercase();
    let lower_search = search.to_lowercase();

    let mut row_elements: Vec<Element<'a, Message>> = Vec::new();
    let mut last = 0;

    for (idx, _) in lower_text.match_indices(&lower_search) {
        if idx > last {
            // Add non-highlighted part
            row_elements.push(text(text_str[last..idx].to_string()).size(14).font(iced::Font::MONOSPACE).into());
        }
        // Add highlighted part with different style
        row_elements.push(
            text(text_str[idx..idx + search.len()].to_string())
                .size(14)
                .font(iced::Font::MONOSPACE)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().primary.strong.color),
                    ..Default::default()
                })
                .into(),
        );
        last = idx + search.len();
    }
    if last < text_str.len() {
        row_elements.push(text(text_str[last..].to_string()).size(14).font(iced::Font::MONOSPACE).into());
    }

    row(row_elements).into()
}

// Helper function to highlight text in address (smaller font) - now takes owned String
fn highlight_addr_text<'a>(text_str: String, search: &str) -> Element<'a, Message> {
    if search.is_empty() || !text_str.to_lowercase().contains(&search.to_lowercase()) {
        return text(text_str)
            .size(12)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().secondary.weak.color),
                ..Default::default()
            })
            .font(iced::Font::MONOSPACE)
            .into();
    }

    let lower_text = text_str.to_lowercase();
    let lower_search = search.to_lowercase();

    let mut row_elements: Vec<Element<'a, Message>> = Vec::new();
    let mut last = 0;

    for (idx, _) in lower_text.match_indices(&lower_search) {
        if idx > last {
            // Add non-highlighted part
            row_elements.push(
                text(text_str[last..idx].to_string())
                    .size(12)
                    .style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().secondary.weak.color),
                        ..Default::default()
                    })
                    .font(iced::Font::MONOSPACE)
                    .into(),
            );
        }
        // Add highlighted part with warning color
        row_elements.push(
            text(text_str[idx..idx + search.len()].to_string())
                .size(12)
                .font(iced::Font::MONOSPACE)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().primary.strong.color),
                    ..Default::default()
                })
                .into(),
        );
        last = idx + search.len();
    }
    if last < text_str.len() {
        row_elements.push(
            text(text_str[last..].to_string())
                .size(12)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().secondary.weak.color),
                    ..Default::default()
                })
                .font(iced::Font::MONOSPACE)
                .into(),
        );
    }

    row(row_elements).into()
}
