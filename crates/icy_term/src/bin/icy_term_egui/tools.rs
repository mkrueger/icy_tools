use eframe::egui;
use icy_term::{TerminalCommand, TerminalEvent};

use super::appearance::{self, Dialog, DialogButton, DialogSize};

#[derive(Default)]
pub struct Tools {
    pub serial_open: bool,
    pub serial: icy_net::serial::Serial,
    pub script_running: bool,
    pub script_result: Option<String>,
    pub script_code: Option<String>,
    pub pause: Option<u64>,
    pub host_info: Option<Vec<(String, String)>>,
    pub terminal_info: Option<super::terminal_info::Dialog>,
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
        use super::hotkeys::{shortcut, Action};
        if ui
            .add_enabled(
                !connected,
                egui::Button::new(&*tr!("egui-serial-command")).shortcut_text(shortcut(Action::Serial)),
            )
            .clicked()
        {
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
            if ui
                .add(egui::Button::new(&*tr!("egui-run-script")).shortcut_text(shortcut(Action::RunScript)))
                .clicked()
            {
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
            let response = Dialog::new("live-terminal-settings").size(DialogSize::Large).show(context, |dialog| {
                dialog.content(|ui| {
                    appearance::group(ui, &tr!("settings-terminal-category"), |ui| {
                        super::dialing_directory::profile_editor::terminal(ui, profile);
                    });
                    appearance::group(ui, &tr!("egui-colors"), |ui| {
                        super::dialing_directory::profile_editor::colors(ui, profile);
                    });
                });
                dialog.buttons([
                    DialogButton::cancel(tr!("egui-cancel"), false),
                    DialogButton::primary(tr!("egui-apply-reset"), true),
                ]);
            });
            if response.action == Some(true) {
                self.commands.push(TerminalCommand::SetTerminalProfile {
                    profile: profile.clone(),
                    scrollback: self.scrollback,
                });
            }
            if response.action.is_some() || response.dismissed {
                self.terminal = None;
            }
        }
        if self.serial_open {
            #[derive(Clone, Copy)]
            enum Serial {
                Detect,
                Cancel,
                Connect,
            }
            let response = Dialog::new("serial").size(DialogSize::Medium).confirm_on_enter(true).show(context, |dialog| {
                dialog.content(|ui| {
                    appearance::group(ui, "", |ui| {
                        super::settings::serial_fields(ui, &mut self.serial);
                    });
                });
                let device = !self.serial.device.trim().is_empty();
                dialog.buttons([
                    DialogButton::secondary(tr!("egui-detect-baud"), Serial::Detect).leading().enabled(device),
                    DialogButton::cancel(tr!("egui-cancel"), Serial::Cancel),
                    DialogButton::primary(tr!("dialing_directory-connect-button"), Serial::Connect).enabled(device),
                ]);
            });
            match response.action {
                Some(Serial::Detect) => self.commands.push(TerminalCommand::AutoDetectSerial(self.serial.clone())),
                Some(Serial::Connect) => self.commands.push(TerminalCommand::OpenSerial(self.serial.clone())),
                _ => {}
            }
            if response.action.is_some() || response.dismissed {
                self.serial_open = false;
            }
        }
        if let Some(code) = &mut self.script_code {
            #[derive(Clone, Copy)]
            enum Script {
                Stop,
                Close,
                Run,
            }
            let running = self.script_running;
            let result = self.script_result.as_deref();
            let response = Dialog::new("script").size(DialogSize::Large).show(context, |dialog| {
                dialog.content(|ui| {
                    appearance::group(ui, "", |ui| {
                        ui.add(egui::TextEdit::multiline(code).code_editor().desired_rows(14).desired_width(f32::INFINITY));
                        if let Some(result) = result {
                            ui.add_space(6.0);
                            ui.add(egui::Label::new(egui::RichText::new(result).weak()).wrap());
                        }
                    });
                });
                let mut buttons = Vec::new();
                if running {
                    buttons.push(DialogButton::secondary(tr!("egui-stop"), Script::Stop).leading());
                }
                buttons.push(DialogButton::cancel(tr!("egui-close"), Script::Close));
                buttons.push(DialogButton::primary(tr!("egui-run"), Script::Run).enabled(!running && !code.trim().is_empty()));
                dialog.buttons(buttons);
            });
            match response.action {
                Some(Script::Stop) => self.commands.push(TerminalCommand::StopScript),
                Some(Script::Run) => {
                    self.commands.push(TerminalCommand::RunScriptCode(code.clone()));
                    self.script_running = true;
                }
                _ => {}
            }
            if matches!(response.action, Some(Script::Close)) || response.dismissed {
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
        if let Some(info) = &mut self.terminal_info {
            if let Some(command) = info.show(context, self.scrollback) {
                self.commands.push(command);
            }
            if info.closed {
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
    let response = Dialog::new(id).size(DialogSize::Medium).show(context, |dialog| {
        dialog.content(|ui| {
            appearance::group(ui, &title, |ui| {
                for (name, value) in fields {
                    appearance::value_row(ui, name, value);
                }
            });
        });
        dialog.buttons([
            DialogButton::secondary(tr!("terminal-menu-copy"), false).leading(),
            DialogButton::primary(tr!("egui-close"), true).cancels(),
        ]);
    });
    match response.action {
        Some(false) => context.copy_text(fields.iter().map(|(name, value)| format!("{name}: {value}")).collect::<Vec<_>>().join("\n")),
        Some(true) => *open = false,
        None if response.dismissed => *open = false,
        None => {}
    }
}
