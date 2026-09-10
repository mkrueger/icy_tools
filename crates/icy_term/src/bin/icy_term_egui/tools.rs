use eframe::egui;
use icy_term::{TerminalCommand, TerminalEvent};

#[derive(Default)]
pub struct Tools {
    pub serial_open: bool,
    pub serial: icy_net::serial::Serial,
    pub script_running: bool,
    pub script_result: Option<String>,
    pub script_code: Option<String>,
    pub pause: Option<u64>,
    pub host_info: Option<Vec<(String, String)>>,
    pub terminal_info: Option<Vec<(String, String)>>,
    pub info_open: bool,
    pub terminal: Option<icy_term::Address>,
    pub scrollback: usize,
    commands: Vec<TerminalCommand>,
}

impl Tools {
    pub fn event(&mut self, event: &TerminalEvent) {
        match event {
            TerminalEvent::ScriptStarted(path) => {
                self.script_running = true;
                self.script_result = Some(path.display().to_string());
            }
            TerminalEvent::ScriptFinished(result) => {
                self.script_running = false;
                self.script_result = Some(match result {
                    Ok(()) => tr!("egui-script-finished"),
                    Err(error) => error.clone(),
                });
            }
            TerminalEvent::InformDelay(delay) => self.pause = Some(*delay),
            TerminalEvent::ContinueAfterDelay => self.pause = None,
            TerminalEvent::EmsiLogin(info) => {
                self.host_info = Some(vec![
                    (tr!("show-iemsi-dialog-name"), info.name.clone()),
                    (tr!("show-iemsi-dialog-location"), info.location.clone()),
                    (tr!("show-iemsi-dialog-operator"), info.operator.clone()),
                    (tr!("show-iemsi-dialog-id"), info.id.clone()),
                    (tr!("show-iemsi-dialog-capabilities"), info.capabilities.clone()),
                    (tr!("show-iemsi-dialog-notice"), info.notice.clone()),
                ])
            }
            TerminalEvent::Disconnected(_) => {
                self.pause = None;
                self.script_running = false;
            }
            TerminalEvent::SerialBaudDetected(baud) => self.serial.baud_rate = *baud,
            TerminalEvent::SerialAutoDetectComplete => self.serial_open = true,
            _ => {}
        }
    }

    pub fn menu(&mut self, ui: &mut egui::Ui, connected: bool, options: &icy_term::Options) {
        if ui.add_enabled(!connected, egui::Button::new(&*tr!("egui-serial-command"))).clicked() {
            self.serial = options.serial.clone();
            self.serial_open = true;
            ui.close();
        }
        if self.script_running {
            if ui.button(&*tr!("egui-stop-script")).clicked() {
                self.commands.push(TerminalCommand::StopScript);
                ui.close();
            }
        } else {
            if ui.button(&*tr!("egui-run-script")).clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("Lua", &["lua"]).pick_file() {
                    self.commands.push(TerminalCommand::RunScript(path));
                    self.script_running = true;
                }
                ui.close();
            }
            if ui.button(&*tr!("egui-lua-command")).clicked() {
                self.script_code = Some(String::new());
                ui.close();
            }
        }
        if ui.button(&*tr!("egui-replay-file")).clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                self.commands.push(TerminalCommand::PlayFile(path));
            }
            ui.close();
        }
        if ui.add_enabled(self.host_info.is_some(), egui::Button::new(&*tr!("egui-host-info"))).clicked() {
            self.info_open = true;
            ui.close();
        }
    }

    pub fn show(&mut self, context: &egui::Context) -> Vec<TerminalCommand> {
        if let Some(profile) = &mut self.terminal {
            let mut close = false;
            egui::Modal::new(egui::Id::new("live-terminal-settings"))
                .frame(super::appearance::dialog_frame(context))
                .show(context, |ui| {
                    ui.set_width((context.content_rect().width() - 48.0).clamp(220.0, 560.0));
                    ui.heading(&*tr!("egui-terminal-settings"));
                    egui::ScrollArea::vertical()
                        .max_height((context.content_rect().height() - 180.0).max(80.0))
                        .show(ui, |ui| {
                            super::dialing_directory::profile_editor::terminal(ui, profile);
                            ui.separator();
                            super::dialing_directory::profile_editor::colors(ui, profile);
                        });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.add(super::appearance::primary_button(tr!("egui-apply-reset"))).clicked() {
                            self.commands.push(TerminalCommand::SetTerminalProfile {
                                profile: profile.clone(),
                                scrollback: self.scrollback,
                            });
                            close = true;
                        }
                        if ui.button(&*tr!("egui-discard")).clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                self.terminal = None;
            }
        }
        if self.serial_open {
            egui::Modal::new(egui::Id::new("serial"))
                .frame(super::appearance::dialog_frame(context))
                .show(context, |ui| {
                    ui.set_width((context.content_rect().width() - 48.0).clamp(220.0, 480.0));
                    ui.heading(&*tr!("egui-serial-connection"));
                    egui::ScrollArea::vertical()
                        .max_height((context.content_rect().height() - 160.0).max(80.0))
                        .show(ui, |ui| {
                            super::settings::serial_fields(ui, &mut self.serial);
                        });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !self.serial.device.trim().is_empty(),
                                super::appearance::primary_button(tr!("dialing_directory-connect-button")),
                            )
                            .clicked()
                        {
                            self.commands.push(TerminalCommand::OpenSerial(self.serial.clone()));
                            self.serial_open = false;
                        }
                        if ui
                            .add_enabled(!self.serial.device.trim().is_empty(), egui::Button::new(&*tr!("egui-detect-baud")))
                            .clicked()
                        {
                            self.commands.push(TerminalCommand::AutoDetectSerial(self.serial.clone()));
                            self.serial_open = false;
                        }
                        if ui.button(&*tr!("egui-cancel")).clicked() {
                            self.serial_open = false;
                        }
                    });
                });
        }
        if let Some(code) = &mut self.script_code {
            let mut close = false;
            egui::Modal::new(egui::Id::new("script"))
                .frame(super::appearance::dialog_frame(context))
                .show(context, |ui| {
                    ui.set_width((context.content_rect().width() - 48.0).clamp(220.0, 680.0));
                    ui.heading(&*tr!("egui-lua-console"));
                    egui::ScrollArea::vertical()
                        .max_height((context.content_rect().height() - 180.0).max(80.0))
                        .show(ui, |ui| {
                            ui.add(egui::TextEdit::multiline(code).code_editor().desired_rows(14).desired_width(f32::INFINITY));
                        });
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !self.script_running && !code.trim().is_empty(),
                                super::appearance::primary_button(tr!("egui-run")),
                            )
                            .clicked()
                        {
                            self.commands.push(TerminalCommand::RunScriptCode(code.clone()));
                            self.script_running = true;
                        }
                        if self.script_running && ui.button(&*tr!("egui-stop")).clicked() {
                            self.commands.push(TerminalCommand::StopScript);
                        }
                        if ui.button(&*tr!("egui-close")).clicked() {
                            close = true;
                        }
                    });
                    if let Some(result) = &self.script_result {
                        ui.label(result);
                    }
                });
            if close {
                self.script_code = None;
            }
        }
        if self.info_open {
            information(
                context,
                "host-information",
                tr!("egui-host-info"),
                &mut self.info_open,
                self.host_info.as_deref().unwrap_or_default(),
            );
        }
        if let Some(info) = &self.terminal_info {
            let mut open = true;
            information(context, "terminal-information", tr!("terminal-menu-info"), &mut open, info);
            if !open {
                self.terminal_info = None;
            }
        }
        std::mem::take(&mut self.commands)
    }

    pub fn blocks_input(&self) -> bool {
        self.serial_open || self.script_code.is_some() || self.info_open || self.terminal.is_some() || self.terminal_info.is_some()
    }
}

fn information(context: &egui::Context, id: &str, title: String, open: &mut bool, fields: &[(String, String)]) {
    egui::Window::new(title).id(egui::Id::new(id)).open(open).show(context, |ui| {
        ui.set_width((context.content_rect().width() - 48.0).clamp(220.0, 480.0));
        egui::ScrollArea::vertical()
            .max_height((context.content_rect().height() - 140.0).max(60.0))
            .show(ui, |ui| {
                for (name, value) in fields {
                    ui.strong(name);
                    ui.label(value);
                    ui.separator();
                }
            });
        if ui.button(tr!("terminal-menu-copy")).clicked() {
            context.copy_text(fields.iter().map(|(name, value)| format!("{name}: {value}")).collect::<Vec<_>>().join("\n"));
        }
    });
}
