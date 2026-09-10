use eframe::egui;
use icy_net::protocol::TransferState;
use icy_term::{Options, TerminalCommand, TerminalEvent, TransferProtocol};

#[derive(Default)]
pub struct Transfers {
    pub open: bool,
    pub active: bool,
    pub download: bool,
    pub protocol_id: String,
    pub state: Option<TransferState>,
    pub result: Option<String>,
    pub capture: Option<String>,
    commands: Vec<TerminalCommand>,
}

impl Transfers {
    pub fn choose(&mut self, download: bool) {
        self.open = true;
        self.download = download;
        self.state = None;
        self.result = None;
        if self.protocol_id.is_empty() {
            self.protocol_id = "@zmodem".into();
        }
    }

    pub fn event(&mut self, event: &TerminalEvent) {
        match event {
            TerminalEvent::CaptureState(path) => self.capture = path.clone(),
            TerminalEvent::AutoTransferTriggered(protocol, download, _) => {
                if !self.active {
                    self.choose(*download);
                    self.protocol_id = protocol.clone();
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

    pub fn menu(&mut self, ui: &mut egui::Ui, connected: bool, options: &Options) {
        ui.add_enabled_ui(connected && !self.active, |ui| {
            if ui.button(&*tr!("egui-upload-command")).clicked() {
                self.choose(false);
                ui.close();
            }
            if ui.button(&*tr!("egui-download-command")).clicked() {
                self.choose(true);
                ui.close();
            }
            ui.separator();
            if self.capture.is_some() {
                if ui.button(&*tr!("toolbar-stop-capture")).clicked() {
                    self.commands.push(TerminalCommand::StopCapture);
                    ui.close();
                }
            } else if ui.button(&*tr!("egui-capture-command")).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_directory(&options.capture_path)
                    .set_file_name("capture.ans")
                    .save_file()
                {
                    let path = path.to_string_lossy().into_owned();
                    self.commands.push(TerminalCommand::StartCapture(path));
                }
                ui.close();
            }
        });
    }

    fn choose_files(&mut self, protocol: TransferProtocol, options: &Options) {
        let dialog = rfd::FileDialog::new().set_directory(&options.download_path);
        if self.download {
            if protocol.ask_for_download_location {
                if let Some(path) = dialog.save_file() {
                    if let Some(parent) = path.parent() {
                        self.commands.push(TerminalCommand::SetDownloadDirectory(parent.into()));
                    }
                    self.commands
                        .push(TerminalCommand::StartDownload(protocol, Some(path.to_string_lossy().into_owned())));
                    self.active = true;
                }
            } else if let Some(path) = dialog.pick_folder() {
                self.commands.push(TerminalCommand::SetDownloadDirectory(path));
                self.commands.push(TerminalCommand::StartDownload(protocol, None));
                self.active = true;
            }
        } else {
            let files = if protocol.batch {
                rfd::FileDialog::new().pick_files()
            } else {
                rfd::FileDialog::new().pick_file().map(|path| vec![path])
            };
            if let Some(files) = files {
                self.commands.push(TerminalCommand::StartUpload(protocol, files));
                self.active = true;
            }
        }
    }

    pub fn show(&mut self, context: &egui::Context, options: &Options, connected: bool) -> Vec<TerminalCommand> {
        if self.open {
            egui::Modal::new(egui::Id::new("transfer"))
                .frame(super::appearance::dialog_frame(context))
                .show(context, |ui| {
                    ui.set_width((context.content_rect().width() - 48.0).clamp(220.0, 520.0));
                    ui.heading(if self.download { tr!("terminal-download") } else { tr!("terminal-upload") });
                    egui::ScrollArea::vertical()
                        .max_height((context.content_rect().height() - 150.0).max(80.0))
                        .show(ui, |ui| {
                            if let Some(state) = &self.state {
                                let info = if self.download { &state.recieve_state } else { &state.send_state };
                                ui.label(&state.protocol_name);
                                ui.label(&info.file_name);
                                ui.add(
                                    egui::ProgressBar::new((info.cur_bytes_transfered as f32 / info.file_size.max(1) as f32).clamp(0.0, 1.0)).show_percentage(),
                                );
                                ui.label(format!("{} / {} bytes", info.cur_bytes_transfered, info.file_size));
                                ui.label(format!("{} bytes/s", state.get_current_bps(self.download)));
                                ui.label(format!("{}s", info.start_time.elapsed().as_secs()));
                                for message in &info.output_log {
                                    use icy_net::protocol::OutputLogMessage;
                                    match message {
                                        OutputLogMessage::Info(message) => {
                                            ui.label(message);
                                        }
                                        OutputLogMessage::Warning(message) => {
                                            ui.colored_label(ui.visuals().warn_fg_color, message);
                                        }
                                        OutputLogMessage::Error(message) => {
                                            ui.colored_label(ui.visuals().error_fg_color, message);
                                        }
                                    }
                                }
                            } else if !self.active && self.result.is_none() {
                                for protocol in options
                                    .transfer_protocols
                                    .iter()
                                    .filter(|protocol| protocol.enabled && (!self.download || protocol.id != "@text"))
                                {
                                    ui.radio_value(&mut self.protocol_id, protocol.id.clone(), protocol.get_name());
                                }
                                if let Some(protocol) = options.transfer_protocols.iter().find(|protocol| protocol.id == self.protocol_id) {
                                    if !protocol.is_internal() {
                                        ui.colored_label(ui.visuals().warn_fg_color, &*tr!("egui-external-warning"));
                                    }
                                }
                            }
                            if let Some(result) = &self.result {
                                ui.label(result);
                            }
                            if self.active {
                                ui.spinner();
                                context.request_repaint_after(std::time::Duration::from_millis(250));
                            }
                        });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if self.active {
                            if ui.button(&*tr!("egui-cancel-transfer")).clicked() {
                                self.commands.push(TerminalCommand::CancelTransfer);
                            }
                        } else {
                            if self.state.is_none() && self.result.is_none() {
                                let protocol = options
                                    .transfer_protocols
                                    .iter()
                                    .find(|protocol| protocol.enabled && protocol.id == self.protocol_id && (!self.download || protocol.id != "@text"))
                                    .cloned();
                                if ui
                                    .add_enabled(connected && protocol.is_some(), super::appearance::primary_button(tr!("egui-choose-files")))
                                    .clicked()
                                {
                                    self.choose_files(protocol.unwrap(), options);
                                }
                            }
                            if ui.button(&*tr!("egui-close")).clicked() {
                                self.open = false;
                            }
                        }
                    });
                });
        }
        std::mem::take(&mut self.commands)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_transfers_require_local_approval_and_disconnect_releases_ui() {
        let mut transfers = Transfers::default();
        transfers.event(&TerminalEvent::AutoTransferTriggered("@zmodem".into(), true, None));
        assert!(transfers.open);
        assert!(!transfers.active);
        assert!(transfers.commands.is_empty());
        transfers.event(&TerminalEvent::ExternalTransferStarted("test".into(), true));
        assert!(transfers.active);
        transfers.event(&TerminalEvent::Disconnected(None));
        assert!(!transfers.active);
        assert_eq!(transfers.result.as_deref(), Some(&*tr!("egui-connection-closed")));
    }
}
