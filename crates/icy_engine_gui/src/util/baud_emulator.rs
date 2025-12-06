use std::time::Instant;

use icy_parser_core::BaudEmulation;

const BITS_PER_BYTE: u32 = 10; // 8 data + 1 start + 1 stop bit

/// Baud rate emulator that controls how many bytes can be processed
/// based on elapsed time, simulating serial line speeds.
#[derive(Clone, Debug)]
pub struct BaudEmulator {
    pub baud_emulation: BaudEmulation,
    /// Baud rate in bits per second (0 = no limit)
    baud_rate: u32,
    /// Last time bytes were sent
    last_send_time: Instant,
}

impl BaudEmulator {
    pub fn new() -> Self {
        Self {
            baud_emulation: BaudEmulation::Off,
            baud_rate: 0,
            last_send_time: Instant::now(),
        }
    }

    pub fn set_baud_rate(&mut self, baud: BaudEmulation) {
        self.baud_rate = baud.get_baud_rate();
        self.baud_emulation = baud;
        self.last_send_time = Instant::now();
    }

    /// Calculate how many bytes can be sent based on elapsed time.
    /// Returns the number of bytes that should be processed from the input data.
    ///
    /// Usage: Process `data[..bytes_to_process]` and call this again for remaining data.
    pub fn calculate_bytes_to_send(&mut self, available_bytes: usize) -> usize {
        // No emulation - process all immediately
        if self.baud_rate == 0 {
            return available_bytes;
        }

        let now = Instant::now();
        let bytes_per_sec = self.baud_rate / BITS_PER_BYTE;
        let elapsed_ms = now.duration_since(self.last_send_time).as_millis() as u32;
        let bytes_allowed = (bytes_per_sec.saturating_mul(elapsed_ms)) / 1000;

        let bytes_to_send = (bytes_allowed as usize).min(available_bytes);

        if bytes_to_send > 0 {
            self.last_send_time = now;
        }

        bytes_to_send
    }

    pub fn reset(&mut self) {
        self.last_send_time = Instant::now();
    }
}
