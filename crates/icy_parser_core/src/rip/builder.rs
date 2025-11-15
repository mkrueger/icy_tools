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

    pub fn _parse_base36_2digit(&mut self, ch: u8, target_idx: usize) -> Result<bool, ()> {
        let digit = parse_base36_digit(ch).ok_or(())?;
        if self.param_state % 2 == 0 {
            if self.i32_params.len() <= target_idx {
                self.i32_params.resize(target_idx + 1, 0);
            }
            self.i32_params[target_idx] = digit;
        } else {
            self.i32_params[target_idx] = self.i32_params[target_idx].wrapping_mul(36).wrapping_add(digit);
        }
        self.param_state += 1;
        Ok(false) // Not done yet
    }

    pub fn parse_base36_complete(&mut self, ch: u8, target_idx: usize, final_state: usize) -> Result<bool, ()> {
        let digit = parse_base36_digit(ch).ok_or(())?;
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
#[inline]
pub fn parse_base36_digit(ch: u8) -> Option<i32> {
    match ch {
        b'0'..=b'9' => Some((ch - b'0') as i32),
        b'A'..=b'Z' => Some((ch - b'A' + 10) as i32),
        b'a'..=b'z' => Some((ch - b'a' + 10) as i32),
        _ => None,
    }
}
