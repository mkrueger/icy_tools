#![allow(unsafe_code, clippy::wildcard_imports)]

use std::collections::VecDeque;

use icy_net::Connection;
use tokio::sync::mpsc;
#[cfg(target_arch = "wasm32")]
use wasm_thread as thread;
use web_time::Instant;

use super::connect::SendData;

pub struct ConnectionThreadData {
    pub _tx: mpsc::Sender<SendData>,
    pub rx: mpsc::Receiver<SendData>,
    pub com: Box<dyn Connection>,
    pub _thread_is_running: bool,
    pub _is_connected: bool,

    // used for baud rate emulation
    pub _data_buffer: VecDeque<u8>,
    pub baud_rate: u32,
    pub _last_send_time: Instant,
}
