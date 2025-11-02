use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length,
    widget::{Space, button, column, container, row, svg, text, vertical_slider},
};
use iced_engine_gui::{Terminal, terminal_view::TerminalView};
use icy_engine::{Buffer, TextPane, ansi::BaudEmulation, editor::EditState};
use icy_net::telnet::TerminalEmulation;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
// use iced_aw::{menu, menu_bar, menu_items};

use crate::{Address, LATEST_VERSION, Options, VERSION, ui::Message, util::SoundThread};

// Icon SVG constants
const DISCONNECT_SVG: &[u8] = include_bytes!("../../data/icons/logout.svg");
const PHONEBOOK_SVG: &[u8] = include_bytes!("../../data/icons/call.svg");
const UPLOAD_SVG: &[u8] = include_bytes!("../../data/icons/upload.svg");
const DOWNLOAD_SVG: &[u8] = include_bytes!("../../data/icons/download.svg");
const _SETTINGS_SVG: &[u8] = include_bytes!("../../data/icons/menu.svg");
const MAIN_SCREEN_ANSI: &[u8] = include_bytes!("../../data/main_screen.icy");

pub struct TerminalWindow {
    pub scene: Terminal,
    pub is_connected: bool,
    pub is_capturing: bool,
    pub current_address: Option<Address>,
    pub terminal_emulation: TerminalEmulation,
    pub baud_emulation: BaudEmulation,
    pub sound_thread: Arc<Mutex<SoundThread>>,
    pub iemsi_info: Option<icy_net::iemsi::EmsiISI>,
}

impl TerminalWindow {
    pub fn new(sound_thread: Arc<Mutex<SoundThread>>) -> Self {
        // Create a default EditState wrapped in Arc<Mutex>
        let edit_state: Arc<Mutex<EditState>> = Arc::new(Mutex::new(EditState::default()));
        // If parsing fails, try using the ANSI parser directly
        let mut buffer = Buffer::from_bytes(&Path::new("a.icy"), true, MAIN_SCREEN_ANSI, None, None).unwrap();
        buffer.buffer_type = icy_engine::BufferType::CP437;
        buffer.is_terminal_buffer = true;
        buffer.terminal_state.fixed_size = true;
        buffer.update_hyperlinks();

        edit_state.lock().unwrap().set_buffer(buffer);
        edit_state.lock().unwrap().get_caret_mut().set_is_visible(false);

        Self {
            scene: Terminal::new(edit_state),
            is_connected: false,
            is_capturing: false,
            current_address: None,
            terminal_emulation: TerminalEmulation::Ansi,
            baud_emulation: BaudEmulation::Off,
            sound_thread,
            iemsi_info: None,
        }
    }

    pub fn view(&self, options: &Options) -> Element<'_, Message> {
        // Create the button bar at the top
        let button_bar = self.create_button_bar();

        // Create the main terminal area
        let terminal_view = TerminalView::show_with_effects(&self.scene, options.monitor_settings.clone()).map(|terminal_msg| {
            match terminal_msg {
                iced_engine_gui::Message::Scroll(lines) => Message::ScrollRelative(lines),
                iced_engine_gui::Message::OpenLink(url) => Message::OpenLink(url),
                iced_engine_gui::Message::Copy => Message::Copy,
                iced_engine_gui::Message::RipCommand(_cmd) => {
                    // TODO: Handle RIP command
                    Message::None
                } // _ => Message::None,
            }
        });

        // Get scrollback info from EditState
        let (has_scrollback, scroll_position, max_scroll) = if let Ok(edit_state) = self.scene.edit_state.lock() {
            let buffer = edit_state.get_buffer();
            let has_scrollback = !buffer.scrollback_lines.is_empty();
            let scroll_offset = edit_state.scrollback_offset as i32;
            let max_scroll = buffer.scrollback_lines.len() as i32;
            (has_scrollback, scroll_offset, max_scroll)
        } else {
            (false, 0, 0)
        };

        // Create terminal area with optional scrollbar
        let terminal_area = if has_scrollback && max_scroll > 0 {
            // Create a custom scrollbar using a vertical slider
            let scrollbar = vertical_slider(
                0..=max_scroll,
                (scroll_position) as i32, // Invert: 0 at bottom, max at top
                move |value| Message::ScrollTerminal((value) as usize),
            )
            .width(12)
            .height(Length::Fill)
            .step(1);

            // Combine terminal view and scrollbar side by side
            let terminal_with_scrollbar = row![
                container(terminal_view).width(Length::Fill).height(Length::Fill),
                container(scrollbar)
                    .width(Length::Fixed(16.0))
                    .height(Length::Fill)
                    .style(|theme: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(theme.extended_palette().background.weak.color)),
                        border: Border {
                            color: theme.extended_palette().background.strong.color,
                            width: 0.0,
                            radius: 0.0.into(),
                        },
                        ..Default::default()
                    })
            ]
            .spacing(0);

            // Add scroll position indicator if not at bottom
            if scroll_position < 0 {
                let lines_scrolled = -scroll_position;
                let scroll_indicator = container(
                    text(format!("↑ {} lines", lines_scrolled))
                        .size(10)
                        .style(|theme: &iced::Theme| iced::widget::text::Style {
                            color: Some(theme.extended_palette().primary.weak.color),
                            ..Default::default()
                        }),
                )
                .padding([2, 8])
                .style(|theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.7))),
                    border: Border {
                        color: theme.extended_palette().background.strong.color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                });

                // Overlay the indicator on top-right of terminal
                container(iced::widget::stack![
                    terminal_with_scrollbar,
                    container(scroll_indicator)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(iced::alignment::Horizontal::Right)
                        .align_y(iced::alignment::Vertical::Top)
                        .padding(8)
                ])
                .width(Length::Fill)
                .height(Length::Fill)
            } else {
                container(terminal_with_scrollbar).width(Length::Fill).height(Length::Fill)
            }
        } else {
            // No scrollback - just show terminal without scrollbar
            container(terminal_view).width(Length::Fill).height(Length::Fill)
        };

        // Status bar at the bottom - add scrollback info
        let status_bar = self.create_status_bar(options, scroll_position);

        // Combine all elements
        column![button_bar, terminal_area, status_bar].spacing(0).into()
    }

    fn create_update_notification(&self) -> Element<'_, Message> {
        container(
            button(
                row![
                    text("🎉 "),
                    text(fl!(crate::LANGUAGE_LOADER, "menu-upgrade_version", version = LATEST_VERSION.to_string())).size(12),
                    text(" →").size(12)
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .on_press(Message::OpenReleaseLink)
            .padding([4, 8])
            .style(|_theme: &iced::Theme, status| {
                use iced::widget::button::{Status, Style};

                let info_color = Color::from_rgb(0.2, 0.6, 1.0);
                let base = Style {
                    background: Some(iced::Background::Color(Color::TRANSPARENT)),
                    text_color: info_color,
                    border: Border::default(),
                    shadow: Default::default(),
                    snap: false,
                };

                match status {
                    Status::Active => base,
                    Status::Hovered => Style {
                        background: Some(iced::Background::Color(Color::from_rgba(info_color.r, info_color.g, info_color.b, 0.1))),
                        ..base
                    },
                    Status::Pressed => Style {
                        background: Some(iced::Background::Color(Color::from_rgba(info_color.r, info_color.g, info_color.b, 0.15))),
                        ..base
                    },
                    Status::Disabled => base,
                }
            }),
        )
        .width(Length::Shrink)
        .padding([2, 6])
        .style(|theme: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(Color::from_rgba(0.2, 0.6, 1.0, 0.05))),
            border: iced::Border {
                color: theme.extended_palette().background.strong.color,
                width: 0.0,
                radius: 0.0.into(),
            },
            text_color: None,
            shadow: Default::default(),
            snap: false,
        })
        .into()
    }

    fn create_button_bar(&self) -> Element<'_, Message> {
        // Phonebook/Connect button (serves dual purpose)
        let phonebook_btn = {
            // When disconnected, show phonebook (connect) button
            button(
                row![
                    svg(svg::Handle::from_memory(PHONEBOOK_SVG))
                        .width(Length::Fixed(16.0))
                        .height(Length::Fixed(16.0)),
                    text(fl!(crate::LANGUAGE_LOADER, "terminal-dialing_directory")).size(12)
                ]
                .spacing(3)
                .align_y(Alignment::Center),
            )
            .on_press(Message::ShowDialingDirectory)
            .padding([4, 6])
            .style(button::primary)
        };

        // Upload button
        let mut upload_btn = button(
            row![
                svg(svg::Handle::from_memory(UPLOAD_SVG)).width(Length::Fixed(16.0)).height(Length::Fixed(16.0)),
                text(fl!(crate::LANGUAGE_LOADER, "terminal-upload")).size(12)
            ]
            .spacing(3)
            .align_y(Alignment::Center),
        )
        .padding([4, 6]);

        // Download button
        let mut download_btn = button(
            row![
                svg(svg::Handle::from_memory(DOWNLOAD_SVG))
                    .width(Length::Fixed(16.0))
                    .height(Length::Fixed(16.0)),
                text(fl!(crate::LANGUAGE_LOADER, "terminal-download")).size(12)
            ]
            .spacing(3)
            .align_y(Alignment::Center),
        )
        .padding([4, 6]);

        if self.is_connected {
            upload_btn = upload_btn.on_press(Message::Upload);
            download_btn = download_btn.on_press(Message::Download);
        }

        let mut bar_content = row![phonebook_btn, container(text(" | ").size(10)).padding([0, 2]), upload_btn, download_btn,]
            .spacing(3)
            .align_y(Alignment::Center);
        /*
        // Only show Send Login button when connected and credentials are available
        if self.is_connected {
            if let Some(address) = &self.current_address {
                if !address.user_name.is_empty() && !address.password.is_empty() {
                    let send_login_btn = button(
                        row![
                            text("🔑").size(14), // or use svg(svg::Handle::from_memory(LOGIN_SVG))
                            text(fl!(crate::LANGUAGE_LOADER, "terminal-autologin")).size(12)
                        ]
                        .spacing(3)
                        .align_y(Alignment::Center),
                    )
                    .on_press(Message::SendLoginAndPassword(true, true))
                    .padding([4, 6]);

                    bar_content = bar_content.push(send_login_btn);
                }
            }
        }*/

        bar_content = bar_content.push(container(text(" | ").size(10)).padding([0, 2]));

        // Add Stop Playing Sound button if music is playing
        if let Ok(mut sound_guard) = self.sound_thread.lock() {
            let _ = sound_guard.update_state();
            if sound_guard.is_playing() {
                let button_text = match sound_guard.stop_button {
                    0 => fl!(crate::LANGUAGE_LOADER, "toolbar-stop-playing1"),
                    1 => fl!(crate::LANGUAGE_LOADER, "toolbar-stop-playing2"),
                    2 => fl!(crate::LANGUAGE_LOADER, "toolbar-stop-playing3"),
                    3 => fl!(crate::LANGUAGE_LOADER, "toolbar-stop-playing4"),
                    4 => fl!(crate::LANGUAGE_LOADER, "toolbar-stop-playing5"),
                    _ => fl!(crate::LANGUAGE_LOADER, "toolbar-stop-playing6"),
                };

                let stop_sound_btn = button(
                    row![
                        text("🔇").size(14), // Music stop icon
                        text(button_text).size(12)
                    ]
                    .spacing(3)
                    .align_y(Alignment::Center),
                )
                .on_press(Message::StopSound)
                .padding([4, 6])
                .style(|_theme: &iced::Theme, status| {
                    use iced::widget::button::{Status, Style};

                    let base = Style {
                        background: Some(iced::Background::Color(Color::from_rgba(1.0, 0.5, 0.0, 0.2))), // Orange tint
                        text_color: Color::from_rgb(1.0, 0.6, 0.0),                                      // Orange text
                        border: Border {
                            color: Color::from_rgb(1.0, 0.5, 0.0),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        shadow: Default::default(),
                        snap: false,
                    };

                    match status {
                        Status::Active => base,
                        Status::Hovered => Style {
                            background: Some(iced::Background::Color(Color::from_rgba(1.0, 0.5, 0.0, 0.3))),
                            ..base
                        },
                        Status::Pressed => Style {
                            background: Some(iced::Background::Color(Color::from_rgba(1.0, 0.5, 0.0, 0.4))),
                            ..base
                        },
                        Status::Disabled => base,
                    }
                });

                bar_content = bar_content.push(stop_sound_btn);
                bar_content = bar_content.push(container(text(" | ").size(10)).padding([0, 2]));
            }
        }

        if self.is_capturing {
            let stop_capture_btn = button(
                row![text("⏹").size(14), text(fl!(crate::LANGUAGE_LOADER, "toolbar-stop-capture")).size(12)]
                    .spacing(3)
                    .align_y(Alignment::Center),
            )
            .on_press(Message::StopCapture)
            .padding([4, 6])
            .style(button::danger);

            bar_content = bar_content.push(stop_capture_btn);
        }

        if *VERSION < *LATEST_VERSION {
            bar_content = bar_content.push(self.create_update_notification());
        }

        bar_content = bar_content.push(Space::new().width(Length::Fill));

        // Settings dropdown menu
        /*let settings_menu = button(
            row![
                svg(svg::Handle::from_memory(SETTINGS_SVG))
                    .width(Length::Fixed(16.0))
                    .height(Length::Fixed(16.0))
            ]
            .spacing(3)
            .align_y(Alignment::Center),
        )*/

        if self.is_connected {
            // When connected, show disconnect button
            let hangup_button = button(
                row![
                    svg(svg::Handle::from_memory(DISCONNECT_SVG))
                        .width(Length::Fixed(16.0))
                        .height(Length::Fixed(16.0)),
                    text(fl!(crate::LANGUAGE_LOADER, "terminal-hangup")).size(12)
                ]
                .spacing(3)
                .align_y(Alignment::Center),
            )
            .on_press(Message::Hangup)
            .padding([4, 6])
            .style(button::danger);
            bar_content = bar_content.push(hangup_button);
            bar_content = bar_content.push(container(text(" | ").size(10)).padding([0, 2]));
        }

        let settings_menu = button(
            text("⚙").size(16), // Gear symbol - most common for settings
        )
        .on_press(Message::ShowSettings)
        .padding([4, 6]);

        bar_content = bar_content.push(settings_menu);

        container(bar_content.padding([3, 6]))
            .style(|theme: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(theme.extended_palette().background.weak.color)),
                border: iced::Border {
                    color: theme.extended_palette().background.strong.color,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                text_color: None,
                shadow: Default::default(),
                snap: false,
            })
            .into()
    }

    fn create_status_bar(&self, options: &Options, scrollback_lines: i32) -> Element<'_, Message> {
        let connection_status = if self.is_connected {
            text("● Connected").style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().success.strong.color),
                ..Default::default()
            })
        } else {
            text("○ Disconnected").style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().danger.base.color),
                ..Default::default()
            })
        };

        let capture_status = if self.is_capturing {
            text("● REC").style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().danger.strong.color),
                ..Default::default()
            })
        } else {
            text("")
        };

        let emulation_str = match self.terminal_emulation {
            TerminalEmulation::Ansi => "ANSI",
            TerminalEmulation::Utf8Ansi => "UTF-8 ANSI",
            TerminalEmulation::Ascii => "ASCII",
            TerminalEmulation::PETscii => "PETSCII",
            TerminalEmulation::ViewData => "ViewData",
            TerminalEmulation::Mode7 => "Mode7",
            TerminalEmulation::Avatar => "AVATAR",
            TerminalEmulation::Rip => "RIP",
            TerminalEmulation::ATAscii => "Atari",
            TerminalEmulation::Skypix => "Amiga Skypix",
            TerminalEmulation::AtariST => "Atari ST",
        };

        let (buffer_width, buffer_height) = if let Ok(edit_state) = self.scene.edit_state.lock() {
            let size = edit_state.get_buffer().get_size();
            (size.width, size.height)
        } else {
            (80, 25)
        };

        let connection_string = if let Some(address) = &self.current_address {
            match address.protocol {
                icy_net::ConnectionType::Telnet => "Telnet".to_string(),
                icy_net::ConnectionType::SSH => "SSH".to_string(),
                icy_net::ConnectionType::Raw => "Raw".to_string(),
                icy_net::ConnectionType::Modem => {
                    if let Some(modem) = options.modems.iter().find(|p| p.name == address.address) {
                        format!("{} baud", modem.baud_rate)
                    } else {
                        "Modem".to_string()
                    }
                }
                icy_net::ConnectionType::Websocket => "WebSocket".to_string(),
                icy_net::ConnectionType::SecureWebsocket => "WSS".to_string(),
                _ => "Unknown".to_string(),
            }
        } else {
            "OFFLINE".to_string()
        };

        // Build the status bar row
        let mut status_row = row![connection_status].spacing(8).align_y(Alignment::Center);

        if scrollback_lines > 0 {
            let scrollback_text = text(format!("↑{:04}", scrollback_lines))
                .size(14)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().secondary.base.color),
                    ..Default::default()
                });

            status_row = status_row.push(text("|")).push(scrollback_text);
        }

        status_row = status_row.push(Space::new().width(Length::Fill)).push(capture_status);

        // Add Baud Emulation button
        let baud_text = if !self.is_connected {
            "LOCAL".to_string()
        } else {
            match self.baud_emulation {
                BaudEmulation::Off => fl!(crate::LANGUAGE_LOADER, "select-bps-dialog-bps-max"),
                BaudEmulation::Rate(rate) => fl!(crate::LANGUAGE_LOADER, "select-bps-dialog-bps", bps = rate),
            }
        };

        let mut baud_button = button(text(baud_text).size(12)).padding([2, 8]).style(move |theme: &iced::Theme, status| {
            use iced::widget::button::{Status, Style};

            let palette = theme.extended_palette();

            // Different styling for disabled state (not connected)
            if !self.is_connected {
                return Style {
                    background: Some(iced::Background::Color(Color::TRANSPARENT)),
                    text_color: palette.secondary.base.color.scale_alpha(0.5),
                    border: Border {
                        color: palette.secondary.base.color.scale_alpha(0.3),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    shadow: Default::default(),
                    snap: false,
                };
            }

            // Normal styling when connected
            let base = Style {
                background: Some(iced::Background::Color(Color::TRANSPARENT)),
                text_color: palette.primary.strong.color,
                border: Border {
                    color: palette.primary.weak.color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                shadow: Default::default(),
                snap: false,
            };

            match status {
                Status::Active | Status::Disabled => base,
                Status::Hovered => Style {
                    background: Some(iced::Background::Color(palette.primary.weak.color)),
                    text_color: palette.primary.weak.text,
                    ..base
                },
                Status::Pressed => Style {
                    background: Some(iced::Background::Color(palette.primary.strong.color)),
                    text_color: palette.primary.strong.text,
                    ..base
                },
            }
        });

        if self.is_connected {
            baud_button = baud_button.on_press(Message::ShowBaudEmulationDialog);
        }

        status_row = status_row.push(baud_button);

        // Only add IEMSI button if we have IEMSI info
        if self.iemsi_info.is_some() {
            let iemsi_button = button(text("IEMSI").size(12))
                .on_press(Message::ShowIemsiDialog)
                .padding([2, 8])
                .style(|theme: &iced::Theme, status| {
                    use iced::widget::button::{Status, Style};

                    let palette = theme.extended_palette();
                    let base = Style {
                        background: Some(iced::Background::Color(Color::TRANSPARENT)),
                        text_color: palette.primary.strong.color,
                        border: Border {
                            color: palette.primary.weak.color,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        shadow: Default::default(),
                        snap: false,
                    };

                    match status {
                        Status::Active => base,
                        Status::Hovered => Style {
                            background: Some(iced::Background::Color(palette.primary.weak.color)),
                            text_color: palette.primary.weak.text,
                            ..base
                        },
                        Status::Pressed => Style {
                            background: Some(iced::Background::Color(palette.primary.strong.color)),
                            text_color: palette.primary.strong.text,
                            ..base
                        },
                        Status::Disabled => base,
                    }
                });

            status_row = status_row.push(iemsi_button);
        }

        // Add separator and terminal info
        status_row = status_row
            .push(text(" | "))
            .push(text(format!("{emulation_str} • {buffer_width}x{buffer_height} • {connection_string}")).size(12));

        container(status_row.padding([4, 12]))
            .style(|theme: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(theme.extended_palette().background.weak.color)),
                border: iced::Border {
                    color: theme.extended_palette().background.strong.color,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                text_color: Some(theme.extended_palette().secondary.base.color),
                shadow: Default::default(),
                snap: false,
            })
            .into()
    }

    // Helper methods for terminal operations
    pub fn connect(&mut self, address: Option<Address>) {
        self.is_connected = true;
        self.current_address = address;

        self.terminal_emulation = match &self.current_address {
            Some(addr) => {
                self.baud_emulation = addr.baud_emulation;
                addr.terminal_type.clone()
            }
            None => TerminalEmulation::Ansi,
        };
    }

    pub fn disconnect(&mut self) {
        self.is_connected = false;
        self.current_address = None;
        self.iemsi_info = None;
    }

    pub fn toggle_capture(&mut self) {
        self.is_capturing = !self.is_capturing;
    }
}

// Helper function to create menu buttons
fn _menu_button<'a>(content: impl Into<Element<'a, Message>>, msg: Message) -> button::Button<'a, Message> {
    button(content)
        .padding([6, 12])
        .width(Length::Fill)
        .style(|theme: &iced::Theme, status| {
            use iced::widget::button::{Status, Style};

            let palette = theme.extended_palette();
            let base = Style {
                text_color: palette.background.base.text,
                border: Border::default().rounded(4.0),
                ..Style::default()
            };

            match status {
                Status::Active => base.with_background(Color::TRANSPARENT),
                Status::Hovered => base.with_background(Color::from_rgba(
                    palette.primary.weak.color.r,
                    palette.primary.weak.color.g,
                    palette.primary.weak.color.b,
                    0.3,
                )),
                Status::Pressed => base.with_background(palette.primary.weak.color),
                _ => base,
            }
        })
        .on_press(msg)
}
