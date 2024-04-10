use crate::{Address, Modem, Terminal, TerminalResult};
use std::{collections::VecDeque, sync::mpsc};
use web_time::{Duration, Instant};

pub trait DataConnection {
    fn is_data_available(&mut self) -> TerminalResult<bool>;
    fn read_buffer(&mut self) -> Vec<u8>;
    fn read_u8(&mut self) -> TerminalResult<u8>;
    fn read_exact(&mut self, size: usize) -> TerminalResult<Vec<u8>>;
    fn send(&mut self, vec: Vec<u8>) -> TerminalResult<()>;
}

/// Connection is used for the ui and com thread to communicate.
#[derive(Debug)]
pub struct Connection {
    time: Instant,
    is_connected: bool,
    pub rx: mpsc::Receiver<SendData>,
    pub tx: mpsc::Sender<SendData>,
    end_transfer: bool,
    buf: std::collections::VecDeque<u8>,
}

impl DataConnection for Connection {
    fn is_data_available(&mut self) -> TerminalResult<bool> {
        if let Err(err) = self.fill_buffer() {
            log::error!("Error in is_data_available: {err}");
            self.is_connected = false;
            return Err(err);
        }
        Ok(!self.buf.is_empty())
    }

    fn read_buffer(&mut self) -> Vec<u8> {
        self.buf.drain(0..self.buf.len()).collect()
    }

    fn read_u8(&mut self) -> TerminalResult<u8> {
        while !self.is_data_available()? {
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(self.buf.pop_front().unwrap())
    }

    fn read_exact(&mut self, size: usize) -> TerminalResult<Vec<u8>> {
        while self.buf.len() < size {
            self.fill_buffer()?;
            std::thread::sleep(Duration::from_millis(10));
        }

        Ok(self.buf.drain(0..size).collect())
    }

    fn send(&mut self, vec: Vec<u8>) -> TerminalResult<()> {
        if let Err(err) = self.tx.send(SendData::Data(vec)) {
            log::error!("Error sending data: {err}");
            self.is_connected = false;
            self.disconnect()?;
        }
        Ok(())
    }
}

impl Connection {
    pub fn new(rx: mpsc::Receiver<SendData>, tx: mpsc::Sender<SendData>) -> Self {
        Self {
            time: Instant::now(),
            is_connected: false,
            end_transfer: false,
            rx,
            tx,
            buf: VecDeque::new(),
        }
    }

    pub fn should_end_transfer(&mut self) -> bool {
        self.fill_buffer().unwrap_or_default();
        self.end_transfer
    }

    pub fn get_connection_time(&self) -> Instant {
        self.time
    }

    pub fn update_state(&mut self) -> TerminalResult<()> {
        self.fill_buffer()
    }

    pub fn start_transfer(&mut self) {
        self.end_transfer = false;
    }

    pub fn set_raw_mode(&self, raw_mode: bool) -> TerminalResult<()> {
        self.tx.send(SendData::SetRawMode(raw_mode))?;
        Ok(())
    }

    fn fill_buffer(&mut self) -> TerminalResult<()> {
        loop {
            match self.rx.try_recv() {
                Ok(data) => match data {
                    SendData::Data(v) => {
                        self.buf.extend(v);
                    }
                    SendData::Disconnect => {
                        self.is_connected = false;
                        break;
                    }
                    SendData::EndTransfer => {
                        self.end_transfer = true;
                        break;
                    }
                    SendData::Connected => {
                        self.is_connected = true;
                        break;
                    }
                    SendData::ConnectionError(err) => {
                        log::error!("Connection aborted while fill_buffer: {err}");
                        self.is_connected = false;
                        self.end_transfer = true;
                        return Err(anyhow::anyhow!("{err}"));
                    }
                    _ => {
                        return Err(anyhow::anyhow!("Unsupported send data: {data:?}"));
                    }
                },

                Err(err) => match err {
                    mpsc::TryRecvError::Empty => break,
                    mpsc::TryRecvError::Disconnected => {
                        if !self.is_connected {
                            break;
                        }
                        self.is_connected = false;
                        return Err(anyhow::anyhow!("disconnected: {err}"));
                    }
                },
            }
        }
        Ok(())
    }

    pub fn disconnect(&self) -> TerminalResult<()> {
        self.tx.send(SendData::Disconnect)?;
        Ok(())
    }

    pub fn cancel_transfer(&self) -> TerminalResult<()> {
        self.tx.send(SendData::CancelTransfer)?;
        Ok(())
    }

    pub fn is_disconnected(&self) -> bool {
        !self.is_connected
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    pub fn set_baud_rate(&self, baud_rate: u32) -> TerminalResult<()> {
        self.tx.send(SendData::SetBaudRate(baud_rate))?;
        Ok(())
    }

    pub fn connect(&self, call_adr: &Address, timeout: Duration, window_size: icy_engine::Size, modem: Option<Modem>) -> TerminalResult<()> {
        self.tx
            .send(SendData::OpenConnection(OpenConnectionData::from(call_adr, timeout, window_size, modem)))?;
        Ok(())
    }
}

/// A more lightweight version of `Address` that is used for the connection
///Using Addreess in `SendData` makes just the enum larger without adding any value.
#[derive(Debug, Clone)]
pub struct OpenConnectionData {
    pub address: String,
    pub terminal: Terminal,
    pub user_name: String,
    pub password: String,
    pub protocol: crate::Protocol,
    pub timeout: Duration,
    pub window_size: icy_engine::Size,
    pub modem: Option<Modem>,
}

impl OpenConnectionData {
    pub fn from(call_adr: &Address, timeout: Duration, window_size: icy_engine::Size, modem: Option<Modem>) -> Self {
        Self {
            address: call_adr.address.clone(),
            user_name: call_adr.user_name.clone(),
            password: call_adr.password.clone(),
            terminal: call_adr.terminal_type,
            protocol: call_adr.protocol,
            timeout,
            window_size,
            modem,
        }
    }
}

/// Data that is sent to the connection thread
#[derive(Debug)]
pub enum SendData {
    OpenConnection(OpenConnectionData),
    ConnectionError(String),
    Connected,

    Data(Vec<u8>),
    Disconnect,
    EndTransfer,
    CancelTransfer,
    SetBaudRate(u32),
    SetRawMode(bool),
}

#[cfg(test)]
pub struct TestConnection {
    pub is_sender: bool,
    send_buffer: std::collections::VecDeque<u8>,
    recv_buffer: std::collections::VecDeque<u8>,
}

#[cfg(test)]
impl TestConnection {
    pub fn new(is_sender: bool) -> Self {
        Self {
            is_sender,
            send_buffer: VecDeque::new(),
            recv_buffer: VecDeque::new(),
        }
    }

    fn get_send_buffer(&mut self) -> &mut VecDeque<u8> {
        if self.is_sender {
            &mut self.send_buffer
        } else {
            &mut self.recv_buffer
        }
    }

    fn get_recv_buffer(&mut self) -> &mut VecDeque<u8> {
        if self.is_sender {
            &mut self.recv_buffer
        } else {
            &mut self.send_buffer
        }
    }

    pub fn read_receive_buffer(&self) -> Vec<u8> {
        self.send_buffer.clone().into()
    }
}

#[cfg(test)]
impl DataConnection for TestConnection {
    fn is_data_available(&mut self) -> TerminalResult<bool> {
        Ok(!self.get_recv_buffer().is_empty())
    }

    fn read_buffer(&mut self) -> Vec<u8> {
        let len = self.get_recv_buffer().len();
        self.get_recv_buffer().drain(0..len).collect()
    }

    fn read_u8(&mut self) -> TerminalResult<u8> {
        Ok(self.get_recv_buffer().pop_front().unwrap())
    }

    fn read_exact(&mut self, size: usize) -> TerminalResult<Vec<u8>> {
        Ok(self.get_recv_buffer().drain(..size).collect())
    }

    fn send(&mut self, vec: Vec<u8>) -> TerminalResult<()> {
        self.get_send_buffer().extend(vec);
        Ok(())
    }
}
