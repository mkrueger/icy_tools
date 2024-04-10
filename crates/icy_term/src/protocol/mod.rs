#![allow(dead_code)]

use crate::ui::connect::DataConnection;
use crate::TerminalResult;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub mod file_storage_handler;
pub use file_storage_handler::*;

pub mod xymodem;
pub use xymodem::*;

pub mod zmodem;
pub use zmodem::*;

#[derive(Debug, Clone)]
pub struct FileDescriptor {
    pub path_name: String,
    pub file_name: String,
    pub size: usize,
    pub date: u64,
    path: PathBuf,
    data: Option<Vec<u8>>,
}

impl FileDescriptor {
    pub fn from_paths(paths: &Vec<PathBuf>) -> TerminalResult<Vec<FileDescriptor>> {
        let mut res = Vec::new();
        for p in paths {
            let fd = FileDescriptor::create(p)?;
            res.push(fd);
        }
        Ok(res)
    }

    pub fn create(path: &PathBuf) -> TerminalResult<Self> {
        let data = fs::metadata(path)?;
        let size = usize::try_from(data.len()).unwrap();
        let date_duration = Duration::from_secs(1); //data.modified()?.duration_since(crate::START_TIME).unwrap();

        Ok(Self {
            path_name: path.to_str().unwrap().to_string(),
            file_name: path.file_name().unwrap().to_str().unwrap().to_string(),
            path: path.clone(),
            size,
            date: date_duration.as_secs(),
            data: None,
        })
    }

    #[cfg(test)]
    pub fn create_test(file_name: String, data: Vec<u8>) -> Self {
        Self {
            path_name: String::new(),
            file_name,
            path: PathBuf::new(),
            size: data.len(),
            date: 0,
            data: Some(data),
        }
    }

    pub fn get_data(&self) -> std::vec::Vec<u8> {
        if let Some(data) = &self.data {
            data.clone()
        } else {
            let res = std::fs::read(&self.path);

            match res {
                Ok(res) => res,
                Err(err) => {
                    log::error!("Error reading file: {err}");
                    Vec::new()
                }
            }
        }
    }
}

impl Default for FileDescriptor {
    fn default() -> Self {
        Self {
            path_name: String::new(),
            file_name: String::new(),
            size: 0,
            date: 0,
            path: PathBuf::new(),
            data: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum OutputLogMessage {
    Info(String),
    Warning(String),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct TransferInformation {
    pub file_name: String,
    pub file_size: usize,
    pub bytes_transfered: usize,

    errors: usize,
    warnings: usize,
    pub files_finished: Vec<String>,
    pub check_size: String,
    time: Instant,
    bytes_transferred_timed: usize,
    pub bps: u64,

    output_log: Vec<OutputLogMessage>,
}

impl TransferInformation {
    pub fn update_bps(&mut self) {
        let bytes = self.bytes_transfered.saturating_sub(self.bytes_transferred_timed);
        let length = Instant::now().duration_since(self.time);

        if length > Duration::from_secs(10) {
            self.bytes_transferred_timed = self.bytes_transfered;
        }

        let length = length.as_secs();
        if length > 0 {
            self.bps = self.bps / 2 + bytes as u64 / length;
        }

        let length = Instant::now().duration_since(self.time);
        if length > Duration::from_secs(5) {
            self.bytes_transferred_timed = self.bytes_transfered;
            self.time = Instant::now();
        }
    }

    pub fn get_bps(&self) -> u64 {
        self.bps
    }

    pub fn has_log_entries(&self) -> bool {
        !self.output_log.is_empty()
    }

    pub fn errors(&self) -> usize {
        self.errors
    }

    pub fn warnings(&self) -> usize {
        self.warnings
    }

    pub fn log_count(&self) -> usize {
        self.output_log.len()
    }

    /// Get's a log message where
    /// `category` 0 = all, 1 = warnings, 2 = errors
    /// `index` is the index of the message
    pub fn get_log_message(&self, category: usize, index: usize) -> Option<&OutputLogMessage> {
        match category {
            0 => self.output_log.get(index),
            1 => self.output_log.iter().filter(|p| matches!(p, OutputLogMessage::Warning(_))).nth(index),
            2 => self.output_log.iter().filter(|p| matches!(p, OutputLogMessage::Error(_))).nth(index),
            _ => None,
        }
    }

    pub fn log_info(&mut self, txt: impl Into<String>) {
        self.output_log.push(OutputLogMessage::Info(txt.into()));
    }

    pub fn log_warning(&mut self, txt: impl Into<String>) {
        self.warnings += 1;
        self.output_log.push(OutputLogMessage::Warning(txt.into()));
    }

    pub fn log_error(&mut self, txt: impl Into<String>) {
        self.errors += 1;
        self.output_log.push(OutputLogMessage::Error(txt.into()));
    }
}

impl Default for TransferInformation {
    fn default() -> Self {
        Self {
            file_name: String::new(),
            file_size: 0,
            bytes_transfered: 0,
            errors: 0,
            warnings: 0,
            files_finished: Vec::new(),
            check_size: String::new(),
            time: Instant::now(),
            output_log: Vec::new(),
            bytes_transferred_timed: 0,
            bps: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransferState {
    pub current_state: &'static str,
    pub is_finished: bool,
    pub protocol_name: String,
    pub start_time: Instant,
    pub end_time: Instant,
    pub send_state: TransferInformation,
    pub recieve_state: TransferInformation,
    pub request_cancel: bool,
}

impl Default for TransferState {
    fn default() -> Self {
        Self {
            current_state: "",
            protocol_name: String::new(),
            is_finished: false,
            start_time: Instant::now(),
            end_time: Instant::now(),
            send_state: TransferInformation::default(),
            recieve_state: TransferInformation::default(),
            request_cancel: false,
        }
    }
}

impl TransferState {
    pub fn update_time(&mut self) {
        self.end_time = Instant::now();
    }
}

pub trait Protocol {
    fn update(
        &mut self,
        com: &mut dyn DataConnection,
        transfer_state: &Arc<Mutex<TransferState>>,
        storage_handler: &mut dyn FileStorageHandler,
    ) -> TerminalResult<bool>;

    fn initiate_send(&mut self, com: &mut dyn DataConnection, files: Vec<FileDescriptor>, transfer_state: &mut TransferState) -> TerminalResult<()>;

    fn initiate_recv(&mut self, com: &mut dyn DataConnection, transfer_state: &mut TransferState) -> TerminalResult<()>;

    fn cancel(&mut self, com: &mut dyn DataConnection) -> TerminalResult<()>;

    fn use_raw_transfer(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TransferType {
    #[default]
    ZModem,
    ZedZap,
    XModem,
    XModem1k,
    XModem1kG,
    YModem,
    YModemG,
    Text,
}

impl TransferType {
    pub fn create(self) -> Box<dyn Protocol> {
        match self {
            TransferType::ZModem => Box::new(Zmodem::new(1024)),
            TransferType::ZedZap => Box::new(Zmodem::new(8 * 1024)),
            TransferType::XModem => Box::new(XYmodem::new(XYModemVariant::XModem)),
            TransferType::XModem1k => Box::new(XYmodem::new(XYModemVariant::XModem1k)),
            TransferType::XModem1kG => Box::new(XYmodem::new(XYModemVariant::XModem1kG)),
            TransferType::YModem => Box::new(XYmodem::new(XYModemVariant::YModem)),
            TransferType::YModemG => Box::new(XYmodem::new(XYModemVariant::YModemG)),
            TransferType::Text => panic!("Not implemented"),
        }
    }
}

pub fn str_from_null_terminated_utf8_unchecked(s: &[u8]) -> String {
    let mut res = String::new();

    for b in s {
        if *b == 0 {
            break;
        }
        res.push(char::from_u32(u32::from(*b)).unwrap());
    }
    res
}
