#![allow(unsafe_code, clippy::wildcard_imports)]

use std::collections::VecDeque;
use std::sync::mpsc::{self};

use icy_net::Connection;
#[cfg(target_arch = "wasm32")]
use wasm_thread as thread;
use web_time::Instant;

use super::connect::SendData;

pub struct ConnectionThreadData {
    pub tx: mpsc::Sender<SendData>,
    pub rx: mpsc::Receiver<SendData>,
    pub com: Box<dyn Connection>,
    pub thread_is_running: bool,
    pub is_connected: bool,

    // used for baud rate emulation
    pub data_buffer: VecDeque<u8>,
    pub baud_rate: u32,
    pub last_send_time: Instant,
}
