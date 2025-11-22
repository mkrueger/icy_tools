#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LineStyle {
    Solid = 0,
    Dotted = 1,
    Center = 2,
    Dashed = 3,
    User = 4,
}

impl LineStyle {
    const LINE_PATTERNS: [u32; 5] = [
        0xFFFF, // Solid
        0xCCCC, // Dotted
        0xF878, // Center
        0xF8F8, // Dashed
        0xFFFF, // User
    ];

    pub fn get_line_pattern(&self) -> Vec<bool> {
        let offset = (*self as u8) as usize;

        let mut res = Vec::new();
        for i in 0..16 {
            res.push((LineStyle::LINE_PATTERNS[offset] & (1 << i)) != 0);
        }
        res
    }
}

impl TryFrom<i32> for LineStyle {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(LineStyle::Solid),
            1 => Ok(LineStyle::Dotted),
            2 => Ok(LineStyle::Center),
            3 => Ok(LineStyle::Dashed),
            4 => Ok(LineStyle::User),
            _ => Err(format!("Invalid LineStyle value: {}", value)),
        }
    }
}
