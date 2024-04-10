#![allow(dead_code, clippy::wildcard_imports, clippy::needless_range_loop)]

// IEMSI autologin implementation http://ftsc.org/docs/fsc-0056.001
use std::fmt;

use icy_engine::{get_crc16, get_crc32, update_crc32};

use crate::{
    ui::connect::{Connection, DataConnection},
    IEMSISettings, TerminalResult, VERSION,
};

/// EMSI Inquiry is transmitted by the calling system to identify it as
/// EMSI capable. If an `EMSI_REQ` sequence is received in response, it is
/// safe to assume the answering system to be EMSI capable.
pub const EMSI_INQ: &[u8; 15] = b"**EMSI_INQC816\r";

/// EMSI Request is transmitted by the answering system in response to an
/// EMSI Inquiry sequence. It should also be transmitted prior to or
/// immediately following the answering system has identified itself by
/// transmitting its program name and/or banner. If the calling system
/// receives an EMSI Request sequence, it can safely assume that the
/// answering system is EMSI capable.
pub const EMSI_REQ: &[u8; 15] = b"**EMSI_REQA77E\r";

/// EMSI Client is used by terminal emulation software to force a mailer
/// front-end to bypass any unnecessary mail session negotiation and
/// treat the call as an incoming human caller. The `EMSI_CLI` sequence may
/// not be issued by any software attempting to establish a mail session
/// between two systems and must only be acted upon by an answering
/// system.
pub const EMSI_CLI: &[u8; 15] = b"**EMSI_CLIFA8C\r";

/// EMSI Heartbeat is used to prevent unnecessary timeouts from occurring
/// while attempting to handshake. It is most commonly used when the
/// answering system turns around to transmit its `EMSI_DAT` packet. It is
/// quite normal that any of the timers of the calling system (which at
/// this stage is waiting for an `EMSI_DAT` packet) expires while the
/// answering system is processing the recently received `EMSI_DAT` packet.
pub const EMSI_HBT: &[u8; 15] = b"**EMSI_HBTEAEE\r";

/// EMSI ACK is transmitted by either system as a positive
/// acknowledgement of the valid receipt of a `EMSI_DAT` packet. This should
/// only be used as a response to `EMSI_DAT` and not any other packet.
/// Redundant `EMSI_ACK` sequences should be ignored.
pub const EMSI_ACK: &[u8; 15] = b"**EMSI_ACKA490\r";
pub const EMSI_2ACK: &[u8; 30] = b"**EMSI_ACKA490\r**EMSI_ACKA490\r";

/// EMSI NAK is transmitted by either system as a negative
/// acknowledgement of the valid receipt of a `EMSI_DAT` packet. This
/// should only be used as a response to `EMSI_DAT` and not any other
/// packet. Redundant `EMSI_NAK` packets should be ignored.
pub const EMSI_NAK: &[u8; 15] = b"**EMSI_NAKEEC3\r";

/// Similar to `EMSI_REQ` which is used by mailer software to negotiate a
/// mail session. IRQ identifies the Server as being capable of
/// negotiating an IEMSI session. When the Client detects an IRQ sequence
/// in its inbound data stream, it attempts to negotiate an IEMSI
/// session.
pub const EMSI_IRQ: &[u8; 15] = b"**EMSI_IRQ8E08\r";

/// The IIR (Interactive Interrupt Request) sequence is used by either
/// Client or Server to abort the current negotiation. This could be
/// during the initial IEMSI handshake or during other interactions
/// between the Client and the Server.
pub const EMSI_IIR: &[u8; 15] = b"**EMSI_IIR61E2\r";

/// The CHT sequence is used by the Server to instruct the Client
/// software to enter its full-screen conversation mode function (CHAT).
/// Whether or not the Client software supports this is indicated in the
/// ICI packet.
///
/// If the Server transmits this sequence to the Client, it must wait for
/// an `EMSI_ACK` prior to engaging its conversation mode. If no `EMSI_ACK`
/// sequence is received with ten seconds, it is safe to assume that the
/// Client does not support `EMSI_CHT`. If, however, an `EMSI_NAK` sequence
/// is received from the Client, the Server must re-transmit the
/// `EMSI_CHT` sequence. Once the on-line conversation function has been
/// sucessfully activated, the Server must not echo any received
/// characters back to the Client.
pub const EMSI_CHT: &[u8; 15] = b"**EMSI_CHTF5D4\r";

/// The TCH sequence is used by the Server to instruct the Client
/// software to terminate its full-screen conversation mode function
/// (CHAT).
///
/// If the Server transmits this sequence to the Client, it must wait for
/// an `EMSI_ACK` prior to leaving its conversation mode. If no `EMSI_ACK`
/// sequence is received with ten seconds, a second `EMSI_TCH` sequence
/// should be issued before the Server resumes operation. If, however, an
/// `EMSI_NAK` sequence is received from the Client, the Server must
/// re-transmit the `EMSI_TCH` sequence.
pub const EMSI_TCH: &[u8; 15] = b"**EMSI_TCH3C60\r";

pub struct EmsiDAT {
    pub system_address_list: String,
    pub password: String,
    pub link_codes: String,
    pub compatibility_codes: String,
    pub mailer_product_code: String,
    pub mailer_name: String,
    pub mailer_version: String,
    pub mailer_serial_number: String,
    pub extra_field: Vec<String>,
}

impl std::fmt::Display for EmsiDAT {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::fmt::Result {
        let v = self.encode();
        write!(f, "{}", std::str::from_utf8(&v).unwrap())
    }
}

impl EmsiDAT {
    pub fn new() -> Self {
        EmsiDAT {
            system_address_list: String::new(),
            password: String::new(),
            link_codes: String::new(),
            compatibility_codes: String::new(),
            mailer_product_code: String::new(),
            mailer_name: String::new(),
            mailer_version: String::new(),
            mailer_serial_number: String::new(),
            extra_field: Vec::new(),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let data = format!(
            "{{EMSI}}{{{}}}{{{}}}{{{}}}{{{}}}{{{}}}{{{}}}{{{}}}{{{}}}",
            self.system_address_list,
            self.password,
            self.link_codes,
            self.compatibility_codes,
            self.mailer_product_code,
            self.mailer_name,
            self.mailer_version,
            self.mailer_serial_number
        );

        // todo: etxra fields - are they even used ?

        let block = format!("EMSI_DAT{:04X}{}", data.len(), data);
        let mut result = Vec::new();
        result.extend_from_slice(b"**EMSI_DAT");
        let bytes = block.as_bytes();
        result.extend_from_slice(bytes);
        result.extend_from_slice(get_crc16string(bytes).as_bytes());
        result.push(b'\r');
        result
    }
}

/// The ICI packet is used by the Client to transmit its configuration
/// and Server-related information to the Server. It contains Server
/// parameters, Client options, and Client capabilities.
/// Note that the information in the `EMSI_ICI` packet may not exceed 2,048 bytes.
pub struct EmsiICI {
    ///  The name of the user (Client). This must be treated case insensitively by the Server.
    pub name: String,

    ///  The alias (AKA) of the user (Client). This must be treated case insensitively by the Server.
    pub alias: String,

    /// The geographical location of the user, ie. Stockholm, Sweden.
    pub location: String,

    /// Unformatted data and voice telephone numbers of the user. Unformatted
    /// is defined as the full telephone number, including country and local
    /// area code. Eg. 46-8-90510 is a telephone number in Stockholm, Sweden.
    pub data_telephone: String,

    /// Unformatted data and voice telephone numbers of the user. Unformatted
    /// is defined as the full telephone number, including country and local
    /// area code. Eg. 46-8-90510 is a telephone number in Stockholm, Sweden.
    pub voice_telephone: String,

    /// The password for the user. This must be treated case insensitively by the Server.
    pub password: String,

    /// Hexadecimal string representing a long integer containing the birth-date of the user in UNIX notation (number of seconds since midnight,
    /// Jan 1 1970). This must be treated case insensitively by the Server
    pub birthdate: String,

    /// Consisting of four sub-fields separated by commas, this field
    /// contains from left to right: The requested terminal emulation
    /// protocol, the number of rows of the user's CRT, the number of columns
    /// of the user's CRT, and the number of ASCII NUL (00H) characters the
    /// user's software requires to be transmitted between each line of text.
    ///
    /// The following terminal emulation protocols are defined:
    ///
    ///  AVT0    AVATAR/0+. Used in conjunction with ANSI. If AVT0 is
    ///          specified by the Client, support for ANSI X3.64 emulation
    ///          should be assumed to be present.
    ///  ANSI    ANSI X3.64
    ///  VT52    DEC VT52
    ///  VT100   DEC VT100
    ///  TTY     No terminal emulation, also referred to as RAW mode.
    pub crtdef: String,

    /// The file transfer protocol option specifies the preferred method of
    /// transferring files between the Client and the Server in either
    /// direction. The Client presents all transfer protocols it is capable
    /// of supporting and the Server chooses the most appropriate protocol.
    ///
    ///     DZA*    DirectZAP (Zmodem variant)
    ///     ZAP     ZedZap (Zmodem variant)
    ///     ZMO     Zmodem w/1,024 byte data packets
    ///     SLK     SEAlink
    ///     KER     Kermit
    ///
    /// (*) DirectZAP is a variant of ZedZap. The difference is that the
    /// transmitter only escapes CAN (18H). It is not recommended to use the
    /// DirectZAP protocol when the Client and the Server are connected via a
    /// packet switching network, or via another layer sensitive to control
    /// characters such as XON and XOFF.
    pub protocols: String,

    /// The capabilities of the user's software. If more than one capability
    /// is listed, each capability is separated by a comma.
    /// The following capability codes are defined:
    ///     CHT     Can do full-screen on-line conversation (CHAT).
    ///     MNU     Can do ASCII image download (see ISM packet).
    ///     TAB     Can handle TAB (ASCII 09H) characters.
    ///     ASCII8  Can handle 8-bit IBM PC ASCII characters.
    pub capabilities: String,

    /// The requests field specifies what the user wishes to do once the
    /// initial IEMSI negotiation has been successfully completed. If more
    /// than one capability is listed, each capability is separated by a
    /// comma.
    ///
    /// The following request codes are defined:
    ///     NEWS    Show bulletins, announcements, etc.
    ///     MAIL    Check for new mail.
    ///     FILE    Check for new files.
    ///     HOT     Hot-Keys.
    ///     CLR     Screen clearing.
    ///     HUSH    Do not disturb.
    ///     MORE    Page pausing, often referred to as "More".
    ///     FSED*   Full-screen editor.
    ///     XPRS    <reserved>.
    /// (*) Note that this allows the Client to request use of a full-screen
    /// editor without requiring that it also supports a full-screen terminal
    /// emulation protocol.
    pub requests: String,

    /// The name, version number, and optionally the serial number of the
    /// user's software. Eg. {FrontDoor,2.00,AE000001}.
    pub software: String,

    /// Used for character translation between the Server and the Client.
    /// This field has not been completely defined yet and should always be
    /// transmitted as {} (empty).
    pub xlattabl: String,
}

impl std::fmt::Display for EmsiICI {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::fmt::Result {
        let v = self.encode().unwrap();
        write!(f, "{}", std::str::from_utf8(&v).unwrap())
    }
}

impl EmsiICI {
    const MAX_SIZE: usize = 2048;

    pub fn new() -> Self {
        EmsiICI {
            name: String::new(),
            alias: String::new(),
            location: ".........".to_string(),
            data_telephone: "-Unpublished-".to_string(),
            voice_telephone: "-Unpublished-".to_string(),
            password: String::new(),
            birthdate: String::new(),
            crtdef: "ANSI,24,80,0".to_string(),
            protocols: "ZAP,ZMO,KER".to_string(),
            capabilities: "CHT,TAB,ASCII8".to_string(),
            requests: "HOT,MORE,FSED,NEWS,CLR".to_string(),
            software: format!("-Icy-Term-,{},egui", *VERSION),
            xlattabl: String::new(),
        }
    }

    pub fn encode(&self) -> TerminalResult<Vec<u8>> {
        // **EMSI_ICI<len><data><crc32><CR>
        let data = encode_emsi(&[
            &self.name,
            &self.alias,
            &self.location,
            &self.data_telephone,
            &self.voice_telephone,
            &self.password,
            &self.birthdate,
            &self.crtdef,
            &self.protocols,
            &self.capabilities,
            &self.requests,
            &self.software,
            &self.xlattabl,
        ])?;

        if data.len() > EmsiICI::MAX_SIZE {
            return Err(anyhow::anyhow!("maximum size exceeded"));
        }
        let mut result = Vec::new();
        result.extend_from_slice(b"**EMSI_ICI");
        result.extend_from_slice(get_length_string(data.len()).as_bytes());
        result.extend_from_slice(&data);
        result.extend_from_slice(get_crc32string(&result[2..]).as_bytes());
        result.push(b'\r');
        // need to send 2*ACK for the ici to be recognized - see the spec
        result.extend_from_slice(EMSI_2ACK);
        Ok(result)
    }
}

pub fn get_crc32string(block: &[u8]) -> String {
    let crc = get_crc32(block);
    format!("{:08X}", !crc)
}

pub fn get_crc16string(block: &[u8]) -> String {
    let crc = get_crc16(block);
    format!("{crc:04X}")
}

pub fn get_length_string(len: usize) -> String {
    format!("{len:04X}")
}

/// The ISI packet is used by the Server to transmit its configuration
/// and Client-related information to the Client. It contains Server data
/// and capabilities.
#[derive(Clone)]
pub struct EmsiISI {
    /// The name, version number, and optionally the serial number of the
    /// Server software. Eg. {RemoteAccess,1.10/b5,CS000001}.
    pub id: String,
    /// The name of the Server system. Eg. {Advanced Engineering S.A.R.L.}.
    pub name: String,
    /// The geographical location of the user, ie. Stockholm, Sweden.
    pub location: String,
    /// The name of the primary operator of the Server software. Eg. {Joaquim H. Homrighausen}.
    pub operator: String,
    /// Hexadecimal string representing a long integer containing the current
    /// time of the Server in UNIX notation (number of seconds since midnight,
    /// Jan 1 1970). This must be treated case insensitively by the Client.
    pub localtime: String,
    /// May contain copyright notices, system information, etc. This field may optionally be displayed by the Client.
    pub notice: String,
    /// A single character used by the Server to indicate that the user
    /// has to press the <Enter> key to resume operation. This is used in
    /// conjunction with ASCII Image Downloads (see ISM packet).
    pub wait: String,
    /// The capabilities of the Server software. No Server software
    /// capabilities have currently been defined.
    pub capabilities: String,
}

/// The ISM packet is used to transfer ASCII images from the Server to
/// the Client. These images can then be recalled by the Client when
/// the Server needs to display a previously displayed image.
/// This will be further described in future revisions of this document.
/// SPOILER: There will me no future revisions :)
pub fn _encode_ism(data: &[u8]) -> Vec<u8> {
    let mut block = Vec::new();
    block.extend_from_slice(format!("EMSI_ISM{:X}", data.len()).as_bytes());
    block.extend_from_slice(data);
    let crc = get_crc16(&block);

    let mut result = Vec::new();
    result.extend_from_slice(b"**");
    result.extend_from_slice(&block);
    result.push((crc >> 8) as u8);
    result.push(u8::try_from(crc & 0xFF).unwrap());
    result.push(b'\r');
    result
}

#[derive(Default)]
pub struct IEmsi {
    irq_requested: bool,
    nak_requested: bool,
    pub retries: usize,
    pub isi: Option<EmsiISI>,

    stars_read: i32,
    irq_seq: usize,
    isi_seq: usize,
    nak_seq: usize,
    isi_len: usize,
    isi_crc: usize,
    isi_check_crc: u32,
    pub got_invalid_isi: bool,
    isi_data: Vec<u8>,

    pub settings: IEMSISettings,
    pub aborted: bool,
    logged_in: bool,
}

// **EMSI_ISI<len><data><crc32><CR>
const ISI_START: &[u8; 8] = b"EMSI_ISI";

impl IEmsi {
    pub fn parse_char(&mut self, ch: u8) -> TerminalResult<bool> {
        if self.stars_read >= 2 {
            if self.isi_seq > 7 {
                match self.isi_seq {
                    8..=11 => {
                        self.isi_check_crc = update_crc32(self.isi_check_crc, ch);
                        self.isi_len = self.isi_len * 16 + get_value(ch);
                        self.isi_seq += 1;
                        return Ok(false);
                    }
                    12.. => {
                        if self.isi_seq < self.isi_len + 12 {
                            // Read data
                            self.isi_check_crc = update_crc32(self.isi_check_crc, ch);
                            self.isi_data.push(ch);
                        } else if self.isi_seq < self.isi_len + 12 + 8 {
                            // Read CRC
                            self.isi_crc = self.isi_crc * 16 + get_value(ch);
                        } else if self.isi_seq >= self.isi_len + 12 + 8 {
                            // end - should be marked with b'\r'
                            if ch == b'\r' {
                                if self.isi_crc == self.isi_check_crc as usize {
                                    let group = parse_emsi_blocks(&self.isi_data)?;
                                    if group.len() == 8 {
                                        // valid ISI !!!
                                        self.isi = Some(EmsiISI {
                                            id: group[0].clone(),
                                            name: group[1].clone(),
                                            location: group[2].clone(),
                                            operator: group[3].clone(),
                                            localtime: group[4].clone(),
                                            notice: group[5].clone(),
                                            wait: group[6].clone(),
                                            capabilities: group[7].clone(),
                                        });
                                        self.stars_read = 0;
                                        self.reset_sequences();
                                        return Ok(true);
                                    }
                                    self.got_invalid_isi = true;
                                } else {
                                    self.got_invalid_isi = true;
                                }
                            }
                            self.stars_read = 0;
                            self.reset_sequences();
                        }
                        self.isi_seq += 1;
                        return Ok(false);
                    }
                    _ => {}
                }
                return Ok(false);
            }
            let mut got_seq = false;

            if ch == ISI_START[self.isi_seq] {
                self.isi_check_crc = update_crc32(self.isi_check_crc, ch);
                self.isi_seq += 1;
                self.isi_len = 0;
                got_seq = true;
            } else {
                self.isi_seq = 0;
            }

            if ch == EMSI_NAK[2 + self.nak_seq] {
                self.nak_seq += 1;
                if self.nak_seq + 2 >= EMSI_IRQ.len() {
                    self.nak_requested = true;
                    self.stars_read = 0;
                    self.reset_sequences();
                }
                got_seq = true;
            } else {
                self.nak_seq = 0;
            }

            if ch == EMSI_IRQ[2 + self.irq_seq] {
                self.irq_seq += 1;
                if self.irq_seq + 2 >= EMSI_NAK.len() {
                    self.irq_requested = true;
                    self.stars_read = 0;
                    self.reset_sequences();
                }
                got_seq = true;
            } else {
                self.irq_seq = 0;
            }

            if got_seq {
                return Ok(false);
            }
            self.stars_read = 0;
            self.reset_sequences();
        }

        if ch == b'*' {
            self.stars_read += 1;
            self.reset_sequences();
            return Ok(false);
        }
        self.stars_read = 0;

        Ok(false)
    }

    pub fn try_login(&mut self, con: &mut Connection, user_name: &str, password: &str, ch: u8) -> TerminalResult<bool> {
        if self.aborted {
            return Ok(false);
        }
        if let Some(data) = self.advance_char(user_name, password, ch)? {
            if con.is_connected() {
                con.send(data)?;
            }
        }
        Ok(self.logged_in)
    }

    pub fn advance_char(&mut self, user_name: &str, password: &str, ch: u8) -> TerminalResult<Option<Vec<u8>>> {
        if self.aborted {
            return Ok(None);
        }
        self.parse_char(ch)?;
        if self.irq_requested {
            self.irq_requested = false;
            // self.log_file.push("Starting IEMSI negotiation…".to_string());
            let data = create_iemsi_ici(user_name, password, &self.settings);
            return Ok(Some(data.encode()?));
        } else if let Some(_isi) = &self.isi {
            // self.log_file.push("Receiving valid IEMSI server info…".to_string());
            // self.log_file.push(format!("Name:{} Location:{} Operator:{} Notice:{} System:{}", isi.name, isi.location, isi.operator, isi.notice, isi.id));
            self.aborted = true;
            self.logged_in = true;
            return Ok(Some(EMSI_2ACK.to_vec()));
        } else if self.got_invalid_isi {
            self.got_invalid_isi = false;
            // self.log_file.push("Got invalid IEMSI server info…".to_string());
            self.aborted = true;
            self.logged_in = true;
            return Ok(Some(EMSI_2ACK.to_vec()));
        } else if self.nak_requested {
            self.nak_requested = false;
            if self.retries < 2 {
                // self.log_file.push("IEMSI retry…".to_string());
                let data = create_iemsi_ici(user_name, password, &self.settings);
                self.retries += 1;
                return Ok(Some(data.encode()?));
            }
            // self.log_file.push("IEMSI aborted…".to_string());
            self.aborted = true;
            return Ok(Some(EMSI_IIR.to_vec()));
        }
        Ok(None)
    }

    fn reset_sequences(&mut self) {
        self.irq_seq = 0;
        self.nak_seq = 0;
        self.isi_seq = 0;
        self.isi_crc = 0;
        self.isi_check_crc = 0xFFFF_FFFF;
        self.isi_data.clear();
    }
}

fn create_iemsi_ici(user_name: &str, password: &str, settings: &IEMSISettings) -> EmsiICI {
    let mut data = EmsiICI::new();
    data.name = user_name.to_string();
    data.password = password.to_string();
    data.location = settings.location.clone();
    data.alias = settings.alias.clone();
    data.data_telephone = settings.data_phone.clone();
    data.voice_telephone = settings.voice_phone.clone();
    data.birthdate = settings.birth_date.clone();
    data
}

fn get_value(ch: u8) -> usize {
    let res = match ch {
        b'0'..=b'9' => ch - b'0',
        b'a'..=b'f' => 10 + ch - b'a',
        b'A'..=b'F' => 10 + ch - b'A',
        _ => 0,
    };
    res as usize
}

fn parse_emsi_blocks(data: &[u8]) -> TerminalResult<Vec<String>> {
    let mut res = Vec::new();
    let mut i = 0;
    let mut str = String::new();
    let mut in_string = false;

    while i < data.len() {
        if data[i] == b'}' {
            if i + 1 < data.len() && data[i + 1] == b'}' {
                str.push('}');
                i += 2;
                continue;
            }
            i += 1;
            res.push(str.clone());
            str.clear();
            in_string = false;
            continue;
        }

        if data[i] == b'{' && !in_string {
            in_string = true;
            i += 1;
            continue;
        }

        if data[i] == b'\\' {
            if i + 1 < data.len() && data[i + 1] == b'\\' {
                str.push('\\');
                i += 2;
                continue;
            }
            if i + 2 < data.len() {
                let b = u32::try_from(get_value(data[i + 1]) * 16 + get_value(data[i + 2])).unwrap();
                str.push(char::from_u32(b).unwrap());
                i += 3;
                continue;
            }
            return Err(anyhow::anyhow!("Escape char in emsi string invalid."));
        }

        str.push(char::from_u32(u32::from(data[i])).unwrap());
        i += 1;
    }
    Ok(res)
}

fn get_hex(n: u32) -> u8 {
    if n < 10 {
        return b'0' + u8::try_from(n).unwrap();
    }
    b'A' + u8::try_from(n - 10).unwrap()
}

fn encode_emsi(data: &[&str]) -> TerminalResult<Vec<u8>> {
    let mut res = Vec::new();
    for i in 0..data.len() {
        let d = data[i];
        res.push(b'{');
        for ch in d.chars() {
            if ch == '}' {
                res.extend_from_slice(b"}}");
                continue;
            }
            if ch == '\\' {
                res.extend_from_slice(b"\\\\");
                continue;
            }
            let val = ch as u32;
            if val > 255 {
                return Err(anyhow::anyhow!("Unicode chars not supported"));
            }
            // control codes.
            if val < 32 || val == 127 {
                res.push(b'\\');
                res.push(get_hex((val >> 4) & 0xF));
                res.push(get_hex(val & 0xF));
                continue;
            }

            res.push((val & 0xFF) as u8);
        }
        res.push(b'}');
    }

    Ok(res)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use crate::Address;

    use super::*;

    #[test]
    fn test_iemsi_irq() {
        let mut state = IEmsi::default();
        state.parse_char(b'b').ok();
        assert!(!state.irq_requested);
        for b in EMSI_IRQ {
            state.parse_char(*b).ok();
        }
        assert!(state.irq_requested);
    }

    #[test]
    fn test_iemsi_nak() {
        let mut state = IEmsi::default();
        assert!(!state.nak_requested);
        for b in EMSI_IRQ {
            state.parse_char(*b).ok();
        }
        assert!(!state.nak_requested);
        assert!(state.irq_requested);
        for b in EMSI_NAK {
            state.parse_char(*b).ok();
        }
        assert!(state.nak_requested);
    }

    #[test]
    fn test_iemsi_isi() {
        let mut state = IEmsi::default();
        let data = b"<garbage>**EMSI_ISI0080{RemoteAccess,2.62.1,1161}{bbs}{Canada, eh!}{sysop}{63555308}{Copyright 1989-2000 Bruce F. Morse, All Rights Reserved}{\\01}{ZAP}4675DB04\r<garbage>";
        for b in data {
            state.parse_char(*b).ok();
        }
        assert!(state.isi.is_some());
        let isi = state.isi.unwrap();
        assert_eq!("RemoteAccess,2.62.1,1161", isi.id);
        assert_eq!("bbs", isi.name);
        assert_eq!("Canada, eh!", isi.location);
        assert_eq!("sysop", isi.operator);
        assert_eq!("63555308", isi.localtime);
        assert_eq!("Copyright 1989-2000 Bruce F. Morse, All Rights Reserved", isi.notice);
        assert_eq!("\x01", isi.wait);
        assert_eq!("ZAP", isi.capabilities);
    }

    #[test]
    fn test_parse_emsi_blocks() {
        let blocks = parse_emsi_blocks(b"{foo}{bar}").unwrap();
        assert_eq!(2, blocks.len());
        assert_eq!("foo", blocks[0]);
        assert_eq!("bar", blocks[1]);
    }

    #[test]
    fn test_parse_emsi_blocks_complex() {
        let blocks = parse_emsi_blocks(b"{f{o}}o}{b\\0da\\45r}").unwrap();
        assert_eq!(2, blocks.len());
        assert_eq!("f{o}o", blocks[0]);
        assert_eq!("b\ra\x45r", blocks[1]);
    }

    #[test]
    fn test_encode_emsi() {
        let enc = encode_emsi(&["foo", "bar"]).unwrap();
        assert_eq!(b"{foo}{bar}", enc.as_slice());
    }

    #[test]
    fn test_encode_emsi_complex() {
        let enc = encode_emsi(&["f{o}o", "b\ra\x7Fr"]).unwrap();
        assert_eq!(b"{f{o}}o}{b\\0Da\\7Fr}", enc.as_slice());
    }

    #[test]
    fn test_correct_crc() {
        assert_eq!("C816", get_crc16string(b"EMSI_INQ"));
        assert_eq!("9F361295", get_crc32string(b"EMSI_INQ"));
    }

    #[test]
    fn test_emsi_ici_encoding() {
        let ici = EmsiICI {
            name: "fooboar".to_string(),
            alias: "foo".to_string(),
            location: "Unit test".to_string(),
            data_telephone: "-Unpublished-".to_string(),
            voice_telephone: "-Unpublished-".to_string(),
            password: "bar".to_string(),
            birthdate: String::new(),
            crtdef: "ANSI,24,80,0".to_string(),
            protocols: "ZAP,ZMO,KER".to_string(),
            capabilities: "CHT,TAB,ASCII8".to_string(),
            requests: "HOT,MORE,FSED,NEWS,CLR".to_string(),
            software: "Rust".to_string(),
            xlattabl: String::new(),
        };
        let result = ici.encode().unwrap();
        assert_eq!("**EMSI_ICI0089{fooboar}{foo}{Unit test}{-Unpublished-}{-Unpublished-}{bar}{}{ANSI,24,80,0}{ZAP,ZMO,KER}{CHT,TAB,ASCII8}{HOT,MORE,FSED,NEWS,CLR}{Rust}{}29535C6F\r**EMSI_ACKA490\r**EMSI_ACKA490\r", std::str::from_utf8(&result).unwrap());
    }

    #[test]
    fn test_auto_logon() {
        let mut state = IEmsi::default();
        let mut adr = Address::new(String::new());
        adr.user_name = "foo".to_string();
        adr.password = "bar".to_string();

        let mut opt = IEMSISettings::default();
        opt.data_phone = "data_phone".to_string();
        opt.voice_phone = "voice_phone".to_string();
        opt.alias = "alias".to_string();
        opt.location = "location".to_string();
        opt.birth_date = "12-30-1976".to_string();
        state.settings = opt;

        let mut back_data = Vec::new();
        for b in EMSI_IRQ {
            if let Some(data) = state.advance_char(&adr.user_name, &adr.password, *b).unwrap() {
                back_data = data;
            }
        }
        let data_str = format!("EMSI_ICI009C{{foo}}{{alias}}{{location}}{{data_phone}}{{voice_phone}}{{bar}}{{12-30-1976}}{{ANSI,24,80,0}}{{ZAP,ZMO,KER}}{{CHT,TAB,ASCII8}}{{HOT,MORE,FSED,NEWS,CLR}}{{-Icy-Term-,{},egui}}{{}}", *VERSION);
        let data = data_str.as_bytes().to_vec();
        assert_eq!(format!("**EMSI_ICI009C{{foo}}{{alias}}{{location}}{{data_phone}}{{voice_phone}}{{bar}}{{12-30-1976}}{{ANSI,24,80,0}}{{ZAP,ZMO,KER}}{{CHT,TAB,ASCII8}}{{HOT,MORE,FSED,NEWS,CLR}}{{-Icy-Term-,{},egui}}{{}}{}\r**EMSI_ACKA490\r**EMSI_ACKA490\r", *VERSION, get_crc32string(&data)), String::from_utf8(back_data).unwrap());
    }
}
