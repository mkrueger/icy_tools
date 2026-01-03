use crate::ui::MainWindow;
use crate::{
    qwk::{MessageDescriptor, QwkPackage},
    ui::Message,
};
use icy_engine_gui::TerminalView;
use icy_ui::widget::{button, column, container, row, scrollable, text, Space};
use icy_ui::{Alignment, Element, Length};

impl MainWindow {
    pub fn mail_reader_view(&self) -> Element<'_, Message> {
        let Some(package) = &self.package else {
            return container(text("No package loaded")).into();
        };

        // Build conference list
        let conference_list = self.build_conference_list(package);

        // Build message list for current conference
        let message_list = self.build_message_list(package);

        // Build message content view
        let message_view = self.build_message_view();

        // Build thread view if enabled
        let thread_view = if self.show_threads { Some(self.build_thread_view()) } else { None };

        // Toolbar
        let toolbar = self.build_toolbar();

        // Layout the panes
        let mut left_pane = row![].spacing(2);

        // Left sidebar - conferences (wider now to accommodate descriptions)
        left_pane = left_pane.push(
            container(conference_list)
                .width(Length::Fixed(350.0)) // Increased from 200.0 to accommodate wider descriptions
                .height(Length::Fill)
                .style(|theme: &icy_ui::Theme| container::Style {
                    border: icy_ui::Border {
                        width: 1.0,
                        color: theme.primary.divider,
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        );

        // Middle - message list
        let right_pane = column![
            container(message_list)
                .width(Length::FillPortion(2))
                .height(Length::Fixed(250.0))
                .style(|theme: &icy_ui::Theme| container::Style {
                    border: icy_ui::Border {
                        width: 1.0,
                        color: theme.primary.divider,
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            container(message_view)
                .width(Length::FillPortion(3))
                .height(Length::Fill)
                .style(|theme: &icy_ui::Theme| container::Style {
                    border: icy_ui::Border {
                        width: 1.0,
                        color: theme.primary.divider,
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        ];
        // Right - thread view on top, message content below
        if let Some(thread_view) = thread_view {
            let threads = container(thread_view)
                .height(Length::FillPortion(1))
                .style(|theme: &icy_ui::Theme| container::Style {
                    border: icy_ui::Border {
                        width: 1.0,
                        color: theme.primary.divider,
                        ..Default::default()
                    },
                    ..Default::default()
                });

            left_pane = left_pane.push(container(threads).width(Length::FillPortion(3)).height(Length::Fill));
        }

        column![toolbar, row![left_pane, right_pane]].into()
    }

    fn build_toolbar(&self) -> Element<'_, Message> {
        row![
            button(text("📁").size(16)).on_press(Message::OpenPackage).padding(8),
            button(text("↻").size(16)).on_press(Message::Refresh).padding(8),
            button(text("📧").size(16)).on_press(Message::NewMessage).padding(8),
            Space::new().width(Length::Fill),
            text("Show:").size(14),
            button(if self.show_threads { "Hide Threads" } else { "Show Threads" })
                .on_press(Message::ToggleThreadView)
                .padding([4, 8]),
            Space::new().width(8),
            text(format!("Filter: ")).size(14),
            // TODO: Add filter input
        ]
        .spacing(4)
        .padding(4)
        .align_y(Alignment::Center)
        .into()
    }

    fn build_conference_list(&self, package: &QwkPackage) -> Element<'_, Message> {
        let mut conf_list = column![
            // Column headers
            container(
                row![
                    container(text("Area").size(11)).width(Length::Fixed(40.0)),
                    container(text("Description").size(11)).width(Length::Fixed(250.0)),
                    container(text("Msgs").size(11)).width(Length::Fixed(45.0)),
                ]
                .spacing(4)
                .padding([4, 8])
            )
            .width(Length::Fixed(335.0)) // Fixed width for the header
            .style(|theme: &icy_ui::Theme| {
                container::Style {
                    background: Some(icy_ui::Background::Color(theme.primary.divider)),
                    ..Default::default()
                }
            }),
        ]
        .spacing(1);

        // Add "All" option
        let all_count = package.descriptors.len();
        let is_selected = self.selected_conference == 0;
        conf_list = conf_list.push(
            button(
                row![
                    container(text("All").size(12)).width(Length::Fixed(40.0)),
                    container(text("All Conferences").size(12)).width(Length::Fixed(250.0)),
                    container(text(format!("{}", all_count)).size(12)).width(Length::Fixed(45.0)),
                ]
                .spacing(4)
                .padding([2, 8]),
            )
            .on_press(Message::SelectConference(0))
            .padding(0)
            .width(Length::Fixed(335.0)) // Fixed width to match header
            .style(move |theme: &icy_ui::Theme, status| button_style(theme, status, is_selected)),
        );

        // Add conferences from control file
        for conference in package.control_file.conferences.iter() {
            let conf_num = conference.number;
            let conf_name = String::from_utf8_lossy(&conference.name).trim().to_string();

            // Skip empty conference names (unused slots)
            if conf_name.is_empty() {
                continue;
            }

            // Count messages in this conference for display
            let message_count = package.descriptors.iter().filter(|desc| desc.conference == conf_num as u16).count();

            // Skip conferences with no messages
            if message_count == 0 {
                continue;
            }

            let is_selected = self.selected_conference == conf_num;

            conf_list = conf_list.push(
                button(
                    row![
                        container(text(format!("{}", conf_num)).size(12)).width(Length::Fixed(40.0)),
                        container(text(conf_name).size(12)).width(Length::Fixed(250.0)), // Just text, no nested scrollable
                        container(text(format!("{}", message_count)).size(12)).width(Length::Fixed(45.0)),
                    ]
                    .spacing(4)
                    .padding([2, 8]),
                )
                .on_press(Message::SelectConference(conf_num))
                .padding(0)
                .width(Length::Fixed(335.0)) // Fixed width to match header
                .style(move |theme: &icy_ui::Theme, status| button_style(theme, status, is_selected)),
            );
        }

        let conference_scrollable = scrollable(conf_list).width(Length::Fill).direction(scrollable::Direction::Both {
            vertical: scrollable::Scrollbar::default(),
            horizontal: scrollable::Scrollbar::new().scroller_width(4).width(4),
        });

        // Add a focus indicator border
        let conference_scrollable = container(conference_scrollable)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |theme: &icy_ui::Theme| container::Style {
                border: if self.conference_list_focused {
                    icy_ui::Border {
                        width: 2.0,
                        color: theme.accent.base,
                        radius: 0.0.into(),
                    }
                } else {
                    icy_ui::Border::default()
                },
                ..Default::default()
            });

        // Use mouse_area to detect clicks on the conference list area
        icy_ui::widget::mouse_area(conference_scrollable).on_press(Message::FocusConferenceList).into()
    }

    fn build_message_list(&self, package: &QwkPackage) -> Element<'_, Message> {
        let mut message_list = column![
            // Header
            container(
                row![
                    container(text("Author").size(12)).width(Length::FillPortion(2)),
                    container(text("Date").size(12)).width(Length::Fixed(80.0)),
                    container(text("Subject").size(12)).width(Length::FillPortion(3)),
                    container(text("Lines").size(12)).width(Length::Fixed(50.0)),
                ]
                .padding(4)
            )
            .width(Length::Fill)
            .style(|theme: &icy_ui::Theme| {
                container::Style {
                    background: Some(icy_ui::Background::Color(theme.primary.divider)),
                    ..Default::default()
                }
            }),
        ]
        .spacing(1);

        // Filter messages by conference
        let messages: Vec<(usize, &MessageDescriptor)> = if self.selected_conference == 0 {
            package.descriptors.iter().enumerate().collect()
        } else {
            package
                .descriptors
                .iter()
                .enumerate()
                .filter(|(_, h)| h.conference == self.selected_conference as u16)
                .collect()
        };

        // Add messages - load each message to display header info
        for (idx, descriptor) in messages.iter().take(100) {
            // Limit for performance
            let is_selected = self.selected_message == Some(*idx);

            // Load the message to get header fields
            let (from_display, date_display, subject_display) = if let Ok(msg) = package.get_message(*idx) {
                (
                    String::from_utf8_lossy(&msg.from).trim().to_string(),
                    String::from_utf8_lossy(&msg.date_time).trim().to_string(),
                    String::from_utf8_lossy(&msg.subj).trim().to_string(),
                )
            } else {
                // Fallback if message can't be loaded
                ("Unknown".to_string(), "".to_string(), format!("Message #{}", descriptor.number))
            };

            message_list = message_list.push(
                button(
                    row![
                        container(text(from_display).size(12)).width(Length::FillPortion(2)),
                        container(text(date_display).size(12)).width(Length::Fixed(80.0)),
                        container(text(subject_display).size(12)).width(Length::FillPortion(3)),
                        container(text(format!("{}", descriptor.block_count)).size(12)).width(Length::Fixed(50.0)),
                    ]
                    .padding(2),
                )
                .on_press(Message::SelectMessage(*idx))
                .padding(0)
                .width(Length::Fill)
                .style(move |theme: &icy_ui::Theme, status| button_style(theme, status, is_selected)),
            );
        }

        scrollable(message_list)
            .id(self.message_list_scroll.clone())
            .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default()))
            .into()
    }

    fn build_message_view(&self) -> Element<'_, Message> {
        if let Some(msg_idx) = self.selected_message {
            if let Some(package) = &self.package {
                if let Ok(msg) = package.get_message(msg_idx) {
                    // Convert to owned strings to avoid borrowing issues
                    let subject = String::from_utf8_lossy(&msg.subj).trim().to_string();
                    let from = String::from_utf8_lossy(&msg.from).trim().to_string();
                    let to = String::from_utf8_lossy(&msg.to).trim().to_string();
                    let date = String::from_utf8_lossy(&msg.date_time).trim().to_string();

                    let header_info = column![
                        row![text("Subject: ").size(12), text(subject).size(12),],
                        row![text("From: ").size(12), text(from).size(12),],
                        row![text("To: ").size(12), text(to).size(12),],
                        row![text("Date: ").size(12), text(date).size(12),],
                    ]
                    .spacing(2)
                    .padding(8);

                    // Use TerminalView to display the message content with ANSI rendering
                    let terminal_view = TerminalView::show_with_effects(&self.terminal, self.monitor_settings.clone(), None).map(Message::TerminalMessage);

                    return column![
                        container(header_info).width(Length::Fill).style(|theme: &icy_ui::Theme| {
                            container::Style {
                                background: Some(icy_ui::Background::Color(theme.secondary.base)),
                                ..Default::default()
                            }
                        }),
                        container(terminal_view).width(Length::Fill).height(Length::Fill),
                    ]
                    .into();
                }
            }
        }

        container(text("Select a message to read").size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    fn build_thread_view(&self) -> Element<'_, Message> {
        let thread_content = column![
            container(text("Thread").size(14))
                .padding(8)
                .width(Length::Fill)
                .style(|theme: &icy_ui::Theme| {
                    container::Style {
                        background: Some(icy_ui::Background::Color(theme.primary.divider)),
                        ..Default::default()
                    }
                }),
            scrollable(column![text("Thread view coming soon...").size(12),].padding(8))
                .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default())),
        ];

        thread_content.into()
    }
}

fn button_style(theme: &icy_ui::Theme, status: icy_ui::widget::button::Status, is_selected: bool) -> icy_ui::widget::button::Style {
    use icy_ui::widget::button::{Status, Style};

    let base = if is_selected {
        Style {
            background: Some(icy_ui::Background::Color(theme.accent.base)),
            text_color: theme.accent.on,
            border: Default::default(),
            shadow: Default::default(),
            snap: false, // Add the missing field
            ..Default::default()
        }
    } else {
        Style {
            background: Some(icy_ui::Background::Color(icy_ui::Color::TRANSPARENT)),
            text_color: theme.background.on,
            border: Default::default(),
            shadow: Default::default(),
            snap: false, // Add the missing field
            ..Default::default()
        }
    };

    match status {
        Status::Active | Status::Selected => base,
        Status::Hovered if !is_selected => Style {
            background: Some(icy_ui::Background::Color(theme.secondary.base)),
            ..base
        },
        Status::Pressed => Style {
            background: Some(icy_ui::Background::Color(theme.accent.hover)),
            text_color: theme.accent.on,
            ..base
        },
        _ => base,
    }
}
