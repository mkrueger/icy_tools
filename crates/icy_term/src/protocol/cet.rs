use std::{fs::File, io::Write, path::PathBuf, time::Duration};

use async_trait::async_trait;
use icy_net::{
    protocol::{OutputLogMessage, Protocol, TransferState},
    Connection,
};

const TIMEOUT: Duration = Duration::from_secs(5);
const MAX_BLOCK: usize = 40 * 25;
const RETRIES: usize = 3;

struct CetBlock {
    frame: u8,
    block_num: u8,
    block_count: u8,
    ends_file: bool,
    data: Vec<u8>,
}

pub struct CetProtocol {
    download_dir: PathBuf,
    file: Option<File>,
    temp_path: Option<PathBuf>,
    next_frame: u8,
    next_block: u8,
    frames_remaining: u16,
}

impl CetProtocol {
    pub fn new(download_dir: PathBuf) -> Self {
        Self {
            download_dir,
            file: None,
            temp_path: None,
            next_frame: b'A',
            next_block: 0,
            frames_remaining: 0,
        }
    }

    async fn read_byte(connection: &mut dyn Connection) -> icy_net::Result<u8> {
        let mut byte = [0u8; 1];
        tokio::time::timeout(TIMEOUT, connection.read_exact(&mut byte))
            .await
            .map_err(|_| -> Box<dyn std::error::Error + Send + Sync> { "CET receive timeout".into() })??;
        Ok(byte[0])
    }

    async fn send(connection: &mut dyn Connection, bytes: &[u8]) -> icy_net::Result<()> {
        connection.send(bytes).await
    }

    async fn read_block_once(connection: &mut dyn Connection) -> icy_net::Result<CetBlock> {
        loop {
            if Self::read_byte(connection).await? == b'|' && Self::read_byte(connection).await? == b'A' {
                break;
            }
        }

        let mut checksum = b'|' ^ b'A';
        let mut shift: i16 = 0;
        let mut block = CetBlock {
            frame: b'A',
            block_num: 0,
            block_count: 0,
            ends_file: false,
            data: Vec::new(),
        };

        loop {
            if block.data.len() >= MAX_BLOCK {
                return Err("CET block exceeds screen-sized limit".into());
            }
            let byte = Self::read_byte(connection).await?;
            if !(32..=127).contains(&byte) {
                return Err(format!("Illegal CET byte 0x{byte:02X}").into());
            }
            if byte != b'|' {
                checksum ^= byte & 0x7F;
                let decoded = if byte == b'}' {
                    b' '
                } else {
                    if shift != 0 && byte < 64 {
                        return Err("Illegal CET shifted byte".into());
                    }
                    (i16::from(byte) + shift) as u8
                };
                block.data.push(decoded);
                continue;
            }

            let command = Self::read_byte(connection).await?;
            checksum ^= b'|' ^ (command & 0x7F);
            match command {
                b'0' => shift = 0,
                b'1' => shift = -64,
                b'2' => shift = 64,
                b'3' => shift = 96,
                b'4' => shift = 128,
                b'5' => shift = 160,
                b'E' => block.data.push(b'|'),
                b'F' => block.ends_file = true,
                b'L' => block.data.push(b'\n'),
                b'}' => block.data.push(b'}'),
                b'G' => {
                    let frame = Self::read_byte(connection).await?;
                    if !frame.is_ascii_lowercase() {
                        return Err("Invalid CET frame identifier".into());
                    }
                    checksum ^= frame;
                    block.frame = frame;
                    let first = Self::read_byte(connection).await?;
                    if first == b'|' {
                        if Self::read_byte(connection).await? != b'I' {
                            return Err("Invalid CET frame terminator".into());
                        }
                        checksum ^= b'|' ^ b'I';
                    } else {
                        let second = Self::read_byte(connection).await?;
                        if !first.is_ascii_digit() || !second.is_ascii_digit() {
                            return Err("Invalid CET block number".into());
                        }
                        checksum ^= first ^ second;
                        block.block_num = first - b'0';
                        block.block_count = second - b'0';
                        if Self::read_byte(connection).await? != b'|' || Self::read_byte(connection).await? != b'I' {
                            return Err("Missing CET frame terminator".into());
                        }
                        checksum ^= b'|' ^ b'I';
                    }
                }
                b'Z' => {
                    let hundreds = Self::read_byte(connection).await?;
                    let tens = Self::read_byte(connection).await?;
                    let ones = Self::read_byte(connection).await?;
                    if !hundreds.is_ascii_digit() || !tens.is_ascii_digit() || !ones.is_ascii_digit() {
                        return Err("Invalid CET checksum".into());
                    }
                    let expected = (hundreds - b'0') * 100 + (tens - b'0') * 10 + (ones - b'0');
                    if checksum != expected {
                        return Err(format!("CET checksum mismatch: {checksum} != {expected}").into());
                    }
                    return Ok(block);
                }
                b'D' | b'T' => loop {
                    if Self::read_byte(connection).await? == b'|' && Self::read_byte(connection).await? == b'I' {
                        break;
                    }
                },
                _ => {}
            }
        }
    }

    async fn read_block(connection: &mut dyn Connection) -> icy_net::Result<CetBlock> {
        let mut last_error = None;
        for attempt in 0..=RETRIES {
            match Self::read_block_once(connection).await {
                Ok(block) => return Ok(block),
                Err(error) => {
                    last_error = Some(error);
                    if attempt < RETRIES {
                        Self::send(connection, b"*00").await?;
                    }
                }
            }
        }
        Err(last_error.unwrap_or_else(|| "CET receive failed".into()))
    }

    fn sanitized_name(name: &str) -> String {
        std::path::Path::new(name)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("telesoftware.bin")
            .to_string()
    }
}

#[async_trait]
impl Protocol for CetProtocol {
    async fn initiate_recv(&mut self, connection: &mut dyn Connection) -> icy_net::Result<TransferState> {
        let header = Self::read_block(connection).await?;
        let header_text = String::from_utf8_lossy(&header.data);
        let mut lines = header_text.lines();
        let file_name = Self::sanitized_name(lines.next().unwrap_or_default());
        self.frames_remaining = lines
            .next()
            .and_then(|line| line.trim().parse().ok())
            .filter(|count| (1..=999).contains(count))
            .ok_or("Invalid CET frame count")?;

        std::fs::create_dir_all(&self.download_dir)?;
        let temp_path = std::env::temp_dir().join(format!("icy_term_cet_{}_{}", std::process::id(), fastrand::u64(..)));
        self.file = Some(File::create(&temp_path)?);
        self.temp_path = Some(temp_path);

        let mut state = TransferState::new("CET Telesoftware".to_string());
        state.current_state = "Receiving";
        state.recieve_state.file_name = file_name;
        state.recieve_state.file_size = 0;
        state.recieve_state.check_size = format!("{} frames", self.frames_remaining);
        state.recieve_state.output_log.push(OutputLogMessage::Info("CET header received".to_string()));
        Ok(state)
    }

    async fn update_transfer(&mut self, connection: &mut dyn Connection, state: &mut TransferState) -> icy_net::Result<()> {
        if state.request_cancel {
            return self.cancel_transfer(connection).await;
        }
        let request = if self.next_frame == b'z' + 1 {
            self.next_frame = b'a';
            b"0".as_slice()
        } else {
            b"_".as_slice()
        };
        Self::send(connection, request).await?;
        let block = Self::read_block(connection).await?;

        if block.frame != b'A' {
            if self.next_frame == b'A' {
                self.next_frame = block.frame;
            }
            if block.frame != self.next_frame || block.block_num != self.next_block {
                return Err("Out-of-order CET frame or block".into());
            }
        } else if self.next_frame == b'A' {
            self.next_frame = b'a';
        }

        self.file.as_mut().ok_or("CET output file is not open")?.write_all(&block.data)?;
        state.recieve_state.cur_bytes_transfered += block.data.len() as u64;
        state.recieve_state.total_bytes_transfered += block.data.len() as u64;
        if self.frames_remaining != 999 {
            self.frames_remaining = self.frames_remaining.saturating_sub(1);
        }
        state.recieve_state.check_size = format!("{} frames remaining", self.frames_remaining);

        if block.block_num == block.block_count {
            self.next_frame = self.next_frame.saturating_add(1);
            self.next_block = 0;
        } else {
            self.next_block = self.next_block.saturating_add(1);
        }

        if block.ends_file {
            self.file.take();
            let path = self.temp_path.take().ok_or("CET temporary file is missing")?;
            state.recieve_state.finish_file(path);
            state.is_finished = true;
            state.current_state = "Complete";
        }
        Ok(())
    }

    async fn initiate_send(&mut self, _connection: &mut dyn Connection, _files: &[PathBuf]) -> icy_net::Result<TransferState> {
        Err("CET Telesoftware upload is not supported".into())
    }

    async fn cancel_transfer(&mut self, _connection: &mut dyn Connection) -> icy_net::Result<()> {
        self.file.take();
        if let Some(path) = self.temp_path.take() {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::CetProtocol;
    use async_trait::async_trait;
    use icy_net::{protocol::Protocol, Connection, ConnectionState, ConnectionType};
    use std::collections::VecDeque;

    struct MockConnection {
        input: VecDeque<u8>,
        output: Vec<u8>,
    }

    #[async_trait]
    impl Connection for MockConnection {
        fn get_connection_type(&self) -> ConnectionType {
            ConnectionType::Raw
        }
        async fn read(&mut self, buffer: &mut [u8]) -> icy_net::Result<usize> {
            let count = buffer.len().min(self.input.len());
            for slot in &mut buffer[..count] {
                *slot = self.input.pop_front().unwrap();
            }
            Ok(count)
        }
        async fn try_read(&mut self, buffer: &mut [u8]) -> icy_net::Result<usize> {
            self.read(buffer).await
        }
        async fn send(&mut self, buffer: &[u8]) -> icy_net::Result<()> {
            self.output.extend_from_slice(buffer);
            Ok(())
        }
        async fn poll(&mut self) -> icy_net::Result<ConnectionState> {
            Ok(ConnectionState::Connected)
        }
    }

    fn block(payload: &[u8]) -> Vec<u8> {
        let mut checksum = 0u8;
        let mut index = 0;
        let mut started = false;
        while index < payload.len() {
            let byte = payload[index];
            if started {
                checksum ^= byte & 0x7F;
            }
            if byte == b'|' && payload.get(index + 1) == Some(&b'A') {
                started = true;
                checksum ^= b'|' ^ b'A';
                index += 2;
                continue;
            }
            index += 1;
        }
        let mut result = payload.to_vec();
        result.extend_from_slice(format!("{checksum:03}").as_bytes());
        result
    }

    #[test]
    fn sanitizes_remote_file_names() {
        assert_eq!(CetProtocol::sanitized_name("../../secret/file.bin"), "file.bin");
        assert_eq!(CetProtocol::sanitized_name(""), "telesoftware.bin");
    }

    #[tokio::test]
    async fn receives_complete_cet_file() {
        let mut input = block(b"|Ahello.txt|L1|Z");
        input.extend(block(b"|A|Ga00|IHello|F|Z"));
        let mut connection = MockConnection {
            input: input.into(),
            output: Vec::new(),
        };
        let mut protocol = CetProtocol::new(std::env::temp_dir());

        let mut state = protocol.initiate_recv(&mut connection).await.unwrap();
        protocol.update_transfer(&mut connection, &mut state).await.unwrap();

        assert!(state.is_finished);
        assert_eq!(connection.output, b"_");
        let path = &state.recieve_state.finished_files[0].1;
        assert_eq!(std::fs::read(path).unwrap(), b"Hello");
        let _ = std::fs::remove_file(path);
    }
}
