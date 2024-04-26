use crate::{
    features::{AutoFileTransfer, AutoLogin},
    get_parser, modem,
    util::SoundThread,
    Res, TerminalResult,
};
use egui::mutex::Mutex;
use icy_engine::{
    ansi::{self, MusicOption},
    rip::bgi::MouseField,
    BufferParser, Caret,
};
use icy_engine_gui::BufferView;
use icy_net::{
    modem::{ModemConfiguration, ModemConnection, Serial},
    protocol::TransferProtocolType,
    raw::RawConnection,
    ssh::{Credentials, SSHConnection},
    telnet::{TelnetConnection, TerminalEmulation},
    Connection, NullConnection,
};
use std::{
    collections::VecDeque,
    mem,
    path::PathBuf,
    sync::{mpsc, Arc},
    thread,
};
use web_time::{Duration, Instant};

use super::{
    com_thread::ConnectionThreadData,
    connect::{OpenConnectionData, SendData},
    dialogs,
};
const BITS_PER_BYTE: u32 = 8;

pub struct BufferUpdateThread {
    pub capture_dialog: dialogs::capture_dialog::DialogState,

    pub buffer_view: Arc<Mutex<BufferView>>,

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

impl BufferUpdateThread {
    pub fn update_state(
        &mut self,
        ctx: &egui::Context,
        connection: &mut ConnectionThreadData,
        buffer_parser: &mut dyn BufferParser,
        data: &[u8],
    ) -> TerminalResult<(u64, usize)> {
        self.sound_thread.lock().update_state()?;
        Ok(self.update_buffer(ctx, connection, buffer_parser, data))
    }

    fn update_buffer(&mut self, ctx: &egui::Context, connection: &mut ConnectionThreadData, buffer_parser: &mut dyn BufferParser, data: &[u8]) -> (u64, usize) {
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
                let (p, ms) = self.handle_action(act, connection, &mut self.buffer_view.lock());
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

            self.capture_dialog.append_data(ch);
            let (p, ms) = self.print_char(connection, &mut self.buffer_view.lock(), buffer_parser, ch);
            idx += 1;

            if p {
                self.buffer_view.lock().get_edit_state_mut().set_is_buffer_dirty();
                ctx.request_repaint();
                return (ms as u64, idx);
            }
            if let Some((protocol_type, download)) = self.auto_file_transfer.try_transfer(ch) {
                self.auto_transfer = Some((protocol_type, download));
            }
        }

        if has_data {
            self.buffer_view.lock().get_buffer_mut().update_hyperlinks();
            (0, data.len())
        } else {
            (10, data.len())
        }
    }

    pub fn print_char(&self, connection: &mut ConnectionThreadData, buffer_view: &mut BufferView, buffer_parser: &mut dyn BufferParser, c: u8) -> (bool, u32) {
        let mut caret: Caret = Caret::default();
        mem::swap(&mut caret, buffer_view.get_caret_mut());
        let buffer = buffer_view.get_buffer_mut();
        let result = buffer_parser.print_char(buffer, 0, &mut caret, c as char);
        mem::swap(&mut caret, buffer_view.get_caret_mut());

        match result {
            Ok(action) => {
                return self.handle_action(action, connection, buffer_view);
            }

            Err(err) => {
                log::error!("print_char: {err}");
            }
        }
        (false, 0)
    }

    fn handle_action(&self, result: icy_engine::CallbackAction, connection: &mut ConnectionThreadData, buffer_view: &mut BufferView) -> (bool, u32) {
        match result {
            icy_engine::CallbackAction::SendString(result) => {
                let r = connection.com.write_all(result.as_bytes());
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
                buffer_view.redraw_view();
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

    pub fn read_data(&mut self, connection: &mut ConnectionThreadData, output: &mut Vec<u8>) -> bool {
        if connection.data_buffer.is_empty() {
            let mut data = [0; 1024 * 64];
            match connection.com.read(&mut data) {
                Ok(bytes) => {
                    connection.data_buffer.extend(&data[0..bytes]);
                }
                Err(err) => {
                    log::error!("connection_thread::read_data2: {err}");
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
}

pub fn start_update_thread(
    ctx: &egui::Context,
    connection_data: OpenConnectionData,
    update_thread: Arc<Mutex<BufferUpdateThread>>,
) -> (thread::JoinHandle<()>, mpsc::Sender<SendData>, mpsc::Receiver<SendData>) {
    let ctx = ctx.clone();
    let (tx, rx) = mpsc::channel::<SendData>();
    let (tx2, rx2) = mpsc::channel::<SendData>();

    (
        thread::spawn(move || {
            let mut data = Vec::new();
            let mut idx = 0;

            let mut buffer_parser = get_parser(
                &connection_data.term_caps.terminal,
                connection_data.use_ansi_music,
                update_thread.lock().cache_directory.clone(),
            );
            let com: Box<dyn Connection> = match open_connection(&connection_data) {
                Ok(com) => com,
                Err(err) => {
                    let _ = tx.send(SendData::Disconnect);

                    let _ = update_thread.lock().buffer_view.lock().print_char('\n');
                    for c in format!("{err}").chars() {
                        let _ = update_thread.lock().buffer_view.lock().print_char(c);
                    }
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
            loop {
                if idx >= data.len() {
                    data.clear();
                    let lock = &mut update_thread.lock();
                    if !lock.read_data(&mut connection, &mut data) {
                        break;
                    }
                    idx = 0;
                }
                if idx < data.len() {
                    {
                        let lock = &mut update_thread.lock();
                    }
                    let update_state = update_thread.lock().update_state(&ctx, &mut connection, &mut *buffer_parser, &data[idx..]);
                    match update_state {
                        Err(err) => {
                            log::error!("run_update_thread::update_state: {err}");
                            idx = data.len();
                        }
                        Ok((sleep_ms, parsed_data)) => {
                            let data = buffer_parser.get_picture_data();
                            if data.is_some() {
                                update_thread.lock().mouse_field = buffer_parser.get_mouse_fields();
                                update_thread.lock().buffer_view.lock().set_reference_image(data);
                            }
                            if sleep_ms > 0 {
                                thread::sleep(Duration::from_millis(sleep_ms));
                            }
                            idx += parsed_data;
                        }
                    }
                } else {
                    data.clear();
                    thread::sleep(Duration::from_millis(10));
                }

                if let Err(err) = handle_receive(&mut connection) {
                    log::error!("run_update_thread::handle_receive: {err}");
                }
            }
        }),
        tx2,
        rx,
    )
}

fn open_connection(connection_data: &OpenConnectionData) -> Res<Box<dyn Connection>> {
    if connection_data.term_caps.window_size.0 == 0 {
        return Ok(Box::new(NullConnection {}));
    }

    match connection_data.connection_type {
        icy_net::ConnectionType::Raw => Ok(Box::new(RawConnection::open(&connection_data.address, connection_data.timeout.clone())?)),
        icy_net::ConnectionType::Telnet => Ok(Box::new(TelnetConnection::open(
            &connection_data.address,
            connection_data.term_caps.clone(),
            connection_data.timeout.clone(),
        )?)),
        icy_net::ConnectionType::SSH => Ok(Box::new(SSHConnection::open(
            &connection_data.address,
            connection_data.term_caps.clone(),
            Credentials {
                user_name: connection_data.user_name.clone(),
                password: connection_data.password.clone(),
            },
        )?)),
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
        icy_net::ConnectionType::Websocket => Ok(Box::new(icy_net::websocket::connect(&connection_data.address, false)?)),
        icy_net::ConnectionType::SecureWebsocket => Ok(Box::new(icy_net::websocket::connect(&connection_data.address, true)?)),

        _ => Ok(Box::new(NullConnection {})),
    }
}

fn handle_receive(c: &mut ConnectionThreadData) -> Res<()> {
    match c.rx.try_recv() {
        Ok(SendData::Data(buf)) => {
            c.com.write_all(&buf)?;
        }

        Ok(SendData::SetBaudRate(baud)) => {
            c.baud_rate = baud;
        }

        Ok(SendData::Disconnect) => {
            c.com.shutdown()?;
        }
        _ => {}
    }
    Ok(())
}
