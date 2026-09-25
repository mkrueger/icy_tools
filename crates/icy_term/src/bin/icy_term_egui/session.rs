use std::{
    sync::{mpsc, Arc},
    time::Duration,
};

use eframe::egui;
use icy_engine::{Screen, ScreenMode};
use icy_net::{telnet::TerminalEmulation, ConnectionType};
use icy_term::{AddressBook, ConnectionConfig, ConnectionInformation, TerminalCommand, TerminalEvent, TerminalThread};
use parking_lot::Mutex;

pub struct Session {
    pub address_book: Arc<Mutex<AddressBook>>,
    commands: tokio::sync::mpsc::UnboundedSender<TerminalCommand>,
    pub events: mpsc::Receiver<TerminalEvent>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Session {
    pub fn start(screen: Arc<Mutex<Box<dyn Screen>>>, config: ConnectionConfig, context: egui::Context) -> Self {
        let session = Self::idle(screen, context);
        let _ = session.command(TerminalCommand::Connect(config));
        session
    }

    pub fn idle(screen: Arc<Mutex<Box<dyn Screen>>>, context: egui::Context) -> Self {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let address_book = Arc::new(Mutex::new(AddressBook::default()));
        let (commands, mut incoming, shutdown) = TerminalThread::spawn_cancellable(screen, Box::new(icy_parser_core::AnsiParser::new()), address_book.clone());
        let (outgoing, events) = mpsc::channel();
        std::thread::spawn(move || {
            while let Some(event) = incoming.blocking_recv() {
                if outgoing.send(event).is_err() {
                    break;
                }
                context.request_repaint();
            }
            context.request_repaint();
        });
        Self {
            address_book,
            commands,
            events,
            shutdown: Some(shutdown),
        }
    }

    pub fn send(&self, data: Vec<u8>) -> Result<(), String> {
        self.command(TerminalCommand::SendData(data))
    }

    pub fn command(&self, command: TerminalCommand) -> Result<(), String> {
        self.commands.send(command).map_err(|_| "Terminal worker stopped".to_string())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

pub fn connection_config(address: &str, utf8: bool) -> Result<ConnectionConfig, String> {
    if let Some((scheme, _)) = address.split_once("://") {
        if !matches!(scheme, "telnet" | "raw") {
            return Err("This prototype supports telnet:// and raw:// connections".into());
        }
    }
    let connection_info = ConnectionInformation::parse(address).map_err(|error| error.to_string())?;
    if connection_info.host.is_empty() || connection_info.host.chars().any(char::is_whitespace) {
        return Err("Enter a host name or IP address".into());
    }
    if !matches!(connection_info.protocol(), ConnectionType::Telnet | ConnectionType::Raw) {
        return Err("This prototype supports Telnet and Raw TCP".into());
    }
    if connection_info.user_name().is_some() || connection_info.password().is_some() {
        return Err("Log in inside the terminal; URL credentials are not supported yet".into());
    }
    Ok(ConnectionConfig {
        connection_info,
        terminal_type: if utf8 { TerminalEmulation::Utf8Ansi } else { TerminalEmulation::Ansi },
        window_size: (80, 25),
        timeout: Duration::from_secs(15),
        user_name: None,
        password: None,
        ssh_authentication: Default::default(),
        ssh_private_key: None,
        ssh_key_passphrase: None,
        ssh_host_key_policy: None,
        websocket_address: None,
        proxy_command: None,
        proxy: None,
        modem: None,
        ansi_music: Default::default(),
        screen_mode: if utf8 { ScreenMode::Unicode(80, 25) } else { ScreenMode::Vga(80, 25) },
        baud_emulation: Default::default(),
        iemsi_auto_login: false,
        auto_login_exp: String::new(),
        max_scrollback_lines: 2000,
        transfer_protocols: icy_term::default_protocols(),
        confirm_auto_transfer: true,
        mouse_reporting_enabled: false,
        lf_expand: true,
        custom_palette: None,
        font: None,
        ice_mode: None,
        default_cursor_shape: Default::default(),
        default_cursor_blinking: true,
        cache_directory: None,
    })
}

pub fn entry_connection_config(entry: &icy_term::Address, options: &icy_term::Options) -> Result<ConnectionConfig, String> {
    if matches!(entry.protocol, ConnectionType::Serial | ConnectionType::Channel) {
        return Err("This connection type is not available in the egui client yet".into());
    }
    super::phonebook::validate_entry(entry)?;
    let mut info = if entry.protocol == ConnectionType::Modem {
        ConnectionInformation::from(entry.clone())
    } else {
        ConnectionInformation::parse(&entry.address).map_err(|_| "Invalid address".to_string())?
    };
    if info.protocol.is_some_and(|protocol| protocol != entry.protocol) {
        return Err("The address URL and selected protocol do not match".into());
    }
    info.protocol = Some(entry.protocol);
    let mut config = connection_config("localhost", entry.terminal_type == TerminalEmulation::Utf8Ansi)?;
    let mut sanitized = entry.clone();
    sanitized.address = if entry.protocol == ConnectionType::Modem {
        entry.address.clone()
    } else {
        info.endpoint()
    };
    sanitized.user_name.clear();
    sanitized.password.clear();
    config.connection_info = sanitized.into();
    config.terminal_type = entry.terminal_type;
    config.screen_mode = entry.get_screen_mode();
    let size = config.screen_mode.window_size();
    if !(1..=500).contains(&size.width) || !(1..=200).contains(&size.height) {
        return Err("Terminal size must be between 1x1 and 500x200".into());
    }
    icy_term::auto_login::AutoLoginParser::parse(&entry.auto_login).map_err(|error| error.to_string())?;
    config.window_size = (size.width as u16, size.height as u16);
    config.user_name = info.user_name().or_else(|| (!entry.user_name.is_empty()).then(|| entry.user_name.clone()));
    config.password = info.password().or_else(|| (!entry.password.is_empty()).then(|| entry.password.clone()));
    if entry.protocol == ConnectionType::SSH {
        if config.user_name.is_none() {
            return Err("SSH requires a user name".into());
        }
        config.ssh_authentication = entry.ssh_authentication;
        config.ssh_private_key = (!entry.ssh_private_key.is_empty()).then(|| entry.ssh_private_key.clone().into());
        config.ssh_key_passphrase = (!entry.ssh_key_passphrase.is_empty()).then(|| entry.ssh_key_passphrase.clone());
        if entry.ssh_authentication == icy_term::SshAuthenticationMode::PrivateKey && config.ssh_private_key.is_none() {
            return Err("Select an SSH private key".into());
        }
        let home = directories::UserDirs::new().ok_or("Cannot locate SSH known_hosts")?;
        config.ssh_host_key_policy = Some(icy_net::ssh::HostKeyPolicy::KnownHosts {
            path: home.home_dir().join(".ssh/known_hosts"),
            accept_new: true,
        });
    }
    if matches!(entry.protocol, ConnectionType::Websocket | ConnectionType::SecureWebsocket) {
        if entry.proxy.is_some() {
            return Err("WebSocket connections do not support SOCKS5 proxies. Remove the proxy explicitly before dialing.".into());
        }
        let scheme = if entry.protocol == ConnectionType::SecureWebsocket { "wss" } else { "ws" };
        let url = url::Url::parse(&if entry.address.contains("://") {
            entry.address.clone()
        } else {
            format!("{scheme}://{}", entry.address)
        })
        .map_err(|_| "Invalid WebSocket URL")?;
        config.websocket_address = Some(format!(
            "{}{}{}",
            info.endpoint(),
            url.path(),
            url.query().map(|query| format!("?{query}")).unwrap_or_default()
        ));
    }
    config.auto_login_exp = entry.auto_login.clone();
    config.iemsi_auto_login = options.iemsi.autologin;
    config.timeout = options.connect_timeout;
    config.max_scrollback_lines = options.max_scrollback_lines;
    config.transfer_protocols = options.transfer_protocols.clone();
    config.default_cursor_shape = options.default_cursor_shape;
    config.default_cursor_blinking = options.default_cursor_blinking;
    config.baud_emulation = entry.baud_emulation;
    config.ansi_music = entry.ansi_music;
    config.proxy = entry.proxy.clone();
    config.proxy_command = (!entry.proxy_command.is_empty()).then(|| entry.proxy_command.clone());
    config.modem = if entry.protocol == ConnectionType::Modem {
        Some(
            options
                .modems
                .iter()
                .find(|modem| modem.name == entry.modem_id)
                .cloned()
                .ok_or("Select a configured modem")?,
        )
    } else {
        None
    };
    config.custom_palette = entry.custom_palette.clone();
    config.font = entry
        .font_name
        .as_deref()
        .map(icy_engine::BitFont::from_sauce_name)
        .transpose()
        .map_err(|_| "Unknown terminal font".to_string())?;
    config.ice_mode = Some(if entry.ice_mode {
        icy_engine::IceMode::Ice
    } else {
        icy_engine::IceMode::Blink
    });
    config.lf_expand = entry.lf_expand();
    config.mouse_reporting_enabled = entry.mouse_reporting_enabled;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::{Position, TextScreen};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::Instant,
    };

    #[test]
    fn phonebook_config_preserves_connection_settings() {
        let mut entry = icy_term::Address::new("Test BBS");
        entry.address = "localhost:2323".into();
        entry.protocol = ConnectionType::Raw;
        entry.user_name = "test-user".into();
        entry.password = "test-password".into();
        entry.terminal_type = TerminalEmulation::Utf8Ansi;
        entry.screen_mode = ScreenMode::Unicode(132, 40);
        entry.set_lf_expand(false);
        entry.custom_palette = Some(vec![[12, 34, 56]; 16]);
        entry.font_name = Some("IBM VGA".into());
        entry.ice_mode = true;
        let options = icy_term::Options::default();
        let config = entry_connection_config(&entry, &options).unwrap();
        assert_eq!(config.connection_info.protocol(), ConnectionType::Raw);
        assert_eq!(config.window_size, (132, 40));
        assert_eq!(config.user_name.as_deref(), Some("test-user"));
        assert_eq!(config.password.as_deref(), Some("test-password"));
        assert_eq!(config.custom_palette, entry.custom_palette);
        assert!(config.font.is_some());
        assert_eq!(config.ice_mode, Some(icy_engine::IceMode::Ice));
        assert!(!config.lf_expand);
        assert_eq!(config.timeout, options.connect_timeout);
        entry.address = "telnet://localhost".into();
        assert!(entry_connection_config(&entry, &options).is_err());
        entry.protocol = ConnectionType::SSH;
        entry.address = "localhost".into();
        let ssh = entry_connection_config(&entry, &options).unwrap();
        assert!(matches!(
            ssh.ssh_host_key_policy,
            Some(icy_net::ssh::HostKeyPolicy::KnownHosts { accept_new: true, .. })
        ));
        entry.protocol = ConnectionType::Telnet;
        entry.address = "telnet://url-user:url-password@localhost:2323".into();
        let config = entry_connection_config(&entry, &options).unwrap();
        assert_eq!(config.password.as_deref(), Some("url-password"));
        assert!(!config.connection_info.to_string().contains("url-password"));
        entry.protocol = ConnectionType::Websocket;
        entry.address = "ws://localhost:8080/terminal?mode=binary".into();
        assert_eq!(
            entry_connection_config(&entry, &options).unwrap().websocket_address.as_deref(),
            Some("localhost:8080/terminal?mode=binary")
        );
    }

    #[test]
    fn profiles_support_worker_protocols_and_terminal_modes() {
        let mut entry = icy_term::Address::new("Compatibility");
        entry.address = "localhost".into();
        let options = icy_term::Options::default();
        for protocol in [
            ConnectionType::Telnet,
            ConnectionType::Raw,
            ConnectionType::Websocket,
            ConnectionType::SecureWebsocket,
            ConnectionType::Rlogin,
            ConnectionType::RloginSwapped,
        ] {
            entry.protocol = protocol;
            for terminal in icy_term::ALL_TERMINALS {
                entry.terminal_type = terminal;
                entry.screen_mode = icy_term::normalize_screen_mode(terminal, ScreenMode::default());
                let config = entry_connection_config(&entry, &options).unwrap();
                assert_eq!(config.connection_info.protocol(), protocol);
                assert_eq!(config.terminal_type, terminal);
                assert_eq!(config.screen_mode, entry.get_screen_mode());
            }
        }
    }

    #[test]
    fn raw_session_receives_sends_and_closes() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut entry = icy_term::Address::default();
        entry.system_name = "Profile roundtrip".into();
        entry.protocol = ConnectionType::Raw;
        entry.address = format!("raw://{}", listener.local_addr().unwrap());
        entry.screen_mode = ScreenMode::Vga(100, 35);
        entry.font_name = Some("IBM VGA".into());
        entry.ice_mode = true;
        let config = entry_connection_config(&entry, &icy_term::Options::default()).unwrap();
        let (server_tx, server_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            stream.write_all(b"\x1b[2J\x1b[H\x1b[31mREADY").unwrap();
            let mut received = [0; 4];
            stream.read_exact(&mut received).unwrap();
            server_tx.send(received).unwrap();
            assert_eq!(stream.read(&mut [0; 1]).unwrap(), 0);
        });
        let screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(TextScreen::default())));
        let context = egui::Context::default();
        let (repaint_tx, repaint_rx) = mpsc::channel();
        context.set_request_repaint_callback(move |_| {
            let _ = repaint_tx.send(());
        });
        let session = Session::start(screen.clone(), config, context);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut connected = false;
        loop {
            let event = session.events.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap();
            match event {
                TerminalEvent::Connected => connected = true,
                TerminalEvent::Disconnected(error) => panic!("Unexpected disconnect: {error:?}"),
                _ => {}
            }
            if connected && screen.lock().char_at(Position::new(0, 0)).ch == 'R' {
                break;
            }
        }
        repaint_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(screen.lock().width(), 100);
        assert_eq!(screen.lock().height(), 35);
        assert_eq!(screen.lock().ice_mode(), icy_engine::IceMode::Ice);
        assert!(screen.lock().font(0).is_some());
        session.send(b"yes\r".to_vec()).unwrap();
        assert_eq!(server_rx.recv_timeout(Duration::from_secs(5)).unwrap(), *b"yes\r");
        drop(session);
        server.join().unwrap();
    }

    #[test]
    fn secure_websocket_reports_invalid_tls_without_panicking() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut entry = icy_term::Address::new("TLS loopback");
        entry.protocol = ConnectionType::SecureWebsocket;
        entry.address = format!("wss://{}", listener.local_addr().unwrap());
        let config = entry_connection_config(&entry, &icy_term::Options::default()).unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut hello = [0; 1024];
            assert!(socket.read(&mut hello).unwrap() > 0);
            socket.write_all(b"HTTP/1.1 400 Not TLS\r\n\r\n").unwrap();
        });
        let session = Session::start(Arc::new(Mutex::new(Box::new(TextScreen::default()))), config, egui::Context::default());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match session.events.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap() {
                TerminalEvent::Disconnected(Some(_)) => break,
                TerminalEvent::Connected => panic!("Invalid TLS must never connect"),
                _ => {}
            }
        }
        server.join().unwrap();
    }

    #[test]
    fn capture_and_text_upload_use_real_worker_commands() {
        let directory = std::env::temp_dir().join(format!(
            "icy-egui-transfer-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let upload = directory.join("upload.txt");
        let capture = directory.join("capture.ans");
        std::fs::write(&upload, b"payload").unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let config = connection_config(&format!("raw://{}", listener.local_addr().unwrap()), false).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut trigger = [0];
            stream.read_exact(&mut trigger).unwrap();
            assert_eq!(trigger, [b'G']);
            stream.write_all(b"CAPTURE").unwrap();
            let mut payload = [0; 7];
            stream.read_exact(&mut payload).unwrap();
            assert_eq!(&payload, b"payload");
            assert_eq!(stream.read(&mut trigger).unwrap(), 0);
        });
        let screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(TextScreen::default())));
        let session = Session::start(screen.clone(), config, egui::Context::default());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if matches!(
                session.events.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap(),
                TerminalEvent::Connected
            ) {
                break;
            }
        }
        session.command(TerminalCommand::StartCapture(capture.to_string_lossy().into_owned())).unwrap();
        session.send(vec![b'G']).unwrap();
        while screen.lock().char_at((6, 0).into()).ch != 'E' {
            let event = session.events.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap();
            assert!(!matches!(event, TerminalEvent::Error(_, _) | TerminalEvent::Disconnected(_)), "{event:?}");
        }
        session.command(TerminalCommand::StopCapture).unwrap();
        session
            .command(TerminalCommand::StartUpload(
                icy_term::TransferProtocol::from_internal_id("@text").unwrap(),
                vec![upload],
            ))
            .unwrap();
        loop {
            let event = session.events.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap();
            if matches!(event, TerminalEvent::TransferCompleted(_)) {
                break;
            }
            assert!(!matches!(event, TerminalEvent::Error(_, _) | TerminalEvent::Disconnected(_)), "{event:?}");
        }
        assert_eq!(std::fs::read(capture).unwrap(), b"CAPTURE");
        drop(session);
        server.join().unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn idle_worker_runs_lua_and_reports_errors() {
        let session = Session::idle(Arc::new(Mutex::new(Box::new(TextScreen::default()))), egui::Context::default());
        for (code, success) in [("assert(1 + 1 == 2)", true), ("error('egui regression')", false)] {
            session.command(TerminalCommand::RunScriptCode(code.into())).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                match session.events.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap() {
                    TerminalEvent::ScriptFinished(result) => {
                        assert_eq!(result.is_ok(), success);
                        break;
                    }
                    TerminalEvent::Disconnected(error) => panic!("Unexpected disconnect: {error:?}"),
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn auto_download_waits_for_approval_on_the_real_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let config = connection_config(&format!("raw://{}", listener.local_addr().unwrap()), false).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            stream.write_all(b"**\x18B00").unwrap();
            let mut answer = [0];
            stream.read_exact(&mut answer).unwrap();
            assert_eq!(answer, [b'G'], "A protocol response was sent before approval");
        });
        let session = Session::start(Arc::new(Mutex::new(Box::new(TextScreen::default()))), config, egui::Context::default());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match session.events.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap() {
                TerminalEvent::AutoTransferTriggered(protocol, download, _) => {
                    assert_eq!(protocol, "@zmodem");
                    assert!(download);
                    break;
                }
                TerminalEvent::TransferStarted(..) | TerminalEvent::Error(..) | TerminalEvent::Disconnected(..) => panic!("Transfer started before approval"),
                _ => {}
            }
        }
        session.send(vec![b'G']).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn stalled_xmodem_can_be_cancelled_without_losing_commands() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let config = connection_config(&format!("raw://{}", listener.local_addr().unwrap()), false).unwrap();
        let (ready, waiting) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut first = [0];
            stream.read_exact(&mut first).unwrap();
            ready.send(()).unwrap();
            let mut rest = Vec::new();
            stream.read_to_end(&mut rest).unwrap();
            assert!(rest.contains(&0x18), "Xmodem cancellation was not sent");
        });
        let session = Session::start(Arc::new(Mutex::new(Box::new(TextScreen::default()))), config, egui::Context::default());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if matches!(
                session.events.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap(),
                TerminalEvent::Connected
            ) {
                break;
            }
        }
        session
            .command(TerminalCommand::StartDownload(
                icy_term::TransferProtocol::from_internal_id("@xmodem").unwrap(),
                Some("cancelled.bin".into()),
            ))
            .unwrap();
        waiting.recv_timeout(Duration::from_secs(5)).unwrap();
        session.command(TerminalCommand::RunScriptCode("assert(2 + 2 == 4)".into())).unwrap();
        session.command(TerminalCommand::CancelTransfer).unwrap();
        let mut cancelled = false;
        loop {
            match session.events.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap() {
                TerminalEvent::TransferCompleted(state) => {
                    assert!(state.request_cancel);
                    cancelled = true;
                }
                TerminalEvent::ScriptFinished(result) => {
                    assert!(result.is_ok());
                    assert!(cancelled);
                    break;
                }
                TerminalEvent::Error(title, detail) => panic!("{title}: {detail}"),
                _ => {}
            }
        }
        drop(session);
        server.join().unwrap();
    }

    #[test]
    fn live_terminal_profile_applies_all_presentation_settings() {
        let screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(TextScreen::default())));
        let session = Session::idle(screen.clone(), egui::Context::default());
        let mut profile = icy_term::Address::default();
        profile.screen_mode = ScreenMode::Vga(100, 35);
        profile.font_name = Some("IBM VGA".into());
        profile.ice_mode = true;
        profile.mouse_reporting_enabled = true;
        profile.set_lf_expand(false);
        session.command(TerminalCommand::SetTerminalProfile { profile, scrollback: 123 }).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if matches!(
                session.events.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap(),
                TerminalEvent::TerminalSettingsChanged { .. }
            ) {
                break;
            }
        }
        let screen = screen.lock();
        assert_eq!((screen.width(), screen.height()), (100, 35));
        assert_eq!(screen.ice_mode(), icy_engine::IceMode::Ice);
        assert!(screen.font(0).is_some());
        assert!(screen.terminal_state().mouse_state.mouse_tracking_enabled);
        assert!(!screen.terminal_state().lf_expand);
    }

    #[test]
    fn rlogin_handshake_preserves_both_credential_orders() {
        for protocol in [ConnectionType::Rlogin, ConnectionType::RloginSwapped] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let mut entry = icy_term::Address::new("Rlogin loopback");
            entry.address = listener.local_addr().unwrap().to_string();
            entry.protocol = protocol;
            entry.user_name = "user".into();
            entry.password = "pass".into();
            let config = entry_connection_config(&entry, &icy_term::Options::default()).unwrap();
            let server = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let expected = if protocol == ConnectionType::Rlogin {
                    b"\0pass\0user\0ANSI/115200\0"
                } else {
                    b"\0user\0pass\0ANSI/115200\0"
                };
                let mut received = vec![0; expected.len()];
                socket.read_exact(&mut received).unwrap();
                assert_eq!(&received, expected);
                socket.write_all(b"READY").unwrap();
                let mut response = [0; 4];
                socket.read_exact(&mut response).unwrap();
                assert_eq!(&response, b"yes\r");
            });
            let screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(TextScreen::default())));
            let session = Session::start(screen.clone(), config, egui::Context::default());
            let deadline = Instant::now() + Duration::from_secs(5);
            while screen.lock().char_at(Position::new(0, 0)).ch != 'R' {
                session.events.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap();
            }
            session.send(b"yes\r".to_vec()).unwrap();
            server.join().unwrap();
        }
    }

    #[test]
    fn websocket_session_uses_real_worker_transport() {
        use icy_net::Connection;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut entry = icy_term::Address::new("WebSocket loopback");
        entry.protocol = ConnectionType::Websocket;
        entry.address = format!("ws://{}/terminal", listener.local_addr().unwrap());
        let config = entry_connection_config(&entry, &icy_term::Options::default()).unwrap();
        let server = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            listener.set_nonblocking(true).unwrap();
            runtime.block_on(async move {
                tokio::time::timeout(Duration::from_secs(5), async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    let (stream, _) = listener.accept().await.unwrap();
                    let mut socket = icy_net::websocket::accept_websocket(stream).await.unwrap();
                    socket.send(b"READY").await.unwrap();
                    let mut buffer = [0; 16];
                    let count = socket.read(&mut buffer).await.unwrap();
                    assert_eq!(&buffer[..count], b"yes\r");
                })
                .await
                .unwrap();
            });
        });
        let screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(TextScreen::default())));
        let session = Session::start(screen.clone(), config, egui::Context::default());
        let deadline = Instant::now() + Duration::from_secs(5);
        while screen.lock().char_at(Position::new(0, 0)).ch != 'R' {
            let event = session.events.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap();
            assert!(!matches!(event, TerminalEvent::Disconnected(_)));
        }
        session.send(b"yes\r".to_vec()).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn validates_connection_targets() {
        assert!(connection_config("", false).is_err());
        assert!(connection_config("ssh://localhost", false).is_err());
        assert!(connection_config("https://localhost", false).is_err());
        assert!(connection_config("telnet://user:password@localhost", false).is_err());
        assert_eq!(connection_config("localhost:2323", false).unwrap().connection_info.port(), 2323);
        assert_eq!(
            connection_config("telnet://localhost", true).unwrap().terminal_type,
            TerminalEmulation::Utf8Ansi
        );
    }

    #[test]
    fn telnet_negotiates_window_size_and_reports_remote_close() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let config = connection_config(&format!("telnet://{}", listener.local_addr().unwrap()), false).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            stream.write_all(&[255, 253, 31]).unwrap();
            let expected = [255, 250, 31, 0, 80, 0, 25, 255, 240];
            let mut received = Vec::new();
            let mut buffer = [0; 256];
            while !received.windows(expected.len()).any(|window| window == expected) {
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0);
                received.extend_from_slice(&buffer[..count]);
            }
            stream.write_all(b"\x1b[2J\x1b[HTELNET").unwrap();
        });
        let screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(TextScreen::default())));
        let session = Session::start(screen, config, egui::Context::default());
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut connected = false;
        loop {
            match session.events.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap() {
                TerminalEvent::Connected => connected = true,
                TerminalEvent::Disconnected(_) => break,
                _ => {}
            }
        }
        assert!(connected);
        server.join().unwrap();
    }

    #[test]
    fn shutdown_interrupts_a_stalled_connection_attempt() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut config = connection_config(&format!("raw://{}", listener.local_addr().unwrap()), false).unwrap();
        config.connection_info.protocol = Some(ConnectionType::SSH);
        let (accepted_tx, accepted_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            accepted_tx.send(()).unwrap();
            let mut buffer = [0; 256];
            loop {
                match stream.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => break,
                    Err(error) => panic!("Connection was not cancelled: {error}"),
                }
            }
        });
        let screen = Arc::new(Mutex::new(Box::new(TextScreen::default()) as Box<dyn Screen>));
        let session = Session::start(screen, config, egui::Context::default());
        accepted_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        drop(session);
        server.join().unwrap();
    }

    #[test]
    fn refused_connection_produces_an_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let config = connection_config(&format!("raw://{}", listener.local_addr().unwrap()), false).unwrap();
        drop(listener);
        let screen = Arc::new(Mutex::new(Box::new(TextScreen::default()) as Box<dyn Screen>));
        let session = Session::start(screen, config, egui::Context::default());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match session.events.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap() {
                TerminalEvent::Connected => panic!("Unexpected connection"),
                TerminalEvent::Disconnected(error) => {
                    assert!(error.is_some());
                    break;
                }
                _ => {}
            }
        }
    }
}
