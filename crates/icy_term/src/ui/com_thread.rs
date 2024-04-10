#![allow(unsafe_code, clippy::wildcard_imports)]

use std::collections::VecDeque;
use std::sync::mpsc::{self};

#[cfg(target_arch = "wasm32")]
use wasm_thread as thread;
use web_time::Instant;

use crate::com::{Com, TermComResult};

use super::connect::{Connection, OpenConnectionData, SendData};
use super::MainWindow;

const BITS_PER_BYTE: u32 = 8;

pub struct ConnectionThreadData {
    tx: mpsc::Sender<SendData>,
    rx: mpsc::Receiver<SendData>,
    com: Box<dyn Com>,
    thread_is_running: bool,
    is_connected: bool,

    // used for baud rate emulation
    data_buffer: VecDeque<u8>,
    baud_rate: u32,
    last_send_time: Instant,
}

impl ConnectionThreadData {
    fn disconnect(&mut self) {
        self.is_connected = false;
        self.com = Box::new(crate::com::NullConnection {});
        self.baud_rate = 0;
        self.data_buffer.clear();
        self.thread_is_running &= self.tx.send(SendData::Disconnect).is_ok();
    }

    fn read_data(&mut self) -> bool {
        if self.data_buffer.is_empty() {
            match self.com.read_data() {
                Ok(Some(data)) => {
                    if self.baud_rate == 0 {
                        if let Err(err) = self.tx.send(SendData::Data(data)) {
                            log::error!("connection_thread::read_data1: {err}");
                            self.thread_is_running &= self.tx.send(SendData::Disconnect).is_ok();
                        }
                    } else {
                        self.data_buffer.extend(data);
                    }
                }
                Ok(None) => return false,

                Err(err) => {
                    log::error!("connection_thread::read_data2: {err}");
                    self.disconnect();
                    return false;
                }
            }
        } else if self.baud_rate == 0 {
            if let Err(err) = self.tx.send(SendData::Data(self.data_buffer.drain(..).collect())) {
                log::error!("connection_thread::read_data3: {err}");
                self.thread_is_running &= self.tx.send(SendData::Disconnect).is_ok();
                self.disconnect();
            }
        } else {
            let cur_time = Instant::now();
            let bytes_per_sec = self.baud_rate / BITS_PER_BYTE;
            let elapsed_ms = cur_time.duration_since(self.last_send_time).as_millis() as u32;
            let bytes_to_send: usize = ((bytes_per_sec.saturating_mul(elapsed_ms)) / 1000).min(self.data_buffer.len() as u32) as usize;

            if bytes_to_send > 0 {
                if let Err(err) = self.tx.send(SendData::Data(self.data_buffer.drain(..bytes_to_send).collect())) {
                    log::error!("Error while sending: {err}");
                    self.thread_is_running &= self.tx.send(SendData::Disconnect).is_ok();
                }
                self.last_send_time = cur_time;
            }
        }
        true
    }

    fn try_connect(&mut self, connection_data: &OpenConnectionData) -> TermComResult<()> {
        self.com = match connection_data.protocol {
            crate::addresses::Protocol::Telnet => Box::new(crate::com::ComTelnetImpl::connect(connection_data)?),
            crate::addresses::Protocol::Raw => Box::new(crate::com::ComRawImpl::connect(connection_data)?),
            crate::addresses::Protocol::Modem => Box::new(crate::com::ComModemImpl::connect(connection_data)?),
            #[cfg(not(target_arch = "wasm32"))]
            crate::addresses::Protocol::Ssh => Box::new(crate::com::ssh::SSHComImpl::connect(connection_data)?),
            crate::addresses::Protocol::WebSocket(_) => {
                #[cfg(target_arch = "wasm32")] //TODO
                panic!("WebSocket is not supported on web");

                Box::new(crate::com::websocket::WebSocketComImpl::connect(connection_data)?)
            }
            #[cfg(target_arch = "wasm32")]
            crate::addresses::Protocol::Ssh => Box::new(crate::com::NullConnection {}),
        };
        Ok(())
    }

    pub fn handle_receive(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(SendData::OpenConnection(connection_data)) => match self.try_connect(&connection_data) {
                    Ok(()) => {
                        self.thread_is_running &= self.tx.send(SendData::Connected).is_ok();
                        self.is_connected = true;
                    }
                    Err(err) => {
                        self.thread_is_running &= self.tx.send(SendData::ConnectionError(err.to_string())).is_ok();
                        self.disconnect();
                    }
                },
                Ok(SendData::Data(buf)) => {
                    if let Err(err) = self.com.send(&buf) {
                        log::error!("connection_thread::handle_receive: {err}");
                        let _ = self.tx.send(SendData::Disconnect);
                        self.disconnect();
                    }
                }

                Ok(SendData::SetBaudRate(baud)) => {
                    self.baud_rate = baud;
                }

                Ok(SendData::SetRawMode(raw_transfer)) => {
                    self.com.set_raw_mode(raw_transfer);
                }
                Ok(SendData::Disconnect) => {
                    self.disconnect();
                }
                Ok(_) => {}
                Err(mpsc::TryRecvError::Empty) => break,
                Err(err) => {
                    log::error!("Error while receiving: {err}");
                    break;
                }
            }
        }
    }

    fn new(tx: mpsc::Sender<SendData>, rx: mpsc::Receiver<SendData>) -> Self {
        Self {
            tx,
            rx,
            baud_rate: 0,
            com: Box::new(crate::com::NullConnection {}),
            data_buffer: VecDeque::<u8>::new(),
            last_send_time: Instant::now(),
            thread_is_running: true,
            is_connected: false,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn poll(&mut self) {
        if self.is_connected {
            self.read_data();
        }
        self.handle_receive();
    }
}

impl MainWindow {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn start_com_thread() -> Connection {
        use web_time::Duration;

        let (tx, rx) = mpsc::channel::<SendData>();
        let (tx2, rx2) = mpsc::channel::<SendData>();
        if let Err(err) = std::thread::Builder::new().name("com_thread".to_string()).spawn(move || {
            let mut data: ConnectionThreadData = ConnectionThreadData::new(tx, rx2);
            while data.thread_is_running {
                if data.is_connected {
                    if !data.read_data() {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                } else {
                    std::thread::sleep(Duration::from_millis(100));
                }
                data.handle_receive();
            }
            log::error!("communication thread closed because it lost connection with the ui thread.");
        }) {
            log::error!("error in communication thread: {}", err);
        }
        Connection::new(rx, tx2)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn start_poll_thead() -> (Connection, ConnectionThreadData) {
        let (tx, rx) = mpsc::channel::<SendData>();
        let (tx2, rx2) = mpsc::channel::<SendData>();
        (Connection::new(rx, tx2), ConnectionThreadData::new(tx, rx2))
    }
}
