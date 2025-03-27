use crate::{
    Res,
    features::{AutoFileTransfer, AutoLogin},
    get_parser,
    util::SoundThread,
};
use directories::UserDirs;
use egui::mutex::Mutex;
use icy_engine::{BufferParser, Caret, ansi::MusicOption, rip::bgi::MouseField};
use icy_engine_gui::BufferView;
use icy_net::{
    Connection,
    NullConnection,
    // modem::{ModemConfiguration, ModemConnection, Serial},
    modem::{ModemConfiguration, ModemConnection},
    protocol::{Protocol, TransferProtocolType, TransferState},
    raw::RawConnection,
    serial::Serial,
    ssh::{Credentials, SSHConnection},
    telnet::{TelnetConnection, TerminalEmulation},
};
use std::{collections::VecDeque, mem, path::PathBuf, sync::Arc, thread};
use tokio::sync::mpsc;
use web_time::{Duration, Instant};

use super::{
    com_thread::ConnectionThreadData,
    connect::{OpenConnectionData, SendData},
    dialogs,
};
pub struct TerminalThread {
    pub capture_dialog: dialogs::capture_dialog::DialogState,

    pub buffer_view: Arc<Mutex<BufferView>>,

    pub current_transfer: TransferState,

    pub auto_file_transfer: AutoFileTransfer,
    pub auto_login: Option<AutoLogin>,
    pub sound_thread: Arc<Mutex<SoundThread>>,

    pub auto_transfer: Option<(TransferProtocolType, bool, Option<String>)>,

    pub terminal_type: Option<(TerminalEmulation, MusicOption)>,

    pub mouse_field: Vec<MouseField>,

    pub cache_directory: PathBuf,

    pub is_connected: bool,
    pub connection_time: Instant,
}
const BITS_PER_BYTE: u32 = 8;

impl TerminalThread {
    pub async fn update_state(&mut self, connection: &mut ConnectionThreadData, buffer_parser: &mut dyn BufferParser, data: &[u8]) -> Res<()> {
        self.sound_thread.lock().update_state()?;
        self.update_buffer(connection, buffer_parser, data).await
    }

    async fn update_buffer(&mut self, connection: &mut ConnectionThreadData, buffer_parser: &mut dyn BufferParser, data: &[u8]) -> Res<()> {
        let mut caret: Caret = Caret::default();
        mem::swap(&mut caret, self.buffer_view.lock().get_caret_mut());
        self.capture_dialog.append_data(&data);

        for ch in data {
            let ch = *ch;
            if let Some((protocol_type, download)) = self.auto_file_transfer.try_transfer(ch) {
                self.auto_transfer = Some((protocol_type, download, None));
            }
            if let Some(autologin) = &mut self.auto_login {
                if let Ok(Some(data)) = autologin.try_login(ch) {
                    connection.com.send(&data).await?;
                    autologin.logged_in = true;
                }
            }
            let result = buffer_parser.print_char(self.buffer_view.lock().get_buffer_mut(), 0, &mut caret, ch as char);
            match result {
                Ok(action) => {
                    self.handle_action(action, connection).await;
                }
                Err(err) => {
                    log::error!("print_char: {err}");
                }
            }
        }
        mem::swap(&mut caret, self.buffer_view.lock().get_caret_mut());
        self.buffer_view.lock().get_buffer_mut().update_hyperlinks();
        self.buffer_view.lock().redraw_view();
        Ok(())
    }

    async fn handle_action(&mut self, result: icy_engine::CallbackAction, connection: &mut ConnectionThreadData) -> (bool, u32) {
        match result {
            icy_engine::CallbackAction::SendString(result) => {
                let r = connection.com.send(result.as_bytes()).await;
                if let Err(r) = r {
                    log::error!("callbackaction::SendString: {r}");
                }
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

            icy_engine::CallbackAction::RunSkypixSequence(seq) => {
                log::error!("unsupported skypix sequence: {:?}", seq);
                return (false, 0);
            }

            icy_engine::CallbackAction::PlayGISTSound(_) | icy_engine::CallbackAction::NoUpdate => {
                return (false, 0);
            }

            icy_engine::CallbackAction::ScrollDown(_) => {
                return (true, 0);
            }

            icy_engine::CallbackAction::Update => {
                return (true, 0);
            }
            icy_engine::CallbackAction::Pause(ms) => {
                // note: doesn't block the UI thread
                return (true, ms);
            }
            icy_engine::CallbackAction::XModemTransfer(file_name) => {
                self.auto_transfer = Some((TransferProtocolType::XModem, true, Some(file_name)));
                // note: doesn't block the UI thread
                return (false, 0);
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
    let (tx, rx) = mpsc::channel(32);
    let (tx2, rx2) = mpsc::channel(32);
    (
        thread::spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let mut buffer_parser: Box<dyn BufferParser> = get_parser(
                        &connection_data.term_caps.terminal,
                        connection_data.use_ansi_music,
                        connection_data.screen_mode,
                        update_thread.lock().cache_directory.clone(),
                    );
                    let com: Box<dyn Connection> = match open_connection(&connection_data).await {
                        Ok(com) => com,
                        Err(err) => {
                            update_thread.lock().is_connected = false;
                            let _ = tx.send(SendData::Disconnect);
                            log::error!("run_update_thread::open_connection: {err}");
                            println(&update_thread, &mut buffer_parser, &format!("\n{err}\n"));
                            return;
                        }
                    };
                    update_thread.lock().is_connected = true;
                    update_thread.lock().connection_time = Instant::now();

                    let mut connection = ConnectionThreadData {
                        _is_connected: false,
                        com,
                        baud_rate: connection_data.baud_emulation.get_baud_rate(),
                        _data_buffer: VecDeque::new(),
                        _thread_is_running: true,
                        _tx: tx,
                        last_send_time: Instant::now(),
                        rx: rx2,
                    };
                    let mut data = [0; 1024 * 64];
                    loop {
                        tokio::select! {
                            read_data = connection.com.read(&mut data) => {
                                match read_data {
                                    Err(err) => {
                                        update_thread.lock().is_connected = false;
                                        println(&update_thread, &mut buffer_parser, &format!("\n{err}\n"));
                                        log::error!("run_update_thread::read_data: {err}");
                                        break;
                                    }
                                    Ok(size) => {
                                        if size > 0 {
                                            let mut cur = 0;
                                            while cur < size {
                                                let next = if connection.baud_rate != 0 {
                                                    let cur_time = Instant::now();
                                                    let bytes_per_sec = connection.baud_rate / BITS_PER_BYTE;
                                                    let elapsed_ms = cur_time.duration_since(connection.last_send_time).as_millis() as u32;
                                                    let bytes_to_send = (bytes_per_sec.saturating_mul(elapsed_ms)) / 1000;
                                                    if bytes_to_send > 0 {
                                                        connection.last_send_time = cur_time;
                                                    }
                                                    (cur + bytes_to_send as usize).min(size)
                                                } else {
                                                    size
                                                };
                                                if next > cur {
                                                    let update_state = update_thread.lock().update_state(&mut connection, &mut *buffer_parser, &data[cur..next]).await;
                                                    cur = next;
                                                    match &update_state {
                                                        Err(err) => {
                                                            println(&update_thread, &mut buffer_parser, &format!("\n{err}\n"));
                                                            log::error!("run_update_thread::update_state: {err}");
                                                        }
                                                        Ok(()) => {
                                                            let data = buffer_parser.get_picture_data();
                                                            if data.is_some() {
                                                                update_thread.lock().mouse_field = buffer_parser.get_mouse_fields();
                                                                update_thread.lock().buffer_view.lock().set_reference_image(data);
                                                            }
                                                        }
                                                    }
                                                    ctx.request_repaint();
                                                } else {
                                                    thread::sleep(std::time::Duration::from_millis(1));
                                                }
                                            }
                                        } else {
                                            thread::sleep(std::time::Duration::from_millis(20));
                                        }
                                    }
                                }
                            }
                            Some(data) = connection.rx.recv() => {
                                let _ = handle_receive(&ctx, &mut connection, data, &update_thread).await;
                            }
                        };
                    }
                });
        }),
        tx2,
        rx,
    )
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
        icy_net::ConnectionType::SSH => Ok(Box::new(
            SSHConnection::open(
                &connection_data.address,
                connection_data.term_caps.clone(),
                Credentials {
                    user_name: connection_data.user_name.clone(),
                    password: connection_data.password.clone(),
                    proxy_command: connection_data.proxy_command.clone(),
                },
            )
            .await?,
        )),
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
            Ok(Box::new(ModemConnection::open(serial, modem, connection_data.address.clone()).await?))
        }
        icy_net::ConnectionType::Websocket => Ok(Box::new(icy_net::websocket::connect(&connection_data.address, false).await?)),
        icy_net::ConnectionType::SecureWebsocket => Ok(Box::new(icy_net::websocket::connect(&connection_data.address, true).await?)),

        _ => panic!("Unsupported connection type"),
    }
}

async fn handle_receive(ctx: &egui::Context, c: &mut ConnectionThreadData, data: SendData, update_thread: &Arc<Mutex<TerminalThread>>) -> Res<()> {
    match data {
        SendData::Data(buf) => {
            c.com.send(&buf).await?;
        }

        SendData::SetBaudRate(baud) => {
            c.baud_rate = baud;
        }

        SendData::Disconnect => {
            c.com.shutdown().await?;
        }

        SendData::Upload(protocol, files) => {
            if let Err(err) = upload(ctx, c, protocol, files, update_thread).await {
                log::error!("Failed to upload files: {err}");
            }
        }

        SendData::Download(protocol, file_name) => {
            if let Err(err) = download(ctx, c, protocol, update_thread, file_name).await {
                log::error!("Failed to download files: {err}");
            }
        }

        _ => {}
    }
    Ok(())
}

async fn download(
    ctx: &egui::Context,
    c: &mut ConnectionThreadData,
    protocol: TransferProtocolType,
    update_thread: &Arc<Mutex<TerminalThread>>,
    file_name: Option<String>,
) -> Res<()> {
    let mut prot = protocol.create();
    let mut transfer_state = prot.initiate_recv(&mut *c.com).await?;
    if let Some(file_name) = file_name {
        transfer_state.recieve_state.file_name = file_name.clone();
    }
    file_transfer(ctx, transfer_state, &mut *prot, c, update_thread).await?;
    Ok(())
}

async fn upload(
    ctx: &egui::Context,
    c: &mut ConnectionThreadData,
    protocol: TransferProtocolType,
    files: Vec<PathBuf>,
    update_thread: &Arc<Mutex<TerminalThread>>,
) -> Res<()> {
    let mut prot = protocol.create();
    let transfer_state = prot.initiate_send(&mut *c.com, &files).await?;
    file_transfer(ctx, transfer_state, &mut *prot, c, update_thread).await?;
    Ok(())
}

async fn file_transfer(
    ctx: &egui::Context,
    mut transfer_state: TransferState,
    prot: &mut dyn Protocol,
    c: &mut ConnectionThreadData,
    update_thread: &Arc<Mutex<TerminalThread>>,
) -> Res<()> {
    ctx.request_repaint();
    let instant = Instant::now();
    while !transfer_state.is_finished {
        tokio::select! {
            data = c.rx.recv() => {
                match data {
                    Some(SendData::CancelTransfer) => {
                        transfer_state.is_finished = true;
                        prot.cancel_transfer(&mut *c.com).await?;
                        break;
                    }
                    _ => {}
                }
            }
            Ok(()) = prot.update_transfer(&mut *c.com, &mut transfer_state) => {
                if instant.elapsed() > Duration::from_millis(500) {
                    update_thread.lock().current_transfer = transfer_state.clone();
                    ctx.request_repaint();
                }
            }
        };
    }
    copy_downloaded_files(&mut transfer_state)?;
    update_thread.lock().current_transfer = transfer_state.clone();
    ctx.request_repaint();
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
                    if let Some(stem) = new_name.file_stem() {
                        if let Some(ext) = new_name.extension() {
                            dest = dest.with_file_name(format!("{}.{}.{}", stem.to_string_lossy(), i, ext.to_string_lossy()));
                        } else {
                            dest = dest.with_file_name(format!("{}.{}", stem.to_string_lossy(), i));
                        }
                    }
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
