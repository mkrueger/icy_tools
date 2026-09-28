use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;
use icy_net::protocol::{OutputLogMessage, TransferInformation, TransferState};
use icy_term::{Options, TerminalCommand, TerminalEvent, TransferProtocol};

use super::appearance::{Dialog, DialogButton};

/// The dialog keeps these measurements no matter what the transfer reports, so it never jumps around.
const DIALOG_WIDTH: f32 = 560.0;
const BODY_HEIGHT: f32 = 344.0;
const HEADER_HEIGHT: f32 = 40.0;
const NOTE_HEIGHT: f32 = 36.0;
const WARNING_HEIGHT: f32 = 20.0;
const PROTOCOL_ROW_HEIGHT: f32 = 48.0;
const SUCCESS: egui::Color32 = egui::Color32::from_rgb(78, 173, 118);

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    #[default]
    Files,
    Log,
}

/// How a transfer ended, which decides the colors and wording of the finished dialog.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Success,
    Warnings,
    Cancelled,
    Failed,
}

#[derive(Default)]
pub struct Transfers {
    pub open: bool,
    pub active: bool,
    pub download: bool,
    pub protocol_id: String,
    pub state: Option<TransferState>,
    pub result: Option<String>,
    pub outcome: Option<Outcome>,
    pub capture: Option<String>,
    pub request_capture: bool,
    /// Directory the running or last download is written to.
    destination: Option<PathBuf>,
    /// The remote side started this transfer instead of the user.
    remote_request: bool,
    /// A remote request that still needs a local file selection.
    pending_file_request: bool,
    detail_tab: DetailTab,
    /// The protocols report per-file times, the dialog shows the whole transfer.
    started: Option<Instant>,
    /// Frozen at the end, so the elapsed time stops counting once the transfer is over.
    duration: Option<Duration>,
    scroll_to_selection: bool,
    commands: Vec<TerminalCommand>,
}

impl Transfers {
    pub fn choose(&mut self, download: bool) {
        self.open = true;
        self.download = download;
        self.state = None;
        self.result = None;
        self.outcome = None;
        self.destination = None;
        self.remote_request = false;
        self.pending_file_request = false;
        self.detail_tab = DetailTab::default();
        self.started = None;
        self.duration = None;
        self.scroll_to_selection = true;
        if self.protocol_id.is_empty() {
            self.protocol_id = "@zmodem".into();
        }
    }

    fn finish(&mut self, outcome: Outcome, result: String) {
        self.active = false;
        self.outcome = Some(outcome);
        self.result = Some(result);
        self.duration = self.started.map(|started| started.elapsed());
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
                        None => self.finish(Outcome::Failed, tr!("egui-transfer-unknown-protocol", protocol = protocol_id.clone())),
                    }
                }
            }
            TerminalEvent::TransferStarted(state, download) => {
                self.open = true;
                self.active = true;
                self.download = *download;
                self.state = Some(state.clone());
                self.result = None;
                self.outcome = None;
                self.duration = None;
                self.started = Some(Instant::now());
            }
            TerminalEvent::TransferProgress(state) => self.state = Some(state.clone()),
            TerminalEvent::TransferCompleted(state) => {
                self.state = Some(state.clone());
                if state.request_cancel {
                    self.finish(Outcome::Cancelled, tr!("egui-transfer-cancelled"));
                } else if state.send_state.errors > 0 || state.recieve_state.errors > 0 {
                    self.finish(Outcome::Warnings, tr!("egui-transfer-errors"));
                } else {
                    self.finish(Outcome::Success, tr!("egui-transfer-finished"));
                }
            }
            TerminalEvent::ExternalTransferStarted(name, download) => {
                self.open = true;
                self.active = true;
                self.download = *download;
                self.outcome = None;
                self.duration = None;
                self.started = Some(Instant::now());
                self.result = Some(format!("{name} · {}", tr!("transfer-external-in-progress")));
            }
            TerminalEvent::ExternalTransferCompleted(_, _, success, error) => {
                let outcome = if *success && error.is_none() { Outcome::Success } else { Outcome::Failed };
                let result = error.clone().unwrap_or_else(|| {
                    if *success {
                        tr!("egui-transfer-complete")
                    } else {
                        tr!("transfer-external-failed")
                    }
                });
                self.finish(outcome, result);
            }
            TerminalEvent::Disconnected(_) => {
                if self.active {
                    self.finish(Outcome::Failed, tr!("egui-connection-closed"));
                }
                self.capture = None;
            }
            TerminalEvent::Error(title, detail) if self.active => {
                self.finish(Outcome::Failed, format!("{title}: {detail}"));
            }
            _ => {}
        }
    }

    pub fn menu(&mut self, ui: &mut egui::Ui, connected: bool) {
        use super::hotkeys::{Action, shortcut};
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
        self.outcome = None;
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

    /// The protocols offered for the current direction; text uploads cannot be downloaded.
    fn protocols<'a>(&self, options: &'a Options) -> Vec<&'a TransferProtocol> {
        options
            .transfer_protocols
            .iter()
            .filter(|protocol| protocol.enabled && (!self.download || protocol.id != "@text"))
            .collect()
    }

    fn protocol_name(&self, options: &Options) -> String {
        options
            .transfer_protocols
            .iter()
            .find(|protocol| protocol.id == self.protocol_id)
            .cloned()
            .or_else(|| TransferProtocol::from_internal_id(&self.protocol_id))
            .map_or_else(|| self.protocol_id.clone(), |protocol| protocol.get_name())
    }

    /// The accent of the header icon, the badge and the progress bar.
    fn tone(&self, ui: &egui::Ui) -> egui::Color32 {
        match self.outcome {
            Some(Outcome::Success) => SUCCESS,
            Some(Outcome::Warnings) => ui.visuals().warn_fg_color,
            Some(Outcome::Cancelled) => ui.visuals().weak_text_color(),
            Some(Outcome::Failed) => ui.visuals().error_fg_color,
            None => accent(ui),
        }
    }

    fn status_label(&self) -> Option<String> {
        match self.outcome {
            Some(Outcome::Success) => Some(tr!("transfer-status-complete")),
            Some(Outcome::Warnings) => Some(tr!("egui-transfer-status-warnings")),
            Some(Outcome::Cancelled) => Some(tr!("egui-transfer-status-cancelled")),
            Some(Outcome::Failed) => Some(tr!("egui-transfer-status-failed")),
            None if self.active => Some(tr!("transfer-status-active")),
            None => None,
        }
    }

    fn subtitle(&self, options: &Options) -> String {
        if let Some(state) = &self.state {
            state.protocol_name.clone()
        } else if self.active || self.outcome.is_some() {
            self.protocol_name(options)
        } else if self.remote_request {
            tr!("egui-transfer-remote-request")
        } else if self.download {
            tr!("egui-transfer-download-hint")
        } else {
            tr!("egui-transfer-upload-hint")
        }
    }

    fn header(&self, ui: &mut egui::Ui, options: &Options) {
        let title = if self.download { tr!("transfer-download") } else { tr!("transfer-upload") };
        let subtitle = self.subtitle(options);
        let tone = self.tone(ui);
        let status = self.status_label();
        let automatic = self.remote_request.then(|| tr!("egui-transfer-automatic"));
        fixed(ui, HEADER_HEIGHT, egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            let (icon, _) = ui.allocate_exact_size(egui::Vec2::splat(HEADER_HEIGHT), egui::Sense::hover());
            ui.painter().rect_filled(icon, 10.0, tone.gamma_multiply(0.16));
            paint_direction(ui.painter(), icon.center(), self.download, tone);

            // The badges are measured first, so a long subtitle truncates instead of pushing them out.
            let badges =
                status.as_deref().map_or(0.0, |label| badge_width(ui, label) + 12.0) + automatic.as_deref().map_or(0.0, |label| chip_width(ui, label) + 8.0);
            let text_width = (ui.available_width() - badges).max(60.0);
            ui.allocate_ui_with_layout(egui::vec2(text_width, HEADER_HEIGHT), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(text_width);
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.add_space(1.0);
                ui.add(egui::Label::new(super::appearance::bold(ui, &title).size(17.0)).truncate());
                ui.add(egui::Label::new(egui::RichText::new(&subtitle).size(12.0).weak()).truncate())
                    .on_hover_text(&subtitle);
            });
            if badges == 0.0 {
                return;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if let Some(status) = &status {
                    super::appearance::status_badge(ui, status, tone);
                }
                if let Some(automatic) = &automatic {
                    super::appearance::chip(ui, automatic, accent(ui));
                }
            });
        });
    }

    fn progress_view(&mut self, ui: &mut egui::Ui, state: &TransferState) {
        let info = if self.download { &state.recieve_state } else { &state.send_state };
        let finished = self.outcome.is_some() || state.is_finished;
        let tone = self.tone(ui);
        let files = info.finished_files.len();
        let current = !info.file_name.is_empty();
        let done = info.cur_bytes_transfered.min(info.file_size);
        // Between files and after the last one the protocols clear the current file, so show the whole transfer.
        let (name, amount, ratio) = if current {
            let ratio = (done as f32 / info.file_size.max(1) as f32).clamp(0.0, 1.0);
            let amount = format!(
                "{} / {}",
                human_bytes::human_bytes(done as f64),
                human_bytes::human_bytes(info.file_size as f64)
            );
            (info.file_name.clone(), amount, Some(ratio))
        } else if finished {
            let amount = human_bytes::human_bytes(info.total_bytes_transfered as f64);
            (tr!("egui-transfer-summary", count = files), amount, None)
        } else if files > 0 {
            (
                tr!("egui-transfer-next-file"),
                human_bytes::human_bytes(info.total_bytes_transfered as f64),
                None,
            )
        } else {
            (tr!("transfer-waiting"), String::new(), None)
        };
        let bar = ratio.unwrap_or(if finished && files > 0 { 1.0 } else { 0.0 });
        let percent = ratio.map(|ratio| format!("{} %", (ratio * 100.0).round() as u32));
        ui.horizontal(|ui| {
            ui.set_min_height(20.0);
            // The figures are measured first, so a long file name truncates instead of running into them.
            let spacing = ui.spacing().item_spacing.x;
            let text_width = |text: &str| {
                ui.fonts_mut(|fonts| {
                    fonts
                        .layout_no_wrap(text.to_owned(), egui::FontId::proportional(14.0), egui::Color32::PLACEHOLDER)
                        .size()
                        .x
                })
            };
            let mut figures = 0.0;
            if !amount.is_empty() {
                figures += text_width(&amount) + spacing;
            }
            if let Some(percent) = &percent {
                figures += text_width(percent) + text_width("·") + spacing * 2.0;
            }
            let name_width = (ui.available_width() - figures - 12.0).max(40.0);
            ui.allocate_ui_with_layout(egui::vec2(name_width, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_width(name_width);
                ui.add(egui::Label::new(super::appearance::bold(ui, &name).size(15.0)).truncate())
                    .on_hover_text(&name);
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !amount.is_empty() {
                    ui.label(egui::RichText::new(&amount).weak());
                }
                if let Some(percent) = &percent {
                    ui.label(egui::RichText::new("·").weak());
                    ui.label(egui::RichText::new(percent).strong().color(tone));
                }
            });
        });
        ui.add_space(6.0);
        ui.add(egui::ProgressBar::new(bar).desired_height(8.0).corner_radius(4).fill(tone));
        ui.add_space(12.0);

        let elapsed_time = self
            .duration
            .or_else(|| self.started.map(|started| started.elapsed()))
            .unwrap_or_else(|| info.start_time.elapsed());
        let (rate, remaining) = if finished {
            // The protocols reset their rate counters per file, so the result shows the average.
            let seconds = elapsed_time.as_secs_f64();
            let rate = if seconds > 0.0 && info.total_bytes_transfered > 0 {
                format!("{}/s", human_bytes::human_bytes(info.total_bytes_transfered as f64 / seconds))
            } else {
                "–".to_owned()
            };
            (rate, "–".to_owned())
        } else {
            let bps = state.get_current_bps(self.download);
            let rate = if bps > 0 {
                format!("{}/s", human_bytes::human_bytes(bps as f64))
            } else {
                "–".to_owned()
            };
            let remaining = if current && bps > 0 && info.file_size > done {
                elapsed(Duration::from_secs((info.file_size - done) / bps as u64))
            } else {
                "–".to_owned()
            };
            (rate, remaining)
        };
        let tiles = [
            (tr!("transfer-rate"), rate),
            (tr!("transfer-elapsedtime"), elapsed(elapsed_time)),
            (tr!("egui-transfer-remaining"), remaining),
            (tr!("egui-transfer-bytes"), human_bytes::human_bytes(info.total_bytes_transfered as f64)),
        ];
        // All four tiles are always drawn, so finishing a transfer cannot resize the dialog.
        // Narrow dialogs stack them two by two, so the values stay readable.
        let spacing = ui.spacing().item_spacing.x;
        let columns = if ui.available_width() < 420.0 { 2 } else { 4 };
        let width = ((ui.available_width() - spacing * (columns - 1) as f32) / columns as f32).max(64.0);
        for row in tiles.chunks(columns) {
            ui.horizontal(|ui| {
                for (label, value) in row {
                    super::appearance::metric_tile_sized(ui, label, value, width);
                }
            });
        }
        ui.add_space(10.0);

        let warnings = info.warnings();
        let errors = info.errors();
        ui.horizontal(|ui| {
            super::appearance::tab(ui, &mut self.detail_tab, DetailTab::Files, &format!("{} ({files})", tr!("egui-transfer-files")));
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
        let tab = self.detail_tab;
        inset(ui, height, |ui| match tab {
            DetailTab::Files => file_list(ui, info),
            DetailTab::Log => log_list(ui, info),
        });
    }

    /// Fixed area below the details, so a result message or a download path never resizes the dialog.
    fn note_view(&self, ui: &mut egui::Ui, show_result: bool) {
        let tone = self.tone(ui);
        fixed(ui, NOTE_HEIGHT, egui::Layout::top_down(egui::Align::Min), |ui| {
            // A scroll area never grows beyond its maximum, whatever the messages contain.
            egui::ScrollArea::vertical()
                .id_salt("transfer-note")
                .auto_shrink([false, false])
                .max_height(NOTE_HEIGHT)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    // A successful transfer is already told by the badge, other endings explain themselves.
                    if show_result && self.outcome != Some(Outcome::Success) {
                        if let Some(result) = &self.result {
                            ui.add(egui::Label::new(egui::RichText::new(result).color(tone)).truncate())
                                .on_hover_text(result);
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
        let height = (ui.available_height() - NOTE_HEIGHT - ui.spacing().item_spacing.y).max(60.0);
        let message = self.result.clone().unwrap_or_else(|| tr!("transfer-waiting"));
        let tone = self.tone(ui);
        fixed(ui, height, egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.add_space((height / 2.0 - 56.0).max(0.0));
            ui.vertical_centered(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(52.0), egui::Sense::hover());
                if let Some(outcome) = self.outcome {
                    ui.painter().circle_filled(rect.center(), 26.0, tone.gamma_multiply(0.16));
                    paint_outcome(ui.painter(), rect.center(), 11.0, outcome, tone);
                } else {
                    egui::Spinner::new().size(30.0).color(tone).paint_at(ui, rect);
                }
                ui.add_space(10.0);
                egui::ScrollArea::vertical()
                    .id_salt("transfer-status")
                    .auto_shrink([false, true])
                    .max_height(60.0)
                    .show(ui, |ui| {
                        let text = if self.outcome.is_some() {
                            egui::RichText::new(message).size(14.0)
                        } else {
                            egui::RichText::new(message).size(14.0).weak()
                        };
                        ui.add(egui::Label::new(text).wrap());
                    });
            });
        });
    }

    /// The protocol cards; returns whether one was double clicked to start right away.
    fn protocol_view(&mut self, ui: &mut egui::Ui, options: &Options) -> bool {
        let protocols = self.protocols(options);
        if !protocols.iter().any(|protocol| protocol.id == self.protocol_id) {
            if let Some(first) = protocols.first() {
                self.protocol_id = first.id.clone();
            }
        }
        // Arrow keys walk the list like a native list box while no text field has the focus.
        if ui.memory(|memory| memory.focused().is_none()) && !protocols.is_empty() {
            let (up, down) = ui.input_mut(|input| {
                (
                    input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                    input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                )
            });
            if up || down {
                let index = protocols.iter().position(|protocol| protocol.id == self.protocol_id).unwrap_or(0);
                let index = if up { index.saturating_sub(1) } else { (index + 1).min(protocols.len() - 1) };
                self.protocol_id = protocols[index].id.clone();
                self.scroll_to_selection = true;
            }
        }

        ui.label(egui::RichText::new(&*tr!("transfer-protocol")).size(12.0).weak());
        ui.add_space(2.0);
        // The hint row below is always reserved, so switching protocols keeps the list height.
        let height = (ui.available_height() - WARNING_HEIGHT - ui.spacing().item_spacing.y).max(60.0);
        let mut activate = false;
        let scroll_to_selection = std::mem::take(&mut self.scroll_to_selection);
        inset(ui, height, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("transfer-protocols")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for protocol in &protocols {
                        let selected = self.protocol_id == protocol.id;
                        let response = protocol_row(ui, protocol, selected);
                        if selected && scroll_to_selection {
                            response.scroll_to_me(None);
                        }
                        if response.clicked() {
                            self.protocol_id = protocol.id.clone();
                        }
                        if response.double_clicked() {
                            self.protocol_id = protocol.id.clone();
                            activate = true;
                        }
                    }
                });
        });
        let external = protocols.iter().any(|protocol| protocol.id == self.protocol_id && !protocol.is_internal());
        fixed(ui, WARNING_HEIGHT, egui::Layout::left_to_right(egui::Align::Center), |ui| {
            if external {
                ui.add(egui::Label::new(egui::RichText::new(&*tr!("egui-external-warning")).color(ui.visuals().warn_fg_color)).truncate());
            }
        });
        activate
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
        #[derive(Clone, Copy)]
        enum Transfer {
            OpenFolder,
            Close,
            ChooseFiles,
            Cancel,
        }
        let finished = self.outcome.is_some();
        let choosing = self.state.is_none() && !self.active && !finished;
        let mut activate = false;
        let response = Dialog::new("transfer")
            .max_width(DIALOG_WIDTH)
            .confirm_on_enter(!self.active)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    let width = ui.available_width();
                    // Everything below lives in a fixed box, so a finishing transfer cannot resize the dialog.
                    ui.allocate_ui(egui::vec2(width, BODY_HEIGHT), |ui| {
                        ui.set_min_size(egui::vec2(width, BODY_HEIGHT));
                        self.header(ui, options);
                        ui.add_space(14.0);
                        if let Some(state) = self.state.clone() {
                            self.progress_view(ui, &state);
                            self.note_view(ui, true);
                        } else if self.active || finished {
                            self.status_view(ui);
                            self.note_view(ui, false);
                        } else {
                            activate = self.protocol_view(ui, options);
                        }
                    });
                    if self.active {
                        context.request_repaint_after(Duration::from_millis(250));
                    }
                });
                let mut buttons = Vec::new();
                if self.active {
                    buttons.push(DialogButton::primary(tr!("egui-cancel-transfer"), Transfer::Cancel).cancels());
                } else {
                    if finished && self.download && self.destination.is_some() {
                        buttons.push(DialogButton::secondary(tr!("egui-open-download-folder"), Transfer::OpenFolder).leading());
                    }
                    if choosing {
                        let enabled = connected && self.selected_protocol(options).is_some();
                        buttons.push(DialogButton::cancel(tr!("egui-close"), Transfer::Close));
                        buttons.push(DialogButton::primary(tr!("egui-choose-files"), Transfer::ChooseFiles).enabled(enabled));
                    } else {
                        buttons.push(DialogButton::primary(tr!("egui-close"), Transfer::Close).cancels());
                    }
                }
                dialog.buttons(buttons);
            });
        let action = if activate && connected {
            Some(Transfer::ChooseFiles)
        } else {
            response.action
        };
        match action {
            Some(Transfer::Cancel) => self.commands.push(TerminalCommand::CancelTransfer),
            Some(Transfer::ChooseFiles) => {
                if let Some(protocol) = self.selected_protocol(options) {
                    self.choose_files(protocol, options);
                }
            }
            Some(Transfer::OpenFolder) => {
                if let Some(destination) = &self.destination {
                    if let Err(error) = open::that(destination) {
                        self.result = Some(error.to_string());
                    }
                }
            }
            Some(Transfer::Close) => self.open = false,
            None if response.dismissed => self.open = false,
            None => {}
        }
        std::mem::take(&mut self.commands)
    }
}

fn accent(ui: &egui::Ui) -> egui::Color32 {
    ui.visuals().selection.stroke.color
}

/// A sunken, bordered box of a fixed height for lists that scroll inside the dialog.
fn inset(ui: &mut egui::Ui, height: f32, add: impl FnOnce(&mut egui::Ui)) {
    let outer_width = ui.available_width();
    // A frame reserves its margins and its stroke, which have to be taken off the fixed size.
    let overhead = 8.0 + ui.visuals().widgets.noninteractive.bg_stroke.width * 2.0;
    egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(8)
        .inner_margin(4)
        .show(ui, |ui| {
            ui.set_width(outer_width - overhead);
            ui.set_height((height - overhead).max(40.0));
            add(ui);
        });
}

/// A selectable card with the protocol name, its description and its capabilities.
fn protocol_row(ui: &mut egui::Ui, protocol: &TransferProtocol, selected: bool) -> egui::Response {
    let width = content_width(ui);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, PROTOCOL_ROW_HEIGHT), egui::Sense::click());
    let visuals = ui.visuals().clone();
    let accent = accent(ui);
    let painter = ui.painter_at(rect);
    if selected {
        painter.rect_filled(rect.shrink(0.5), 6.0, accent.gamma_multiply(0.16));
        painter.rect_stroke(
            rect.shrink(0.5),
            6.0,
            egui::Stroke::new(1.0, accent.gamma_multiply(0.8)),
            egui::StrokeKind::Inside,
        );
    } else if response.hovered() {
        painter.rect_filled(rect.shrink(0.5), 6.0, visuals.widgets.hovered.weak_bg_fill);
    }

    let radio = egui::pos2(rect.left() + 20.0, rect.center().y);
    if selected {
        painter.circle_filled(radio, 8.0, accent);
        painter.circle_filled(radio, 3.0, egui::Color32::WHITE);
    } else {
        painter.circle_stroke(radio, 7.5, egui::Stroke::new(1.5, visuals.weak_text_color()));
    }

    let mut tags = Vec::new();
    if !protocol.is_internal() {
        tags.push((tr!("egui-protocol-external"), visuals.warn_fg_color));
    }
    if protocol.id == "@zmodem" {
        tags.push((tr!("egui-protocol-recommended"), SUCCESS));
    }
    if protocol.batch {
        tags.push((tr!("egui-protocol-batch"), visuals.text_color()));
    }
    if protocol.auto_transfer {
        tags.push((tr!("egui-protocol-auto"), accent));
    }
    let text_left = rect.left() + 38.0;
    // Tags are placed from the right and dropped when they would squeeze the description too much.
    let minimum_text = (rect.width() * 0.45).max(120.0);
    let mut right = rect.right() - 12.0;
    for (label, color) in &tags {
        let chip = chip_width(ui, label);
        if right - chip - text_left < minimum_text {
            break;
        }
        let chip_rect = egui::Rect::from_min_size(egui::pos2(right - chip, rect.center().y - 8.0), egui::vec2(chip, 16.0));
        painter.rect_filled(chip_rect, 4.0, color.gamma_multiply(0.18));
        painter.text(chip_rect.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(10.0), *color);
        right -= chip + 6.0;
    }

    let text_width = (right - text_left - 8.0).max(40.0);
    let name = single_line(
        ui,
        protocol.get_name(),
        egui::FontId::new(14.0, super::appearance::bold_family(ui)),
        visuals.strong_text_color(),
        text_width,
    );
    let description = protocol.get_description();
    let detail = single_line(ui, description.clone(), egui::FontId::proportional(12.0), visuals.weak_text_color(), text_width);
    let gap = 2.0;
    let top = rect.center().y - (name.size().y + gap + detail.size().y) / 2.0;
    let detail_top = top + name.size().y + gap;
    painter.galley(egui::pos2(text_left, top), name, visuals.strong_text_color());
    painter.galley(egui::pos2(text_left, detail_top), detail, visuals.weak_text_color());

    let mut hint = description;
    if protocol.batch {
        hint.push_str(&format!("\n• {}", tr!("egui-protocol-batch-hint")));
    }
    if protocol.auto_transfer {
        hint.push_str(&format!("\n• {}", tr!("egui-protocol-auto-hint")));
    }
    if !protocol.is_internal() {
        hint.push_str(&format!("\n• {}", tr!("egui-external-warning")));
    }
    let response = if hint.trim().is_empty() { response } else { response.on_hover_text(hint) };
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Reserves exactly `height` and lays `add` out inside it, so its content can never resize the dialog.
fn fixed(ui: &mut egui::Ui, height: f32, layout: egui::Layout, add: impl FnOnce(&mut egui::Ui)) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(layout));
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    add(&mut child);
}

/// The width inside a scroll area, which draws its bar next to the content rather than above it.
fn content_width(ui: &egui::Ui) -> f32 {
    (ui.available_width() - ui.spacing().scroll.allocated_width()).max(40.0)
}

/// Lays out one line of text that ends in an ellipsis when it does not fit.
fn single_line(ui: &egui::Ui, text: String, font: egui::FontId, color: egui::Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width);
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}

fn chip_width(ui: &egui::Ui, label: &str) -> f32 {
    ui.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(label.to_owned(), egui::FontId::proportional(10.0), egui::Color32::PLACEHOLDER)
            .size()
            .x
    }) + 10.0
}

fn badge_width(ui: &egui::Ui, label: &str) -> f32 {
    ui.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(label.to_owned(), egui::FontId::proportional(11.0), egui::Color32::PLACEHOLDER)
            .size()
            .x
    }) + 26.0
}

/// An arrow into or out of a tray, drawn as lines so it looks the same in every font.
fn paint_direction(painter: &egui::Painter, center: egui::Pos2, download: bool, color: egui::Color32) {
    let stroke = egui::Stroke::new(2.0, color);
    let (from, tip, head) = if download {
        (center + egui::vec2(0.0, -10.0), center + egui::vec2(0.0, 4.0), -5.0)
    } else {
        (center + egui::vec2(0.0, 4.0), center + egui::vec2(0.0, -10.0), 5.0)
    };
    painter.line_segment([from, tip], stroke);
    painter.add(egui::Shape::line(vec![tip + egui::vec2(-5.0, head), tip, tip + egui::vec2(5.0, head)], stroke));
    let tray = center.y + 10.0;
    painter.add(egui::Shape::line(
        vec![
            egui::pos2(center.x - 9.0, tray - 4.0),
            egui::pos2(center.x - 9.0, tray),
            egui::pos2(center.x + 9.0, tray),
            egui::pos2(center.x + 9.0, tray - 4.0),
        ],
        stroke,
    ));
}

fn paint_check(painter: &egui::Painter, center: egui::Pos2, size: f32, color: egui::Color32, width: f32) {
    painter.add(egui::Shape::line(
        vec![
            center + egui::vec2(-size, 0.0),
            center + egui::vec2(-size * 0.3, size * 0.7),
            center + egui::vec2(size, -size * 0.7),
        ],
        egui::Stroke::new(width, color),
    ));
}

/// Check mark, exclamation mark or cross, drawn so the icon does not depend on the font's symbols.
fn paint_outcome(painter: &egui::Painter, center: egui::Pos2, size: f32, outcome: Outcome, color: egui::Color32) {
    let stroke = egui::Stroke::new(2.6, color);
    match outcome {
        Outcome::Success => paint_check(painter, center, size, color, 2.6),
        Outcome::Warnings | Outcome::Cancelled => {
            painter.line_segment([center + egui::vec2(0.0, -size), center + egui::vec2(0.0, size * 0.35)], stroke);
            painter.circle_filled(center + egui::vec2(0.0, size * 0.9), 1.6, color);
        }
        Outcome::Failed => {
            let arm = size * 0.75;
            painter.line_segment([center + egui::vec2(-arm, -arm), center + egui::vec2(arm, arm)], stroke);
            painter.line_segment([center + egui::vec2(-arm, arm), center + egui::vec2(arm, -arm)], stroke);
        }
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
            ui.spacing_mut().item_spacing.y = 0.0;
            for (name, path) in &info.finished_files {
                let name = if name.is_empty() {
                    path.file_name().unwrap_or_default().to_string_lossy().into_owned()
                } else {
                    name.clone()
                };
                let (rect, response) = ui.allocate_exact_size(egui::vec2(content_width(ui), 26.0), egui::Sense::hover());
                if response.hovered() {
                    ui.painter().rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
                }
                let mark = egui::pos2(rect.left() + 14.0, rect.center().y);
                ui.painter().circle_filled(mark, 8.0, SUCCESS.gamma_multiply(0.2));
                paint_check(ui.painter(), mark, 4.0, SUCCESS, 1.6);
                let text = single_line(ui, name, egui::FontId::proportional(13.0), ui.visuals().text_color(), rect.width() - 36.0);
                let top = rect.center().y - text.size().y / 2.0;
                ui.painter().galley(egui::pos2(rect.left() + 30.0, top), text, ui.visuals().text_color());
                response.on_hover_text(path.display().to_string());
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

fn elapsed(duration: Duration) -> String {
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
    fn outcomes_follow_how_the_transfer_ended() {
        let options = Options::default();
        let started = |transfers: &mut Transfers| {
            transfers.choose(true);
            transfers.event(&TerminalEvent::TransferStarted(TransferState::new("Zmodem".into()), true), &options);
            assert!(transfers.active && transfers.outcome.is_none());
        };
        let mut transfers = Transfers::default();

        started(&mut transfers);
        let mut state = TransferState::new("Zmodem".into());
        state.is_finished = true;
        transfers.event(&TerminalEvent::TransferCompleted(state.clone()), &options);
        assert_eq!(transfers.outcome, Some(Outcome::Success));
        let duration = transfers.duration.expect("the elapsed time is frozen at the end");
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(transfers.duration, Some(duration), "the elapsed time keeps counting after the end");

        started(&mut transfers);
        state.recieve_state.log_error("crc mismatch");
        transfers.event(&TerminalEvent::TransferCompleted(state.clone()), &options);
        assert_eq!(transfers.outcome, Some(Outcome::Warnings));

        started(&mut transfers);
        state.request_cancel = true;
        transfers.event(&TerminalEvent::TransferCompleted(state), &options);
        assert_eq!(transfers.outcome, Some(Outcome::Cancelled));

        started(&mut transfers);
        transfers.event(&TerminalEvent::Disconnected(None), &options);
        assert_eq!(transfers.outcome, Some(Outcome::Failed));
        assert!(!transfers.active);
    }

    #[test]
    fn arrow_keys_walk_the_protocols_and_downloads_skip_text() {
        let options = Options::default();
        let context = egui::Context::default();
        let mut transfers = Transfers::default();
        let frame = |transfers: &mut Transfers, key: Option<egui::Key>| {
            let events = key
                .map(|key| egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                })
                .into_iter()
                .collect();
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 800.0))),
                    events,
                    ..Default::default()
                },
                |context| {
                    transfers.show(context, &options, true);
                },
            );
        };
        transfers.choose(false);
        frame(&mut transfers, None);
        assert_eq!(transfers.protocol_id, "@zmodem");
        frame(&mut transfers, Some(egui::Key::ArrowDown));
        assert_eq!(transfers.protocol_id, "@zmodem8k");
        frame(&mut transfers, Some(egui::Key::ArrowUp));
        frame(&mut transfers, Some(egui::Key::ArrowUp));
        assert_eq!(transfers.protocol_id, "@zmodem", "the selection stays on the first protocol");

        // A text upload cannot be received, so the download dialog falls back to the first protocol.
        transfers.protocol_id = "@text".into();
        transfers.choose(true);
        frame(&mut transfers, None);
        assert_eq!(transfers.protocol_id, "@zmodem");
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
