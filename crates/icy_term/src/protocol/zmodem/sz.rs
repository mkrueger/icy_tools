#![allow(clippy::unused_self)]

use std::{
    cmp::min,
    sync::{Arc, Mutex},
};

use crate::{
    protocol::{zfile_flag, zmodem::err::TransmissionError, FileDescriptor, Header, HeaderType, TransferState, ZFrameType, Zmodem, ZCRCE, ZCRCG},
    ui::connect::DataConnection,
    TerminalResult,
};

use super::{ZCRCQ, ZCRCW};

#[derive(Debug)]
pub enum SendState {
    Await,
    SendZRQInit,
    SendZDATA,
    SendDataPackages,
    Finished,
}

pub struct Sz {
    state: SendState,
    pub files: Vec<FileDescriptor>,
    cur_file: i32,
    cur_file_pos: usize,
    pub errors: usize,
    pub package_len: usize,
    pub transfered_file: bool,
    data: Vec<u8>,
    retries: usize,
    can_count: usize,
    receiver_capabilities: u8,

    nonstop: bool,
}

impl Sz {
    pub fn new(block_length: usize) -> Self {
        Self {
            state: SendState::Finished,
            files: Vec::new(),
            cur_file: 0,
            transfered_file: false,
            cur_file_pos: 0,
            errors: 0,
            data: Vec::new(),
            retries: 0,
            receiver_capabilities: 0,
            can_count: 0,
            package_len: block_length,
            nonstop: true,
        }
    }

    fn can_fdx(&self) -> bool {
        self.receiver_capabilities & super::zrinit_flag::CANFDX != 0
    }
    fn can_receive_data_during_io(&self) -> bool {
        self.receiver_capabilities & super::zrinit_flag::CANOVIO != 0
    }
    fn _can_send_break(&self) -> bool {
        self.receiver_capabilities & super::zrinit_flag::CANBRK != 0
    }
    fn _can_decrypt(&self) -> bool {
        self.receiver_capabilities & super::zrinit_flag::CANCRY != 0
    }
    fn _can_lzw(&self) -> bool {
        self.receiver_capabilities & super::zrinit_flag::CANLZW != 0
    }
    fn can_use_crc32(&self) -> bool {
        self.receiver_capabilities & super::zrinit_flag::CANFC32 != 0
    }
    fn can_esc_control(&self) -> bool {
        self.receiver_capabilities & super::zrinit_flag::ESCCTL != 0
    }
    fn _can_esc_8thbit(&self) -> bool {
        self.receiver_capabilities & super::zrinit_flag::ESC8 != 0
    }

    fn get_header_type(&self) -> HeaderType {
        if self.can_use_crc32() {
            HeaderType::Bin32
        } else {
            HeaderType::Bin
        }
    }

    fn encode_subpacket(&self, zcrc_byte: u8, data: &[u8]) -> Vec<u8> {
        if self.can_use_crc32() {
            Zmodem::encode_subpacket_crc32(zcrc_byte, data, self.can_esc_control())
        } else {
            Zmodem::encode_subpacket_crc16(zcrc_byte, data, self.can_esc_control())
        }
    }

    pub fn is_active(&self) -> bool {
        !matches!(self.state, SendState::Finished)
    }

    fn next_file(&mut self) {
        self.cur_file += 1;
    }

    pub fn update(&mut self, com: &mut dyn DataConnection, transfer_state: &Arc<Mutex<TransferState>>) -> TerminalResult<()> {
        if let SendState::Finished = self.state {
            return Ok(());
        }
        if self.retries > 5 {
            Zmodem::cancel(com)?;
            self.state = SendState::Finished;
            return Ok(());
        }
        if let Ok(mut transfer_state) = transfer_state.lock() {
            transfer_state.update_time();
            let transfer_info = &mut transfer_state.send_state;
            if self.cur_file >= 0 {
                if let Some(fd) = self.files.get(usize::try_from(self.cur_file).unwrap()) {
                    transfer_info.file_name = fd.file_name.clone();
                    transfer_info.file_size = fd.size;
                }
            }
            transfer_info.bytes_transfered = self.cur_file_pos;
            transfer_info.errors = self.errors;
            transfer_info.check_size = format!("Crc32/{}", self.package_len);
            transfer_info.update_bps();
        }
        match self.state {
            SendState::Await => {
                self.read_next_header(com)?;
            }
            SendState::SendZRQInit => {
                //                transfer_state.current_state = "Negotiating transfer";
                //    let now = Instant::now();
                //     if now.duration_since(self.last_send).unwrap().as_millis() > 3000 {
                self.send_zrqinit(com)?;
                self.state = SendState::Await;
                self.retries += 1;
                //         self.last_send = Instant::now();
                //     }
            }
            SendState::SendZDATA => {
                //              transfer_state.current_state = "Sending data";
                if self.cur_file < 0 {
                    //println!("no file to send!");
                    return Ok(());
                }
                Header::from_number(ZFrameType::Data, self.cur_file_pos as u32).write(com, self.get_header_type(), self.can_esc_control())?;
                self.state = SendState::SendDataPackages;
            }
            SendState::SendDataPackages => {
                let mut p = Vec::new();
                if self.cur_file < 0 {
                    return Ok(());
                }
                let old_pos = self.cur_file_pos;
                let end_pos = min(self.data.len(), self.cur_file_pos + self.package_len);
                let crc_byte = if self.cur_file_pos + self.package_len < self.data.len() {
                    if self.nonstop {
                        ZCRCG
                    } else {
                        ZCRCQ
                    }
                } else if self.nonstop {
                    ZCRCE
                } else {
                    ZCRCW
                };
                p.extend_from_slice(&self.encode_subpacket(crc_byte, &self.data[self.cur_file_pos..end_pos]));
                self.cur_file_pos = end_pos;
                if end_pos >= self.data.len() {
                    p.extend_from_slice(&Header::from_number(ZFrameType::Eof, end_pos as u32).build(self.get_header_type(), self.can_esc_control()));
                    // println!("send eof!");
                    //transfer_info.write("Done sending file date.".to_string());
                    // transfer_state.current_state = "Done data";
                    self.transfered_file = true;
                    self.state = SendState::Await;
                }
                com.send(p)?;
                if !self.nonstop {
                    let ack = Header::read(com, &mut self.can_count)?;
                    if let Some(header) = ack {
                        // println!("got header after data package: {header}",);
                        match header.frame_type {
                            ZFrameType::Ack => { /* ok */ }
                            ZFrameType::Nak => {
                                self.cur_file_pos = old_pos; /* resend */
                            }
                            ZFrameType::RPos => {
                                self.cur_file_pos = header.number() as usize;
                            }
                            _ => {
                                log::error!("unexpected header {header:?}");
                                // cancel
                                self.state = SendState::Finished;
                                Zmodem::cancel(com)?;
                            }
                        }
                    }
                }
            }
            SendState::Finished => {
                //                transfer_state.current_state = "Finishing transfer…";
                // let now = Instant::now();
                //if now.duration_since(self.last_send).unwrap().as_millis() > 3000 {
                self.send_zfin(com, 0)?;
                //}
                return Ok(());
            }
        }
        Ok(())
    }

    fn read_next_header(&mut self, com: &mut dyn DataConnection) -> TerminalResult<()> {
        let err = Header::read(com, &mut self.can_count);
        if self.can_count >= 5 {
            // transfer_info.write("Received cancel...".to_string());
            self.state = SendState::Finished;
            return Ok(());
        }
        if let Err(err) = err {
            log::error!("error reading header: {:?}", err);
            if self.errors > 3 {
                self.state = SendState::Finished;
                Zmodem::cancel(com)?;
                return Err(err);
            }
            self.errors += 1;
            return Ok(());
        }
        self.errors = 0;
        let res = err.unwrap();
        if let Some(res) = res {
            // println!("got header {}", res);
            match res.frame_type {
                ZFrameType::RIinit => {
                    if self.transfered_file {
                        self.next_file();
                        self.transfered_file = false;
                    }

                    if self.cur_file as usize >= self.files.len() {
                        self.send_zfin(com, self.cur_file_pos as u32)?;
                        self.cur_file_pos = 0;
                        return Ok(());
                    }
                    self.cur_file_pos = 0;
                    self.receiver_capabilities = res.f0();
                    let block_size = res.p0() as usize + ((res.p1() as usize) << 8);
                    self.nonstop = block_size == 0;
                    if block_size != 0 {
                        self.package_len = block_size;
                    }
                    /*
                    if self._can_decrypt() {
                        println!("receiver can decrypt");
                    }
                    if self.can_fdx() {
                        println!("receiver can send and receive true full duplex");
                    }
                    if self._can_receive_data_during_io() {
                        println!("receiver can receive data during disk I/O");
                    }
                    if self._can_send_break() {
                        println!("receiver can send a break signal");
                    }
                    if self._can_lzw() {
                        println!("receiver can uncompress");
                    }
                    if self.can_use_crc32() {
                        println!("receiver can use 32 bit Frame Check");
                    }
                    if self.can_esc_control() {
                        println!("receiver expects ctl chars to be escaped");
                    }
                    if self._can_esc_8thbit() {
                        println!("receiver expects 8th bit to be escaped");
                    }*/
                    //  transfer_state.current_state = "Sending header";
                    self.send_zfile(com, 0)?;
                    return Ok(());
                }

                ZFrameType::Nak => {
                    // transfer_info
                    //     .write("Package error, resending file header...".to_string());
                }

                ZFrameType::RPos => {
                    self.cur_file_pos = res.number() as usize;
                    self.state = SendState::SendZDATA;

                    if let SendState::SendDataPackages = self.state {
                        if self.package_len > 512 {
                            //reinit transfer.
                            self.package_len /= 2;
                            self.state = SendState::SendZRQInit;
                            //        com.write(b"rz\r")?;
                            self.send_zrqinit(com)?;
                            return Ok(());
                        }
                    }
                }

                ZFrameType::Fin => {
                    self.state = SendState::Finished;
                    com.send(b"OO".to_vec())?;
                    return Ok(());
                }
                ZFrameType::Challenge => {
                    Header::from_number(ZFrameType::Ack, res.number()).write(com, self.get_header_type(), self.can_esc_control())?;
                }
                ZFrameType::Abort | ZFrameType::FErr | ZFrameType::Can => {
                    Header::empty(ZFrameType::Fin).write(com, self.get_header_type(), self.can_esc_control())?;
                    self.state = SendState::Finished;
                }
                unk_frame => {
                    return Err(TransmissionError::UnsupportedFrame(unk_frame).into());
                }
            }
        }
        Ok(())
    }

    fn send_zfile(&mut self, com: &mut dyn DataConnection, tries: i32) -> TerminalResult<()> {
        if self.cur_file < 0 || self.cur_file >= self.files.len() as i32 {
            self.state = SendState::Finished;
            return Ok(());
        }
        let mut b = Vec::new();
        //transfer_state.write("Send file header".to_string());
        // println!("send zfile!");
        b.extend_from_slice(
            &Header::from_flags(ZFrameType::File, 0, 0, zfile_flag::ZMNEW, zfile_flag::ZCRESUM).build(self.get_header_type(), self.can_esc_control()),
        );
        let cur_file_size = usize::try_from(self.cur_file).unwrap();
        let f = &self.files[cur_file_size];
        self.data = f.get_data();
        let data = if f.date > 0 {
            let bytes_left = self.files.iter().skip(cur_file_size + 1).fold(0, |b, f| b + f.size);
            format!(
                "{}\0{} {} 0 0 {} {}\0",
                f.file_name,
                f.size,
                f.date,
                self.files.len() - cur_file_size,
                bytes_left
            )
            .into_bytes()
        } else {
            format!("{}\0{}\0", f.file_name, f.size).into_bytes()
        };

        b.extend_from_slice(&self.encode_subpacket(ZCRCW, &data));
        com.send(b)?;

        let ack = Header::read(com, &mut self.can_count)?;
        if let Some(header) = ack {
            // println!("got header afer zfile: {}", header);
            match header.frame_type {
                ZFrameType::Ack => self.state = SendState::SendZDATA,
                ZFrameType::Skip => {
                    self.next_file();
                    self.send_zfile(com, 0)?;
                }
                ZFrameType::RPos => {
                    self.cur_file_pos = header.number() as usize;
                    self.state = SendState::SendZDATA;
                }

                ZFrameType::Nak => {
                    if tries > 5 {
                        log::error!("too many tries for z_file");
                        self.state = SendState::Finished;
                        Zmodem::cancel(com)?;
                        return Ok(());
                    }
                    // self.send_zfile(com, tries + 1); /* resend */
                }
                _ => {
                    log::error!("unexpected header {header:?}");
                    // cancel
                    self.state = SendState::Finished;
                    Zmodem::cancel(com)?;
                }
            }
        }

        self.cur_file_pos = 0;
        Ok(())
    }

    pub fn send(&mut self, _com: &mut dyn DataConnection, files: Vec<FileDescriptor>) {
        //println!("initiate zmodem send {}", files.len());
        self.state = SendState::SendZRQInit;
        self.files = files;
        self.cur_file = 0;
        self.cur_file_pos = 0;
        self.retries = 0;
        //        com.write(b"rz\r")?;
    }

    pub fn send_zrqinit(&mut self, com: &mut dyn DataConnection) -> TerminalResult<()> {
        self.cur_file = -1;
        self.transfered_file = true;
        Header::empty(ZFrameType::RQInit).write(com, self.get_header_type(), self.can_esc_control())?;
        Ok(())
    }

    pub fn send_zfin(&mut self, com: &mut dyn DataConnection, size: u32) -> TerminalResult<()> {
        Header::from_number(ZFrameType::Fin, size).write(com, self.get_header_type(), self.can_esc_control())?;
        self.state = SendState::Await;
        Ok(())
    }
}
