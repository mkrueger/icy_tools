use std::path::PathBuf;

use eframe::egui;
use icy_net::protocol::{OutputLogMessage, TransferInformation, TransferState};
use icy_term::{Options, TerminalCommand, TerminalEvent, TransferProtocol};

/// The dialog keeps these measurements no matter what the transfer reports, so it never jumps around.
const DIALOG_WIDTH: f32 = 560.0;
const BODY_HEIGHT: f32 = 330.0;
const NOTE_HEIGHT: f32 = 36.0;
const WARNING_HEIGHT: f32 = 20.0;
const SUCCESS: egui::Color32 = egui::Color32::from_rgb(78, 173, 118);

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    #[default]
    Files,
    Log,
}

#[derive(Default)]
pub struct Transfers {
    pub open: bool,
    pub active: bool,
    pub download: bool,
    pub protocol_id: String,
    pub state: Option<TransferState>,
    pub result: Option<String>,
    pub capture: Option<String>,
    pub request_capture: bool,
    /// Directory the running or last download is written to.
    destination: Option<PathBuf>,
    /// The remote side started this transfer instead of the user.
    remote_request: bool,
    /// A remote request that still needs a local file selection.
    pending_file_request: bool,
    detail_tab: DetailTab,
    commands: Vec<TerminalCommand>,
}

impl Transfers {
    pub fn choose(&mut self, download: bool) {
        self.open = true;
        self.download = download;
        self.state = None;
        self.result = None;
        self.destination = None;
        self.remote_request = false;
        self.pending_file_request = false;
        self.detail_tab = DetailTab::default();
        if self.protocol_id.is_empty() {
            self.protocol_id = "@zmodem".into();
        }
    }

    pub fn event(&mut self, event: &TerminalEvent, options: &Options) {
        match event {
            TerminalEvent::CaptureState(path) => self.capture = path.clone(),
            TerminalEvent::AutoTransferTriggered(protocol_id, download, filename) => {
                if !self.active {
                    self.choose(*download);
                    self.protocol_id = protocol_id.clone();
                    self.remote_request = true;
                    let protocol = options
                        .transfer_protocols
                        .iter()
                        .find(|protocol| protocol.enabled && protocol.id == *protocol_id)
                        .cloned()
                        .or_else(|| TransferProtocol::from_internal_id(protocol_id));
                    match protocol {
                        // The remote announces the file names, so a download needs no local input.
                        Some(protocol) if *download && (!protocol.ask_for_download_location || filename.is_some()) => {
                            self.start_download(protocol, filename.clone(), options);
                        }
                        Some(_) => self.pending_file_request = true,
                        None => self.result = Some(tr!("egui-transfer-unknown-protocol", protocol = protocol_id.clone())),
                    }
                }
            }
            TerminalEvent::TransferStarted(state, download) => {
                self.open = true;
                self.active = true;
                self.download = *download;
                self.state = Some(state.clone());
                self.result = None;
            }
            TerminalEvent::TransferProgress(state) => self.state = Some(state.clone()),
            TerminalEvent::TransferCompleted(state) => {
                self.state = Some(state.clone());
                self.active = false;
                self.result = Some(if state.request_cancel {
                    tr!("egui-transfer-cancelled")
                } else if state.send_state.errors > 0 || state.recieve_state.errors > 0 {
                    tr!("egui-transfer-errors")
                } else {
                    tr!("egui-transfer-finished")
                });
            }
            TerminalEvent::ExternalTransferStarted(name, download) => {
                self.open = true;
                self.active = true;
                self.download = *download;
                self.result = Some(format!("Running {name}"));
            }
            TerminalEvent::ExternalTransferCompleted(_, _, success, error) => {
                self.active = false;
                self.result = Some(error.clone().unwrap_or_else(|| {
                    if *success {
                        tr!("egui-transfer-complete")
                    } else {
                        tr!("transfer-external-failed")
                    }
                }));
            }
            TerminalEvent::Disconnected(_) => {
                if self.active {
                    self.result = Some(tr!("egui-connection-closed"));
                }
                self.active = false;
                self.capture = None;
            }
            TerminalEvent::Error(title, detail) => {
                if self.active {
                    self.active = false;
                    self.result = Some(format!("{title}: {detail}"));
                }
            }
            _ => {}
        }
    }

    pub fn menu(&mut self, ui: &mut egui::Ui, connected: bool) {
        use super::hotkeys::{shortcut, Action};
        ui.add_enabled_ui(connected && !self.active, |ui| {
            if ui
                .add(egui::Button::new(&*tr!("egui-upload-command")).shortcut_text(shortcut(Action::Upload)))
                .clicked()
            {
                self.choose(false);
                ui.close();
            }
            if ui
                .add(egui::Button::new(&*tr!("egui-download-command")).shortcut_text(shortcut(Action::Download)))
                .clicked()
            {
                self.choose(true);
                ui.close();
            }
            ui.separator();
            if self.capture.is_some() {
                if ui
                    .add(egui::Button::new(&*tr!("toolbar-stop-capture")).shortcut_text(shortcut(Action::Capture)))
                    .clicked()
                {
                    self.commands.push(TerminalCommand::StopCapture);
                    ui.close();
                }
            } else if ui
                .add(egui::Button::new(&*tr!("egui-capture-command")).shortcut_text(shortcut(Action::Capture)))
                .clicked()
            {
                self.request_capture = true;
                ui.close();
            }
        });
    }

    /// Starts a download into the configured download directory without asking the user.
    fn start_download(&mut self, protocol: TransferProtocol, filename: Option<String>, options: &Options) {
        let directory = PathBuf::from(options.download_path());
        self.commands.push(TerminalCommand::SetDownloadDirectory(directory.clone()));
        self.commands.push(TerminalCommand::StartDownload(protocol, filename));
        self.destination = Some(directory);
        self.open = true;
        self.download = true;
        self.active = true;
        self.state = None;
        self.result = None;
    }

    fn selected_protocol(&self, options: &Options) -> Option<TransferProtocol> {
        options
            .transfer_protocols
            .iter()
            .find(|protocol| protocol.enabled && protocol.id == self.protocol_id && (!self.download || protocol.id != "@text"))
            .cloned()
    }

    fn choose_files(&mut self, protocol: TransferProtocol, options: &Options) {
        let directory = options.download_path();
        let dialog = rfd::FileDialog::new().set_directory(&directory);
        if self.download {
            if protocol.ask_for_download_location {
                if let Some(path) = dialog.save_file() {
                    if let Some(parent) = path.parent() {
                        self.destination = Some(parent.into());
                        self.commands.push(TerminalCommand::SetDownloadDirectory(parent.into()));
                    }
                    self.commands
                        .push(TerminalCommand::StartDownload(protocol, Some(path.to_string_lossy().into_owned())));
                    self.active = true;
                }
            } else if let Some(path) = dialog.pick_folder() {
                self.destination = Some(path.clone());
                self.commands.push(TerminalCommand::SetDownloadDirectory(path));
                self.commands.push(TerminalCommand::StartDownload(protocol, None));
                self.active = true;
            }
        } else {
            let files = if protocol.batch {
                dialog.pick_files()
            } else {
                dialog.pick_file().map(|path| vec![path])
            };
            if let Some(files) = files {
                self.commands.push(TerminalCommand::StartUpload(protocol, files));
                self.active = true;
            }
        }
    }

    fn progress_view(&mut self, ui: &mut egui::Ui, state: &TransferState) {
        let info = if self.download { &state.recieve_state } else { &state.send_state };
        let done = info.cur_bytes_transfered.min(info.file_size);
        let ratio = (done as f32 / info.file_size.max(1) as f32).clamp(0.0, 1.0);
        let name = if info.file_name.is_empty() {
            tr!("transfer-waiting")
        } else {
            info.file_name.clone()
        };
        let accent = ui.visuals().selection.stroke.color;
        ui.horizontal(|ui| {
            ui.set_min_height(20.0);
            // The name gets a fixed share of the row, so a long file name cannot push the numbers out.
            let name_width = ui.available_width() * 0.5;
            ui.allocate_ui_with_layout(egui::vec2(name_width, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_min_width(name_width);
                ui.add(egui::Label::new(egui::RichText::new(&name).strong().size(15.0)).truncate())
                    .on_hover_text(&name);
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{} / {}",
                        human_bytes::human_bytes(done as f64),
                        human_bytes::human_bytes(info.file_size as f64)
                    ))
                    .weak(),
                );
                ui.label(egui::RichText::new("·").weak());
                ui.label(
                    egui::RichText::new(format!("{} %", (ratio * 100.0).round() as u32))
                        .strong()
                        .color(if state.is_finished { SUCCESS } else { accent }),
                );
            });
        });
        ui.add_space(6.0);
        ui.add(
            egui::ProgressBar::new(ratio)
                .desired_height(8.0)
                .corner_radius(4)
                .fill(if state.is_finished { SUCCESS } else { accent }),
        );
        ui.add_space(12.0);

        let bps = state.get_current_bps(self.download);
        let remaining = if !state.is_finished && bps > 0 && info.file_size > done {
            elapsed(std::time::Duration::from_secs((info.file_size - done) / bps as u64))
        } else {
            "–".to_owned()
        };
        // All four tiles are always drawn, so finishing a transfer cannot resize the dialog.
        ui.horizontal(|ui| {
            let spacing = ui.spacing().item_spacing.x;
            let width = ((ui.available_width() - spacing * 3.0) / 4.0).max(64.0);
            let rate = if bps > 0 {
                format!("{}/s", human_bytes::human_bytes(bps as f64))
            } else {
                "–".to_owned()
            };
            super::appearance::metric_tile_sized(ui, &tr!("transfer-rate"), &rate, width);
            super::appearance::metric_tile_sized(ui, &tr!("transfer-elapsedtime"), &elapsed(info.start_time.elapsed()), width);
            super::appearance::metric_tile_sized(ui, &tr!("egui-transfer-remaining"), &remaining, width);
            super::appearance::metric_tile_sized(ui, &tr!("transfer-protocol"), &state.protocol_name, width);
        });
        ui.add_space(10.0);

        let warnings = info.warnings();
        let errors = info.errors();
        ui.horizontal(|ui| {
            super::appearance::tab(
                ui,
                &mut self.detail_tab,
                DetailTab::Files,
                &format!("{} ({})", tr!("egui-transfer-files"), info.finished_files.len()),
            );
            super::appearance::tab(
                ui,
                &mut self.detail_tab,
                DetailTab::Log,
                &format!("{} ({})", tr!("egui-transfer-log"), info.output_log.len()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if errors > 0 {
                    super::appearance::chip(ui, &format!("{} {errors}", tr!("transfer-log-errors")), ui.visuals().error_fg_color);
                }
                if warnings > 0 {
                    super::appearance::chip(ui, &format!("{} {warnings}", tr!("transfer-log-warnings")), ui.visuals().warn_fg_color);
                }
            });
        });
        ui.add_space(4.0);

        let height = (ui.available_height() - NOTE_HEIGHT - ui.spacing().item_spacing.y).max(60.0);
        let outer_width = ui.available_width();
        // A frame reserves its margins and its stroke, which have to be taken off the fixed size.
        let overhead = 16.0 + ui.visuals().widgets.noninteractive.bg_stroke.width * 2.0;
        let tab = self.detail_tab;
        egui::Frame::new()
            .fill(ui.visuals().extreme_bg_color)
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .corner_radius(6)
            .inner_margin(8)
            .show(ui, |ui| {
                ui.set_width(outer_width - overhead);
                ui.set_height((height - overhead).max(40.0));
                match tab {
                    DetailTab::Files => file_list(ui, info),
                    DetailTab::Log => log_list(ui, info),
                }
            });
    }

    /// Fixed area below the details, so a result message or a download path never resizes the dialog.
    fn note_view(&self, ui: &mut egui::Ui, show_result: bool) {
        let width = ui.available_width();
        ui.allocate_ui(egui::vec2(width, NOTE_HEIGHT), |ui| {
            ui.set_min_size(egui::vec2(width, NOTE_HEIGHT));
            // A scroll area never grows beyond its maximum, whatever the messages contain.
            egui::ScrollArea::vertical()
                .id_salt("transfer-note")
                .auto_shrink([false, false])
                .max_height(NOTE_HEIGHT)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    if show_result {
                        if let Some(result) = &self.result {
                            ui.add(egui::Label::new(result).truncate()).on_hover_text(result);
                        }
                    }
                    if let Some(destination) = self.destination.as_ref().filter(|_| self.download) {
                        let path = destination.display().to_string();
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            ui.label(egui::RichText::new(&*tr!("egui-transfer-saved-to")).weak());
                            ui.add(egui::Label::new(egui::RichText::new(&path).weak()).truncate()).on_hover_text(&path);
                        });
                    }
                });
        });
    }

    /// Shown while an external protocol or a not yet started transfer is running.
    fn status_view(&self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        let height = (ui.available_height() - NOTE_HEIGHT - ui.spacing().item_spacing.y).max(60.0);
        let message = self.result.clone().unwrap_or_else(|| tr!("transfer-waiting"));
        ui.allocate_ui(egui::vec2(width, height), |ui| {
            ui.set_min_size(egui::vec2(width, height));
            ui.add_space((height / 2.0 - 46.0).max(0.0));
            ui.vertical_centered(|ui| {
                if self.active {
                    ui.add(egui::Spinner::new().size(28.0));
                } else {
                    ui.label(egui::RichText::new("\u{2713}").size(28.0).color(SUCCESS));
                }
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .id_salt("transfer-status")
                    .auto_shrink([false, true])
                    .max_height(60.0)
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(message).wrap());
                    });
            });
        });
    }

    fn protocol_view(&mut self, ui: &mut egui::Ui, options: &Options) {
        if self.remote_request {
            ui.add(egui::Label::new(&*tr!("egui-transfer-remote-request")).wrap());
            ui.add_space(6.0);
        }
        super::appearance::section(ui, &tr!("transfer-protocol"));
        // The hint row below is always reserved, so switching protocols keeps the list height.
        let height = (ui.available_height() - NOTE_HEIGHT - WARNING_HEIGHT - ui.spacing().item_spacing.y * 2.0).max(60.0);
        egui::ScrollArea::vertical()
            .id_salt("transfer-protocols")
            .auto_shrink([false, false])
            .max_height(height)
            .show(ui, |ui| {
                ui.set_height(height);
                ui.spacing_mut().item_spacing.y = 2.0;
                let row_width = ui.available_width();
                for protocol in options
                    .transfer_protocols
                    .iter()
                    .filter(|protocol| protocol.enabled && (!self.download || protocol.id != "@text"))
                {
                    let selected = self.protocol_id == protocol.id;
                    let mut picked = false;
                    let response = egui::Frame::new()
                        .fill(if selected {
                            ui.visuals().selection.bg_fill.gamma_multiply(0.5)
                        } else {
                            egui::Color32::TRANSPARENT
                        })
                        .corner_radius(6)
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.set_width(row_width - 20.0);
                            ui.horizontal(|ui| {
                                picked |= ui.radio(selected, protocol.get_name()).clicked();
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if !protocol.is_internal() {
                                        super::appearance::chip(ui, "EXTERN", ui.visuals().warn_fg_color);
                                    }
                                });
                            });
                        })
                        .response;
                    if picked || ui.interact(response.rect, ui.id().with(&protocol.id), egui::Sense::click()).clicked() {
                        self.protocol_id = protocol.id.clone();
                    }
                }
            });
        let external = options
            .transfer_protocols
            .iter()
            .any(|protocol| protocol.id == self.protocol_id && !protocol.is_internal());
        ui.allocate_ui(egui::vec2(ui.available_width(), WARNING_HEIGHT), |ui| {
            ui.set_min_height(WARNING_HEIGHT);
            if external {
                ui.add(egui::Label::new(egui::RichText::new(&*tr!("egui-external-warning")).color(ui.visuals().warn_fg_color)).truncate());
            }
        });
    }

    pub fn show(&mut self, context: &egui::Context, options: &Options, connected: bool) -> Vec<TerminalCommand> {
        if !self.open {
            return std::mem::take(&mut self.commands);
        }
        // The remote is already waiting, so answer its request with the file picker right away.
        if self.pending_file_request && connected && !self.active {
            self.pending_file_request = false;
            if let Some(protocol) = self.selected_protocol(options) {
                self.choose_files(protocol, options);
            }
        }
        let title = if self.download { tr!("transfer-download") } else { tr!("transfer-upload") };
        let finished = self.state.as_ref().is_some_and(|state| state.is_finished) || (!self.active && self.result.is_some());
        let response = super::appearance::Dialog::new("transfer", format!("{} {title}", if self.download { "\u{2193}" } else { "\u{2191}" }))
            .max_width(DIALOG_WIDTH)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    let width = ui.available_width();
                    // Everything below lives in a fixed box, so a finishing transfer cannot resize the dialog.
                    ui.allocate_ui(egui::vec2(width, BODY_HEIGHT), |ui| {
                        ui.set_min_size(egui::vec2(width, BODY_HEIGHT));
                        ui.horizontal(|ui| {
                            ui.set_min_height(20.0);
                            if self.state.is_some() || self.active || self.result.is_some() {
                                let (label, color) = if finished {
                                    (tr!("transfer-status-complete"), SUCCESS)
                                } else {
                                    (tr!("transfer-status-active"), ui.visuals().selection.stroke.color)
                                };
                                super::appearance::status_badge(ui, &label, color);
                                if self.remote_request {
                                    super::appearance::chip(ui, &tr!("egui-transfer-automatic"), ui.visuals().selection.stroke.color);
                                }
                            }
                        });
                        ui.add_space(10.0);
                        if let Some(state) = self.state.clone() {
                            self.progress_view(ui, &state);
                            self.note_view(ui, true);
                        } else if self.active || self.result.is_some() {
                            self.status_view(ui);
                            self.note_view(ui, false);
                        } else {
                            self.protocol_view(ui, options);
                            self.note_view(ui, false);
                        }
                    });
                    if self.active {
                        context.request_repaint_after(std::time::Duration::from_millis(250));
                    }
                });
                dialog.actions(|ui| {
                    if self.active {
                        if ui.add(super::appearance::primary_button(tr!("egui-cancel-transfer"))).clicked() {
                            self.commands.push(TerminalCommand::CancelTransfer);
                        }
                    } else {
                        if self.state.is_none() && self.result.is_none() {
                            let protocol = self.selected_protocol(options);
                            if ui
                                .add_enabled(connected && protocol.is_some(), super::appearance::primary_button(tr!("egui-choose-files")))
                                .clicked()
                            {
                                self.choose_files(protocol.unwrap(), options);
                            }
                            if ui.button(&*tr!("egui-close")).clicked() {
                                self.open = false;
                            }
                        } else if ui.add(super::appearance::primary_button(tr!("egui-close"))).clicked() {
                            self.open = false;
                        }
                        if let Some(destination) = self.destination.as_ref().filter(|_| finished && self.download) {
                            if ui.button(&*tr!("egui-open-download-folder")).clicked() {
                                if let Err(error) = open::that(destination) {
                                    self.result = Some(error.to_string());
                                }
                            }
                        }
                    }
                });
            });
        if response.closed {
            if self.active {
                self.commands.push(TerminalCommand::CancelTransfer);
            } else {
                self.open = false;
            }
        }
        std::mem::take(&mut self.commands)
    }
}

fn file_list(ui: &mut egui::Ui, info: &TransferInformation) {
    if info.finished_files.is_empty() {
        empty_hint(ui, &tr!("egui-transfer-no-files"));
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("transfer-files")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            for (name, path) in &info.finished_files {
                let name = if name.is_empty() {
                    path.file_name().unwrap_or_default().to_string_lossy().into_owned()
                } else {
                    name.clone()
                };
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(egui::RichText::new("\u{2713}").color(SUCCESS));
                    ui.add(egui::Label::new(name).truncate()).on_hover_text(path.display().to_string());
                });
            }
        });
}

fn log_list(ui: &mut egui::Ui, info: &TransferInformation) {
    if info.output_log.is_empty() {
        empty_hint(ui, &tr!("egui-transfer-no-log"));
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("transfer-log")
        .stick_to_bottom(true)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for message in &info.output_log {
                let (text, color) = match message {
                    OutputLogMessage::Info(text) => (text, ui.visuals().weak_text_color()),
                    OutputLogMessage::Warning(text) => (text, ui.visuals().warn_fg_color),
                    OutputLogMessage::Error(text) => (text, ui.visuals().error_fg_color),
                };
                ui.add(egui::Label::new(egui::RichText::new(text).monospace().size(11.0).color(color)).wrap());
            }
        });
}

fn empty_hint(ui: &mut egui::Ui, text: &str) {
    ui.centered_and_justified(|ui| {
        ui.add(egui::Label::new(egui::RichText::new(text).weak().size(12.0)).truncate());
    });
}

fn elapsed(duration: std::time::Duration) -> String {
    let seconds = duration.as_secs();
    if seconds >= 3600 {
        format!("{}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60)
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog_size(transfers: &mut Transfers, options: &Options) -> egui::Vec2 {
        let context = egui::Context::default();
        let mut size = egui::Vec2::ZERO;
        // The first frame only lays the modal out, the size is known afterwards.
        for _ in 0..3 {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 800.0))),
                    ..Default::default()
                },
                |context| {
                    transfers.show(context, options, true);
                },
            );
            size = context
                .memory(|memory| memory.area_rect(egui::Id::new("transfer")))
                .map(|rect| rect.size())
                .unwrap_or_default();
        }
        size
    }

    /// Finished transfers add files, log lines and a download path, none of which may resize the dialog.
    #[test]
    fn the_dialog_keeps_its_size_in_every_state() {
        let options = Options::default();
        let mut transfers = Transfers::default();
        transfers.choose(true);
        let choosing = dialog_size(&mut transfers, &options);
        assert!(choosing.x > 0.0 && choosing.y > 0.0, "the modal was not laid out");

        // An automatic download waits for the first progress report and remembers a destination.
        transfers.event(&TerminalEvent::AutoTransferTriggered("@zmodem".into(), true, None), &options);
        assert_eq!(choosing, dialog_size(&mut transfers, &options), "the waiting state resized the dialog");

        let mut state = TransferState::new("Zmodem".to_string());
        state.recieve_state.file_name = "archive.zip".into();
        state.recieve_state.file_size = 512 * 1024;
        state.recieve_state.cur_bytes_transfered = 128 * 1024;
        state.recieve_state.log_info("starting");
        transfers.event(&TerminalEvent::TransferStarted(state.clone(), true), &options);
        assert_eq!(choosing, dialog_size(&mut transfers, &options), "the running state resized the dialog");

        state.is_finished = true;
        state.recieve_state.cur_bytes_transfered = state.recieve_state.file_size;
        for index in 0..40 {
            state.recieve_state.log_info(format!("block {index} received"));
        }
        state.recieve_state.log_warning("retry");
        state.recieve_state.log_error("crc mismatch");
        state.recieve_state.finish_file(PathBuf::from("/tmp/archive.zip"));
        transfers.event(&TerminalEvent::TransferCompleted(state), &options);
        assert_eq!(choosing, dialog_size(&mut transfers, &options), "the finished state resized the dialog");
    }

    #[test]
    fn remote_download_starts_automatically_and_disconnect_releases_ui() {
        let options = Options::default();
        let mut transfers = Transfers::default();
        transfers.event(&TerminalEvent::AutoTransferTriggered("@zmodem".into(), true, None), &options);
        assert!(transfers.open);
        assert!(transfers.active);
        assert!(matches!(transfers.commands.first(), Some(TerminalCommand::SetDownloadDirectory(_))));
        assert!(matches!(transfers.commands.get(1), Some(TerminalCommand::StartDownload(protocol, None)) if protocol.id == "@zmodem"));
        transfers.event(&TerminalEvent::Disconnected(None), &options);
        assert!(!transfers.active);
        assert_eq!(transfers.result.as_deref(), Some(&*tr!("egui-connection-closed")));
    }

    #[test]
    fn remote_upload_requests_wait_for_a_local_file_selection() {
        let options = Options::default();
        let mut transfers = Transfers::default();
        transfers.event(&TerminalEvent::AutoTransferTriggered("@zmodem".into(), false, None), &options);
        assert!(transfers.open);
        assert!(!transfers.active);
        assert!(transfers.pending_file_request);
        assert!(transfers.commands.is_empty());
    }

    #[test]
    fn a_running_transfer_is_not_interrupted_by_further_signatures() {
        let options = Options::default();
        let mut transfers = Transfers::default();
        transfers.event(&TerminalEvent::ExternalTransferStarted("test".into(), true), &options);
        assert!(transfers.active);
        transfers.event(&TerminalEvent::AutoTransferTriggered("@zmodem".into(), true, None), &options);
        assert!(transfers.commands.is_empty());
    }

    /// The remote sends a ZMODEM header and the client has to answer it without any user interaction.
    #[test]
    fn a_sent_zmodem_download_answers_the_remote_on_its_own() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::Arc;
        use std::time::{Duration, Instant};

        use icy_engine::TextScreen;
        use parking_lot::Mutex;

        let directory = std::env::temp_dir().join(format!(
            "icy-egui-auto-download-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let options = Options {
            download_path: directory.to_string_lossy().into_owned(),
            ..Options::default()
        };

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let config = crate::session::connection_config(&format!("raw://{}", listener.local_addr().unwrap()), false).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            stream.write_all(b"**\x18B00").unwrap();
            let mut answer = [0; 4];
            stream.read_exact(&mut answer).unwrap();
            assert_eq!(&answer, b"**\x18B", "The receiver did not answer with a ZMODEM header");
        });

        let session = crate::session::Session::start(Arc::new(Mutex::new(Box::new(TextScreen::default()))), config, egui::Context::default());
        let mut transfers = Transfers::default();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let event = session.events.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap();
            transfers.event(&event, &options);
            if matches!(event, TerminalEvent::AutoTransferTriggered(..)) {
                break;
            }
            assert!(!matches!(event, TerminalEvent::Disconnected(..)), "The connection was lost");
        }
        assert!(transfers.active, "The download did not start on its own");
        assert!(transfers.remote_request);
        for command in std::mem::take(&mut transfers.commands) {
            session.command(command).unwrap();
        }
        server.join().unwrap();
        drop(session);
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
