//! RIP command builder and parsing helpers.
//!
//! Performance optimization history:
//! - 2025-11-15: Replaced match-based parse_base36_digit with LUT for ~2-3x speedup

#[derive(Default)]
pub struct CommandBuilder {
    pub cmd_char: u8,
    pub level: u8,
    pub param_state: usize,
    pub npoints: i32,

    // Reusable buffers for command parameters
    pub i32_params: Vec<i32>,
    pub string_param: String,
    pub char_param: u8,
    pub got_escape: bool,
}

impl CommandBuilder {
    pub fn reset(&mut self) {
        self.cmd_char = 0;
        self.level = 0;
        self.param_state = 0;
        self.npoints = 0;
        self.i32_params.clear();
        self.string_param.clear();
        self.char_param = 0;
        self.got_escape = false;
    }

    pub fn parse_base36_complete(&mut self, ch: u8, target_idx: usize, final_state: usize) -> Result<bool, ()> {
        let digit = BASE36_LUT[ch as usize].ok_or(())?;
        if self.param_state % 2 == 0 {
            if self.i32_params.len() <= target_idx {
                self.i32_params.resize(target_idx + 1, 0);
            }
            self.i32_params[target_idx] = digit;
        } else {
            self.i32_params[target_idx] = self.i32_params[target_idx].wrapping_mul(36).wrapping_add(digit);
        }
        self.param_state += 1;
        Ok(self.param_state > final_state)
    }
}

/// Helper function to parse a base-36 character into a digit
/// Optimized with a lookup table for maximum performance
// Lookup table: 256 entries, invalid chars map to -1
// Valid: 0-9 (0x30-0x39) -> 0-9, A-Z (0x41-0x5A) -> 10-35, a-z (0x61-0x7A) -> 10-35
pub(crate) static BASE36_LUT: [Option<i32>; 256] = {
    let mut table = [None; 256];
    let mut i = 0;
    // '0' to '9' (0x30-0x39)
    while i < 10 {
        table[(b'0' + i) as usize] = Some(i as i32);
        i += 1;
    }
    // 'A' to 'Z' (0x41-0x5A)
    i = 0;
    while i < 26 {
        table[(b'A' + i) as usize] = Some((10 + i) as i32);
        i += 1;
    }
    // 'a' to 'z' (0x61-0x7A)
    i = 0;
    while i < 26 {
        table[(b'a' + i) as usize] = Some((10 + i) as i32);
        i += 1;
    }
    table
};
