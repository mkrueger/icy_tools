use super::TerminalApp;
use eframe::egui;
use icy_term::{
    mcp::{
        types::{ScreenCaptureFormat, TerminalState},
        McpCommand, McpServer, ScriptResult, SenderType,
    },
    TerminalCommand,
};
use std::sync::{mpsc, Arc};

pub enum Incoming {
    Ready(u16),
    Command(McpCommand),
    Error(String),
}

pub struct Bridge {
    pub incoming: mpsc::Receiver<Incoming>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Bridge {
    pub fn start(port: u16, context: egui::Context) -> Self {
        let (outgoing, incoming) = mpsc::channel();
        let (shutdown, mut stopped) = tokio::sync::oneshot::channel();
        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = outgoing.send(Incoming::Error(error.to_string()));
                    context.request_repaint();
                    return;
                }
            };
            runtime.block_on(async {
                let listener = match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
                    Ok(listener) => listener,
                    Err(error) => {
                        let _ = outgoing.send(Incoming::Error(error.to_string()));
                        context.request_repaint();
                        return;
                    }
                };
                if let Ok(address) = listener.local_addr() {
                    let _ = outgoing.send(Incoming::Ready(address.port()));
                    context.request_repaint();
                }
                let (server, mut commands) = McpServer::new();
                let server = Arc::new(server).serve(listener);
                tokio::pin!(server);
                loop {
                    tokio::select! {
                        _ = &mut stopped => break,
                        result = &mut server => {
                            if let Err(error) = result { let _ = outgoing.send(Incoming::Error(error.to_string())); context.request_repaint(); }
                            break;
                        }
                        command = commands.recv() => {
                            let Some(command) = command else { break };
                            if outgoing.send(Incoming::Command(command)).is_err() { break; }
                            context.request_repaint();
                        }
                    }
                }
            });
        });
        Self {
            incoming,
            shutdown: Some(shutdown),
        }
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

pub fn respond<T>(sender: SenderType<T>, value: T) {
    if let Some(sender) = sender.lock().take() {
        let _ = sender.send(value);
    }
}

impl TerminalApp {
    pub fn finish_script_response(&mut self, result: ScriptResult) {
        if let Some(response) = self.pending_script.take() {
            respond(response, result);
        }
    }

    pub fn receive_mcp(&mut self, context: &egui::Context) {
        let incoming: Vec<_> = self.mcp.as_ref().map(|bridge| bridge.incoming.try_iter().collect()).unwrap_or_default();
        for incoming in incoming {
            match incoming {
                Incoming::Ready(port) => log::info!("MCP ready on 127.0.0.1:{port}"),
                Incoming::Error(error) => self.error = Some(format!("MCP: {error}")),
                Incoming::Command(command) => self.handle_mcp(command, context),
            }
        }
    }

    pub fn connect_target(&mut self, target: &str, context: &egui::Context) {
        if self.connected || self.connecting {
            self.error = Some("Disconnect before dialing another entry".into());
            return;
        }
        if self.dialing_directory.phonebook.is_none() {
            self.dialing_directory.reload();
        }
        let entry = self
            .dialing_directory
            .phonebook
            .as_ref()
            .and_then(|book| book.book.addresses.iter().find(|entry| entry.system_name == target).cloned());
        let entry = entry.or_else(|| {
            icy_term::ConnectionInformation::parse(target).ok().map(|info| {
                let mut entry = icy_term::Address::from(info);
                if self.utf8 {
                    entry.terminal_type = icy_net::telnet::TerminalEmulation::Utf8Ansi;
                    entry.screen_mode = icy_engine::ScreenMode::Unicode(80, 25);
                }
                entry
            })
        });
        let Some(entry) = entry else {
            self.error = Some("Invalid connection target".into());
            return;
        };
        match super::session::entry_connection_config(&entry, &self.dialing_directory.options) {
            Ok(config) => {
                self.active_profile = Some(entry.clone());
                self.address = super::phonebook::display_address(&entry);
                self.start_connection(config, entry.system_name, context);
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn handle_mcp(&mut self, command: McpCommand, context: &egui::Context) {
        match command {
            McpCommand::Connect(target) => self.connect_target(&target, context),
            McpCommand::Disconnect => self.disconnect(),
            McpCommand::SendText(text) => self.send_mcp_text(&text, context),
            McpCommand::SendKey(key) => {
                if let Some(bytes) = icy_term::scripting::parse_key_string(self.terminal_emulation, &key) {
                    self.command(TerminalCommand::SendData(bytes), context);
                } else {
                    self.error = Some(format!("Unknown key: {key}"));
                }
            }
            McpCommand::GetState(sender) => {
                let screen = self.terminal.original_screen.as_ref().unwrap_or(&self.terminal.screen).lock();
                let mut selection = icy_engine::Selection::new((0, 0));
                selection.lead = (screen.width() - 1, screen.height() - 1).into();
                let cursor = screen.caret_position();
                respond(
                    sender,
                    TerminalState {
                        cursor_position: (cursor.x.max(0) as usize, cursor.y.max(0) as usize),
                        screen_size: (screen.width().max(0) as usize, screen.height().max(0) as usize),
                        current_buffer: icy_engine::clipboard::text(&**screen, screen.buffer_type(), &selection).unwrap_or_default(),
                        is_connected: self.connected,
                        current_bbs: self.active_profile.as_ref().map(|entry| entry.system_name.clone()),
                    },
                );
            }
            McpCommand::ListAddresses(sender) => {
                if self.dialing_directory.phonebook.is_none() {
                    self.dialing_directory.reload();
                }
                let mut addresses = self
                    .dialing_directory
                    .phonebook
                    .as_ref()
                    .map(|book| book.book.addresses.clone())
                    .unwrap_or_default();
                for entry in &mut addresses {
                    entry.address = super::phonebook::display_address(entry);
                    entry.password.clear();
                    entry.ssh_key_passphrase.clear();
                    entry.ssh_private_key.clear();
                    entry.proxy = None;
                    entry.proxy_command.clear();
                    entry.auto_login.clear();
                }
                respond(sender, addresses);
            }
            McpCommand::CaptureScreen(format, sender) => {
                let mut screen = self.terminal.screen.lock();
                let options = icy_engine::SaveOptions::ansi(icy_engine::AnsiCompatibilityLevel::Utf8Terminal);
                let data = screen.to_bytes(
                    match format {
                        ScreenCaptureFormat::Text => "asc",
                        ScreenCaptureFormat::Ansi => "ans",
                    },
                    &options,
                );
                match data {
                    Ok(data) => respond(sender, data),
                    Err(error) => {
                        self.error = Some(error.to_string());
                        respond(sender, Vec::new());
                    }
                }
            }
            McpCommand::RunScript(code, response) => {
                if self.tools.script_running {
                    if let Some(response) = response {
                        respond(response, Err("A script is already running".into()));
                    }
                    return;
                }
                self.pending_script = response;
                self.tools.script_running = true;
                self.command(TerminalCommand::RunScriptCode(code), context);
            }
            McpCommand::RunMacro { commands, .. } => {
                for text in commands {
                    self.send_mcp_text(&text, context);
                }
            }
            McpCommand::UploadFile { protocol, file_path } => {
                if let Some(protocol) = self.transfer_protocol(&protocol) {
                    self.command(TerminalCommand::StartUpload(protocol, vec![file_path.into()]), context);
                }
            }
            McpCommand::DownloadFile { protocol, save_path } => {
                if let Some(protocol) = self.transfer_protocol(&protocol) {
                    let path = std::path::PathBuf::from(&save_path);
                    if let Some(parent) = path.parent() {
                        self.command(TerminalCommand::SetDownloadDirectory(parent.into()), context);
                    }
                    self.command(TerminalCommand::StartDownload(protocol, Some(save_path)), context);
                }
            }
            McpCommand::SearchBuffer {
                pattern,
                case_sensitive,
                regex,
            } => {
                self.navigation.find_open = true;
                self.navigation.query = pattern;
                self.navigation.case_sensitive = case_sensitive;
                if regex {
                    match regex::RegexBuilder::new(&self.navigation.query).case_insensitive(!case_sensitive).build() {
                        Ok(pattern) => {
                            let found = {
                                let screen = self.terminal.screen.lock();
                                (0..screen.height()).find_map(|row| {
                                    let line: String = (0..screen.width())
                                        .map(|column| screen.buffer_type().convert_to_unicode(screen.char_at((column, row).into()).ch))
                                        .collect();
                                    pattern.find(&line).filter(|matched| !matched.is_empty()).map(|matched| {
                                        let mut selection = icy_engine::Selection::new((line[..matched.start()].chars().count() as i32, row));
                                        selection.lead = (line[..matched.end()].chars().count() as i32 - 1, row).into();
                                        selection
                                    })
                                })
                            };
                            if let Some(selection) = found {
                                let _ = self.terminal.screen.lock().set_selection(selection);
                                self.navigation.scroll_to = Some(selection.anchor.y as f32 * self.terminal.render_info.read().font_height);
                            } else {
                                self.navigation.search_failed = true;
                            }
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    }
                } else {
                    self.navigation.find(&self.terminal, false);
                }
            }
            McpCommand::ClearScreen => {
                if let Some(screen) = self.terminal.original_screen.as_ref().unwrap_or(&self.terminal.screen).lock().as_editable() {
                    screen.clear_screen();
                }
            }
        }
    }

    fn send_mcp_text(&mut self, text: &str, context: &egui::Context) {
        let bytes = {
            let screen = self.terminal.original_screen.as_ref().unwrap_or(&self.terminal.screen).lock();
            super::input::encode_terminal_events(&[egui::Event::Text(text.into())], screen.buffer_type(), false, self.terminal_emulation)
        };
        self.command(TerminalCommand::SendData(bytes), context);
    }

    fn transfer_protocol(&mut self, name: &str) -> Option<icy_term::TransferProtocol> {
        let protocol = self
            .dialing_directory
            .options
            .transfer_protocols
            .iter()
            .find(|protocol| protocol.enabled && (protocol.id == name || protocol.id == format!("@{name}")))
            .cloned();
        if protocol.is_none() {
            self.error = Some(format!("Unknown or disabled protocol: {name}"));
        }
        protocol
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;

    #[tokio::test]
    async fn mcp_bridge_serves_a_real_http_initialize_request() {
        let bridge = Bridge::start(0, egui::Context::default());
        let port = match bridge.incoming.recv_timeout(std::time::Duration::from_secs(5)).unwrap() {
            Incoming::Ready(port) => port,
            Incoming::Error(error) => panic!("{error}"),
            _ => panic!("MCP did not report readiness"),
        };
        let response = reqwest::Client::new().post(format!("http://127.0.0.1:{port}/"))
            .timeout(std::time::Duration::from_secs(5))
            .header("Accept", "application/json, text/event-stream")
            .json(&serde_json::json!({"jsonrpc":"2.0", "id":1, "method":"initialize", "params":{"protocolVersion":"2025-03-26", "capabilities":{}, "clientInfo":{"name":"egui-regression", "version":"1"}}}))
            .send().await.unwrap();
        assert!(response.status().is_success(), "{}", response.status());
        let body = response.text().await.unwrap();
        assert!(body.contains("protocolVersion") && body.contains("serverInfo"), "{body}");
        drop(bridge);
    }

    #[test]
    fn state_and_lua_requests_receive_real_responses() {
        let mut app = TerminalApp::new(icy_engine::TextScreen::default(), "test".into());
        let context = egui::Context::default();
        let (sender, mut receiver) = tokio::sync::oneshot::channel();
        app.handle_mcp(McpCommand::GetState(Arc::new(Mutex::new(Some(sender)))), &context);
        let state = receiver.try_recv().unwrap();
        assert_eq!(state.screen_size, (80, 25));
        assert!(!state.is_connected);
        let (sender, mut receiver) = tokio::sync::oneshot::channel();
        let (wake_tx, wake_rx) = std::sync::mpsc::channel();
        context.set_request_repaint_callback(move |_| {
            let _ = wake_tx.send(());
        });
        app.handle_mcp(
            McpCommand::RunScript("assert(2 + 2 == 4)".into(), Some(Arc::new(Mutex::new(Some(sender))))),
            &context,
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let _ = context.run(egui::RawInput::default(), |context| app.show(context));
            if let Ok(result) = receiver.try_recv() {
                assert!(result.is_ok());
                break;
            }
            wake_rx.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now())).unwrap();
        }
        assert!(!app.tools.script_running);
    }
}
