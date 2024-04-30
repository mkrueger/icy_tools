use crate::{
    features::{AutoFileTransfer, AutoLogin},
    get_parser,
    util::SoundThread,
    Res, TerminalResult,
};
use directories::UserDirs;
use egui::mutex::Mutex;
use icy_engine::{ansi::MusicOption, rip::bgi::MouseField, BufferParser, Caret};
use icy_engine_gui::BufferView;
use icy_net::{
    // modem::{ModemConfiguration, ModemConnection, Serial},
    protocol::{TransferProtocolType, TransferState},
    raw::RawConnection,
    // ssh::{Credentials, SSHConnection},
    telnet::{TelnetConnection, TerminalEmulation},
    Connection,
    NullConnection,
};
use std::{
    collections::VecDeque,
    mem,
    path::PathBuf,
    sync::{mpsc, Arc},
    thread,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use web_time::{Duration, Instant};

use super::{
    com_thread::ConnectionThreadData,
    connect::{OpenConnectionData, SendData},
    dialogs,
};
const BITS_PER_BYTE: u32 = 8;

pub struct TerminalThread {
    pub capture_dialog: dialogs::capture_dialog::DialogState,

    pub buffer_view: Arc<Mutex<BufferView>>,

    pub current_transfer: TransferState,

    pub auto_file_transfer: AutoFileTransfer,
    pub auto_login: Option<AutoLogin>,
    pub sound_thread: Arc<Mutex<SoundThread>>,

    pub auto_transfer: Option<(TransferProtocolType, bool)>,
    pub enabled: bool,

    pub terminal_type: Option<(TerminalEmulation, MusicOption)>,

    pub mouse_field: Vec<MouseField>,

    pub cache_directory: PathBuf,

    pub is_connected: bool,
    pub connection_time: Instant,
}

impl TerminalThread {
    pub async fn update_state(
        &mut self,
        ctx: &egui::Context,
        connection: &mut ConnectionThreadData,
        buffer_parser: &mut dyn BufferParser,
        data: &[u8],
    ) -> TerminalResult<(u64, usize)> {
        self.sound_thread.lock().update_state()?;
        let res = self.update_buffer(ctx, connection, buffer_parser, data).await;
        self.buffer_view.lock().get_buffer_mut().update_hyperlinks();
        Ok(res)
    }

    async fn update_buffer(
        &mut self,
        ctx: &egui::Context,
        connection: &mut ConnectionThreadData,
        buffer_parser: &mut dyn BufferParser,
        data: &[u8],
    ) -> (u64, usize) {
        let has_data = !data.is_empty();
        if !self.enabled {
            return (10, 0);
        }

        {
            let mut caret: Caret = Caret::default();
            mem::swap(&mut caret, self.buffer_view.lock().get_caret_mut());
            loop {
                let Some(act) = buffer_parser.get_next_action(self.buffer_view.lock().get_buffer_mut(), &mut caret, 0) else {
                    break;
                };
                let (p, ms) = self.handle_action(act, connection).await;
                if p {
                    self.buffer_view.lock().get_edit_state_mut().set_is_buffer_dirty();
                    ctx.request_repaint();
                    mem::swap(&mut caret, self.buffer_view.lock().get_caret_mut());

                    return (ms as u64, 0);
                }
            }
            mem::swap(&mut caret, self.buffer_view.lock().get_caret_mut());
        }

        let mut idx = 0;
        for ch in data {
            let ch = *ch;
            if let Some((protocol_type, download)) = self.auto_file_transfer.try_transfer(ch) {
                self.auto_transfer = Some((protocol_type, download));
            }
            if let Some(autologin) = &mut self.auto_login {
                if let Ok(Some(data)) = autologin.try_login(ch) {
                    connection.com.write_all(&data).await.unwrap();
                    autologin.logged_in = true;
                }
            }
            self.capture_dialog.append_data(ch);
            let (p, ms) = self.print_char(connection, buffer_parser, ch).await;
            idx += 1;

            if p {
                self.buffer_view.lock().get_edit_state_mut().set_is_buffer_dirty();
                ctx.request_repaint();
                return (ms as u64, idx);
            }
        }

        if has_data {
            (0, data.len())
        } else {
            (10, data.len())
        }
    }

    pub async fn print_char(&self, connection: &mut ConnectionThreadData, buffer_parser: &mut dyn BufferParser, c: u8) -> (bool, u32) {
        let mut caret: Caret = Caret::default();
        mem::swap(&mut caret, self.buffer_view.lock().get_caret_mut());
        let result = buffer_parser.print_char(self.buffer_view.lock().get_buffer_mut(), 0, &mut caret, c as char);
        mem::swap(&mut caret, self.buffer_view.lock().get_caret_mut());

        match result {
            Ok(action) => {
                return self.handle_action(action, connection).await;
            }

            Err(err) => {
                log::error!("print_char: {err}");
            }
        }
        (false, 0)
    }

    async fn handle_action(&self, result: icy_engine::CallbackAction, connection: &mut ConnectionThreadData) -> (bool, u32) {
        match result {
            icy_engine::CallbackAction::SendString(result) => {
                let r = connection.com.write_all(result.as_bytes()).await;
                if let Err(r) = r {
                    log::error!("callbackaction::SendString: {r}");
                }
                let _ = connection.com.flush();
            }
            icy_engine::CallbackAction::PlayMusic(music) => {
                let r = self.sound_thread.lock().play_music(music);
                if let Err(r) = r {
                    log::error!("callbackaction::PlayMusic: {r}");
                }
            }
            icy_engine::CallbackAction::Beep => {
                let r = self.sound_thread.lock().beep();
                if let Err(r) = r {
                    log::error!("callbackaction::Beep: {r}");
                }
            }
            icy_engine::CallbackAction::ChangeBaudEmulation(baud_emulation) => {
                connection.baud_rate = baud_emulation.get_baud_rate();
            }
            icy_engine::CallbackAction::ResizeTerminal(_, _) => {
                return (true, 0);
            }

            icy_engine::CallbackAction::NoUpdate => {
                return (false, 0);
            }

            icy_engine::CallbackAction::Update => {
                return (true, 0);
            }
            icy_engine::CallbackAction::Pause(ms) => {
                // note: doesn't block the UI thread
                return (true, ms);
            }
        }
        (false, 0)
    }
}

pub fn start_update_thread(
    ctx: &egui::Context,
    connection_data: OpenConnectionData,
    update_thread: Arc<Mutex<TerminalThread>>,
) -> (thread::JoinHandle<()>, mpsc::Sender<SendData>, mpsc::Receiver<SendData>) {
    let ctx = ctx.clone();
    let (tx, rx) = mpsc::channel::<SendData>();
    let (tx2, rx2) = mpsc::channel::<SendData>();

    (
        thread::spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let mut idx = 0;

                    let mut buffer_parser = get_parser(
                        &connection_data.term_caps.terminal,
                        connection_data.use_ansi_music,
                        update_thread.lock().cache_directory.clone(),
                    );
                    let com: Box<dyn Connection> = match open_connection(&connection_data).await {
                        Ok(com) => com,
                        Err(err) => {
                            let _ = tx.send(SendData::Disconnect);
                            println(&update_thread, &mut buffer_parser, &format!("\n{err}\n"));
                            update_thread.lock().is_connected = false;
                            return;
                        }
                    };
                    update_thread.lock().is_connected = true;
                    update_thread.lock().connection_time = Instant::now();

                    let mut connection = ConnectionThreadData {
                        is_connected: false,
                        com,
                        baud_rate: connection_data.baud_emulation.get_baud_rate(),
                        data_buffer: VecDeque::new(),
                        thread_is_running: true,
                        tx: tx,
                        last_send_time: Instant::now(),
                        rx: rx2,
                    };
                    let mut data = [0; 1024 * 64];

                    loop {
                        tokio::select! {

                            Ok(size) = connection.com.read(&mut data) => {
                                let mut idx = 0;
                                while idx < size {
                                    let update_state = update_thread.lock().update_state(&ctx, &mut connection, &mut *buffer_parser, &data[idx..]).await;
                                    match &update_state {
                                        Err(err) => {
                                            println(&update_thread, &mut buffer_parser, &format!("\n{err}\n"));
                                            log::error!("run_update_thread::update_state: {err}");
                                            idx = size;
                                        }
                                        Ok((sleep_ms, parsed_data)) => {
                                            let data = buffer_parser.get_picture_data();
                                            if data.is_some() {
                                                update_thread.lock().mouse_field = buffer_parser.get_mouse_fields();
                                                update_thread.lock().buffer_view.lock().set_reference_image(data);
                                            }
                                            if *sleep_ms > 0 {
                                                thread::sleep(Duration::from_millis(*sleep_ms));
                                            }
                                            idx += parsed_data;
                                        }
                                    }
                                }
                            }
                            else => {
                                if let Ok(data) = connection.rx.try_recv() {
                                    handle_receive(&mut connection, data, &update_thread).await;
                                }
                            }
                        };
                    }
                });
        }),
        tx2,
        rx,
    )
}

pub async fn read_data(connection: &mut ConnectionThreadData, output: &mut Vec<u8>) -> bool {
    if connection.data_buffer.is_empty() {
        let mut data = [0; 1024 * 64];
        match connection.com.read(&mut data).await {
            Ok(bytes) => {
                connection.data_buffer.extend(&data[0..bytes]);
            }
            Err(err) => {
                log::error!("connection_thread::read_data: {err}");
                return false;
            }
        }
    }

    if connection.baud_rate == 0 {
        output.extend(connection.data_buffer.drain(..));
    } else {
        let cur_time = Instant::now();
        let bytes_per_sec = connection.baud_rate / BITS_PER_BYTE;
        let elapsed_ms = cur_time.duration_since(connection.last_send_time).as_millis() as u32;
        let bytes_to_send: usize = ((bytes_per_sec.saturating_mul(elapsed_ms)) / 1000).min(connection.data_buffer.len() as u32) as usize;
        if bytes_to_send > 0 {
            output.extend(connection.data_buffer.drain(..bytes_to_send));
            connection.last_send_time = cur_time;
        }
    }
    true
}

fn println(update_thread: &Arc<Mutex<TerminalThread>>, buffer_parser: &mut Box<dyn BufferParser>, str: &str) {
    let ut = update_thread.lock();
    let mut bv = ut.buffer_view.lock();
    let mut caret: Caret = Caret::default();
    mem::swap(&mut caret, bv.get_caret_mut());
    let state = bv.get_edit_state_mut();
    let buffer = state.get_buffer_mut();
    for c in str.chars() {
        let _ = buffer_parser.print_char(buffer, 0, &mut caret, c);
    }
    mem::swap(&mut caret, bv.get_caret_mut());
}

async fn open_connection(connection_data: &OpenConnectionData) -> Res<Box<dyn Connection>> {
    if connection_data.term_caps.window_size.0 == 0 {
        return Ok(Box::new(NullConnection {}));
    }

    match connection_data.connection_type {
        icy_net::ConnectionType::Raw => Ok(Box::new(RawConnection::open(&connection_data.address, connection_data.timeout.clone()).await?)),
        icy_net::ConnectionType::Telnet => Ok(Box::new(
            TelnetConnection::open(&connection_data.address, connection_data.term_caps.clone(), connection_data.timeout.clone()).await?,
        )),
        /*
                icy_net::ConnectionType::SSH => Ok(Box::new(SSHConnection::open(
                    &connection_data.address,
                    connection_data.term_caps.clone(),
                    Credentials {
                        user_name: connection_data.user_name.clone(),
                        password: connection_data.password.clone(),
                        proxy_command: connection_data.proxy_command.clone(),
                    },
                ).await?)),
                icy_net::ConnectionType::Modem => {
                    let Some(m) = &connection_data.modem else {
                        return Err("Modem configuration is required for modem connections".into());
                    };
                    let serial = Serial {
                        device: m.device.clone(),
                        baud_rate: m.baud_rate,
                        char_size: m.char_size,
                        parity: m.parity,
                        stop_bits: m.stop_bits,
                        flow_control: m.flow_control,
                    };
                    let modem = ModemConfiguration {
                        init_string: m.init_string.clone(),
                        dial_string: m.dial_string.clone(),
                    };
                    Ok(Box::new(ModemConnection::open(serial, modem, connection_data.address.clone())?))
                }
                icy_net::ConnectionType::Websocket => Ok(Box::new(icy_net::websocket::connect(&connection_data.address, false).await?)),
                icy_net::ConnectionType::SecureWebsocket => Ok(Box::new(icy_net::websocket::connect(&connection_data.address, true).await?)),
        */
        _ => Ok(Box::new(NullConnection {})),
    }
}

async fn handle_receive(c: &mut ConnectionThreadData, data: SendData, update_thread: &Arc<Mutex<TerminalThread>>) -> Res<()> {
    match data {
        SendData::Data(buf) => {
            c.com.write_all(&buf).await?;
        }

        SendData::SetBaudRate(baud) => {
            c.baud_rate = baud;
        }

        SendData::Disconnect => {
            c.com.shutdown().await?;
        }

        SendData::Upload(protocol, files) => {
            if let Err(err) = upload(c, protocol, files, update_thread).await {
                log::error!("Failed to upload files: {err}");
            }
        }

        SendData::Download(protocol) => {
            if let Err(err) = download(c, protocol, update_thread).await {
                log::error!("Failed to download files: {err}");
            }
        }

        _ => {}
    }
    Ok(())
}

async fn download(c: &mut ConnectionThreadData, protocol: TransferProtocolType, update_thread: &Arc<Mutex<TerminalThread>>) -> Res<()> {
    let mut prot = protocol.create();
    let mut transfer_state = prot.initiate_recv(&mut *c.com).await?;
    while !transfer_state.is_finished {
        prot.update_transfer(&mut *c.com, &mut transfer_state).await?;
        update_thread.lock().current_transfer = transfer_state.clone();
        match c.rx.try_recv() {
            Ok(SendData::CancelTransfer) => {
                prot.cancel_transfer(&mut *c.com).await?;
                break;
            }
            _ => {}
        }
    }
    copy_downloaded_files(&mut transfer_state)?;
    update_thread.lock().current_transfer = transfer_state.clone();
    Ok(())
}

async fn upload(c: &mut ConnectionThreadData, protocol: TransferProtocolType, files: Vec<PathBuf>, update_thread: &Arc<Mutex<TerminalThread>>) -> Res<()> {
    let mut prot = protocol.create();
    let mut transfer_state = prot.initiate_send(&mut *c.com, &files).await?;
    while !transfer_state.is_finished {
        prot.update_transfer(&mut *c.com, &mut transfer_state).await?;
        update_thread.lock().current_transfer = transfer_state.clone();
        match c.rx.try_recv() {
            Ok(SendData::CancelTransfer) => {
                prot.cancel_transfer(&mut *c.com).await?;
                break;
            }
            _ => {}
        }
    }
    // needed for potential bi-directional protocols
    copy_downloaded_files(&mut transfer_state)?;
    update_thread.lock().current_transfer = transfer_state.clone();
    Ok(())
}

fn copy_downloaded_files(transfer_state: &mut TransferState) -> Res<()> {
    if let Some(dirs) = UserDirs::new() {
        if let Some(upload_location) = dirs.download_dir() {
            let mut lines = Vec::new();
            for (name, path) in &transfer_state.recieve_state.finished_files {
                let mut dest = upload_location.join(name);

                let mut i = 1;
                let new_name = PathBuf::from(name);
                while dest.exists() {
                    dest = dest.with_file_name(format!(
                        "{}.{}.{}",
                        new_name.file_stem().unwrap().to_string_lossy(),
                        i,
                        new_name.extension().unwrap().to_string_lossy()
                    ));
                    i += 1;
                }
                std::fs::copy(&path, &dest)?;
                std::fs::remove_file(&path)?;
                lines.push(format!("File copied to: {}", dest.display()));
            }
            for line in lines {
                transfer_state.recieve_state.log_info(line);
            }
        } else {
            log::error!("Failed to get user download directory");
        }
    } else {
        log::error!("Failed to get user directories");
    }

    Ok(())
}
