#![allow(clippy::unused_self, clippy::wildcard_imports)]
use std::{
    cmp::Ordering,
    sync::{Arc, Mutex},
};

use icy_engine::{get_crc32, update_crc32};
use web_time::Instant;

use crate::{
    protocol::{str_from_null_terminated_utf8_unchecked, FileStorageHandler, Header, HeaderType, TransferState, ZFrameType, Zmodem, ZCRCE, ZCRCG, ZCRCW},
    ui::connect::DataConnection,
    TerminalResult,
};

use super::{constants::*, err::TransmissionError, read_zdle_bytes, zrinit_flag::CANFDX};

#[derive(Debug)]
pub enum RecvState {
    Idle,
    Await,
    AwaitZDATA,
    AwaitFileData,
    AwaitEOF,
    SendZRINIT,
}

pub struct Rz {
    state: RecvState,
    pub errors: usize,
    retries: usize,
    can_count: usize,
    block_length: usize,
    sender_flags: u8,
    use_crc32: bool,
    last_send: Instant,

    can_fullduplex: bool,
    can_esc_control: bool,
    no_streaming: bool,
    can_break: bool,
    want_fcs_16: bool,
    escape_8th_bit: bool,
    attn_seq: Vec<u8>,
}

impl Rz {
    pub fn new(block_length: usize) -> Self {
        Self {
            state: RecvState::Idle,
            block_length,
            retries: 0,
            can_count: 0,
            errors: 0,
            sender_flags: 0,
            use_crc32: true,
            last_send: Instant::now(),
            can_fullduplex: true,
            can_esc_control: false,
            can_break: false,
            no_streaming: false,
            want_fcs_16: true,
            escape_8th_bit: false,
            attn_seq: vec![0],
        }
    }

    pub fn is_active(&self) -> bool {
        !matches!(self.state, RecvState::Idle)
    }

    fn cancel(&mut self, com: &mut dyn DataConnection) -> TerminalResult<()> {
        self.state = RecvState::Idle;
        Zmodem::cancel(com)
    }

    pub fn update(
        &mut self,
        com: &mut dyn DataConnection,
        transfer_state: &Arc<Mutex<TransferState>>,
        storage_handler: &mut dyn FileStorageHandler,
    ) -> TerminalResult<()> {
        if let RecvState::Idle = self.state {
            return Ok(());
        }
        if self.retries > 5 {
            self.cancel(com)?;
            return Ok(());
        }
        if let Ok(mut transfer_state) = transfer_state.lock() {
            transfer_state.update_time();
            let transfer_info = &mut transfer_state.recieve_state;

            if let Some(file) = storage_handler.current_file_name() {
                transfer_info.file_name = file;
                transfer_info.file_size = storage_handler.get_current_file_total_size();
                transfer_info.bytes_transfered = storage_handler.current_file_length();
            }
            transfer_info.errors = self.errors;
            transfer_info.check_size = "Crc32".to_string();
            transfer_info.update_bps();
        }
        // println!("rz state {:?}", self.state);
        match self.state {
            RecvState::SendZRINIT => {
                if self.read_header(com, storage_handler, transfer_state)? {
                    return Ok(());
                }
                /*  let now = Instant::now();
                 if now.duration_since(self.last_send).unwrap().as_millis() > 3000 {
                    self.send_zrinit(com)?;
                    self.retries += 1;
                    self.last_send = Instant::now();
                }*/
            }
            /*     RecvState::AwaitZDATA => {
                self.read_header(com, storage_handler, transfer_state)?;
            }*/
            RecvState::AwaitFileData => {
                let pck = read_subpacket(com, self.block_length, self.use_crc32, self.can_esc_control);
                match pck {
                    Ok((block, is_last, expect_ack)) => {
                        if expect_ack {
                            Header::from_number(ZFrameType::Ack, block.len() as u32).write(com, HeaderType::Hex, self.can_esc_control)?;
                        }
                        storage_handler.append(&block);
                        if is_last {
                            self.state = RecvState::AwaitEOF;
                        }
                    }
                    Err(err) => {
                        self.errors += 1;
                        log::error!("{err}");
                        if let Ok(mut transfer_state) = transfer_state.lock() {
                            let transfer_info = &mut transfer_state.recieve_state;
                            transfer_info.log_error(format!("sub package error: {err}"));
                        }
                        if storage_handler.current_file_name().is_some() {
                            Header::from_number(ZFrameType::RPos, u32::try_from(storage_handler.current_file_length()).unwrap()).write(
                                com,
                                HeaderType::Hex,
                                self.can_esc_control,
                            )?;
                            self.state = RecvState::AwaitZDATA;
                        }
                        return Ok(());
                    }
                }
            }
            _ => {
                self.read_header(com, storage_handler, transfer_state)?;
            }
        }
        Ok(())
    }

    fn request_zpos(&mut self, com: &mut dyn DataConnection, pos: u32) -> TerminalResult<usize> {
        Header::from_number(ZFrameType::RPos, pos).write(com, HeaderType::Hex, self.can_esc_control)
    }

    fn read_header(
        &mut self,
        com: &mut dyn DataConnection,
        storage_handler: &mut dyn FileStorageHandler,
        transfer_state: &Arc<Mutex<TransferState>>,
    ) -> TerminalResult<bool> {
        let result = Header::read(com, &mut self.can_count);
        if result.is_err() {
            if self.can_count >= 5 {
                //transfer_state.write("Received cancel...".to_string());
                self.cancel(com)?;
                self.cancel(com)?;
                self.cancel(com)?;
                self.state = RecvState::Idle;
                return Ok(false);
            }
            //transfer_state.write(format!("{}", err));
            self.errors += 1;
            return Ok(false);
        }
        self.can_count = 0;
        let header_opt = result?;
        if let Some(header) = header_opt {
            // println!("got header: {header}");
            match header.frame_type {
                ZFrameType::Sinit => {
                    let pck = read_subpacket(com, self.block_length, self.use_crc32, self.can_esc_control);
                    match pck {
                        Ok(attn_seq) => {
                            self.attn_seq = attn_seq.0;
                            self.sender_flags = header.f0();
                            Header::empty(ZFrameType::Ack).write(com, HeaderType::Hex, self.can_esc_control)?;
                            return Ok(true);
                        }
                        Err(err) => {
                            //transfer_state.write(format!("{}", err));
                            log::error!("{err}");
                            Header::empty(ZFrameType::Nak).write(com, HeaderType::Hex, self.can_esc_control)?;
                            return Ok(false);
                        }
                    }
                }

                ZFrameType::RQInit => {
                    self.state = RecvState::SendZRINIT;
                    return Ok(true);
                }
                ZFrameType::File => {
                    let pck = read_subpacket(com, self.block_length, self.use_crc32, self.can_esc_control);

                    match pck {
                        Ok((block, _, _)) => {
                            let file_name = str_from_null_terminated_utf8_unchecked(&block).to_string();
                            let mut file_size = 0;
                            for b in &block[(file_name.len() + 1)..] {
                                if *b < b'0' || *b > b'9' {
                                    break;
                                }
                                file_size = file_size * 10 + (*b - b'0') as usize;
                            }
                            if let Ok(mut transfer_state) = transfer_state.lock() {
                                let transfer_info = &mut transfer_state.recieve_state;

                                transfer_info.log_info(format!("Start file transfer: {file_name} ({file_size} bytes)"));
                            }
                            // println!("start file transfer: {file_name} ({file_size})");
                            storage_handler.open_file(&file_name, file_size);

                            self.state = RecvState::AwaitZDATA;
                            self.request_zpos(com, storage_handler.current_file_length() as u32)?;

                            return Ok(true);
                        }
                        Err(err) => {
                            log::error!("{err}");
                            self.errors += 1;
                            com.send(self.attn_seq.clone())?;
                            Header::from_number(ZFrameType::RPos, storage_handler.current_file_length() as u32).write(
                                com,
                                HeaderType::Hex,
                                self.can_esc_control,
                            )?;
                            //transfer_state.write(format!("{}", err));
                            return Ok(false);
                        }
                    }
                }
                ZFrameType::Data => {
                    let offset = header.number();
                    if storage_handler.current_file_name().is_none() {
                        self.cancel(com)?;
                        return Err(TransmissionError::ZDataBeforeZFILE.into());
                    }
                    let len = storage_handler.current_file_length();
                    match len.cmp(&(offset as usize)) {
                        Ordering::Greater => storage_handler.set_current_size_to(offset as usize),
                        Ordering::Less => {
                            Header::from_number(ZFrameType::RPos, len as u32).write(com, HeaderType::Hex, self.can_esc_control)?;
                            return Ok(false);
                        }
                        Ordering::Equal => {}
                    }
                    self.state = RecvState::AwaitFileData;
                    return Ok(true);
                }
                ZFrameType::Eof => {
                    self.send_zrinit(com)?;
                    if let Ok(mut transfer_state) = transfer_state.lock() {
                        let transfer_info = &mut transfer_state.recieve_state;
                        transfer_info.log_info("File transferred.");
                        if let Some(file) = storage_handler.current_file_name() {
                            transfer_info.files_finished.push(file);
                        }
                    }
                    storage_handler.close();
                    self.state = RecvState::SendZRINIT;
                    return Ok(true);
                }
                ZFrameType::Fin => {
                    Header::empty(ZFrameType::Fin).write(com, HeaderType::Hex, self.can_esc_control)?;
                    //transfer_state.write("Transfer finished.".to_string());
                    self.state = RecvState::Idle;
                    return Ok(true);
                }
                ZFrameType::Challenge => {
                    // isn't specfied for receiver side.
                    Header::from_number(ZFrameType::Ack, header.number()).write(com, HeaderType::Hex, self.can_esc_control)?;
                }
                ZFrameType::FreeCnt => {
                    // 0 means unlimited space but sending free hd space to an unknown source is a security issue
                    Header::from_number(ZFrameType::Ack, 0).write(com, HeaderType::Hex, self.can_esc_control)?;
                }
                ZFrameType::Command => {
                    // just protocol it.
                    let package = read_subpacket(com, self.block_length, self.use_crc32, self.can_esc_control);
                    match &package {
                        Ok((block, _, _)) => {
                            let cmd = str_from_null_terminated_utf8_unchecked(block);
                            log::error!("Remote wanted to execute {cmd} on the system. (did not execute)");
                        }
                        Err(err) => {
                            log::error!("{err}");
                        }
                    }
                    Header::from_number(ZFrameType::Compl, 0).write(com, HeaderType::Hex, self.can_esc_control)?;
                }
                ZFrameType::Abort | ZFrameType::FErr | ZFrameType::Can => {
                    Header::empty(ZFrameType::Fin).write(com, HeaderType::Hex, self.can_esc_control)?;
                    self.state = RecvState::Idle;
                }
                unk_frame => {
                    return Err(TransmissionError::UnsupportedFrame(unk_frame).into());
                }
            }
        }
        Ok(false)
    }

    pub fn recv(&mut self, com: &mut dyn DataConnection) -> TerminalResult<()> {
        self.state = RecvState::Await;
        self.retries = 0;
        self.send_zrinit(com)?;
        Ok(())
    }

    pub fn send_zrinit(&mut self, com: &mut dyn DataConnection) -> TerminalResult<()> {
        let mut flags = 0;
        if self.can_fullduplex {
            flags |= CANFDX;
        }
        if !self.no_streaming {
            flags |= zrinit_flag::CANOVIO;
        }
        if self.can_break {
            flags |= zrinit_flag::CANBRK;
        }
        if self.want_fcs_16 {
            flags |= zrinit_flag::CANFC32;
        }
        if self.can_esc_control {
            flags |= zrinit_flag::ESCCTL;
        }
        if self.escape_8th_bit {
            flags |= zrinit_flag::ESC8;
        }
        Header::from_flags(ZFrameType::RIinit, 0, 0, 0, flags).write(com, HeaderType::Hex, self.can_esc_control)?;
        Ok(())
    }
}

pub fn read_subpacket(com: &mut dyn DataConnection, block_length: usize, use_crc32: bool, escape_ctrl_chars: bool) -> TerminalResult<(Vec<u8>, bool, bool)> {
    let mut data = Vec::with_capacity(block_length);
    loop {
        match read_zdle_byte(com, escape_ctrl_chars)? {
            ZModemResult::Ok(b) => data.push(b),
            ZModemResult::CrcCheckRequested(first_byte, frame_ends, zack_requested) => match check_crc(com, use_crc32, &data, first_byte) {
                Ok(_) => {
                    return Ok((data, frame_ends, zack_requested));
                }
                Err(err) => {
                    return Err(anyhow::anyhow!("Error during subpacket crc check: {err}"));
                }
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ZModemResult {
    Ok(u8),

    /// first bool:frame ends
    /// second bool:zack requested
    CrcCheckRequested(u8, bool, bool),
}

pub fn read_zdle_byte(com: &mut dyn DataConnection, escape_ctrl_chars: bool) -> TerminalResult<ZModemResult> {
    loop {
        let c = com.read_u8()?;
        match c {
            ZDLE => {
                loop {
                    let c = com.read_u8()?;
                    match c {
                        XON | XON_0x80 | XOFF | XOFF_0x80 | ZDLE => {
                            continue;
                        }
                        ZRUB0 => return Ok(ZModemResult::Ok(0x7F)),
                        ZRUB1 => return Ok(ZModemResult::Ok(0xFF)),
                        ZCRCE => {
                            return Ok(ZModemResult::CrcCheckRequested(c, true, false));
                        }
                        ZCRCG => {
                            return Ok(ZModemResult::CrcCheckRequested(c, false, false));
                        }
                        ZCRCQ => {
                            return Ok(ZModemResult::CrcCheckRequested(c, false, true));
                        }
                        ZCRCW => {
                            return Ok(ZModemResult::CrcCheckRequested(c, true, true));
                        }

                        _ => {
                            // TODO: is that correct?
                            if escape_ctrl_chars && c & 0x60 == 0 {
                                // Drop unescaped ctrl char
                                continue;
                            }

                            if c & 0x60 == 0x40 {
                                return Ok(ZModemResult::Ok(c ^ 0x40));
                            }

                            return Err(TransmissionError::InvalidSubpacket(c).into());
                        }
                    }
                }
            }
            XON | XON_0x80 | XOFF | XOFF_0x80 => {
                // they should be ignored, not errored according to spec
                // log::info("ignored byte");
                continue;
            }
            _ => {
                // TODO: is that correct?
                if escape_ctrl_chars && c & 0x60 == 0 {
                    continue;
                }
                return Ok(ZModemResult::Ok(c));
            }
        }
    }
}

fn check_crc(com: &mut dyn DataConnection, use_crc32: bool, data: &[u8], zcrc_byte: u8) -> TerminalResult<bool> {
    if use_crc32 {
        let mut crc = get_crc32(data);
        crc = !update_crc32(!crc, zcrc_byte);
        let crc_bytes = read_zdle_bytes(com, 4)?;
        let check_crc = u32::from_le_bytes(crc_bytes.try_into().unwrap());
        if crc == check_crc {
            Ok(true)
        } else {
            Err(TransmissionError::CRC32Mismatch(crc, check_crc).into())
        }
    } else {
        let crc = icy_engine::get_crc16_buggy_zlde(data, zcrc_byte);

        let crc_bytes = read_zdle_bytes(com, 2)?;
        let check_crc = u16::from_le_bytes(crc_bytes.try_into().unwrap());
        if crc == check_crc {
            Ok(true)
        } else {
            Err(TransmissionError::CRC16Mismatch(crc, check_crc).into())
        }
    }
}
