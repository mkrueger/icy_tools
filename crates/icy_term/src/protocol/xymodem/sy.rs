use icy_engine::get_crc16;
use std::{
    cmp::min,
    sync::{Arc, Mutex},
};

use super::{
    constants::{CAN, DEFAULT_BLOCK_LENGTH},
    err::TransmissionError,
    get_checksum, Checksum, XYModemConfiguration, XYModemVariant,
};
use crate::{
    protocol::{
        xymodem::constants::{ACK, CPMEOF, EOT, EXT_BLOCK_LENGTH, NAK, SOH, STX},
        FileDescriptor, TransferState,
    },
    ui::connect::DataConnection,
    TerminalResult,
};

#[derive(Debug)]
pub enum SendState {
    None,
    InitiateSend,
    SendYModemHeader(usize),
    AckSendYmodemHeader(usize),
    SendData(usize, usize),
    AckSendData(usize, usize),
    YModemEndHeader(u8),
}

pub struct Sy {
    pub bytes_send: usize,
    configuration: XYModemConfiguration,

    pub files: Vec<FileDescriptor>,
    cur_file: usize,

    block_number: u8,
    errors: usize,
    send_state: SendState,

    pub data: Vec<u8>,

    transfer_stopped: bool,
}

impl Sy {
    pub fn new(configuration: XYModemConfiguration) -> Self {
        Self {
            configuration,

            send_state: SendState::None,
            files: Vec::new(),
            data: Vec::new(),
            errors: 0,
            bytes_send: 0,
            block_number: match configuration.variant {
                XYModemVariant::YModem | XYModemVariant::YModemG => 0,
                _ => 1,
            },
            cur_file: 0,
            transfer_stopped: false,
        }
    }

    pub fn is_finished(&self) -> bool {
        matches!(self.send_state, SendState::None)
    }

    pub fn update(&mut self, com: &mut dyn DataConnection, transfer_state: &Arc<Mutex<TransferState>>) -> TerminalResult<()> {
        if let Ok(mut transfer_state) = transfer_state.lock() {
            transfer_state.update_time();
            let transfer_info = &mut transfer_state.send_state;
            if self.cur_file < self.files.len() {
                let f = &self.files[self.cur_file];
                transfer_info.file_name = f.file_name.clone();
                transfer_info.file_size = f.size;
            }
            transfer_info.bytes_transfered = self.bytes_send;
            transfer_info.errors = self.errors;
            transfer_info.check_size = self.configuration.get_check_and_size();
            transfer_info.update_bps();
        }

        match self.send_state {
            SendState::None => {}
            SendState::InitiateSend => {
                if let Ok(mut transfer_state) = transfer_state.lock() {
                    transfer_state.current_state = "Initiate send…";
                }
                self.get_mode(com)?;
                if self.configuration.is_ymodem() {
                    self.send_state = SendState::SendYModemHeader(0);
                } else {
                    self.send_state = SendState::SendData(0, 0);
                }
            }

            SendState::SendYModemHeader(retries) => {
                if retries > 3 {
                    if let Ok(mut transfer_state) = transfer_state.lock() {
                        transfer_state.current_state = "Too many retries...aborting";
                    }
                    self.cancel(com)?;
                    return Ok(());
                }
                self.block_number = 0;
                //transfer_info.write("Send header...".to_string());
                self.send_ymodem_header(com)?;
                self.send_state = SendState::AckSendYmodemHeader(retries);
            }

            SendState::AckSendYmodemHeader(retries) => {
                // let now = Instant::now();
                let ack = self.read_command(com)?;
                if ack == NAK {
                    if let Ok(mut transfer_state) = transfer_state.lock() {
                        transfer_state.current_state = "Encountered error";
                    }
                    self.errors += 1;
                    if retries > 5 {
                        self.send_state = SendState::None;
                        return Err(TransmissionError::TooManyRetriesSendingHeader.into());
                    }
                    self.send_state = SendState::SendYModemHeader(retries + 1);
                    return Ok(());
                }
                if ack == ACK {
                    if self.transfer_stopped {
                        self.send_state = SendState::None;
                        return Ok(());
                    }
                    if let Ok(mut transfer_state) = transfer_state.lock() {
                        transfer_state.current_state = "Header accepted.";
                    }
                    self.data = self.files[self.cur_file].get_data();
                    let _ = self.read_command(com)?;
                    // SKIP - not needed to check that
                    self.send_state = SendState::SendData(0, 0);
                }

                /*  if now
                    .duration_since(self.last_header_send)
                    .unwrap()
                    .as_millis()
                    > 3000
                {
                    self.send_state = SendState::SendYModemHeader(retries + 1);
                }*/
            }
            SendState::SendData(cur_offset, retries) => {
                if let Ok(mut transfer_state) = transfer_state.lock() {
                    transfer_state.current_state = "Send data...";
                }
                if self.send_data_block(com, cur_offset)? {
                    if self.configuration.is_streaming() {
                        self.bytes_send = cur_offset + self.configuration.block_length;
                        self.send_state = SendState::SendData(self.bytes_send, 0);
                        self.check_eof(com)?;
                    } else {
                        self.send_state = SendState::AckSendData(cur_offset, retries);
                    }
                } else {
                    self.send_state = SendState::None;
                };
            }
            SendState::AckSendData(cur_offset, retries) => {
                let ack = self.read_command(com)?;
                if ack == CAN {
                    // need 2 CAN
                    let can2 = self.read_command(com)?;
                    if can2 == CAN {
                        self.send_state = SendState::None;
                        //transfer_info.write("Got cancel ...".to_string());
                        return Err(TransmissionError::Cancel.into());
                    }
                }

                if ack != ACK {
                    self.errors += 1;

                    // fall back to short block length after too many errors
                    if retries > 3 && self.configuration.block_length == EXT_BLOCK_LENGTH {
                        self.configuration.block_length = DEFAULT_BLOCK_LENGTH;
                        self.send_state = SendState::SendData(cur_offset, retries + 2);
                        return Ok(());
                    }

                    if retries > 5 {
                        self.eot(com)?;
                        return Err(TransmissionError::TooManyRetriesSendingHeader.into());
                    }
                    self.send_state = SendState::SendData(cur_offset, retries + 1);
                    return Ok(());
                }
                self.bytes_send = cur_offset + self.configuration.block_length;
                self.send_state = SendState::SendData(self.bytes_send, 0);
                self.check_eof(com)?;
            }
            SendState::YModemEndHeader(step) => match step {
                0 => {
                    let read_command = self.read_command(com)?;
                    if read_command == NAK {
                        com.send(vec![EOT])?;
                        self.send_state = SendState::YModemEndHeader(1);
                        return Ok(());
                    }
                    if read_command == ACK {
                        self.send_state = SendState::None;
                        return Ok(());
                    }
                    self.cancel(com)?;
                }
                1 => {
                    if self.read_command(com)? == ACK {
                        self.send_state = SendState::YModemEndHeader(2);
                        return Ok(());
                    }
                    self.cancel(com)?;
                }
                2 => {
                    if self.read_command(com)? == b'C' {
                        self.send_state = SendState::SendYModemHeader(0);
                        self.cur_file += 1;
                        return Ok(());
                    }
                    self.cancel(com)?;
                }
                _ => {
                    self.send_state = SendState::None;
                }
            },
        }
        Ok(())
    }

    fn check_eof(&mut self, com: &mut dyn DataConnection) -> TerminalResult<()> {
        if self.bytes_send >= self.files[self.cur_file].size {
            self.eot(com)?;
            if self.configuration.is_ymodem() {
                self.send_state = SendState::YModemEndHeader(0);
            } else {
                self.send_state = SendState::None;
            }
        }
        Ok(())
    }

    #[allow(clippy::unused_self)]
    fn read_command(&self, com: &mut dyn DataConnection) -> TerminalResult<u8> {
        let ch = com.read_u8()?;
        /*
         let cmd = match ch {
            b'C' => "[C]",
            EOT => "[EOT]",
            ACK => "[ACK]",
            NAK => "[NAK]",
            CAN => "[CAN]",
            _ => ""
        };
        println!("GOT CMD: #{} (0x{:X})", cmd, ch);*/

        Ok(ch)
    }

    #[allow(clippy::unused_self)]
    fn eot(&self, com: &mut dyn DataConnection) -> TerminalResult<usize> {
        // println!("[EOT]");
        com.send(vec![EOT])?;
        self.read_command(com)?; // read ACK

        Ok(1)
    }

    pub fn get_mode(&mut self, com: &mut dyn DataConnection) -> TerminalResult<()> {
        let ch = self.read_command(com)?;
        match ch {
            NAK => {
                self.configuration.checksum_mode = Checksum::Default;
                Ok(())
            }
            b'C' => {
                self.configuration.checksum_mode = Checksum::CRC16;
                Ok(())
            }
            b'G' => {
                self.configuration = if self.configuration.is_ymodem() {
                    XYModemConfiguration::new(XYModemVariant::YModemG)
                } else {
                    XYModemConfiguration::new(XYModemVariant::XModem1kG)
                };
                Ok(())
            }
            CAN => Err(TransmissionError::Cancel.into()),
            _ => Err(TransmissionError::InvalidMode(ch).into()),
        }
    }

    fn send_block(&mut self, com: &mut dyn DataConnection, data: &[u8], pad_byte: u8) -> TerminalResult<()> {
        let block_len = if data.len() <= DEFAULT_BLOCK_LENGTH { SOH } else { STX };
        let mut block = Vec::new();
        block.push(block_len);
        block.push(self.block_number);
        block.push(!self.block_number);
        block.extend_from_slice(data);
        block.resize((if block_len == SOH { DEFAULT_BLOCK_LENGTH } else { EXT_BLOCK_LENGTH }) + 3, pad_byte);

        match self.configuration.checksum_mode {
            Checksum::Default => {
                let chk_sum = get_checksum(&block[3..]);
                block.push(chk_sum);
            }
            Checksum::CRC16 => {
                let crc = get_crc16(&block[3..]);
                block.extend_from_slice(&u16::to_be_bytes(crc));
            }
        }
        // println!("Send block {:X?}", block);
        com.send(block)?;
        self.block_number = self.block_number.wrapping_add(1);
        Ok(())
    }

    fn send_ymodem_header(&mut self, com: &mut dyn DataConnection) -> TerminalResult<()> {
        if self.cur_file < self.files.len() {
            // restart from 0
            let mut block = Vec::new();
            let fd = &self.files[self.cur_file];
            let name = fd.file_name.as_bytes();
            block.extend_from_slice(name);
            block.push(0);
            block.extend_from_slice(format!("{}", fd.size).as_bytes());
            self.send_block(com, &block, 0)?;
            Ok(())
        } else {
            self.end_ymodem(com)?;
            Ok(())
        }
    }

    fn send_data_block(&mut self, com: &mut dyn DataConnection, offset: usize) -> TerminalResult<bool> {
        let data_len = self.data.len();
        if offset >= data_len {
            return Ok(false);
        }
        let mut block_end = min(offset + self.configuration.block_length, data_len);

        if block_end - offset < EXT_BLOCK_LENGTH - 2 * DEFAULT_BLOCK_LENGTH {
            self.configuration.block_length = DEFAULT_BLOCK_LENGTH;
            block_end = min(offset + self.configuration.block_length, data_len);
        }
        let d = self.data[offset..block_end].to_vec();
        self.send_block(com, &d, CPMEOF)?;
        Ok(true)
    }

    pub fn cancel(&mut self, com: &mut dyn DataConnection) -> TerminalResult<()> {
        self.send_state = SendState::None;
        super::cancel(com)
    }

    pub fn send(&mut self, files: Vec<FileDescriptor>) {
        self.send_state = SendState::InitiateSend;
        self.files = files;
        self.cur_file = 0;
        self.bytes_send = 0;
    }

    pub fn end_ymodem(&mut self, com: &mut dyn DataConnection) -> TerminalResult<()> {
        self.send_block(com, &[0], 0)?;
        self.transfer_stopped = true;
        Ok(())
    }
}
