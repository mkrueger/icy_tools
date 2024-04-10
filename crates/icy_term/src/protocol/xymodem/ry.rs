use icy_engine::get_crc16;
use std::{
    sync::{Arc, Mutex},
    thread,
};
use web_time::Duration;

use super::{constants::DEFAULT_BLOCK_LENGTH, get_checksum, Checksum, XYModemConfiguration};
use crate::{
    protocol::{
        str_from_null_terminated_utf8_unchecked,
        xymodem::constants::{ACK, EOT, EXT_BLOCK_LENGTH, NAK, SOH, STX},
        FileStorageHandler, TransferState,
    },
    ui::connect::DataConnection,
    TerminalResult,
};

#[derive(Debug)]
pub enum RecvState {
    None,

    StartReceive(usize),
    ReadYModemHeader(usize),
    ReadBlock(usize, usize),
    ReadBlockStart(u8, usize),
}

/// specification: <http://pauillac.inria.fr/~doligez/zmodem/ymodem.txt>
pub struct Ry {
    configuration: XYModemConfiguration,
    errors: usize,
    recv_state: RecvState,
}

impl Ry {
    pub fn new(configuration: XYModemConfiguration) -> Self {
        Ry {
            configuration,
            recv_state: RecvState::None,
            errors: 0,
        }
    }

    pub fn is_finished(&self) -> bool {
        matches!(self.recv_state, RecvState::None)
    }

    pub fn update(
        &mut self,
        com: &mut dyn DataConnection,
        transfer_state: &Arc<Mutex<TransferState>>,
        storage_handler: &mut dyn FileStorageHandler,
    ) -> TerminalResult<()> {
        if let Ok(mut transfer_state) = transfer_state.lock() {
            transfer_state.update_time();
            let transfer_info = &mut transfer_state.recieve_state;
            transfer_info.errors = self.errors;
            transfer_info.check_size = self.configuration.get_check_and_size();
            transfer_info.update_bps();
        }
        match self.recv_state {
            RecvState::None => {}

            RecvState::StartReceive(retries) => {
                if let Ok(mut transfer_state) = transfer_state.lock() {
                    transfer_state.current_state = "Start receiving...";
                }
                let start = com.read_u8()?;
                if start == SOH {
                    if self.configuration.is_ymodem() {
                        self.recv_state = RecvState::ReadYModemHeader(0);
                    } else {
                        storage_handler.open_unnamed_file();
                        self.recv_state = RecvState::ReadBlock(DEFAULT_BLOCK_LENGTH, 0);
                    }
                } else if start == STX {
                    storage_handler.open_unnamed_file();
                    self.recv_state = RecvState::ReadBlock(EXT_BLOCK_LENGTH, 0);
                } else {
                    if retries < 3 {
                        self.await_data(com)?;
                    } else if retries == 4 {
                        com.send(vec![NAK])?;
                    } else {
                        self.cancel(com)?;
                        return Err(anyhow::anyhow!("too many retries starting the communication"));
                    }
                    self.errors += 1;
                    self.recv_state = RecvState::StartReceive(retries + 1);
                }
            }

            RecvState::ReadYModemHeader(retries) => {
                let len = 128; // constant header length

                if let Ok(mut transfer_state) = transfer_state.lock() {
                    transfer_state.current_state = "Get header...";
                }
                let chksum_size = if let Checksum::CRC16 = self.configuration.checksum_mode { 2 } else { 1 };

                let block = com.read_exact(2 + len + chksum_size)?;
                if block[0] != block[1] ^ 0xFF {
                    com.send(vec![NAK])?;
                    self.errors += 1;
                    self.recv_state = RecvState::StartReceive(0);
                    return Ok(());
                }
                let block = &block[2..];
                if !self.check_crc(block) {
                    //println!("NAK CRC FAIL");
                    self.errors += 1;
                    com.send(vec![NAK])?;
                    self.recv_state = RecvState::ReadYModemHeader(retries + 1);
                    return Ok(());
                }
                if block[0] == 0 {
                    // END transfer
                    //println!("END TRANSFER");
                    com.send(vec![ACK])?;
                    self.recv_state = RecvState::None;
                    return Ok(());
                }

                let file_name = str_from_null_terminated_utf8_unchecked(block);

                let num = str_from_null_terminated_utf8_unchecked(&block[(file_name.len() + 1)..]).to_string();
                let file_size = if let Ok(file_size) = num.parse::<usize>() { file_size } else { 0 };
                if let Ok(mut transfer_state) = transfer_state.lock() {
                    transfer_state.recieve_state.file_name = file_name.clone();
                    transfer_state.recieve_state.file_size = file_size;
                }

                storage_handler.open_file(&file_name, file_size);

                if self.configuration.is_ymodem() {
                    com.send(vec![ACK, b'C'])?;
                } else {
                    com.send(vec![ACK])?;
                }
                self.recv_state = RecvState::ReadBlockStart(0, 0);
            }

            RecvState::ReadBlockStart(step, retries) => {
                if step == 0 {
                    let start = com.read_u8()?;
                    if start == SOH {
                        self.recv_state = RecvState::ReadBlock(DEFAULT_BLOCK_LENGTH, 0);
                    } else if start == STX {
                        self.recv_state = RecvState::ReadBlock(EXT_BLOCK_LENGTH, 0);
                    } else if start == EOT {
                        storage_handler.remove_cpm_eof();
                        storage_handler.close();

                        if let Ok(mut transfer_state) = transfer_state.lock() {
                            let transfer_info = &mut transfer_state.recieve_state;
                            transfer_info.log_info("File transferred.");
                        }

                        if self.configuration.is_ymodem() {
                            com.send(vec![NAK])?;
                            self.recv_state = RecvState::ReadBlockStart(1, 0);
                        } else {
                            com.send(vec![ACK])?;
                            self.recv_state = RecvState::None;
                        }
                    } else {
                        if retries < 5 {
                            com.send(vec![NAK])?;
                        } else {
                            self.cancel(com)?;
                            return Err(anyhow::anyhow!("too many retries"));
                        }
                        self.errors += 1;
                        self.recv_state = RecvState::ReadBlockStart(0, retries + 1);
                    }
                } else if step == 1 {
                    let eot = com.read_u8()?;
                    if eot != EOT {
                        self.recv_state = RecvState::None;
                        return Ok(());
                    }
                    if self.configuration.is_ymodem() {
                        com.send(vec![ACK, b'C'])?;
                    } else {
                        com.send(vec![ACK])?;
                    }
                    self.recv_state = RecvState::StartReceive(retries);
                }
            }

            RecvState::ReadBlock(len, retries) => {
                if let Ok(mut transfer_state) = transfer_state.lock() {
                    transfer_state.current_state = "Receiving data...";
                }
                let chksum_size = if let Checksum::CRC16 = self.configuration.checksum_mode { 2 } else { 1 };
                let block = com.read_exact(2 + len + chksum_size)?;
                if block[0] != block[1] ^ 0xFF {
                    com.send(vec![NAK])?;

                    self.errors += 1;
                    let start = com.read_u8()?;
                    if start == SOH {
                        self.recv_state = RecvState::ReadBlock(DEFAULT_BLOCK_LENGTH, retries + 1);
                    } else if start == STX {
                        self.recv_state = RecvState::ReadBlock(EXT_BLOCK_LENGTH, retries + 1);
                    } else {
                        self.cancel(com)?;
                        return Err(anyhow::anyhow!("too many retries"));
                    }
                    self.recv_state = RecvState::ReadBlock(EXT_BLOCK_LENGTH, retries + 1);
                    return Ok(());
                }
                let block = &block[2..];
                if !self.check_crc(block) {
                    //println!("\t\t\t\t\t\trecv crc mismatch");
                    self.errors += 1;
                    com.send(vec![NAK])?;
                    self.recv_state = RecvState::ReadBlockStart(0, retries + 1);
                    return Ok(());
                }

                storage_handler.append(&block[0..len]);
                if let Ok(mut transfer_state) = transfer_state.lock() {
                    transfer_state.recieve_state.bytes_transfered = storage_handler.current_file_length();
                }

                if !self.configuration.is_streaming() {
                    com.send(vec![ACK])?;
                }
                self.recv_state = RecvState::ReadBlockStart(0, 0);
            }
        }
        Ok(())
    }

    pub fn cancel(&mut self, com: &mut dyn DataConnection) -> TerminalResult<()> {
        self.recv_state = RecvState::None;
        super::cancel(com)
    }

    pub fn recv(&mut self, com: &mut dyn DataConnection) -> TerminalResult<()> {
        self.await_data(com)?;
        self.recv_state = RecvState::StartReceive(0);
        Ok(())
    }

    fn await_data(&mut self, com: &mut dyn DataConnection) -> TerminalResult<usize> {
        if self.configuration.is_streaming() {
            com.send(vec![b'G'])?;
        } else if self.configuration.use_crc() {
            com.send(vec![b'C'])?;
        } else {
            com.send(vec![NAK])?;
        }
        let mut i = 0;
        while i < 5 && !com.is_data_available()? {
            thread::sleep(Duration::from_millis(50));
            i += 1;
        }
        Ok(1)
    }

    fn check_crc(&self, block: &[u8]) -> bool {
        if block.len() < 3 {
            return false;
        }
        match self.configuration.checksum_mode {
            Checksum::Default => {
                let chk = get_checksum(&block[..block.len() - 1]);
                block[block.len() - 1] == chk
            }
            Checksum::CRC16 => {
                let check_crc = get_crc16(&block[..block.len() - 2]);
                let crc = u16::from_be_bytes(block[block.len() - 2..].try_into().unwrap());
                crc == check_crc
            }
        }
    }
    /*
    fn print_block(&self, block: &[u8]) {
        if block[0] == block[1] ^ 0xFF {
            print!("{:02X} {:02X}", block[0], block[1]);
        } else {
            println!("ERR  ERR");
            return;
        }
        let chksum_size = if let Checksum::CRC16 = self.configuration.checksum_mode {
            2
        } else {
            1
        };
        print!(" Data[{}] ", block.len() - 2 - chksum_size);

        if self.check_crc(&block[2..]) {
            println!("CRC OK ");
        } else {
            println!("CRC ERR");
        }
    }*/
}
