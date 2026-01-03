use i18n_embed_fl::fl;
use icy_engine_gui::settings::effect_box;
use icy_engine_gui::ui::*;
use icy_net::serial::{CharSize, FlowControl, Parity, Serial, StopBits};
use icy_ui::{
    widget::{column, container, pick_list, row, text, text_input},
    Alignment, Element, Length,
};

use crate::ui::MainWindowMode;

/// Standard baud rates for serial connections
pub const BAUD_RATES: [u32; 11] = [300, 1200, 2400, 4800, 9600, 14400, 19200, 28800, 38400, 57600, 115200];

pub struct OpenSerialDialog {
    pub serial: Serial,
}

#[derive(Debug, Clone)]
pub enum OpenSerialMsg {
    DeviceChanged(String),
    BaudRateChanged(u32),
    CharSizeChanged(CharSizeOption),
    ParityChanged(ParityOption),
    StopBitsChanged(StopBitsOption),
    FlowControlChanged(FlowControlOption),
    AutoDetect,
}

impl OpenSerialDialog {
    pub fn new(serial: Serial) -> Self {
        Self { serial }
    }

    pub fn update(&mut self, message: OpenSerialMsg) -> Option<crate::ui::Message> {
        match message {
            OpenSerialMsg::DeviceChanged(device) => {
                self.serial.device = device;
                None
            }
            OpenSerialMsg::BaudRateChanged(rate) => {
                self.serial.baud_rate = rate;
                None
            }
            OpenSerialMsg::CharSizeChanged(size) => {
                self.serial.format.char_size = size.0;
                None
            }
            OpenSerialMsg::ParityChanged(parity) => {
                self.serial.format.parity = parity.0;
                None
            }
            OpenSerialMsg::StopBitsChanged(stop_bits) => {
                self.serial.format.stop_bits = stop_bits.0;
                None
            }
            OpenSerialMsg::FlowControlChanged(flow_control) => {
                self.serial.flow_control = flow_control.0;
                None
            }
            OpenSerialMsg::AutoDetect => Some(crate::ui::Message::AutoDetectSerial),
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
        let title = dialog_title(fl!(crate::LANGUAGE_LOADER, "open-serial-dialog-title"));

        // Device input
        let device_input = text_input("", &self.serial.device)
            .on_input(|s| crate::ui::Message::OpenSerialMsg(OpenSerialMsg::DeviceChanged(s)))
            .padding(8)
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        let device_row = row![left_label_small(fl!(crate::LANGUAGE_LOADER, "open-serial-dialog-device")), device_input,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Baud rate picker
        let baud_pick = pick_list(BAUD_RATES.to_vec(), Some(self.serial.baud_rate), |rate| {
            crate::ui::Message::OpenSerialMsg(OpenSerialMsg::BaudRateChanged(rate))
        })
        .width(Length::Fixed(120.0))
        .text_size(TEXT_SIZE_NORMAL);

        let baud_row = row![left_label_small(fl!(crate::LANGUAGE_LOADER, "open-serial-dialog-baud-rate")), baud_pick,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Data bits (char size) picker
        let char_size_pick = pick_list(&CharSizeOption::ALL[..], Some(CharSizeOption(self.serial.format.char_size)), |size| {
            crate::ui::Message::OpenSerialMsg(OpenSerialMsg::CharSizeChanged(size))
        })
        .width(Length::Fixed(80.0))
        .text_size(TEXT_SIZE_NORMAL);

        // Parity picker
        let parity_pick = pick_list(&ParityOption::ALL[..], Some(ParityOption(self.serial.format.parity)), |parity| {
            crate::ui::Message::OpenSerialMsg(OpenSerialMsg::ParityChanged(parity))
        })
        .width(Length::Fixed(80.0))
        .text_size(TEXT_SIZE_NORMAL);

        // Stop bits picker
        let stop_bits_pick = pick_list(&StopBitsOption::ALL[..], Some(StopBitsOption(self.serial.format.stop_bits)), |stop_bits| {
            crate::ui::Message::OpenSerialMsg(OpenSerialMsg::StopBitsChanged(stop_bits))
        })
        .width(Length::Fixed(80.0))
        .text_size(TEXT_SIZE_NORMAL);

        // Combined row for data format (8N1 style)
        let format_row = row![
            left_label_small(fl!(crate::LANGUAGE_LOADER, "open-serial-dialog-format")),
            char_size_pick,
            text("-").size(TEXT_SIZE_NORMAL),
            parity_pick,
            text("-").size(TEXT_SIZE_NORMAL),
            stop_bits_pick,
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        // Flow control picker
        let flow_control_pick = pick_list(&FlowControlOption::ALL[..], Some(FlowControlOption(self.serial.flow_control)), |fc| {
            crate::ui::Message::OpenSerialMsg(OpenSerialMsg::FlowControlChanged(fc))
        })
        .width(Length::Fixed(120.0))
        .text_size(TEXT_SIZE_NORMAL);

        let flow_row = row![
            left_label_small(fl!(crate::LANGUAGE_LOADER, "open-serial-dialog-flow-control")),
            flow_control_pick,
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        // Buttons
        let auto_detect_button = secondary_button(
            fl!(crate::LANGUAGE_LOADER, "open-serial-dialog-auto-detect"),
            Some(crate::ui::Message::OpenSerialMsg(OpenSerialMsg::AutoDetect)),
        );

        let cancel_button = secondary_button(
            format!("{}", icy_engine_gui::ButtonType::Cancel),
            Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal))),
        );

        let connect_button = primary_button(
            fl!(crate::LANGUAGE_LOADER, "open-serial-dialog-connect"),
            Some(crate::ui::Message::ConnectSerial),
        );

        let buttons = button_row_with_left(vec![auto_detect_button.into()], vec![cancel_button.into(), connect_button.into()]);

        // Settings content wrapped in effect_box
        let settings_content = effect_box(column![device_row, baud_row, format_row, flow_row,].spacing(DIALOG_SPACING).into());

        let dialog_content = dialog_area(column![title, settings_content,].spacing(DIALOG_SPACING).into());

        let button_area = dialog_area(buttons);

        let modal = modal_container(
            column![container(dialog_content).height(Length::Fill), separator(), button_area,].into(),
            DIALOG_WIDTH_LARGE,
        );

        container(modal)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}

// Wrapper types for pick_list compatibility
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharSizeOption(pub CharSize);

impl CharSizeOption {
    pub const ALL: [CharSizeOption; 4] = [
        CharSizeOption(CharSize::Bits5),
        CharSizeOption(CharSize::Bits6),
        CharSizeOption(CharSize::Bits7),
        CharSizeOption(CharSize::Bits8),
    ];
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
pub struct StopBitsOption(pub StopBits);

impl StopBitsOption {
    pub const ALL: [StopBitsOption; 2] = [StopBitsOption(StopBits::One), StopBitsOption(StopBits::Two)];
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
pub struct ParityOption(pub Parity);

impl ParityOption {
    pub const ALL: [ParityOption; 3] = [ParityOption(Parity::None), ParityOption(Parity::Odd), ParityOption(Parity::Even)];
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowControlOption(pub FlowControl);

impl FlowControlOption {
    pub const ALL: [FlowControlOption; 3] = [
        FlowControlOption(FlowControl::None),
        FlowControlOption(FlowControl::XonXoff),
        FlowControlOption(FlowControl::RtsCts),
    ];
}

impl std::fmt::Display for FlowControlOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            FlowControl::None => write!(f, "None"),
            FlowControl::XonXoff => write!(f, "XON/XOFF"),
            FlowControl::RtsCts => write!(f, "RTS/CTS"),
        }
    }
}
