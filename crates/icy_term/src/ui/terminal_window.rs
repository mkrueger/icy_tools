use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length,
    widget::{Space, button, column, container, row, svg, text},
};
use icy_engine::Screen;
use icy_engine_gui::{ScrollbarOverlay, Terminal, terminal_view::TerminalView};
use icy_engine_gui::{
    music::music::SoundThread,
    ui::{DIALOG_SPACING, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL},
};
use icy_net::serial::Serial;
use icy_net::telnet::TerminalEmulation;
use icy_parser_core::BaudEmulation;
use parking_lot::Mutex;
use std::sync::Arc;
// use iced_aw::{menu, menu_bar, menu_items};

use crate::{Address, LATEST_VERSION, Options, VERSION, ui::Message};

// Icon SVG constants
const DISCONNECT_SVG: &[u8] = include_bytes!("../../data/icons/logout.svg");
const PHONEBOOK_SVG: &[u8] = include_bytes!("../../data/icons/call.svg");
const UPLOAD_SVG: &[u8] = include_bytes!("../../data/icons/upload.svg");
const DOWNLOAD_SVG: &[u8] = include_bytes!("../../data/icons/download.svg");
const _SETTINGS_SVG: &[u8] = include_bytes!("../../data/icons/menu.svg");

pub struct TerminalWindow {
    pub terminal: Terminal,
    pub is_connected: bool,
    pub is_capturing: bool,
    pub current_address: Option<Address>,
    pub serial_connected: Option<Serial>,
    pub terminal_emulation: TerminalEmulation,
    pub baud_emulation: BaudEmulation,
    pub sound_thread: Arc<Mutex<SoundThread>>,
    pub iemsi_info: Option<icy_net::iemsi::EmsiISI>,
}

impl TerminalWindow {
    pub fn new(sound_thread: Arc<Mutex<SoundThread>>) -> Self {
        let edit_screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(super::welcome_screen::create_welcome_screen())));

        Self {
            terminal: Terminal::new(edit_screen),
            is_connected: false,
            is_capturing: false,
            current_address: None,
            serial_connected: None,
            terminal_emulation: TerminalEmulation::Ansi,
            baud_emulation: BaudEmulation::Off,
            sound_thread,
            iemsi_info: None,
        }
    }

    pub fn view(&self, options: &Options, pause_message: &Option<String>) -> Element<'_, Message> {
        // Create the button bar at the top
        let button_bar = self.create_button_bar();

        // Create the main terminal area
        let terminal_view = TerminalView::show_with_effects(&self.terminal, options.monitor_settings.clone()).map(|terminal_msg| match terminal_msg {
            icy_engine_gui::Message::OpenLink(url) => Message::OpenLink(url),
            icy_engine_gui::Message::Copy => Message::Copy,
            icy_engine_gui::Message::Paste => Message::Paste,
            icy_engine_gui::Message::RipCommand(clear_screen, cmd) => Message::RipCommand(clear_screen, cmd),
            icy_engine_gui::Message::SendMouseEvent(evt) => Message::SendMouseEvent(evt),
            icy_engine_gui::Message::ScrollViewport(dx, dy) => Message::ScrollViewport(dx, dy),
            icy_engine_gui::Message::StartSelection(sel) => Message::StartSelection(sel),
            icy_engine_gui::Message::UpdateSelection(pos) => Message::UpdateSelection(pos),
            icy_engine_gui::Message::EndSelection => Message::EndSelection,
            icy_engine_gui::Message::ClearSelection => Message::ClearSelection,
        });

        // Get scrollback info from Box<dyn Screen>

        // Calculate scrollback lines for status bar
        let scrollback_lines = if self.terminal.is_in_scrollback_mode() {
            // Get font dimensions to calculate how many lines we've scrolled
            let screen = self.terminal.screen.lock();
            let font_height = screen.get_font_dimensions().height as f32;
            // Calculate how many lines we've scrolled from the bottom
            let max_scroll_y = (self.terminal.viewport.content_height - self.terminal.viewport.visible_height).max(0.0);
            let scroll_from_bottom = max_scroll_y - self.terminal.viewport.scroll_y;
            (scroll_from_bottom / font_height) as i32
        } else {
            0
        };
        // Create terminal area with optional scrollbar
        let terminal_area = {
            // Create overlay scrollbar - actually drawn using canvas
            let scrollbar_visibility = self.terminal.scrollbar.visibility;
            let scrollbar_height_ratio = self.terminal.viewport.visible_height / self.terminal.viewport.content_height.max(1.0);
            let scrollbar_position = self.terminal.scrollbar.scroll_position;
            let max_scroll_y = self.terminal.viewport.max_scroll_y();

            if self.terminal.is_in_scrollback_mode() {
                let scrollbar_view = ScrollbarOverlay::new(
                    scrollbar_visibility,
                    scrollbar_position,
                    scrollbar_height_ratio,
                    max_scroll_y,
                    self.terminal.scrollbar_hover_state.clone(),
                    |x, y| Message::ScrollViewportTo(false, x, y),
                    |is_hovered| Message::ScrollbarHovered(is_hovered),
                )
                .view();

                // Add scroll position indicator if not at bottom
                let scroll_indicator =
                    container(
                        text(format!("↑ {:04}", scrollback_lines))
                            .size(TEXT_SIZE_SMALL)
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

                // Overlay both indicator and scrollbar on top of terminal
                container(iced::widget::stack![
                    container(terminal_view).width(Length::Fill).height(Length::Fill),
                    container(scroll_indicator)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(iced::alignment::Horizontal::Right)
                        .align_y(iced::alignment::Vertical::Top)
                        .padding([8, 16]),
                    container(scrollbar_view)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(iced::alignment::Horizontal::Right)
                        .align_y(iced::alignment::Vertical::Center)
                        .padding(iced::Padding::from([0.0, 0.0]))
                ])
                .width(Length::Fill)
                .height(Length::Fill)
            } else {
                container(terminal_view)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .width(Length::Fill)
                    .height(Length::Fill)
            }
        };

        // Status bar at the bottom - add scrollback info
        let status_bar = self.create_status_bar(options, pause_message);

        // Combine all elements
        column![button_bar, terminal_area, status_bar].spacing(0).into()
    }

    fn create_update_notification(&self) -> Element<'_, Message> {
        container(
            button(
                row![
                    text(fl!(crate::LANGUAGE_LOADER, "menu-upgrade_version", version = LATEST_VERSION.to_string())).size(TEXT_SIZE_SMALL),
                    text(" →").size(TEXT_SIZE_SMALL)
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
                    text(fl!(crate::LANGUAGE_LOADER, "terminal-dialing_directory")).size(TEXT_SIZE_SMALL)
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
                text(fl!(crate::LANGUAGE_LOADER, "terminal-upload")).size(TEXT_SIZE_SMALL)
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
                text(fl!(crate::LANGUAGE_LOADER, "terminal-download")).size(TEXT_SIZE_SMALL)
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

        bar_content = bar_content.push(container(text(" | ").size(10)).padding([0, 2]));

        // Add Stop Playing Sound button if music is playing
        {
            let mut sound_guard = self.sound_thread.lock();
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
                        text("🔇").size(TEXT_SIZE_NORMAL), // Music stop icon
                        text(button_text).size(TEXT_SIZE_SMALL)
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
                    text(fl!(crate::LANGUAGE_LOADER, "terminal-hangup")).size(TEXT_SIZE_SMALL)
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

    fn create_status_bar(&self, options: &Options, pause_message: &Option<String>) -> Element<'_, Message> {
        let connection_status = if let Some(serial) = &self.serial_connected {
            // Serial connection
            text(format!("{} ({} baud)", serial.device, serial.baud_rate))
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().success.strong.color),
                    ..Default::default()
                })
                .size(16.0)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..iced::Font::default()
                })
        } else if let Some(addr) = &self.current_address {
            if !self.is_connected {
                text("DIALING...").style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().background.base.text),
                    ..Default::default()
                })
            } else {
                let system = if addr.system_name.is_empty() {
                    addr.address.clone()
                } else {
                    addr.system_name.clone()
                };

                text(system)
                    .style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().success.strong.color),
                        ..Default::default()
                    })
                    .size(16.0)
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..iced::Font::default()
                    })
            }
        } else {
            text("NO CARRIER").style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().danger.base.color),
                ..Default::default()
            })
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

        let (buffer_width, buffer_height) = {
            let edit_screen = self.terminal.screen.lock();
            let size = edit_screen.get_size();
            (size.width, size.height)
        };

        let is_serial_mode = self.serial_connected.is_some();

        let connection_string = if is_serial_mode {
            "Serial".to_string()
        } else if let Some(address) = &self.current_address {
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
                icy_net::ConnectionType::Rlogin => "Rlogin".to_string(),
                icy_net::ConnectionType::RloginSwapped => "Rlogin (Swapped)".to_string(),
                _ => "Unknown".to_string(),
            }
        } else {
            "OFFLINE".to_string()
        };

        // Build the status bar row
        let pause_text = if let Some(msg) = pause_message {
            format!(" | {}", msg)
        } else {
            String::new()
        };

        let mut status_row = row![connection_status].spacing(DIALOG_SPACING).align_y(Alignment::Center);

        if self.terminal.is_in_scrollback_mode() {
            status_row = status_row.push(container(text(" | SCROLLBACK").size(TEXT_SIZE_NORMAL)).padding([0, 2]));
        }

        status_row = status_row.push(Space::new().width(Length::Fill));

        if self.is_capturing {
            let stop_capture_btn = button(
                row![
                    text("⏹").size(TEXT_SIZE_SMALL),
                    text(fl!(crate::LANGUAGE_LOADER, "toolbar-stop-capture")).size(TEXT_SIZE_SMALL)
                ]
                .spacing(3)
                .align_y(Alignment::Center),
            )
            .on_press(Message::StopCapture)
            .padding([2, 8])
            .style(|theme: &iced::Theme, status| {
                use iced::widget::button::{Status, Style};

                let palette = theme.extended_palette();
                let danger_color = palette.danger.base.color;

                let base = Style {
                    background: Some(iced::Background::Color(Color::TRANSPARENT)),
                    text_color: danger_color,
                    border: Border {
                        color: danger_color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    shadow: Default::default(),
                    snap: false,
                };

                match status {
                    Status::Active | Status::Disabled => base,
                    Status::Hovered => Style {
                        background: Some(iced::Background::Color(danger_color.scale_alpha(0.15))),
                        text_color: danger_color,
                        border: Border {
                            color: palette.danger.strong.color,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..base
                    },
                    Status::Pressed => Style {
                        background: Some(iced::Background::Color(danger_color.scale_alpha(0.25))),
                        text_color: palette.danger.strong.color,
                        border: Border {
                            color: palette.danger.strong.color,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..base
                    },
                }
            });

            status_row = status_row.push(stop_capture_btn);
        }

        // Add Baud Emulation button
        let baud_text = if !self.is_connected {
            "LOCAL".to_string()
        } else {
            match self.baud_emulation {
                BaudEmulation::Off => fl!(crate::LANGUAGE_LOADER, "select-bps-dialog-bps-max"),
                BaudEmulation::Rate(rate) => fl!(crate::LANGUAGE_LOADER, "select-bps-dialog-bps", bps = rate),
            }
        };

        let mut baud_button = button(text(baud_text).size(TEXT_SIZE_SMALL))
            .padding([2, 8])
            .style(move |theme: &iced::Theme, status| {
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

        if self.is_connected && !is_serial_mode {
            baud_button = baud_button.on_press(Message::ShowBaudEmulationDialog);
        }

        // Don't show baud emulation button in serial mode
        if !is_serial_mode {
            status_row = status_row.push(baud_button);
        }

        // Only add IEMSI button if we have IEMSI info
        if self.iemsi_info.is_some() {
            let iemsi_button = button(text("IEMSI").size(TEXT_SIZE_SMALL))
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

        // Add separator and terminal info as clickable button (without connection status)
        let info_text = format!("{emulation_str} • {buffer_width}x{buffer_height}");
        let info_button = button(text(info_text).size(TEXT_SIZE_SMALL))
            .on_press(Message::ShowTerminalInfoDialog)
            .padding(0)
            .style(|theme: &iced::Theme, status| {
                use iced::widget::button::{Status, Style};

                let palette = theme.extended_palette();
                let base = Style {
                    text_color: palette.secondary.base.color,
                    border: Border::default(),
                    background: None,
                    ..Style::default()
                };

                match status {
                    Status::Active => base,
                    Status::Hovered => Style {
                        text_color: palette.primary.base.color,
                        ..base
                    },
                    Status::Pressed => Style {
                        text_color: palette.primary.strong.color,
                        ..base
                    },
                    Status::Disabled => base,
                }
            });
        status_row = status_row.push(text(" | ").size(TEXT_SIZE_SMALL));
        status_row = status_row.push(info_button);

        // Add connection status as non-clickable text
        status_row = status_row.push(text(format!(" • {connection_string}{pause_text}")).size(TEXT_SIZE_SMALL));

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
        self.serial_connected = None;
        self.iemsi_info = None;
    }

    pub fn toggle_capture(&mut self) {
        self.is_capturing = !self.is_capturing;
    }

    pub fn set_focus(&mut self, has_focus: bool) {
        self.terminal.has_focus = has_focus;
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
