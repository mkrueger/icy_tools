use std::cmp::max;
use std::time::Duration;

use human_bytes::human_bytes;
use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length, Padding, gradient,
    widget::{Space, button, column, container, progress_bar, row, scrollable, text},
};
use iced_engine_gui::ui::{danger_button_style, success_button_style};
use icy_net::protocol::{OutputLogMessage, TransferState};

use crate::ui::MainWindowMode;

const MODAL_WIDTH: f32 = 550.0;
const MODAL_HEIGHT: f32 = 450.0;

#[derive(Debug, Clone)]
pub enum TransferMsg {
    SelectLogTab(LogTab),
    Cancel,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogTab {
    All,
    Warnings,
    Errors,
}

pub struct FileTransferDialogState {
    pub selected_log: LogTab,
    pub transfer_state: Option<TransferState>,
}

impl FileTransferDialogState {
    pub fn new() -> Self {
        Self {
            selected_log: LogTab::All,
            transfer_state: None,
        }
    }

    pub fn update_transfer_state(&mut self, state: TransferState) {
        self.transfer_state = Some(state);
    }

    pub fn update(&mut self, message: TransferMsg) -> Option<crate::ui::Message> {
        match message {
            TransferMsg::SelectLogTab(tab) => {
                self.selected_log = tab;
                None
            }
            TransferMsg::Cancel => {
                if let Some(state) = &self.transfer_state {
                    if state.is_finished {
                        Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)))
                    } else {
                        Some(crate::ui::Message::CancelFileTransfer)
                    }
                } else {
                    Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)))
                }
            }
            TransferMsg::Close => Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal))),
        }
    }

    pub fn view<'a>(&'a self, is_download: bool, terminal_content: Element<'a, crate::ui::Message>) -> Element<'a, crate::ui::Message> {
        let overlay = self.create_modal_content(is_download);
        crate::ui::modal(terminal_content, overlay, crate::ui::Message::TransferDialog(TransferMsg::Close))
    }

    fn create_modal_content(&self, is_download: bool) -> Element<'_, crate::ui::Message> {
        if let Some(state) = &self.transfer_state {
            let transfer_info = if is_download { &state.recieve_state } else { &state.send_state };

            // Header with icon and title
            let icon = if is_download { "⬇" } else { "⬆" };
            let title_text = if is_download {
                fl!(crate::LANGUAGE_LOADER, "transfer-download")
            } else {
                fl!(crate::LANGUAGE_LOADER, "transfer-upload")
            };

            let header = container(
                row![
                    text(icon).size(24).style(if is_download { text::primary } else { text::success }),
                    Space::new().width(8.0),
                    text(title_text).size(20),
                    Space::new().width(Length::Fill),
                    self.create_status_badge(state.is_finished),
                ]
                .align_y(Alignment::Center),
            )
            .padding(Padding::new(16.0).top(16).right(20).bottom(12).left(20))
            .style(|theme: &iced::Theme| container::Style {
                background: Some(iced::Background::Gradient(iced::Gradient::Linear(
                    gradient::Linear::new(0.0)
                        .add_stop(0.0, theme.extended_palette().background.weak.color)
                        .add_stop(1.0, theme.palette().background),
                ))),
                ..Default::default()
            });

            // Progress section with animated style
            let progress = transfer_info.cur_bytes_transfered as f32 / max(1, transfer_info.file_size) as f32;
            let percentage = (progress * 100.0) as u32;

            let progress_section = column![
                row![
                    text(&transfer_info.file_name).size(16).style(text::default),
                    Space::new().width(Length::Fill),
                    text(format!("{}%", percentage))
                        .size(16)
                        .style(if percentage == 100 { text::success } else { text::primary }),
                    Space::new().width(8.0),
                    text(format!(
                        "({} / {})",
                        human_bytes(transfer_info.cur_bytes_transfered as f64),
                        human_bytes(transfer_info.file_size as f64)
                    ))
                    .size(14)
                    .style(text::secondary),
                ]
                .align_y(Alignment::Center),
                Space::new().height(8.0),
                container(progress_bar(0.0..=1.0, progress).style(|theme: &iced::Theme| progress_bar::Style {
                    background: iced::Background::Color(theme.extended_palette().background.strong.color),
                    bar: iced::Background::Color(theme.palette().primary),

                    /*
                    bar: iced::Background::Gradient(iced::Gradient::Linear(
                        gradient::Linear::new(0.0)
                            .add_stop(0.0, theme.palette().primary)
                            .add_stop(1.0, theme.palette().)
                    )),*/
                    border: Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: 4.0.into(),
                    },
                }))
                .height(8.0)
                .style(|_theme: &iced::Theme| container::Style {
                    border: Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                }),
            ]
            .spacing(4);

            // Stats grid
            let bps = state.get_current_bps(is_download);

            let rate_text = format!("{}/s", human_bytes(bps as f64));
            let elapsed_str = format_duration(if is_download {
                state.recieve_state.start_time.elapsed()
            } else {
                state.send_state.start_time.elapsed()
            });

            let stats_grid = row![
                self.create_stat_card("⚡", fl!(crate::LANGUAGE_LOADER, "transfer-rate"), rate_text),
                Space::new().width(12.0),
                self.create_stat_card("⏱", fl!(crate::LANGUAGE_LOADER, "transfer-elapsedtime"), elapsed_str),
                Space::new().width(12.0),
                self.create_stat_card("🔍", fl!(crate::LANGUAGE_LOADER, "transfer-protocol"), state.protocol_name.clone()),
            ];

            let log_section = Some(self.create_log_section(transfer_info));

            // Action buttons
            let button_label = if state.is_finished {
                fl!(crate::LANGUAGE_LOADER, "dialog-ok_button")
            } else {
                fl!(crate::LANGUAGE_LOADER, "dialog-cancel_button")
            };

            let action_button = button(
                row![
                    text(if state.is_finished { "✓" } else { "✕" }).size(14),
                    Space::new().width(4.0),
                    text(button_label).size(14),
                ]
                .align_y(Alignment::Center),
            )
            .on_press(crate::ui::Message::TransferDialog(TransferMsg::Cancel))
            .padding([10, 20])
            .style(if state.is_finished { success_button_style } else { danger_button_style });

            // Build main content
            let mut main_column = column![
                header,
                // rule::Rule::horizontal(1),
                column![progress_section, Space::new().height(16.0), stats_grid,].padding(Padding::new(12.0).right(20).left(20)),
            ];

            if let Some(log) = log_section {
                main_column = main_column.push(log);
            }

            main_column = main_column.push(Space::new().height(Length::Fill));
            main_column = main_column
                .push(container(row![Space::new().width(Length::Fill), action_button,]).padding(Padding::new(12.0).top(12).right(20).bottom(16).left(20)));

            // Modal container with enhanced style
            let modal_content = container(main_column)
                .width(Length::Fixed(MODAL_WIDTH))
                .height(Length::Fixed(MODAL_HEIGHT))
                .style(|theme: &iced::Theme| {
                    let palette = theme.palette();
                    container::Style {
                        background: Some(iced::Background::Color(palette.background)),
                        border: Border {
                            color: theme.extended_palette().background.strong.color,
                            width: 1.0,
                            radius: 12.0.into(),
                        },
                        text_color: Some(palette.text),
                        shadow: iced::Shadow {
                            color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                            offset: iced::Vector::new(0.0, 8.0),
                            blur_radius: 20.0,
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
        } else {
            self.create_loading_state(is_download)
        }
    }

    fn create_status_badge(&self, is_finished: bool) -> Element<'_, crate::ui::Message> {
        let (icon, text_str, style_fn) = if is_finished {
            (
                "✓",
                fl!(crate::LANGUAGE_LOADER, "transfer-status-complete"),
                text::success as fn(&iced::Theme) -> text::Style,
            )
        } else {
            (
                "●",
                fl!(crate::LANGUAGE_LOADER, "transfer-status-active"),
                text::primary as fn(&iced::Theme) -> text::Style,
            )
        };

        container(
            row![
                text(icon).size(10).style(style_fn),
                Space::new().width(4.0),
                text(text_str).size(11).style(style_fn),
            ]
            .align_y(Alignment::Center),
        )
        .padding(Padding::from([4.0, 8.0]))
        .style(move |theme: &iced::Theme| {
            let color = if is_finished { theme.palette().success } else { theme.palette().primary };
            container::Style {
                background: Some(iced::Background::Color(Color::from_rgba(color.r, color.g, color.b, 0.15))),
                border: Border {
                    color,
                    width: 1.0,
                    radius: 12.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
    }

    fn create_stat_card<'a>(&self, icon: &'a str, label: String, value: String) -> Element<'a, crate::ui::Message> {
        container(column![
            row![text(icon).size(14), Space::new().width(4.0), text(label).size(11).style(text::secondary),],
            Space::new().height(4.0),
            text(value).size(13).style(text::primary),
        ])
        .padding(10)
        .width(Length::Fill)
        .style(|theme: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(theme.extended_palette().background.weak.color)),
            border: Border {
                color: theme.extended_palette().background.strong.color,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .into()
    }

    fn create_log_section(&self, transfer_info: &icy_net::protocol::TransferInformation) -> Element<'_, crate::ui::Message> {
        let all_count = transfer_info.log_count();
        let warning_count = transfer_info.warnings();
        let error_count = transfer_info.errors();

        let label_all = fl!(crate::LANGUAGE_LOADER, "transfer-log-all");
        let label_warnings = fl!(crate::LANGUAGE_LOADER, "transfer-log-warnings");
        let label_errors = fl!(crate::LANGUAGE_LOADER, "transfer-log-errors");

        let all_button = self.create_log_tab_button("📋", label_all, all_count, LogTab::All, None);
        let warnings_button = self.create_log_tab_button("⚠", label_warnings, warning_count, LogTab::Warnings, Some(text::warning));
        let errors_button = self.create_log_tab_button("❌", label_errors, error_count, LogTab::Errors, Some(text::danger));

        let tab_row = container(
            row![
                all_button,
                Space::new().width(8.0),
                warnings_button,
                Space::new().width(8.0),
                errors_button,
                Space::new().width(Length::Fill),
            ]
            .align_y(Alignment::Center),
        )
        .padding(Padding::from([0.0, 20.0]).top(0).bottom(8));

        // Log messages with enhanced styling
        let selected_tab = match self.selected_log {
            LogTab::All => 0,
            LogTab::Warnings => 1,
            LogTab::Errors => 2,
        };

        let count = match self.selected_log {
            LogTab::All => all_count,
            LogTab::Warnings => warning_count,
            LogTab::Errors => error_count,
        };

        let mut log_column = column![].spacing(4);
        for i in 0..count {
            if let Some(msg) = transfer_info.get_log_message(selected_tab, i) {
                let log_entry = self.create_log_entry(msg.clone());
                log_column = log_column.push(log_entry);
            }
        }

        let log_scroll =
            container(scrollable(container(log_column).width(Length::Fill).padding(12)).height(Length::Fixed(120.0))).style(|theme: &iced::Theme| {
                container::Style {
                    background: Some(iced::Background::Color(theme.extended_palette().background.weak.color)),
                    border: Border {
                        color: theme.extended_palette().background.strong.color,
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                }
            });

        column![
            //   rule::Rule::horizontal(1),
            Space::new().height(12.0),
            tab_row,
            container(log_scroll).padding(Padding::from([0.0, 20.0])),
        ]
        .into()
    }

    fn create_log_tab_button<'a>(
        &self,
        icon: &'a str,
        label: String,
        count: usize,
        tab: LogTab,
        _text_style: Option<fn(&iced::Theme) -> text::Style>,
    ) -> Element<'a, crate::ui::Message> {
        let is_selected = self.selected_log == tab;

        button(row![text(icon).size(12), Space::new().width(4.0), text(format!("{} ({})", label, count)).size(12)].align_y(Alignment::Center))
            .on_press(crate::ui::Message::TransferDialog(TransferMsg::SelectLogTab(tab)))
            .padding([6, 12])
            .style(move |theme: &iced::Theme, status| {
                if is_selected {
                    button::primary(theme, status)
                } else {
                    button::secondary(theme, status)
                }
            })
            .into()
    }

    fn create_log_entry(&self, msg: OutputLogMessage) -> Element<'_, crate::ui::Message> {
        let (icon, text_style, message): (String, fn(&iced::Theme) -> text::Style, String) = match &msg {
            OutputLogMessage::Error(msg) => ("●".to_string(), text::danger, msg.clone()),
            OutputLogMessage::Warning(msg) => ("▲".to_string(), text::warning, msg.clone()),
            OutputLogMessage::Info(msg) => ("ℹ".to_string(), text::secondary, msg.clone()),
        };

        container(row![
            text(icon).size(10).style(text_style),
            Space::new().width(8.0),
            text(message).size(12).style(text::default),
        ])
        .padding(Padding::from([4.0, 8.0]))
        .style(move |theme: &iced::Theme| {
            let color = match &msg {
                OutputLogMessage::Error(_) => theme.palette().danger,
                OutputLogMessage::Warning(_) => theme.extended_palette().danger.weak.color,
                OutputLogMessage::Info(_) => theme.extended_palette().secondary.weak.color,
            };
            container::Style {
                background: Some(iced::Background::Color(Color::from_rgba(color.r, color.g, color.b, 0.08))),
                border: Border {
                    color: Color::from_rgba(color.r, color.g, color.b, 0.2),
                    width: 0.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
    }

    fn create_loading_state(&self, is_download: bool) -> Element<'_, crate::ui::Message> {
        let icon = if is_download { "⬇" } else { "⬆" };
        let title = if is_download {
            fl!(crate::LANGUAGE_LOADER, "transfer-download")
        } else {
            fl!(crate::LANGUAGE_LOADER, "transfer-upload")
        };
        let waiting_text = fl!(crate::LANGUAGE_LOADER, "transfer-waiting");

        let loading_content = container(
            column![
                row![
                    text(icon).size(32).style(if is_download { text::primary } else { text::success }),
                    Space::new().width(12.0),
                    text(title).size(20),
                ]
                .align_y(Alignment::Center),
                Space::new().height(24.0),
                container(
                    row![
                        text("⏳").size(14).style(text::secondary),
                        Space::new().width(6.0),
                        text(waiting_text).size(14).style(text::secondary),
                    ]
                    .align_y(Alignment::Center)
                )
                .padding(16)
                .style(|theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(theme.extended_palette().background.weak.color)),
                    border: Border {
                        color: theme.extended_palette().background.strong.color,
                        width: 1.0,
                        radius: 8.0.into(),
                    },
                    ..Default::default()
                }),
                Space::new().height(24.0),
                row![
                    Space::new().width(Length::Fill),
                    button(row![
                        text("✕").size(14),
                        Space::new().width(4.0),
                        text(fl!(crate::LANGUAGE_LOADER, "dialog-cancel_button")).size(14),
                    ])
                    .on_press(crate::ui::Message::TransferDialog(TransferMsg::Close))
                    .padding([10, 20])
                    .style(button::secondary),
                ]
            ]
            .padding(24)
            .spacing(8),
        )
        .width(Length::Fixed(420.0))
        .height(Length::Shrink)
        .style(|theme: &iced::Theme| {
            let palette = theme.palette();
            container::Style {
                background: Some(iced::Background::Color(palette.background)),
                border: Border {
                    color: theme.extended_palette().background.strong.color,
                    width: 1.0,
                    radius: 12.0.into(),
                },
                text_color: Some(palette.text),
                shadow: iced::Shadow {
                    color: Color::from_rgba8(0, 0, 0, 1.0),
                    offset: iced::Vector::new(0.0, 8.0),
                    blur_radius: 20.0,
                },
                snap: false,
            }
        });

        container(loading_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}

fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}
