use std::time::{Duration, Instant};

use icy_parser_core::BaudEmulation;

#[derive(Clone, Debug)]
pub struct BaudEmulator {
    pub baud_emulation: BaudEmulation,
    last_byte_time: Instant,
    bytes_per_second: f64,
    buffer: Vec<u8>,
}

impl BaudEmulator {
    pub fn new() -> Self {
        Self {
            baud_emulation: BaudEmulation::Off,
            last_byte_time: Instant::now(),
            bytes_per_second: 0.0,
            buffer: Vec::new(),
        }
    }

    pub fn set_baud_rate(&mut self, baud: BaudEmulation) {
        self.bytes_per_second = match baud {
            BaudEmulation::Off => 0.0,                     // No limit
            BaudEmulation::Rate(bps) => bps as f64 / 10.0, // Convert bits to bytes (8 data + 1 start + 1 stop bit)
        };
        self.baud_emulation = baud;
    }

    pub fn emulate(&mut self, data: Vec<u8>) -> Vec<u8> {
        if self.bytes_per_second == 0.0 {
            // No emulation, return all data immediately
            return data;
        }

        // Add to buffer
        self.buffer.extend_from_slice(&data);

        // Calculate how many bytes we can send based on elapsed time
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_byte_time);
        let elapsed_secs = elapsed.as_secs_f64();

        let bytes_allowed = (elapsed_secs * self.bytes_per_second) as usize;

        if bytes_allowed == 0 {
            // Not enough time has passed to send any bytes
            return Vec::new();
        }

        // Take the allowed bytes from the buffer
        let bytes_to_send = bytes_allowed.min(self.buffer.len());
        let result: Vec<u8> = self.buffer.drain(..bytes_to_send).collect();

        // Update the last byte time based on how many bytes we sent
        if bytes_to_send > 0 {
            let time_consumed = Duration::from_secs_f64(bytes_to_send as f64 / self.bytes_per_second);
            self.last_byte_time = self.last_byte_time + time_consumed;
        }

        result
    }

    pub fn has_buffered_data(&self) -> bool {
        !self.buffer.is_empty()
    }
}
