use std::cmp::max;
use std::time::Duration;

use eframe::egui::{self, ProgressBar, RichText};
use egui::{Label, Layout, ScrollArea};
use egui_extras::{Column, TableBuilder};
use gabi::BytesConfig;
use i18n_embed_fl::fl;

use crate::protocol::{OutputLogMessage, TransferState};

#[derive(Default)]
pub struct FileTransferDialog {
    pub selected_log: usize,
}

impl FileTransferDialog {
    pub fn new() -> Self {
        Self { selected_log: 0 }
    }

    pub fn show_dialog(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame, transfer_state: &TransferState, download: bool) -> bool {
        let mut open = true;
        let mut close_dialog = false;
        if ctx.input(|i: &egui::InputState| i.key_down(egui::Key::Escape)) {
            open = false;
        }

        let title: RichText = RichText::new(if download {
            fl!(crate::LANGUAGE_LOADER, "transfer-download")
        } else {
            fl!(crate::LANGUAGE_LOADER, "transfer-upload")
        });

        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .min_width(450.)
            .resizable(false)
            .show(ctx, |ui| {
                let state = transfer_state;
                let transfer_info = if download { &state.recieve_state } else { &state.send_state };

                let check = transfer_info.check_size.clone();
                let file_name = transfer_info.file_name.clone();

                let bb = BytesConfig::default();

                let elapsed_time: Duration = state.end_time.duration_since(state.start_time);
                let elapsed_time = format!("{:02}:{:02}", elapsed_time.as_secs() / 60, elapsed_time.as_secs() % 60);

                let cur_state = if download {
                    &transfer_state.recieve_state
                } else {
                    &transfer_state.send_state
                };
                if state.is_finished {
                    ui.label("Completed");
                }
                let table = TableBuilder::new(ui)
                    .striped(false)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto())
                    .column(Column::auto())
                    .resizable(false);
                let row_height = 30.;

                table.body(|mut body| {
                    body.row(row_height, |mut row| {
                        row.col(|ui| {
                            ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "transfer-protocol")));
                            ui.label(RichText::new(state.protocol_name.clone()));
                        });
                    });

                    body.row(row_height, |mut row| {
                        row.col(|ui| {
                            ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "transfer-checksize")));
                            ui.label(RichText::new(check));
                        });

                        row.col(|ui| {
                            ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "transfer-elapsedtime")));
                            ui.label(RichText::new(elapsed_time));
                        });
                    });
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "transfer-file")));
                    ui.label(RichText::new(file_name));
                });
                ui.add(
                    ProgressBar::new(transfer_info.bytes_transfered as f32 / transfer_info.file_size as f32).text(RichText::new(format!(
                        "{}% {}/{}",
                        (transfer_info.bytes_transfered * 100) / max(1, transfer_info.file_size),
                        bb.bytes(transfer_info.bytes_transfered as u64),
                        bb.bytes(transfer_info.file_size as u64)
                    ))),
                );
                ui.horizontal(|ui| {
                    ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "transfer-rate")));
                    let bps = bb.bytes(transfer_info.get_bps()).to_string();
                    ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "transfer-bps", bps = bps)));
                });

                if cur_state.has_log_entries() {
                    ui.add_space(8.0);
                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(self.selected_log == 0, format!("All ({})", cur_state.log_count()))
                            .clicked()
                        {
                            self.selected_log = 0;
                        }

                        if ui
                            .selectable_label(self.selected_log == 1, format!("Warnings ({})", cur_state.warnings()))
                            .clicked()
                        {
                            self.selected_log = 1;
                        }

                        if ui
                            .selectable_label(self.selected_log == 2, format!("Errors ({})", cur_state.errors()))
                            .clicked()
                        {
                            self.selected_log = 2;
                        }
                    });
                    ui.separator();

                    let count = match self.selected_log {
                        0 => cur_state.log_count(),
                        1 => cur_state.warnings(),
                        2 => cur_state.errors(),
                        _ => 0,
                    };

                    ScrollArea::vertical()
                        .id_source("output_log_scroll_area")
                        .min_scrolled_height(200.)
                        .show_rows(ui, 23., count, |ui, range| {
                            for i in range {
                                match transfer_info.get_log_message(self.selected_log, i) {
                                    Some(msg) => match msg {
                                        OutputLogMessage::Error(msg) => {
                                            ui.add(Label::new(RichText::new(msg).color(ctx.style().visuals.error_fg_color)).wrap(false));
                                        }
                                        OutputLogMessage::Warning(msg) => {
                                            ui.add(Label::new(RichText::new(msg).color(ctx.style().visuals.warn_fg_color)).wrap(false));
                                        }
                                        OutputLogMessage::Info(msg) => {
                                            ui.add(Label::new(RichText::new(msg)).wrap(false));
                                        }
                                    },
                                    None => {
                                        ui.label("-");
                                    }
                                }
                            }
                        });
                }
                ui.with_layout(Layout::right_to_left(egui::Align::TOP), |ui| {
                    let button_label = if transfer_state.is_finished {
                        fl!(crate::LANGUAGE_LOADER, "dialing_directory-ok-button")
                    } else {
                        fl!(crate::LANGUAGE_LOADER, "dialing_directory-cancel-button")
                    };
                    if ui.button(button_label).clicked() {
                        close_dialog = true;
                    }
                });
            });
        open && !close_dialog
    }
}
