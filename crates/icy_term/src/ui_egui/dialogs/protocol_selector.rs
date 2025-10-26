use eframe::egui::{self, RichText};
use egui_modal::Modal;
use i18n_embed_fl::fl;
use icy_net::protocol::TransferProtocolType;

use crate::ui::{MainWindow, MainWindowMode};

use lazy_static::lazy_static;
lazy_static! {
    static ref PROTOCOL_TABLE: [(TransferProtocolType, String, String); 8] = [
        (
            TransferProtocolType::ZModem,
            "Zmodem".to_string(),
            fl!(crate::LANGUAGE_LOADER, "protocol-zmodem-description")
        ),
        (
            TransferProtocolType::ZModem8k,
            "ZedZap".to_string(),
            fl!(crate::LANGUAGE_LOADER, "protocol-zmodem8k-description"),
        ),
        (
            TransferProtocolType::XModem,
            "Xmodem".to_string(),
            fl!(crate::LANGUAGE_LOADER, "protocol-xmodem-description")
        ),
        (
            TransferProtocolType::XModem1k,
            "Xmodem 1k".to_string(),
            fl!(crate::LANGUAGE_LOADER, "protocol-xmodem1k-description")
        ),
        (
            TransferProtocolType::XModem1kG,
            "Xmodem 1k-G".to_string(),
            fl!(crate::LANGUAGE_LOADER, "protocol-xmodem1kG-description")
        ),
        (
            TransferProtocolType::YModem,
            "Ymodem".to_string(),
            fl!(crate::LANGUAGE_LOADER, "protocol-ymodem-description")
        ),
        (
            TransferProtocolType::YModemG,
            "Ymodem-G".to_string(),
            fl!(crate::LANGUAGE_LOADER, "protocol-ymodemg-description")
        ),
        (
            TransferProtocolType::ASCII,
            "Text".to_string(),
            fl!(crate::LANGUAGE_LOADER, "protocol-text-description")
        )
    ];
}

pub fn view_selector(window: &mut MainWindow, ctx: &egui::Context, _frame: &mut eframe::Frame, download: bool) {
    if ctx.input(|i| i.key_down(egui::Key::Escape)) {
        window.set_mode(ctx, MainWindowMode::ShowTerminal);
    }

    let title = RichText::new(if download {
        fl!(crate::LANGUAGE_LOADER, "protocol-select-download")
    } else {
        fl!(crate::LANGUAGE_LOADER, "protocol-select-upload")
    });
    let modal = Modal::new(ctx, "protocol_modal");
    modal.show(|ui| {
        modal.title(ui, title);

        modal.frame(ui, |ui: &mut egui::Ui| {
            egui::Grid::new("some_unique_id")
                .num_columns(2)
                .spacing([4.0, 8.0])
                .min_col_width(130.)
                .min_row_height(24.)
                .show(ui, |ui| {
                    for (protocol, title, descr) in &*PROTOCOL_TABLE {
                        if download && matches!(*protocol, TransferProtocolType::ASCII) {
                            continue;
                        }
                        ui.with_layout(ui.layout().with_cross_justify(true), |ui| {
                            if ui.selectable_label(false, RichText::new(title).strong()).clicked() {
                                window.initiate_file_transfer(ctx, protocol.clone(), download, None);
                            }
                        });
                        ui.label(RichText::new(descr));
                        ui.end_row();
                    }
                });
        });
        modal.buttons(ui, |ui| {
            if modal.button(ui, fl!(crate::LANGUAGE_LOADER, "dialing_directory-cancel-button")).clicked() {
                window.set_mode(ctx, MainWindowMode::ShowTerminal);
            }
        });
    });
    modal.open();
}
