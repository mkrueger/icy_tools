use std::fmt::Display;

use crate::{ui::connect::DataConnection, TerminalResult};
use icy_engine::{get_crc16_buggy, get_crc32, update_crc16};

use crate::protocol::{frame_types::ZACK, XON};

use super::{
    append_zdle_encoded,
    err::TransmissionError,
    frame_types::{self},
    from_hex, get_hex, read_zdle_bytes, ZBIN, ZBIN32, ZDLE, ZHEX, ZPAD,
};

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum HeaderType {
    Bin,
    Bin32,
    Hex,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum ZFrameType {
    /// Request receive init (s->r)
    RQInit = 0,
    /// Receive init (r->s)
    RIinit = 1,
    // Send init sequence (optional) (s->r)
    Sinit = 2,
    // ACK to RQInit, RInit or SInit (s<->r)
    Ack = 3,
    /// File name from sender (s->r)
    File = 4,
    /// To sender: skip this file (r->s)
    Skip = 5,
    /// Last packet was garbled (???)
    Nak = 6,
    /// Abort batch transfers (???)
    Abort = 7,
    /// Finish session (s<->r)
    Fin = 8,
    /// Resume data trans at this position (r->s)
    RPos = 9,
    /// Data packet(s) follow (s->r)
    Data = 10,
    /// End of file (s->r)
    Eof = 11,
    /// Fatal Read or Write error Detected (?)
    FErr = 12,
    /// Request for file CRC and response (?)
    Crc = 13,
    /// Receiver's Challenge (r->s)
    Challenge = 14,
    /// Request is complete (?)
    Compl = 15,
    /// Other end canned session with CAN*5 (?)
    Can = 16,
    /// Request for free bytes on filesystem (s->r)
    FreeCnt = 17,
    /// Command from sending program (s->r)
    Command = 18,
    /// Output to standard error, data follows (?)
    StdErr = 19,
}

#[derive(PartialEq, Clone, Debug)]
pub struct Header {
    pub frame_type: ZFrameType,
    pub data: [u8; 4],
}

impl Display for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.frame_type {
            ZFrameType::RPos | ZFrameType::Eof | ZFrameType::FreeCnt | ZFrameType::Data => {
                write!(f, "[Header with {:?} number = {}]", self.frame_type, self.number())
            }
            ZFrameType::Crc | ZFrameType::Challenge => write!(f, "[Header with {:?} number = x{:08X}]", self.frame_type, self.number()),
            _ => write!(
                f,
                "[Header with {:?} frame flags = x{:02X}, x{:02X}, x{:02X}, x{:02X}]",
                self.frame_type,
                self.f3(),
                self.f2(),
                self.f1(),
                self.f0()
            ),
        }
    }
}

impl Header {
    pub fn empty(frame_type: ZFrameType) -> Self {
        Self {
            frame_type,
            data: [0, 0, 0, 0],
        }
    }

    pub fn from_flags(frame_type: ZFrameType, f3: u8, f2: u8, f1: u8, f0: u8) -> Self {
        Self {
            frame_type,
            data: [f3, f2, f1, f0],
        }
    }

    pub fn from_number(frame_type: ZFrameType, number: u32) -> Self {
        Self {
            frame_type,
            data: u32::to_le_bytes(number),
        }
    }

    pub fn f0(&self) -> u8 {
        self.data[3]
    }
    pub fn p3(&self) -> u8 {
        self.data[3]
    }

    pub fn f1(&self) -> u8 {
        self.data[2]
    }
    pub fn p2(&self) -> u8 {
        self.data[2]
    }

    pub fn f2(&self) -> u8 {
        self.data[1]
    }
    pub fn p1(&self) -> u8 {
        self.data[1]
    }

    pub fn f3(&self) -> u8 {
        self.data[0]
    }
    pub fn p0(&self) -> u8 {
        self.data[0]
    }

    pub fn number(&self) -> u32 {
        u32::from_le_bytes(self.data)
    }

    pub fn build(&self, header_type: HeaderType, escape_ctrl_chars: bool) -> Vec<u8> {
        let mut res = Vec::new();

        match header_type {
            HeaderType::Bin => {
                res.extend_from_slice(&[ZPAD, ZDLE, ZBIN, self.frame_type as u8]);
                append_zdle_encoded(&mut res, &self.data, escape_ctrl_chars);
                let crc16 = get_crc16_buggy(&res[3..]);
                append_zdle_encoded(&mut res, &u16::to_le_bytes(crc16), escape_ctrl_chars);
            }

            HeaderType::Bin32 => {
                res.extend_from_slice(&[ZPAD, ZDLE, ZBIN32, self.frame_type as u8]);
                append_zdle_encoded(&mut res, &self.data, escape_ctrl_chars);
                let crc32 = get_crc32(&res[3..]);
                append_zdle_encoded(&mut res, &u32::to_le_bytes(crc32), escape_ctrl_chars);
            }

            HeaderType::Hex => {
                res.extend_from_slice(&[ZPAD, ZPAD, ZDLE, ZHEX]);
                let ft = self.frame_type as u8;
                res.push(get_hex((ft >> 4) & 0xF));
                res.push(get_hex(ft & 0xF));

                for f in self.data {
                    res.push(get_hex((f >> 4) & 0xF));
                    res.push(get_hex(f & 0xF));
                }

                let mut crc16 = update_crc16(0, self.frame_type as u8);
                for b in self.data {
                    crc16 = update_crc16(crc16, b);
                }
                res.push(get_hex((crc16 >> 12) as u8 & 0xF));
                res.push(get_hex((crc16 >> 8) as u8 & 0xF));
                res.push(get_hex((crc16 >> 4) as u8 & 0xF));
                res.push(get_hex((crc16 & 0xF) as u8));
                res.push(b'\n'); // only 1 is required, 2 if it starts with \r then windows EOL is expected
                if self.frame_type != ZFrameType::Ack && self.frame_type != ZFrameType::Fin {
                    res.push(XON);
                }
            }
        }
        res
    }

    pub fn write(&self, com: &mut dyn DataConnection, header_type: HeaderType, escape_ctrl_chars: bool) -> TerminalResult<usize> {
        //println!("send header:{:?}  - {:?}", header_type, self);
        com.send(self.build(header_type, escape_ctrl_chars))?;
        Ok(12)
    }

    pub fn get_frame_type(ftype: u8) -> TerminalResult<ZFrameType> {
        match ftype {
            frame_types::ZRQINIT => Ok(ZFrameType::RQInit),
            frame_types::ZRINIT => Ok(ZFrameType::RIinit),
            frame_types::ZSINIT => Ok(ZFrameType::Sinit),
            frame_types::ZACK => Ok(ZFrameType::Ack),
            frame_types::ZFILE => Ok(ZFrameType::File),
            frame_types::ZSKIP => Ok(ZFrameType::Skip),
            frame_types::ZNAK => Ok(ZFrameType::Nak),
            frame_types::ZABORT => Ok(ZFrameType::Abort),
            frame_types::ZFIN => Ok(ZFrameType::Fin),
            frame_types::ZRPOS => Ok(ZFrameType::RPos),
            frame_types::ZDATA => Ok(ZFrameType::Data),
            frame_types::ZEOF => Ok(ZFrameType::Eof),
            frame_types::ZFERR => Ok(ZFrameType::FErr),
            frame_types::ZCRC => Ok(ZFrameType::Crc),
            frame_types::ZCHALLENGE => Ok(ZFrameType::Challenge),
            frame_types::ZCOMPL => Ok(ZFrameType::Compl),
            frame_types::ZCAN => Ok(ZFrameType::Can),
            frame_types::ZFREECNT => Ok(ZFrameType::FreeCnt),
            frame_types::ZCOMMAND => Ok(ZFrameType::Command),
            frame_types::ZSTDERR => Ok(ZFrameType::StdErr),
            _ => Err(TransmissionError::InvalidFrameType(ftype).into()),
        }
    }

    pub fn read(com: &mut dyn DataConnection, can_count: &mut usize) -> TerminalResult<Option<Header>> {
        let zpad = com.read_u8()?;
        if zpad == 0x18 {
            // CAN
            *can_count += 1;
        }
        if zpad != ZPAD {
            return Err(TransmissionError::ZPADExected(zpad).into());
        }
        *can_count = 0;
        let mut next = com.read_u8()?;
        if next == ZPAD {
            next = com.read_u8()?;
        }
        if next != ZDLE {
            return Err(TransmissionError::ZLDEExected(next).into());
        }

        let header_type = com.read_u8()?;
        let header_data_size = match header_type {
            ZBIN => 7,
            ZBIN32 => 9,
            ZHEX => 14,
            _ => {
                return Err(TransmissionError::UnknownHeaderType(header_type).into());
            }
        };

        let header_data = read_zdle_bytes(com, header_data_size)?;
        match header_type {
            ZBIN => {
                let crc16 = get_crc16_buggy(&header_data[0..5]);
                let check_crc16 = u16::from_le_bytes(header_data[5..7].try_into().unwrap());
                if crc16 != check_crc16 {
                    return Err(TransmissionError::CRC16Mismatch(crc16, check_crc16).into());
                }
                Ok(Some(Header {
                    frame_type: Header::get_frame_type(header_data[0])?,
                    data: header_data[1..5].try_into().unwrap(),
                }))
            }
            ZBIN32 => {
                let data = &header_data[0..5];
                let crc32 = get_crc32(data);
                let check_crc32 = u32::from_le_bytes(header_data[5..9].try_into().unwrap());
                if crc32 != check_crc32 {
                    return Err(anyhow::anyhow!("crc32 mismatch got {crc32:08X} expected {check_crc32:08X}"));
                }
                Ok(Some(Header {
                    frame_type: Header::get_frame_type(header_data[0])?,
                    data: header_data[1..5].try_into().unwrap(),
                }))
            }
            ZHEX => {
                let data = [
                    from_hex(header_data[0])? << 4 | from_hex(header_data[1])?,
                    from_hex(header_data[2])? << 4 | from_hex(header_data[3])?,
                    from_hex(header_data[4])? << 4 | from_hex(header_data[5])?,
                    from_hex(header_data[6])? << 4 | from_hex(header_data[7])?,
                    from_hex(header_data[8])? << 4 | from_hex(header_data[9])?,
                ];

                let crc16 = get_crc16_buggy(&data);

                let mut check_crc16 = 0;
                for b in &header_data[10..14] {
                    check_crc16 = check_crc16 << 4 | u16::from(from_hex(*b)?);
                }
                if crc16 != check_crc16 {
                    return Err(TransmissionError::CRC16Mismatch(crc16, check_crc16).into());
                }
                // read rest;
                let eol = com.read_u8()?;
                // don't check the next bytes. Errors there don't impact much
                if eol == b'\r' {
                    com.read_u8()?; // \n windows eol
                }
                if data[0] != ZACK && data[0] != frame_types::ZFIN {
                    com.read_u8()?; // read XON
                }

                Ok(Some(Header {
                    frame_type: Header::get_frame_type(data[0])?,
                    data: data[1..5].try_into().unwrap(),
                }))
            }
            _ => {
                panic!("should never happen");
            }
        }
    }
}
/*
#[cfg(test)]
mod tests {
    use crate::{
        com::TestChannel,
        protocol::{
            zmodem::headers::{Header, HeaderType},
            ZFrameType, ZBIN32, ZDLE, ZHEX, ZPAD,
        },
    };

    #[test]
    fn test_from_number() {
        assert_eq!(
            Header::from_number(HeaderType::Hex, ZFrameType::Fin, 2).build(false),
            vec![
                0x2a, 0x2a, 0x18, 0x42, 0x30, 0x38, 0x30, 0x32, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
                0x65, 0x66, 0x34, 0x35, 0x0a
            ]
        );
    }

    #[test]
    fn test_bin32_header_data() {
        assert_eq!(
            Header::empty(HeaderType::Bin32, ZFrameType::Data).build(false),
            vec![ZPAD, ZDLE, ZBIN32, 0x0A, 0, 0, 0, 0, 0xBC, 0xEF, 0x92, 0x8C]
        );
    }

    #[test]
    fn test_hex_header_data() {
        assert_eq!(
            Header::empty(HeaderType::Hex, ZFrameType::RPos).build(false),
            vec![
                ZPAD, ZPAD, ZDLE, ZHEX, b'0', b'9', b'0', b'0', b'0', b'0', b'0', b'0', b'0', b'0',
                b'a', b'8', b'7', b'c', b'\n', 0x11
            ]
        );

        assert_eq!(
            "**\x18B00000000000000\n\x11".to_string(),
            String::from_utf8(
                Header::from_flags(HeaderType::Hex, ZFrameType::RQInit, 0, 0, 0, 0).build(false)
            )
            .unwrap()
        );
        assert_eq!(
            "**\x18B0100000000aa51\n\x11".to_string(),
            String::from_utf8(
                Header::from_flags(HeaderType::Hex, ZFrameType::RIinit, 0, 0, 0, 0).build(false)
            )
            .unwrap()
        );
        assert_eq!(
            "**\x18B02000000004483\n\x11".to_string(),
            String::from_utf8(
                Header::from_flags(HeaderType::Hex, ZFrameType::Sinit, 0, 0, 0, 0).build(false)
            )
            .unwrap()
        );
        assert_eq!(
            "**\x18B0300000000eed2\n".to_string(),
            String::from_utf8(
                Header::from_flags(HeaderType::Hex, ZFrameType::Ack, 0, 0, 0, 0).build(false)
            )
            .unwrap()
        );
        assert_eq!(
            "**\x18B087e0400003ec2\n".to_string(),
            String::from_utf8(
                Header::from_flags(HeaderType::Hex, ZFrameType::Fin, 126, 4, 0, 0).build(false)
            )
            .unwrap()
        );
    }
    #[test]
    fn test_bin_header() {
        let mut com = TestChannel::new(false);
        let header = Header::from_flags(HeaderType::Bin, ZFrameType::Data, 3, 2, 1, 0);
        let mut i = 0;
        header.write(&mut com.sender, false).expect("err");
        let read_header = Header::read(&mut com.receiver, &mut i).unwrap().unwrap();
        assert_eq!(read_header, header);
    }

    #[test]
    fn test_bin32_header() {
        let mut com = TestChannel::new(false);
        let header = Header::from_flags(HeaderType::Bin32, ZFrameType::Data, 3, 2, 1, 0);
        header.write(&mut com.sender, false).expect("err");
        let mut i = 0;
        let read_header = Header::read(&mut com.receiver, &mut i).unwrap().unwrap();
        assert_eq!(read_header, header);
    }

    #[test]
    fn test_hex_header() {
        let mut com = TestChannel::new(false);
        let header = Header::from_flags(HeaderType::Hex, ZFrameType::Data, 3, 2, 1, 0);
        header.write(&mut com.sender, false).expect("err");
        let mut i = 0;

        let read_header = Header::read(&mut com.receiver, &mut i).unwrap().unwrap();
        assert_eq!(read_header, header);
    }
}
*/
