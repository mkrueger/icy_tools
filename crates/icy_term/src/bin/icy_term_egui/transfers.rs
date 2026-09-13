use std::path::PathBuf;

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
    pub request_capture: bool,
    /// Directory the running or last download is written to.
    destination: Option<PathBuf>,
    /// The remote side started this transfer instead of the user.
    remote_request: bool,
    /// A remote request that still needs a local file selection.
    pending_file_request: bool,
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

    fn progress_view(&self, ui: &mut egui::Ui, state: &TransferState) {
        let info = if self.download { &state.recieve_state } else { &state.send_state };
        let done = info.cur_bytes_transfered.min(info.file_size);
        let ratio = (done as f32 / info.file_size.max(1) as f32).clamp(0.0, 1.0);
        let name = if info.file_name.is_empty() {
            tr!("transfer-waiting")
        } else {
            info.file_name.clone()
        };
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(egui::RichText::new(name).strong().size(15.0)).truncate());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{} / {}",
                        human_bytes::human_bytes(done as f64),
                        human_bytes::human_bytes(info.file_size as f64)
                    ))
                    .weak(),
                );
            });
        });
        ui.add_space(4.0);
        ui.add(egui::ProgressBar::new(ratio).desired_height(10.0).corner_radius(5).show_percentage());
        ui.add_space(10.0);
        let bps = state.get_current_bps(self.download);
        ui.horizontal_wrapped(|ui| {
            super::appearance::metric_tile(ui, &tr!("transfer-rate"), &format!("{}/s", human_bytes::human_bytes(bps as f64)));
            super::appearance::metric_tile(ui, &tr!("transfer-elapsedtime"), &elapsed(info.start_time.elapsed()));
            if !state.is_finished && bps > 0 && info.file_size > done {
                let remaining = std::time::Duration::from_secs((info.file_size - done) / bps as u64);
                super::appearance::metric_tile(ui, &tr!("egui-transfer-remaining"), &elapsed(remaining));
            }
            super::appearance::metric_tile(ui, &tr!("transfer-protocol"), &state.protocol_name);
        });
        if !info.finished_files.is_empty() {
            ui.add_space(10.0);
            super::appearance::section(ui, &tr!("egui-transfer-files"));
            for (name, path) in &info.finished_files {
                let name = if name.is_empty() {
                    path.file_name().unwrap_or_default().to_string_lossy().into_owned()
                } else {
                    name.clone()
                };
                ui.add(egui::Label::new(format!("\u{2713} {name}")).truncate())
                    .on_hover_text(path.display().to_string());
            }
        }
        if info.output_log.is_empty() {
            return;
        }
        ui.add_space(10.0);
        super::appearance::section(ui, &tr!("transfer-log-all"));
        egui::Frame::new()
            .fill(ui.visuals().extreme_bg_color)
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .corner_radius(6)
            .inner_margin(8)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical()
                    .id_salt("transfer-log")
                    .max_height(120.0)
                    .stick_to_bottom(true)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for message in &info.output_log {
                            use icy_net::protocol::OutputLogMessage;
                            let (text, color) = match message {
                                OutputLogMessage::Info(text) => (text, ui.visuals().text_color()),
                                OutputLogMessage::Warning(text) => (text, ui.visuals().warn_fg_color),
                                OutputLogMessage::Error(text) => (text, ui.visuals().error_fg_color),
                            };
                            ui.add(egui::Label::new(egui::RichText::new(text).monospace().size(11.0).color(color)).wrap());
                        }
                    });
            });
    }

    fn protocol_view(&mut self, ui: &mut egui::Ui, options: &Options) {
        super::appearance::section(ui, &tr!("transfer-protocol"));
        ui.spacing_mut().item_spacing.y = 2.0;
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
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        picked |= ui.radio(selected, protocol.get_name()).clicked();
                        if !protocol.is_internal() {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                super::appearance::chip(ui, "EXTERN", ui.visuals().warn_fg_color);
                            });
                        }
                    });
                })
                .response;
            if picked || ui.interact(response.rect, ui.id().with(&protocol.id), egui::Sense::click()).clicked() {
                self.protocol_id = protocol.id.clone();
            }
        }
        if let Some(protocol) = options.transfer_protocols.iter().find(|protocol| protocol.id == self.protocol_id) {
            if !protocol.is_internal() {
                ui.add_space(6.0);
                ui.colored_label(ui.visuals().warn_fg_color, &*tr!("egui-external-warning"));
            }
        }
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
            .max_width(560.0)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    if self.state.is_some() || self.active {
                        ui.horizontal(|ui| {
                            let (label, color) = if finished {
                                (tr!("transfer-status-complete"), egui::Color32::from_rgb(78, 173, 118))
                            } else {
                                (tr!("transfer-status-active"), ui.visuals().selection.stroke.color)
                            };
                            super::appearance::status_badge(ui, &label, color);
                            if self.remote_request {
                                super::appearance::chip(ui, &tr!("egui-transfer-automatic"), ui.visuals().selection.stroke.color);
                            }
                        });
                        ui.add_space(8.0);
                    }
                    if let Some(state) = self.state.clone() {
                        self.progress_view(ui, &state);
                    } else if !self.active && self.result.is_none() {
                        if self.remote_request {
                            ui.add(egui::Label::new(&*tr!("egui-transfer-remote-request")).wrap());
                            ui.add_space(6.0);
                        }
                        self.protocol_view(ui, options);
                    }
                    if let Some(result) = &self.result {
                        ui.add_space(8.0);
                        ui.add(egui::Label::new(result).wrap());
                    }
                    if let Some(destination) = self.destination.as_ref().filter(|_| finished && self.download) {
                        ui.add_space(4.0);
                        super::appearance::value_row(ui, &tr!("egui-transfer-saved-to"), &destination.display().to_string());
                    }
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
                        }
                        if ui.button(&*tr!("egui-close")).clicked() {
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
