use crate::ConnectionInformation;
use crate::ui::Message;
use crate::ui::dialing_directory_dialog::{AddressFieldChange, DialingDirectoryMsg};
use i18n_embed_fl::fl;
use iced::Padding;
use iced::widget::tooltip;
use iced::{
    Alignment, Element, Length,
    widget::{Space, button, column, container, pick_list, row, scrollable, svg, text, text_input},
};
use icy_engine::{ScreenMode, TerminalResolutionExt, VGA_MODES};
use icy_engine_gui::settings::{effect_box, left_label};
use icy_engine_gui::ui::{DIALOG_SPACING, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL, primary_button, secondary_button, text_button_style};
use icy_engine_gui::{LABEL_WIDTH, SECTION_SPACING, section_header};
use icy_net::{ConnectionType, telnet::TerminalEmulation};
use icy_parser_core::{BaudEmulation, MusicOption};
use once_cell::sync::Lazy;
use std::fmt;

static COMMENT_PLACEHOLDER: Lazy<String> = Lazy::new(|| fl!(crate::LANGUAGE_LOADER, "dialing_directory-comment-placeholder"));

const VISIBILITY_SVG: &[u8] = include_bytes!("../../../../data/icons/visibility.svg");
const VISIBILITY_OFF_SVG: &[u8] = include_bytes!("../../../../data/icons/visibility_off.svg");

// Wrapper types to implement Display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionTypeWrapper(pub ConnectionType);
/*
static AUTO_LOGIN_PRESETS: &[(&str, &str)] = &[
    ("Autologin disabled", ""),
    ("Escapes, Name, Password", "@E@W@N@P"),
    ("Escapes, Name, Y, Password", "@E@W@NY@13@P"),
    ("Escapes, Name, J, Password", "@E@W@NJ@13@P"),
    ("Escapes, First, Last name, Password", "@E@W@F@L@P"),
    ("Escapes, IEMSI", "@E"),
    ("Name, Password", "@W@N@P"),
    ("Name, Y, Password", "@W@NY@13@P"),
    ("Name, J, Password", "@W@NJ@13@P"),
    ("First name, Last name, Password", "@W@F@L@P"),
    ("Disable autologin and IEMSI", "@I"),
    ("1 Escape, Name, Password", "@27@W@N@P"),
];

#[derive(Debug, Clone, PartialEq)]
struct AutoLoginOption {
    label: String,
    value: String,
}

impl fmt::Display for AutoLoginOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)
    }
}*/

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
            ConnectionType::Rlogin => write!(f, "Rlogin"),
            ConnectionType::RloginSwapped => write!(f, "Rlogin (Swapped)"),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalResolutionWrapper(pub icy_engine::TerminalResolution);

impl fmt::Display for TerminalResolutionWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            icy_engine::TerminalResolution::Low => write!(f, "Low"),
            icy_engine::TerminalResolution::Medium => write!(f, "Medium"),
            icy_engine::TerminalResolution::High => write!(f, "High"),
        }
    }
}

impl super::DialingDirectoryState {
    pub fn create_option_panel(&self, options: &crate::Options) -> Element<'_, Message> {
        let addr = if let Some(addr_idx) = self.selected_bbs {
            self.addresses.lock().addresses[addr_idx].clone()
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

            let star_btn: button::Button<'_, Message> = button(text(if addr.is_favored { "★" } else { "☆" }).wrapping(text::Wrapping::None))
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
                row![name_input, star_btn].spacing(DIALOG_SPACING).align_y(Alignment::Center),
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
            let mut server_content = column![].spacing(DIALOG_SPACING);

            // Address field (or phone number for modem connections)
            if addr.protocol == ConnectionType::Modem {
                // Phone number field for modem connections
                let phone_field = text_input("", &addr.address)
                    .on_input(move |s| {
                        Message::from(DialingDirectoryMsg::AddressFieldChanged {
                            id,
                            field: AddressFieldChange::Address(s),
                        })
                    })
                    .padding(6)
                    .size(TEXT_SIZE_NORMAL)
                    .width(Length::Fill);

                server_content = server_content.push(
                    row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-phone-number")), phone_field]
                        .spacing(DIALOG_SPACING)
                        .align_y(Alignment::Center),
                );
            } else {
                let address_field = text_input("", &addr.address)
                    .on_input(move |s| {
                        Message::from(DialingDirectoryMsg::AddressFieldChanged {
                            id,
                            field: AddressFieldChange::Address(s),
                        })
                    })
                    .padding(6)
                    .size(TEXT_SIZE_NORMAL)
                    .width(Length::Fill);

                server_content = server_content.push(
                    row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-address")), address_field]
                        .spacing(DIALOG_SPACING)
                        .align_y(Alignment::Center),
                );
            }

            let address_has_protocol = if let Ok(info) = ConnectionInformation::parse(&addr.address) {
                info.protocol.is_some()
            } else {
                false
            };

            if !address_has_protocol {
                let protocols = vec![
                    ConnectionTypeWrapper(ConnectionType::Telnet),
                    ConnectionTypeWrapper(ConnectionType::Raw),
                    ConnectionTypeWrapper(ConnectionType::Modem),
                    ConnectionTypeWrapper(ConnectionType::SSH),
                    ConnectionTypeWrapper(ConnectionType::Websocket),
                    ConnectionTypeWrapper(ConnectionType::SecureWebsocket),
                    ConnectionTypeWrapper(ConnectionType::Rlogin),
                    ConnectionTypeWrapper(ConnectionType::RloginSwapped),
                ];

                let protocol_pick = pick_list(protocols, Some(ConnectionTypeWrapper(addr.protocol)), move |p: ConnectionTypeWrapper| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::Protocol(p.0),
                    })
                })
                .width(Length::Fixed(180.0))
                .text_size(TEXT_SIZE_NORMAL);

                server_content = server_content.push(
                    row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-protocol")), protocol_pick,]
                        .spacing(DIALOG_SPACING)
                        .align_y(Alignment::Center),
                );
            }

            // Modem picker (only for modem protocol, placed after protocol picker)
            if addr.protocol == ConnectionType::Modem {
                let modem_names: Vec<String> = options.modems.iter().map(|m| m.name.clone()).collect();
                let selected_modem = modem_names.iter().position(|name| name == &addr.modem_id);

                let modem_picker = pick_list(modem_names.clone(), selected_modem.map(|idx| modem_names[idx].clone()), move |modem_name| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::ModemId(modem_name),
                    })
                })
                .placeholder(fl!(crate::LANGUAGE_LOADER, "dialing_directory-select-modem"))
                .width(Length::Fill)
                .text_size(TEXT_SIZE_NORMAL);

                let modem_row = row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-modem")), modem_picker,]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center);

                server_content = server_content.push(modem_row);

                // Show error message in a separate row if no modem is selected but modem_id is set
                if selected_modem.is_none() && !addr.modem_id.is_empty() {
                    let err_msg = if modem_names.is_empty() {
                        fl!(crate::LANGUAGE_LOADER, "dialing_directory-no_modem_configured")
                    } else {
                        fl!(crate::LANGUAGE_LOADER, "dialing_directory-invalid_modem")
                    };

                    let error_row = row![
                        left_label(String::new()), // Offset to align with the field
                        text(format!("⚠ {}", err_msg))
                            .size(TEXT_SIZE_SMALL)
                            .style(|theme: &iced::Theme| iced::widget::text::Style {
                                color: Some(theme.extended_palette().danger.base.color),
                                ..Default::default()
                            })
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center);

                    server_content = server_content.push(error_row);
                }
            }

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
                .text_size(TEXT_SIZE_NORMAL);

                server_content = server_content.push(
                    row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-baud-emulation")), baud_pick]
                        .spacing(DIALOG_SPACING)
                        .align_y(Alignment::Center),
                );
            }

            // Terminal type row
            let terms = vec![
                TerminalEmulationWrapper(TerminalEmulation::Ansi),
                TerminalEmulationWrapper(TerminalEmulation::Utf8Ansi),
                TerminalEmulationWrapper(TerminalEmulation::Ascii),
                TerminalEmulationWrapper(TerminalEmulation::Rip),
                TerminalEmulationWrapper(TerminalEmulation::AtariST),
                TerminalEmulationWrapper(TerminalEmulation::PETscii),
                TerminalEmulationWrapper(TerminalEmulation::ATAscii),
                TerminalEmulationWrapper(TerminalEmulation::ViewData),
                TerminalEmulationWrapper(TerminalEmulation::Mode7),
                TerminalEmulationWrapper(TerminalEmulation::Skypix),
                TerminalEmulationWrapper(TerminalEmulation::Avatar),
            ];

            let term_pick = pick_list(terms, Some(TerminalEmulationWrapper(addr.terminal_type)), move |t: TerminalEmulationWrapper| {
                Message::from(DialingDirectoryMsg::AddressFieldChanged {
                    id,
                    field: AddressFieldChange::Terminal(t.0),
                })
            })
            .width(Length::Fixed(COMBO_WIDTH))
            .text_size(TEXT_SIZE_NORMAL);

            server_content = server_content.push(
                row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-terminal_type")), term_pick,]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
            );

            // Screen mode row (only for certain terminal types)
            if addr.terminal_type == TerminalEmulation::Ansi
                || addr.terminal_type == TerminalEmulation::Utf8Ansi
                || addr.terminal_type == TerminalEmulation::Avatar
                || addr.terminal_type == TerminalEmulation::Ascii
            {
                // Build dynamic VGA mode list so current custom size appears selected.
                let mut vga_modes = VGA_MODES.to_vec();
                if addr.screen_mode.is_custom_vga() && !vga_modes.contains(&addr.screen_mode) {
                    vga_modes.push(addr.screen_mode);
                }

                let screen_mode_pick = pick_list(vga_modes, Some(addr.screen_mode), move |sm| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::ScreenMode(sm),
                    })
                })
                .width(Length::Fixed(120.0))
                .text_size(TEXT_SIZE_NORMAL);

                server_content = server_content.push(
                    row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-screen_mode")), screen_mode_pick]
                        .spacing(DIALOG_SPACING)
                        .align_y(Alignment::Center),
                );

                // If custom VGA, expose editable columns/rows.
                if let ScreenMode::Vga(w, h) = addr.screen_mode {
                    // Determine if custom via helper; shows inputs for any non-standard size.
                    if addr.screen_mode.is_custom_vga() {
                        let cols_str = w.to_string();
                        let rows_str = h.to_string();

                        // Cols input
                        let cols_input = text_input("", &cols_str)
                            .on_input(move |s| {
                                let new_w = s.parse::<i32>().map(|v| v.clamp(1, 255)).unwrap_or(w);
                                Message::from(DialingDirectoryMsg::AddressFieldChanged {
                                    id,
                                    field: AddressFieldChange::ScreenMode(ScreenMode::Vga(new_w, h)),
                                })
                            })
                            .padding(6)
                            .size(TEXT_SIZE_NORMAL)
                            .width(Length::Fixed(70.0));

                        // Rows input
                        let rows_input = text_input("", &rows_str)
                            .on_input(move |s| {
                                let new_h = s.parse::<i32>().map(|v| v.clamp(1, 80)).unwrap_or(h);
                                Message::from(DialingDirectoryMsg::AddressFieldChanged {
                                    id,
                                    field: AddressFieldChange::ScreenMode(ScreenMode::Vga(w, new_h)),
                                })
                            })
                            .padding(6)
                            .size(TEXT_SIZE_NORMAL)
                            .width(Length::Fixed(70.0));

                        server_content = server_content.push(
                            row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-resolution")), cols_input, rows_input,]
                                .spacing(DIALOG_SPACING)
                                .align_y(Alignment::Center),
                        );
                    }
                }
            } else if addr.terminal_type == TerminalEmulation::ATAscii {
                // ATAscii: Provide XEP80 module checkbox to choose 80 vs 40 column mode.
                let is_xep80 = matches!(addr.screen_mode, ScreenMode::Atascii(80));
                let toggle = iced::widget::checkbox(is_xep80)
                    .on_toggle(move |checked| {
                        let width = if checked { 80 } else { 40 };
                        Message::from(DialingDirectoryMsg::AddressFieldChanged {
                            id,
                            field: AddressFieldChange::ScreenMode(ScreenMode::Atascii(width)),
                        })
                    })
                    .text_size(TEXT_SIZE_NORMAL);

                server_content = server_content.push(
                    row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-xep80-module")), toggle]
                        .spacing(DIALOG_SPACING)
                        .align_y(Alignment::Center),
                );
            } else if addr.terminal_type == TerminalEmulation::AtariST {
                // AtariST: Resolution pick list and IGS checkbox
                let (current_resolution, current_igs) = match addr.screen_mode {
                    ScreenMode::AtariST(res, igs) => (res, igs),
                    _ => (icy_engine::TerminalResolution::Medium, false),
                };

                // Resolution pick list
                let resolutions = vec![
                    TerminalResolutionWrapper(icy_engine::TerminalResolution::Low),
                    TerminalResolutionWrapper(icy_engine::TerminalResolution::Medium),
                    TerminalResolutionWrapper(icy_engine::TerminalResolution::High),
                ];

                let resolution_pick = pick_list(
                    resolutions,
                    Some(TerminalResolutionWrapper(current_resolution)),
                    move |wrapper: TerminalResolutionWrapper| {
                        Message::from(DialingDirectoryMsg::AddressFieldChanged {
                            id,
                            field: AddressFieldChange::ScreenMode(ScreenMode::AtariST(wrapper.0, current_igs)),
                        })
                    },
                )
                .width(Length::Fixed(100.0))
                .text_size(TEXT_SIZE_NORMAL);

                // Resolution info label
                let size = current_resolution.resolution();
                let (cols, _rows) = current_resolution.text_resolution();
                let colors = current_resolution.max_colors();
                let info_text = fl!(
                    crate::LANGUAGE_LOADER,
                    "dialing_directory-resolution-info",
                    width = size.width,
                    height = size.height,
                    cols = cols,
                    colors = colors
                );
                let info_label = text(info_text).size(TEXT_SIZE_SMALL).style(text::secondary);
                server_content = server_content.push(
                    row![
                        left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-resolution")),
                        resolution_pick,
                        info_label
                    ]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
                );

                // IGS (Interactive Graphics System) checkbox
                let igs_toggle = iced::widget::checkbox(current_igs)
                    .on_toggle(move |checked| {
                        Message::from(DialingDirectoryMsg::AddressFieldChanged {
                            id,
                            field: AddressFieldChange::ScreenMode(ScreenMode::AtariST(current_resolution, checked)),
                        })
                    })
                    .text_size(TEXT_SIZE_NORMAL);

                let igs_link = button(text("Enable IGS").size(TEXT_SIZE_NORMAL).wrapping(text::Wrapping::None))
                    .on_press(Message::OpenLink(
                        "https://breakintochat.com/blog/category/instant-graphics-and-sound".to_string(),
                    ))
                    .style(|theme: &iced::Theme, status| {
                        let palette = theme.extended_palette();
                        let base = button::Style {
                            background: None,
                            text_color: palette.primary.strong.color,
                            border: iced::Border::default(),
                            shadow: iced::Shadow::default(),
                            snap: false,
                        };
                        match status {
                            button::Status::Hovered => button::Style {
                                text_color: palette.primary.base.color,
                                ..base
                            },
                            _ => base,
                        }
                    })
                    .width(Length::Fixed(LABEL_WIDTH))
                    .padding(0);

                server_content = server_content.push(row![igs_link, igs_toggle].spacing(DIALOG_SPACING).align_y(Alignment::Center));
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
                .text_size(TEXT_SIZE_NORMAL);

                server_content = server_content.push(
                    row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-music-option")), music_pick]
                        .spacing(DIALOG_SPACING)
                        .align_y(Alignment::Center),
                );
            }

            effect_box(server_content.into()).into()
        };

        // Login Settings Section (only for non-quick connect)
        let login_section: Option<Element<'_, Message>> = if !is_quick {
            let mut login_content = column![].spacing(DIALOG_SPACING);

            // User field
            let user_field = text_input("", &addr.user_name)
                .on_input(move |s| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::User(s),
                    })
                })
                .padding(6)
                .size(TEXT_SIZE_NORMAL)
                .width(Length::Fill);

            login_content = login_content.push(
                row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-user")), user_field]
                    .spacing(DIALOG_SPACING)
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
                .size(TEXT_SIZE_NORMAL)
                .width(Length::Fill);

            let visibility_icon = if self.show_passwords {
                svg(svg::Handle::from_memory(VISIBILITY_SVG))
            } else {
                svg(svg::Handle::from_memory(VISIBILITY_OFF_SVG))
            }
            .width(Length::Fixed(20.0))
            .height(Length::Fixed(20.0));

            let toggler_pw = button(visibility_icon)
                .on_press(Message::from(DialingDirectoryMsg::ToggleShowPasswords))
                .padding(4)
                .style(text_button_style);

            let (generate_btn, tooltip_label) = if addr.password.is_empty() {
                (
                    primary_button(
                        fl!(crate::LANGUAGE_LOADER, "dialing_directory-generate"),
                        Some(Message::from(DialingDirectoryMsg::GeneratePassword)),
                    ),
                    fl!(crate::LANGUAGE_LOADER, "dialing_directory-generate-tooltip"),
                )
            } else {
                (
                    secondary_button(fl!(crate::LANGUAGE_LOADER, "dialing_directory-generate"), None),
                    fl!(crate::LANGUAGE_LOADER, "dialing_directory-generate-disabled-tooltip"),
                )
            };

            let generate_btn = tooltip(
                generate_btn,
                container(text(tooltip_label).size(TEXT_SIZE_SMALL)).style(container::rounded_box),
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
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center),
            );

            /*
            // Auto login combo box
            let mut auto_login_options: Vec<AutoLoginOption> = AUTO_LOGIN_PRESETS
                .iter()
                .map(|(label, value)| AutoLoginOption {
                    label: label.to_string(),
                    value: value.to_string(),
                })
                .collect();

            // Check if current auto_login matches any preset
            let selected_option = auto_login_options.iter().find(|opt| opt.value == addr.auto_login);

            // If no match and not empty, add custom option
            let selected_option = if selected_option.is_none() && !addr.auto_login.is_empty() {
                let custom_option = AutoLoginOption {
                    label: format!("Custom: {}", addr.auto_login),
                    value: addr.auto_login.clone(),
                };
                auto_login_options.push(custom_option.clone());
                Some(custom_option)
            } else {
                selected_option.cloned()
            };

            let auto_login_picker = pick_list(auto_login_options, selected_option, move |option: AutoLoginOption| {
                Message::from(DialingDirectoryMsg::AddressFieldChanged {
                    id,
                    field: AddressFieldChange::AutoLogin(option.value),
                })
            })
            .width(Length::Fill)
            .text_size(TEXT_SIZE_NORMAL);

            login_content = login_content.push(
                row![left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-autologin")), auto_login_picker]
                    .spacing(DIALOG_SPACING)
                    .align_y(Alignment::Center),
            );*/

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
                .size(TEXT_SIZE_NORMAL)
                .width(Length::Fill);

            Some(effect_box(comment.into()).into())
        } else {
            None
        };

        // Options section (only for non-quick connect)
        let options_section: Option<Element<'_, Message>> = if !is_quick {
            let mut options_content = column![].spacing(DIALOG_SPACING);

            // Mouse reporting checkbox
            let mouse_reporting_checkbox = iced::widget::checkbox(addr.mouse_reporting_enabled)
                .on_toggle(move |checked| {
                    Message::from(DialingDirectoryMsg::AddressFieldChanged {
                        id,
                        field: AddressFieldChange::MouseReporting(checked),
                    })
                })
                .text_size(TEXT_SIZE_NORMAL);

            options_content = options_content.push(
                row![
                    left_label(fl!(crate::LANGUAGE_LOADER, "dialing_directory-mouse-reporting")),
                    mouse_reporting_checkbox
                ]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center),
            );

            Some(effect_box(options_content.into()).into())
        } else {
            None
        };

        // Main content layout
        let mut content: iced::widget::Column<'_, Message> = column![header, Space::new().height(SECTION_SPACING), server_section,];

        if !is_quick {
            if let Some(login) = login_section {
                content = content
                    .push(Space::new().height(SECTION_SPACING))
                    .push(section_header(fl!(crate::LANGUAGE_LOADER, "dialing_directory-login-settings")))
                    .push(login);
            }

            if let Some(options) = options_section {
                content = content
                    .push(Space::new().height(SECTION_SPACING))
                    .push(section_header(fl!(crate::LANGUAGE_LOADER, "dialing_directory-options")))
                    .push(options);
            }

            if let Some(notes) = comment_section {
                content = content
                    .push(Space::new().height(SECTION_SPACING))
                    .push(section_header(fl!(crate::LANGUAGE_LOADER, "dialing_directory-notes")))
                    .push(notes);
            }
        } else {
            // Quick connect "Add BBS" button
            let add_btn = if self.quick_connect_address.address.is_empty() {
                secondary_button(fl!(crate::LANGUAGE_LOADER, "dialing_directory-add-bbs-button"), None)
            } else {
                primary_button(
                    fl!(crate::LANGUAGE_LOADER, "dialing_directory-add-bbs-button"),
                    Some(Message::from(DialingDirectoryMsg::AddAddress)),
                )
            };

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
