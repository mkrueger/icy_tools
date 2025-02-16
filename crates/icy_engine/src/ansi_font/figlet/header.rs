use regex::Regex;
use std::io::{BufReader, Read};

lazy_static::lazy_static! {
    static ref FIG_HEADER : Regex = Regex::new(r"flf2a(.) (\d+) (\d+) (\d+) (\d+) (\d+)\s*(\d+)?\s*(\d+)?\s*(\d+)?").unwrap();
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PrintDirection {
    LeftToRight,
    RightToLeft,
}

use crate::EngineResult;

use super::{errors::FigError, read_line};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum LayoutMode {
    Full,
    Fitting,
    Smushing,
}

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq)]
    pub struct HorizontalSmushing: i32 {
        const NONE            =  0;
        const EQUAL_CHARACTER =  1;
        const UNDERSCORE      =  2;
        const HIERARCHY       =  4;
        const OPPOSITE_PAIR   =  8;
        const BIG_X           = 16;
        const HARDBLANK       = 32;
    }
}

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq)]
    pub struct VerticalSmushing: i32 {
        const NONE            =  0;
        const EQUAL_CHARACTER =  1;
        const UNDERSCORE      =  2;
        const HIERARCHY       =  4;
        const HORIZONTAL_LINE =  8;
        const VERTICAL_LINE   =  16;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    hard_blank_char: char,
    height: usize,
    baseline: usize,
    max_length: usize,
    vert_layout: LayoutMode,
    horiz_layout: LayoutMode,
    horizontal_smushing: HorizontalSmushing,
    vertical_smushing: VerticalSmushing,
    comment: String,
    print_direction: PrintDirection,
    codetag_count: Option<u32>,
}

impl Header {
    pub(crate) fn read<R: Read>(reader: &mut BufReader<R>) -> EngineResult<Self> {
        let line = read_line(reader)?;
        let Some(captures) = FIG_HEADER.captures(&line) else {
            return Err(FigError::InvalidHeader.into());
        };
        if captures[1].len() != 1 {
            return Err(FigError::InvalidHeaderHardBlank.into());
        }
        let hard_blank_char = captures[1].chars().next().unwrap();

        let height = captures[2].parse()?;
        let baseline = captures[3].parse()?;
        let max_length = captures[4].parse()?;
        let old_layout: i32 = captures[5].parse()?;
        let comment_lines: usize = captures[6].parse()?;
        let print_direction = if let Some(capture) = captures.get(7) {
            match capture.as_str().parse()? {
                0 => PrintDirection::LeftToRight,
                1 => PrintDirection::RightToLeft,
                e => return Err(FigError::InvalidHeaderPrintDirection(e).into()),
            }
        } else {
            PrintDirection::LeftToRight
        };

        let horiz_layout;
        let horizontal_smushing;
        let vert_layout;
        let vertical_smushing;

        if let Some(capture) = captures.get(8) {
            let bits: i32 = capture.as_str().parse()?;
            horizontal_smushing = HorizontalSmushing::from_bits_truncate(bits & 0b111111);
            horiz_layout = if bits & 0x80 != 0 {
                LayoutMode::Smushing
            } else if bits & 0x40 != 0 {
                LayoutMode::Fitting
            } else {
                LayoutMode::Full
            };
            vertical_smushing = VerticalSmushing::from_bits_truncate((bits >> 8) & 0b11111);
            vert_layout = if bits & 0x4000 != 0 {
                LayoutMode::Smushing
            } else if bits & 0x2000 != 0 {
                LayoutMode::Fitting
            } else {
                LayoutMode::Full
            };
        } else {
            if old_layout < 0 {
                horiz_layout = LayoutMode::Full;
                horizontal_smushing = HorizontalSmushing::NONE;
            } else if old_layout == 0 {
                horiz_layout = LayoutMode::Fitting;
                horizontal_smushing = HorizontalSmushing::NONE;
            } else {
                horiz_layout = LayoutMode::Smushing;
                horizontal_smushing = HorizontalSmushing::from_bits_truncate(old_layout);
            }
            vertical_smushing = VerticalSmushing::NONE;
            vert_layout = LayoutMode::Full;
        };

        let codetag_count = if let Some(capture) = captures.get(9) {
            Some(capture.as_str().parse()?)
        } else {
            None
        };
        let mut comment = String::new();
        for i in 0..comment_lines {
            if i > 0 {
                comment.push('\n');
            }
            comment.push_str(&read_line(reader)?);
        }

        Ok(Self {
            hard_blank_char,
            height,
            baseline,
            max_length,
            horiz_layout,
            horizontal_smushing,
            vert_layout,
            vertical_smushing,
            comment,
            print_direction,
            codetag_count,
        })
    }

    pub fn hard_blank_char(&self) -> char {
        self.hard_blank_char
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn baseline(&self) -> usize {
        self.baseline
    }

    pub fn max_length(&self) -> usize {
        self.max_length
    }

    pub fn comment(&self) -> &str {
        &self.comment
    }

    pub fn print_direction(&self) -> PrintDirection {
        self.print_direction
    }

    pub fn codetag_count(&self) -> Option<u32> {
        self.codetag_count
    }

    pub fn horizontal_smushing(&self) -> HorizontalSmushing {
        self.horizontal_smushing
    }

    pub fn vertical_smushing(&self) -> VerticalSmushing {
        self.vertical_smushing
    }

    pub fn vert_layout(&self) -> LayoutMode {
        self.vert_layout
    }

    pub fn horiz_layout(&self) -> LayoutMode {
        self.horiz_layout
    }

    pub(crate) fn generate_string(&self) -> String {
        let old_layout;
        let mut full_layout;

        match self.horiz_layout {
            LayoutMode::Full => {
                old_layout = -1;
                full_layout = 0;
            }
            LayoutMode::Fitting => {
                old_layout = 0;
                full_layout = 64;
            }
            LayoutMode::Smushing => {
                old_layout = self.horizontal_smushing.bits();
                full_layout = old_layout;
                full_layout |= 128;
            }
        }

        match self.vert_layout {
            LayoutMode::Full => {}
            LayoutMode::Fitting => {
                full_layout |= 8192;
            }
            LayoutMode::Smushing => {
                full_layout |= 16384;
                full_layout |= (self.vertical_smushing.bits() << 8) as i32;
            }
        }

        format!(
            "flf2a{} {} {} {} {} {} {} {}{}{}",
            self.hard_blank_char,
            self.height,
            self.baseline,
            self.max_length,
            old_layout,
            self.comment.lines().count(),
            match self.print_direction {
                PrintDirection::LeftToRight => 0,
                PrintDirection::RightToLeft => 1,
            },
            full_layout,
            if let Some(count) = &self.codetag_count {
                format!(" {}", count)
            } else {
                String::new()
            },
            if !self.comment.is_empty() {
                format!("\n{}", self.comment)
            } else {
                String::new()
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header() {
        let input = "flf2a$ 6 5 20 15 0 0 143 229";
        let mut reader = BufReader::new(input.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        assert_eq!(header.hard_blank_char(), '$');
        assert_eq!(header.height(), 6);
        assert_eq!(header.baseline(), 5);
        assert_eq!(header.max_length(), 20);
        assert_eq!(header.comment(), "");
        assert_eq!(header.print_direction(), PrintDirection::LeftToRight);

        assert_eq!(header.horiz_layout(), LayoutMode::Smushing);
        assert_eq!(
            header.horizontal_smushing(),
            HorizontalSmushing::EQUAL_CHARACTER | HorizontalSmushing::UNDERSCORE | HorizontalSmushing::HIERARCHY | HorizontalSmushing::OPPOSITE_PAIR
        );

        assert_eq!(header.vert_layout(), LayoutMode::Full);
        assert_eq!(header.vertical_smushing(), VerticalSmushing::NONE);

        assert_eq!(header.codetag_count(), Some(229));
    }

    #[test]
    fn test_header_no_codetag() {
        let input = "flf2a$ 6 5 20 15 0 0 143";
        let mut reader: BufReader<&[u8]> = BufReader::new(input.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        assert_eq!(header.hard_blank_char(), '$');
        assert_eq!(header.height(), 6);
        assert_eq!(header.baseline(), 5);
        assert_eq!(header.max_length(), 20);
        assert_eq!(header.comment(), "");
        assert_eq!(header.print_direction(), PrintDirection::LeftToRight);
        assert_eq!(header.horiz_layout(), LayoutMode::Smushing);
        assert_eq!(
            header.horizontal_smushing(),
            HorizontalSmushing::EQUAL_CHARACTER | HorizontalSmushing::UNDERSCORE | HorizontalSmushing::HIERARCHY | HorizontalSmushing::OPPOSITE_PAIR
        );

        assert_eq!(header.vert_layout(), LayoutMode::Full);
        assert_eq!(header.vertical_smushing(), VerticalSmushing::NONE);

        assert_eq!(header.codetag_count(), None);
    }

    #[test]
    fn test_header_no_full_layout() {
        let input = "flf2a$ 6 5 20 15 0 0";
        let mut reader = BufReader::new(input.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        assert_eq!(header.hard_blank_char(), '$');
        assert_eq!(header.height(), 6);
        assert_eq!(header.baseline(), 5);
        assert_eq!(header.max_length(), 20);
        assert_eq!(header.comment(), "");
        assert_eq!(header.print_direction(), PrintDirection::LeftToRight);

        assert_eq!(header.horiz_layout(), LayoutMode::Smushing);
        assert_eq!(
            header.horizontal_smushing(),
            HorizontalSmushing::EQUAL_CHARACTER | HorizontalSmushing::UNDERSCORE | HorizontalSmushing::HIERARCHY | HorizontalSmushing::OPPOSITE_PAIR
        );

        assert_eq!(header.vert_layout(), LayoutMode::Full);
        assert_eq!(header.vertical_smushing(), VerticalSmushing::NONE);

        assert_eq!(header.codetag_count(), None);
    }

    #[test]
    fn test_comments() {
        let input = "flf2a$ 6 5 20 15 3 0 143 229\nfoo\nbar\nbaz";
        let mut reader = BufReader::new(input.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        assert_eq!(header.comment(), "foo\nbar\nbaz");
    }

    #[test]
    fn test_header_generation() {
        let input = "flf2a$ 6 5 20 15 0 0 143 229";
        let mut reader = BufReader::new(input.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        let generated = header.generate_string();
        assert_eq!(generated, input);
    }

    #[test]
    fn test_header_generation_comments() {
        let input = "flf2a$ 6 5 20 15 3 0 143 229\nfoo\nbar\nbaz";
        let mut reader = BufReader::new(input.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        let generated = header.generate_string();
        assert_eq!(generated, input);
    }
}
