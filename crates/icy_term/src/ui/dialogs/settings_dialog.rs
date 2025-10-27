use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Length,
    widget::{Space, button, checkbox, column, container, pick_list, row, scrollable, text, text_input},
};
use icy_net::serial::{CharSize, Parity, StopBits};

use crate::Options;

// Constants for sizing
const MODAL_WIDTH: f32 = 680.0;
const MODAL_HEIGHT: f32 = 460.0;
const LABEL_WIDTH: f32 = 150.0;
const INPUT_SPACING: f32 = 8.0;
const SECTION_SPACING: f32 = 16.0;

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsCategory {
    Monitor,
    IEMSI,
    Terminal,
    Keybinds,
    Modem,
}

impl SettingsCategory {
    fn name(&self) -> String {
        match self {
            Self::Monitor => fl!(crate::LANGUAGE_LOADER, "settings-monitor-category"),
            Self::IEMSI => fl!(crate::LANGUAGE_LOADER, "settings-iemsi-category"),
            Self::Terminal => fl!(crate::LANGUAGE_LOADER, "settings-terminal-category"),
            Self::Keybinds => fl!(crate::LANGUAGE_LOADER, "settings-keybinds-category"),
            Self::Modem => fl!(crate::LANGUAGE_LOADER, "settings-modem-category"),
        }
    }

    fn all() -> Vec<Self> {
        vec![Self::Monitor, Self::IEMSI, Self::Terminal, Self::Keybinds, Self::Modem]
    }
}

#[derive(Debug, Clone)]
pub enum SettingsMsg {
    SwitchCategory(SettingsCategory),
    UpdateOptions(Options),
    ResetCategory(SettingsCategory),
    OpenSettingsFolder,
    SelectModem(usize),
    AddModem,
    RemoveModem(usize),
    Save,
    Cancel,
    Noop,
}

pub struct SettingsDialogState {
    pub current_category: SettingsCategory,
    pub temp_options: Options,
    pub original_options: Options,
    pub selected_modem_index: usize,
}

impl SettingsDialogState {
    pub fn new(options: Options) -> Self {
        Self {
            current_category: SettingsCategory::Monitor,
            temp_options: options.clone(),
            original_options: options,
            selected_modem_index: 0,
        }
    }

    pub fn reset(&mut self, options: &Options) {
        self.temp_options = options.clone();
        self.original_options = options.clone();
    }

    pub fn update(&mut self, message: SettingsMsg) -> Option<crate::ui::Message> {
        match message {
            SettingsMsg::SwitchCategory(category) => {
                self.current_category = category;
                None
            }
            SettingsMsg::UpdateOptions(options) => {
                self.temp_options = options;
                None
            }
            SettingsMsg::ResetCategory(category) => {
                match category {
                    SettingsCategory::Monitor => {
                        self.temp_options.reset_monitor_settings();
                    }
                    SettingsCategory::Keybinds => {
                        // self.temp_options.reset_keybindings();
                    }
                    _ => {}
                }
                None
            }
            SettingsMsg::OpenSettingsFolder => {
                if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_term") {
                    let _ = open::that(proj_dirs.config_dir());
                }
                None
            }
            SettingsMsg::Save => {
                // Save the options and close dialog
                self.original_options = self.temp_options.clone();
                if let Err(e) = self.temp_options.store_options() {
                    log::error!("Failed to save options: {}", e);
                }
                Some(crate::ui::Message::CloseDialog)
            }
            SettingsMsg::Cancel => {
                // Reset to original options and close
                self.temp_options = self.original_options.clone();
                Some(crate::ui::Message::CloseDialog)
            }
            SettingsMsg::SelectModem(index) => {
                self.selected_modem_index = index;
                None
            }
            SettingsMsg::AddModem => {
                let new_modem = crate::data::modem::Modem {
                    name: format!("Modem {}", self.temp_options.modems.len() + 1),
                    ..Default::default()
                };
                self.temp_options.modems.push(new_modem);
                self.selected_modem_index = self.temp_options.modems.len() - 1;
                None
            }
            SettingsMsg::RemoveModem(index) => {
                if index < self.temp_options.modems.len() {
                    self.temp_options.modems.remove(index);
                    if !self.temp_options.modems.is_empty() {
                        self.selected_modem_index = index.min(self.temp_options.modems.len() - 1);
                    } else {
                        self.selected_modem_index = 0;
                    }
                }
                None
            }
            SettingsMsg::Noop => None,
        }
    }

    pub fn view<'a>(&'a self, terminal_content: Element<'a, crate::ui::Message>) -> Element<'a, crate::ui::Message> {
        let overlay = self.create_modal_content();
        crate::ui::modal(terminal_content, overlay, crate::ui::Message::SettingsDialog(SettingsMsg::Cancel))
    }

    fn create_modal_content(&self) -> Element<'_, crate::ui::Message> {
        let title = text(fl!(crate::LANGUAGE_LOADER, "settings-heading")).size(20);

        // Category tabs
        let mut category_row = row![].spacing(8);
        for category in SettingsCategory::all() {
            let is_selected = self.current_category == category;
            let cat = category.clone();
            let cat_button = button(text(category.name()).size(14))
                .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::SwitchCategory(cat)))
                .style(move |theme: &iced::Theme, status| {
                    use iced::widget::button::{Status, Style};

                    let palette = theme.extended_palette();
                    let base = if is_selected {
                        Style {
                            background: Some(iced::Background::Color(palette.primary.weak.color)),
                            text_color: palette.primary.weak.text,
                            border: Border::default().rounded(4.0),
                            shadow: Default::default(),
                            snap: false,
                        }
                    } else {
                        Style {
                            background: Some(iced::Background::Color(Color::TRANSPARENT)),
                            text_color: palette.background.base.text,
                            border: Border::default().rounded(4.0),
                            shadow: Default::default(),
                            snap: false,
                        }
                    };

                    match status {
                        Status::Active => base,
                        Status::Hovered if !is_selected => Style {
                            background: Some(iced::Background::Color(Color::from_rgba(
                                palette.primary.weak.color.r,
                                palette.primary.weak.color.g,
                                palette.primary.weak.color.b,
                                0.2,
                            ))),
                            ..base
                        },
                        Status::Pressed => Style {
                            background: Some(iced::Background::Color(palette.primary.strong.color)),
                            ..base
                        },
                        _ => base,
                    }
                })
                .padding([6, 12]);
            category_row = category_row.push(cat_button);
        }

        // Settings content for current category
        let settings_content = match self.current_category {
            SettingsCategory::Monitor => self.monitor_settings_content(),
            SettingsCategory::IEMSI => self.iemsi_settings_content(),
            SettingsCategory::Terminal => self.terminal_settings_content(),
            SettingsCategory::Keybinds => self.keybinds_settings_content(),
            SettingsCategory::Modem => self.modem_settings_content(),
        };

        // Buttons
        let ok_button = button(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-ok-button")))
            .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::Save))
            .padding([8, 16])
            .style(button::primary);

        let cancel_button = button(text(fl!(crate::LANGUAGE_LOADER, "dialing_directory-cancel-button")))
            .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::Cancel))
            .padding([8, 16])
            .style(button::secondary);

        let reset_button = match self.current_category {
            SettingsCategory::Monitor | SettingsCategory::Keybinds => Some(
                button(text(fl!(crate::LANGUAGE_LOADER, "settings-reset-button")))
                    .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::ResetCategory(self.current_category.clone())))
                    .padding([8, 16])
                    .style(button::danger),
            ),
            _ => None,
        };

        let mut button_row = row![Space::new().width(Length::Fill),];

        if let Some(reset_btn) = reset_button {
            button_row = button_row.push(reset_btn).push(Space::new().width(8.0));
        }

        button_row = button_row.push(cancel_button).push(Space::new().width(8.0)).push(ok_button);

        let modal_content = container(
            column![
                container(title).width(Length::Fill).align_x(Alignment::Center),
                category_row,
                container(Space::new())
                    .height(Length::Fixed(1.0))
                    .width(Length::Fill)
                    .style(|theme: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(theme.extended_palette().background.strong.color)),
                        ..Default::default()
                    }),
                container(scrollable(settings_content).direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default())))
                    .height(Length::Fixed(290.0))
                    .width(Length::Fill)
                    .padding([0, 12]),
                Space::new().height(Length::Fixed(12.0)),
                button_row,
            ]
            .padding(10)
            .spacing(8),
        )
        .width(Length::Fixed(MODAL_WIDTH))
        .height(Length::Fixed(MODAL_HEIGHT))
        .style(|theme: &iced::Theme| {
            let palette = theme.palette();
            container::Style {
                background: Some(iced::Background::Color(palette.background)),
                border: Border {
                    color: palette.text,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                text_color: Some(palette.text),
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
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

    fn monitor_settings_content(&self) -> Element<'_, crate::ui::Message> {
        let settings = &self.temp_options.monitor_settings;

        column![
            // Monitor settings would go here
            // For now, placeholder
            text("Monitor settings - TODO: Implement monitor controls").size(14),
            Space::new().height(SECTION_SPACING),
            text(format!("Blur: {:.2}", settings.blur)).size(14),
            text(format!("Brightness: {:.2}", settings.brightness)).size(14),
            text(format!("Contrast: {:.2}", settings.contrast)).size(14),
            text(format!("Saturation: {:.2}", settings.saturation)).size(14),
        ]
        .spacing(INPUT_SPACING)
        .into()
    }

    fn iemsi_settings_content(&self) -> Element<'_, crate::ui::Message> {
        let iemsi = &self.temp_options.iemsi;

        column![
            checkbox(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-autologin-checkbox"), iemsi.autologin)
                .on_toggle(|checked| {
                    let mut new_options = self.temp_options.clone();
                    new_options.iemsi.autologin = checked;
                    crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                })
                .size(14),
            Space::new().height(SECTION_SPACING),
            // Alias
            row![
                container(text(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-alias")).size(14))
                    .width(Length::Fixed(LABEL_WIDTH))
                    .align_x(iced::alignment::Horizontal::Right),
                text_input("", &iemsi.alias)
                    .on_input(|value| {
                        let mut new_options = self.temp_options.clone();
                        new_options.iemsi.alias = value;
                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                    })
                    .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            // Location
            row![
                container(text(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-location")).size(14))
                    .width(Length::Fixed(LABEL_WIDTH))
                    .align_x(iced::alignment::Horizontal::Right),
                text_input("", &iemsi.location)
                    .on_input(|value| {
                        let mut new_options = self.temp_options.clone();
                        new_options.iemsi.location = value;
                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                    })
                    .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            // Data Phone
            row![
                container(text(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-data-phone")).size(14))
                    .width(Length::Fixed(LABEL_WIDTH))
                    .align_x(iced::alignment::Horizontal::Right),
                text_input("", &iemsi.data_phone)
                    .on_input(|value| {
                        let mut new_options = self.temp_options.clone();
                        new_options.iemsi.data_phone = value;
                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                    })
                    .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            // Voice Phone
            row![
                container(text(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-voice-phone")).size(14))
                    .width(Length::Fixed(LABEL_WIDTH))
                    .align_x(iced::alignment::Horizontal::Right),
                text_input("", &iemsi.voice_phone)
                    .on_input(|value| {
                        let mut new_options = self.temp_options.clone();
                        new_options.iemsi.voice_phone = value;
                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                    })
                    .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            // Birth Date
            row![
                container(text(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-birth-date")).size(14))
                    .width(Length::Fixed(LABEL_WIDTH))
                    .align_x(iced::alignment::Horizontal::Right),
                text_input("", &iemsi.birth_date)
                    .on_input(|value| {
                        let mut new_options = self.temp_options.clone();
                        new_options.iemsi.birth_date = value;
                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                    })
                    .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(INPUT_SPACING)
        .into()
    }

    fn terminal_settings_content(&self) -> Element<'_, crate::ui::Message> {
        column![
            checkbox(
                fl!(crate::LANGUAGE_LOADER, "settings-terminal-console-beep-checkbox"),
                self.temp_options.console_beep
            )
            .on_toggle(|checked| {
                let mut new_options = self.temp_options.clone();
                new_options.console_beep = checked;
                crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
            })
            .size(14),
            Space::new().height(SECTION_SPACING),
            button(text(fl!(crate::LANGUAGE_LOADER, "settings-terminal-open-settings-dir-button")).size(14))
                .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::OpenSettingsFolder))
                .padding([6, 12]),
        ]
        .spacing(INPUT_SPACING)
        .into()
    }

    fn keybinds_settings_content(&self) -> Element<'_, crate::ui::Message> {
        // TODO: Implement keybindings editor
        column![text("Keybindings editor - TODO: Implement keybinding controls").size(14),].into()
    }

    fn modem_settings_content(&self) -> Element<'_, crate::ui::Message> {
        let modems = &self.temp_options.modems; // Assuming options now has a Vec<Modem>
        let selected_index = self.selected_modem_index;

        let mut modem_list = column![].spacing(4);

        // Modem list with selection
        for (idx, modem) in modems.iter().enumerate() {
            let is_selected = idx == selected_index;
            let modem_button = button(container(text(&modem.name).size(14)).width(Length::Fill).padding([4, 8]))
                .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::SelectModem(idx)))
                .width(Length::Fill)
                .style(move |theme: &iced::Theme, status| {
                    use iced::widget::button::{Status, Style};

                    let palette = theme.extended_palette();
                    let base = if is_selected {
                        Style {
                            background: Some(iced::Background::Color(palette.primary.weak.color)),
                            text_color: palette.primary.weak.text,
                            border: Border::default().rounded(4.0),
                            shadow: Default::default(),
                            snap: false,
                        }
                    } else {
                        Style {
                            background: Some(iced::Background::Color(palette.background.weak.color)),
                            text_color: palette.background.weak.text,
                            border: Border::default().rounded(4.0),
                            shadow: Default::default(),
                            snap: false,
                        }
                    };

                    match status {
                        Status::Active => base,
                        Status::Hovered if !is_selected => Style {
                            background: Some(iced::Background::Color(palette.background.strong.color)),
                            ..base
                        },
                        _ => base,
                    }
                });

            modem_list = modem_list.push(modem_button);
        }

        // Add/Remove buttons
        let add_button = button(text("+").size(14))
            .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::AddModem))
            .width(Length::Shrink)
            .padding([6, 8]);

        let remove_button = if !modems.is_empty() {
            button(text("-").size(14))
                .on_press(crate::ui::Message::SettingsDialog(SettingsMsg::RemoveModem(selected_index)))
                .width(Length::Shrink)
                .padding([6, 8])
        } else {
            button(text("-").size(14)).width(Length::Shrink).padding([6, 8])
        };

        let list_controls = row![add_button, Space::new().width(4.0), remove_button,];

        // Modem settings for selected modem
        let modem_settings = if let Some(modem) = modems.get(selected_index) {
            column![
                // Name
                row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "settings-modem-name")).size(14))
                        .width(Length::Fixed(LABEL_WIDTH))
                        .align_x(iced::alignment::Horizontal::Right),
                    text_input("Modem Name", &modem.name)
                        .on_input(move |value| {
                            let mut new_options = self.temp_options.clone();
                            if let Some(m) = new_options.modems.get_mut(selected_index) {
                                m.name = value;
                            }
                            crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                        })
                        .width(Length::Fill),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                // Device
                row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "settings-modem-device")).size(14))
                        .width(Length::Fixed(LABEL_WIDTH))
                        .align_x(iced::alignment::Horizontal::Right),
                    text_input("", &modem.device)
                        .on_input(move |value| {
                            let mut new_options = self.temp_options.clone();
                            if let Some(m) = new_options.modems.get_mut(selected_index) {
                                m.device = value;
                            }
                            crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                        })
                        .width(Length::Fill),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                // Baud Rate
                row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "settings-modem-baud_rate")).size(14))
                        .width(Length::Fixed(LABEL_WIDTH))
                        .align_x(iced::alignment::Horizontal::Right),
                    text_input("", &modem.baud_rate.to_string())
                        .on_input(move |value| {
                            if let Ok(baud) = value.parse::<u32>() {
                                let mut new_options = self.temp_options.clone();
                                if let Some(m) = new_options.modems.get_mut(selected_index) {
                                    m.baud_rate = baud;
                                }
                                crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                            } else {
                                crate::ui::Message::SettingsDialog(SettingsMsg::Noop)
                            }
                        })
                        .width(Length::Fill),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                // Data Bits, Stop Bits, Parity
                row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "settings-modem-data_bits")).size(14))
                        .width(Length::Fixed(LABEL_WIDTH))
                        .align_x(iced::alignment::Horizontal::Right),
                    pick_list(&CharSizeOption::ALL[..], Some(CharSizeOption::from(modem.char_size)), move |value| {
                        let mut new_options = self.temp_options.clone();
                        if let Some(m) = new_options.modems.get_mut(selected_index) {
                            m.char_size = value.into();
                        }
                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                    })
                    .width(Length::Fixed(80.0)),
                    pick_list(&StopBitsOption::ALL[..], Some(StopBitsOption::from(modem.stop_bits)), move |value| {
                        let mut new_options = self.temp_options.clone();
                        if let Some(m) = new_options.modems.get_mut(selected_index) {
                            m.stop_bits = value.into();
                        }
                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                    })
                    .width(Length::Fixed(80.0)),
                    pick_list(&ParityOption::ALL[..], Some(ParityOption::from(modem.parity)), move |value| {
                        let mut new_options = self.temp_options.clone();
                        if let Some(m) = new_options.modems.get_mut(selected_index) {
                            m.parity = value.into();
                        }
                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                    })
                    .width(Length::Fixed(100.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                // Flow Control
                row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "settings-modem-flow_control")).size(14))
                        .width(Length::Fixed(LABEL_WIDTH))
                        .align_x(iced::alignment::Horizontal::Right),
                    pick_list(&FlowControlOption::ALL[..], Some(FlowControlOption::from(modem.flow_control)), move |value| {
                        let mut new_options = self.temp_options.clone();
                        if let Some(m) = new_options.modems.get_mut(selected_index) {
                            m.flow_control = value.into();
                        }
                        crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                    })
                    .width(Length::Fixed(150.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                // Init String
                row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "settings-modem-init_string")).size(14))
                        .width(Length::Fixed(LABEL_WIDTH))
                        .align_x(iced::alignment::Horizontal::Right),
                    text_input("", &modem.init_string)
                        .on_input(move |value| {
                            let mut new_options = self.temp_options.clone();
                            if let Some(m) = new_options.modems.get_mut(selected_index) {
                                m.init_string = value;
                            }
                            crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                        })
                        .width(Length::Fill),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                // Dial String
                row![
                    container(text(fl!(crate::LANGUAGE_LOADER, "settings-modem_dial_string")).size(14))
                        .width(Length::Fixed(LABEL_WIDTH))
                        .align_x(iced::alignment::Horizontal::Right),
                    text_input("", &modem.dial_string)
                        .on_input(move |value| {
                            let mut new_options = self.temp_options.clone();
                            if let Some(m) = new_options.modems.get_mut(selected_index) {
                                m.dial_string = value;
                            }
                            crate::ui::Message::SettingsDialog(SettingsMsg::UpdateOptions(new_options))
                        })
                        .width(Length::Fill),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ]
            .spacing(INPUT_SPACING)
        } else {
            column![
                container(text(fl!(crate::LANGUAGE_LOADER, "settings-modem_nothing_selected")).size(14))
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .padding(20),
            ]
        };

        // Layout: List on left, settings on right
        row![
            container(column![
                text("Modems:").size(14),
                Space::new().height(8.0),
                container(scrollable(modem_list).direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default())))
                    .height(Length::Fixed(150.0))
                    .width(Length::Fill)
                    .style(|theme: &iced::Theme| container::Style {
                        border: Border {
                            color: theme.extended_palette().background.strong.color,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    }),
                Space::new().height(8.0),
                list_controls,
            ])
            .width(Length::Fixed(200.0))
            .padding([0, 12]),
            container(modem_settings).width(Length::Fill),
        ]
        .into()
    }
}

// Wrapper types for external enums to implement Display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CharSizeOption(CharSize);

impl CharSizeOption {
    const ALL: [CharSizeOption; 4] = [
        CharSizeOption(CharSize::Bits5),
        CharSizeOption(CharSize::Bits6),
        CharSizeOption(CharSize::Bits7),
        CharSizeOption(CharSize::Bits8),
    ];
}

impl From<CharSize> for CharSizeOption {
    fn from(value: CharSize) -> Self {
        CharSizeOption(value)
    }
}

impl From<CharSizeOption> for CharSize {
    fn from(value: CharSizeOption) -> Self {
        value.0
    }
}

impl std::fmt::Display for CharSizeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            CharSize::Bits5 => write!(f, "5"),
            CharSize::Bits6 => write!(f, "6"),
            CharSize::Bits7 => write!(f, "7"),
            CharSize::Bits8 => write!(f, "8"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StopBitsOption(StopBits);

impl StopBitsOption {
    const ALL: [StopBitsOption; 2] = [StopBitsOption(StopBits::One), StopBitsOption(StopBits::Two)];
}

impl From<StopBits> for StopBitsOption {
    fn from(value: StopBits) -> Self {
        StopBitsOption(value)
    }
}

impl From<StopBitsOption> for StopBits {
    fn from(value: StopBitsOption) -> Self {
        value.0
    }
}

impl std::fmt::Display for StopBitsOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            StopBits::One => write!(f, "1"),
            StopBits::Two => write!(f, "2"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParityOption(Parity);

impl ParityOption {
    const ALL: [ParityOption; 3] = [ParityOption(Parity::None), ParityOption(Parity::Odd), ParityOption(Parity::Even)];
}

impl From<Parity> for ParityOption {
    fn from(value: Parity) -> Self {
        ParityOption(value)
    }
}

impl From<ParityOption> for Parity {
    fn from(value: ParityOption) -> Self {
        value.0
    }
}

impl std::fmt::Display for ParityOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Parity::None => write!(f, "None"),
            Parity::Odd => write!(f, "Odd"),
            Parity::Even => write!(f, "Even"),
        }
    }
}

// Add FlowControl wrapper
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FlowControlOption(icy_net::serial::FlowControl);

impl FlowControlOption {
    const ALL: [FlowControlOption; 3] = [
        FlowControlOption(icy_net::serial::FlowControl::None),
        FlowControlOption(icy_net::serial::FlowControl::XonXoff),
        FlowControlOption(icy_net::serial::FlowControl::RtsCts),
    ];
}

impl From<icy_net::serial::FlowControl> for FlowControlOption {
    fn from(value: icy_net::serial::FlowControl) -> Self {
        FlowControlOption(value)
    }
}

impl From<FlowControlOption> for icy_net::serial::FlowControl {
    fn from(value: FlowControlOption) -> Self {
        value.0
    }
}

impl std::fmt::Display for FlowControlOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            icy_net::serial::FlowControl::None => write!(f, "None"),
            icy_net::serial::FlowControl::XonXoff => write!(f, "Software"),
            icy_net::serial::FlowControl::RtsCts => write!(f, "Hardware"),
        }
    }
}
