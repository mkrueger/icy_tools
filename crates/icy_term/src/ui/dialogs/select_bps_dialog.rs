use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length,
    widget::{Space, column, container, radio, row, scrollable, text, text_input},
};
use icy_engine_gui::ui::*;
use icy_parser_core::BaudEmulation;

use crate::ui::MainWindowMode;

// Standard baud rates - index 0 is Off, 1-12 are rates, 13 is custom
pub const STANDARD_RATES: [Option<u32>; 13] = [
    None,         // 0: Off
    Some(300),    // 1
    Some(1200),   // 2
    Some(2400),   // 3
    Some(4800),   // 4
    Some(9600),   // 5
    Some(14400),  // 6
    Some(19200),  // 7
    Some(28800),  // 8
    Some(38400),  // 9
    Some(57600),  // 10
    Some(115200), // 11
    None,         // 12: Custom (placeholder)
];

const CUSTOM_INDEX: usize = 12;

pub struct SelectBpsDialog {
    pub selected_index: usize,
    pub custom_rate: String,
}

#[derive(Debug, Clone)]
pub enum SelectBpsMsg {
    SelectBps(usize),
    CustomBpsChanged(String),
}

impl SelectBpsDialog {
    pub fn new(current_bps: BaudEmulation) -> Self {
        let (selected_index, custom_rate) = match current_bps {
            BaudEmulation::Off => (0, String::new()),
            BaudEmulation::Rate(rate) => {
                // Check if it's a standard rate
                if let Some(idx) = STANDARD_RATES[1..12].iter().position(|&r| r == Some(rate)) {
                    (idx + 1, String::new())
                } else {
                    // Custom rate
                    (CUSTOM_INDEX, rate.to_string())
                }
            }
        };

        Self { selected_index, custom_rate }
    }

    pub fn set_emulation(&mut self, baud: BaudEmulation) {
        let (selected_index, custom_rate) = match baud {
            BaudEmulation::Off => (0, String::new()),
            BaudEmulation::Rate(rate) => {
                if let Some(idx) = STANDARD_RATES[1..12].iter().position(|&r| r == Some(rate)) {
                    (idx + 1, self.custom_rate.clone())
                } else {
                    (CUSTOM_INDEX, rate.to_string())
                }
            }
        };
        self.selected_index = selected_index;
        if selected_index == CUSTOM_INDEX {
            self.custom_rate = custom_rate;
        }
    }

    pub fn get_emulation(&self) -> BaudEmulation {
        if self.selected_index == 0 {
            BaudEmulation::Off
        } else if self.selected_index == CUSTOM_INDEX {
            if let Ok(rate) = self.custom_rate.parse::<u32>() {
                if rate > 0 { BaudEmulation::Rate(rate) } else { BaudEmulation::Off }
            } else {
                BaudEmulation::Off
            }
        } else if let Some(Some(rate)) = STANDARD_RATES.get(self.selected_index) {
            BaudEmulation::Rate(*rate)
        } else {
            BaudEmulation::Off
        }
    }

    pub fn update(&mut self, message: SelectBpsMsg) -> Option<crate::ui::Message> {
        match message {
            SelectBpsMsg::SelectBps(index) => {
                self.selected_index = index;
                if index == CUSTOM_INDEX && self.custom_rate.is_empty() {
                    self.custom_rate = "2400".to_string();
                }
                None
            }
            SelectBpsMsg::CustomBpsChanged(value) => {
                // Only allow digits
                self.custom_rate = value.chars().filter(|c| c.is_ascii_digit()).collect();
                None
            }
        }
    }

    pub fn view<'a>(&'a self, terminal_content: Element<'a, crate::ui::Message>) -> Element<'a, crate::ui::Message> {
        crate::ui::modal(
            terminal_content,
            self.create_modal_content(),
            crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)),
        )
    }

    fn create_modal_content(&self) -> Element<'_, crate::ui::Message> {
        let title = dialog_title(fl!(crate::LANGUAGE_LOADER, "select-bps-dialog-heading"));

        let mut list = column![].spacing(4);

        // Add standard options (indices 0-11)
        for (idx, rate_opt) in STANDARD_RATES[0..12].iter().enumerate() {
            let label = match rate_opt {
                None if idx == 0 => fl!(crate::LANGUAGE_LOADER, "select-bps-dialog-bps-max"),
                Some(rate) => fl!(crate::LANGUAGE_LOADER, "select-bps-dialog-bps", bps = rate),
                _ => continue,
            };

            let is_selected = self.selected_index == idx;

            let item = radio(
                label,
                idx,
                if self.selected_index == CUSTOM_INDEX {
                    None
                } else {
                    Some(self.selected_index)
                },
                |idx| crate::ui::Message::SelectBpsMsg(SelectBpsMsg::SelectBps(idx)),
            )
            .size(14)
            .spacing(8)
            .text_size(14)
            .style(move |theme: &iced::Theme, _status| {
                let palette = theme.extended_palette();
                iced::widget::radio::Style {
                    background: iced::Background::Color(if is_selected {
                        palette.primary.base.color
                    } else {
                        Color::from_rgba(0.18, 0.18, 0.18, 0.85)
                    }),
                    dot_color: Color::WHITE,
                    border_color: if is_selected {
                        palette.primary.strong.color
                    } else {
                        Color::from_rgba(0.5, 0.5, 0.5, 0.4)
                    },
                    border_width: 1.0,
                    text_color: Some(theme.palette().text),
                }
            });

            list = list.push(item);
        }

        // Custom rate row (index 12)
        let is_custom_selected = self.selected_index == CUSTOM_INDEX;
        let custom_row = {
            let radio_custom = radio(
                fl!(crate::LANGUAGE_LOADER, "select-bps-dialog-bps-custom"),
                CUSTOM_INDEX,
                Some(self.selected_index),
                |idx| crate::ui::Message::SelectBpsMsg(SelectBpsMsg::SelectBps(idx)),
            )
            .size(14)
            .spacing(8)
            .text_size(14)
            .style(move |theme: &iced::Theme, _| {
                let palette = theme.extended_palette();
                iced::widget::radio::Style {
                    background: iced::Background::Color(if is_custom_selected {
                        palette.primary.base.color
                    } else {
                        Color::from_rgba(0.18, 0.18, 0.18, 0.85)
                    }),
                    dot_color: Color::WHITE,
                    border_color: if is_custom_selected {
                        palette.primary.strong.color
                    } else {
                        Color::from_rgba(0.5, 0.5, 0.5, 0.4)
                    },
                    border_width: 1.0,
                    text_color: Some(theme.palette().text),
                }
            });

            let input = text_input("", &self.custom_rate)
                .on_input(|s| crate::ui::Message::SelectBpsMsg(SelectBpsMsg::CustomBpsChanged(s)))
                .padding(6)
                .size(14)
                .width(Length::Fixed(110.0))
                .style(move |theme: &iced::Theme, status| {
                    let palette = theme.extended_palette();
                    let focused = matches!(status, text_input::Status::Focused { .. });
                    text_input::Style {
                        background: iced::Background::Color(if is_custom_selected {
                            if focused {
                                Color::from_rgba(0.12, 0.12, 0.2, 0.2)
                            } else {
                                Color::from_rgba(0.08, 0.08, 0.1, 0.12)
                            }
                        } else {
                            Color::from_rgba(0.05, 0.05, 0.05, 0.05)
                        }),
                        border: Border {
                            color: if is_custom_selected && focused {
                                palette.primary.base.color
                            } else if is_custom_selected {
                                palette.primary.weak.color
                            } else {
                                Color::from_rgba(0.3, 0.3, 0.3, 0.2)
                            },
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        icon: theme.palette().text,
                        placeholder: Color::from_rgba(0.6, 0.6, 0.6, if is_custom_selected { 0.7 } else { 0.4 }),
                        value: theme.palette().text.scale_alpha(if is_custom_selected { 1.0 } else { 0.5 }),
                        selection: palette.primary.strong.color,
                    }
                });

            row![radio_custom, input, text("BPS").size(14)].spacing(8).align_y(Alignment::Center)
        };

        let scrollable_list = scrollable(column![list, Space::new().height(4), custom_row,].spacing(6).padding([4, 8])).height(Length::Fixed(280.0));

        // Buttons
        let cancel_button = secondary_button(
            format!("{}", icy_engine_gui::ButtonType::Cancel),
            Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal))),
        );

        let ok_button = primary_button(format!("{}", icy_engine_gui::ButtonType::Ok), Some(crate::ui::Message::ApplyBaudEmulation));

        let buttons = button_row(vec![cancel_button.into(), ok_button.into()]);

        let dialog_content = dialog_area(column![title, Space::new().height(DIALOG_SPACING), scrollable_list,].into());

        let button_area = dialog_area(buttons);

        let modal = modal_container(
            column![container(dialog_content).height(Length::Fill), separator(), button_area,].into(),
            DIALOG_WIDTH_SMALL,
        );

        container(modal)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}
