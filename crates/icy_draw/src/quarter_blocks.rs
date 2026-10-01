//! Drawing with 2 × 2 pixels per character, using a character set's quarter blocks.
//!
//! The characters are found in the font: those whose four quarters are each either set or
//! empty. ATASCII has fourteen of the sixteen combinations (its inverse characters included);
//! the two diagonal pairs are missing, so a pixel that would make one keeps only the pixels
//! of its own row. PETSCII has all sixteen.

use icy_engine::{BitFont, Position};

const UPPER_LEFT: u8 = 1;
const UPPER_RIGHT: u8 = 2;
const LOWER_LEFT: u8 = 4;
const LOWER_RIGHT: u8 = 8;

/// The quarter block characters of a font, by the pixels they set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuarterBlocks {
    codes: [Option<char>; 16],
}

impl QuarterBlocks {
    /// The quarter blocks of `font`, `None` when its characters cannot be split into quarters or
    /// it lacks an empty or full one.
    pub fn of(font: &BitFont) -> Option<Self> {
        let size = font.size();
        let (width, height) = (size.width as usize, size.height as usize);
        if width % 2 != 0 || height % 2 != 0 || width == 0 {
            return None;
        }
        let mut codes = [None; 16];
        // The space first, then the normal characters before the inverse ones.
        for code in std::iter::once(0x20).chain(0..256u32) {
            let ch = char::from_u32(code)?;
            let rows = font.glyph(ch).to_bitmap_pixels();
            let quarter = |left: usize, top: usize| {
                let pixels = rows
                    .iter()
                    .skip(top)
                    .take(height / 2)
                    .flat_map(|row| row.iter().skip(left).take(width / 2).copied());
                let lit = pixels.clone().filter(|&on| on).count();
                match lit {
                    0 => Some(false),
                    n if n == width * height / 4 => Some(true),
                    _ => None,
                }
            };
            let quarters = [quarter(0, 0), quarter(width / 2, 0), quarter(0, height / 2), quarter(width / 2, height / 2)];
            if quarters.iter().any(Option::is_none) {
                continue;
            }
            let pixels = quarters
                .iter()
                .enumerate()
                .fold(0u8, |mask, (bit, set)| mask | (u8::from(set == &Some(true)) << bit));
            codes[pixels as usize].get_or_insert(ch);
        }
        (codes[0].is_some() && codes[15].is_some()).then_some(Self { codes })
    }

    /// The pixels a character sets, `None` for characters that are not quarter blocks.
    pub fn pixels_of(&self, code: char) -> Option<u8> {
        self.codes.iter().position(|candidate| *candidate == Some(code)).map(|pixels| pixels as u8)
    }

    /// The character `code` with the pixel at `pixel` set or cleared. Other characters count as empty.
    pub fn with_pixel(&self, code: char, pixel: Position, set: bool) -> char {
        let bit = bit(pixel);
        let pixels = self.pixels_of(code).unwrap_or(0);
        let pixels = if set { pixels | bit } else { pixels & !bit };
        let row = if bit & (UPPER_LEFT | UPPER_RIGHT) != 0 {
            UPPER_LEFT | UPPER_RIGHT
        } else {
            LOWER_LEFT | LOWER_RIGHT
        };
        self.codes[pixels as usize].or(self.codes[(pixels & row) as usize]).unwrap_or(' ')
    }
}

/// The pixel of a character at the pixel position `pixel` (in pixels, two per character).
fn bit(pixel: Position) -> u8 {
    1 << ((pixel.x.rem_euclid(2)) + 2 * pixel.y.rem_euclid(2))
}

/// The character cell of the pixel position `pixel`.
pub fn cell_of(pixel: Position) -> Position {
    Position::new(pixel.x.div_euclid(2), pixel.y.div_euclid(2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atascii_pixels_build_up_to_a_full_block_and_back() {
        let blocks = QuarterBlocks::of(&icy_engine::ATARI).unwrap();
        let mut code = ' ';
        for pixel in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            code = blocks.with_pixel(code, Position::new(pixel.0, pixel.1), true);
        }
        assert_eq!(code as u32, 0xA0);
        code = blocks.with_pixel(code, Position::new(1, 1), false);
        assert_eq!(code as u32, 0x89, "all but the lower right is the inverse ▗");
        for pixel in [(0, 0), (1, 0), (0, 1)] {
            code = blocks.with_pixel(code, Position::new(pixel.0, pixel.1), false);
        }
        assert_eq!(code, ' ');
    }

    #[test]
    fn atascii_diagonals_keep_the_row_of_the_new_pixel() {
        let blocks = QuarterBlocks::of(&icy_engine::ATARI).unwrap();
        let upper_left = blocks.with_pixel(' ', Position::new(0, 0), true);
        assert_eq!(blocks.with_pixel(upper_left, Position::new(1, 1), true) as u32, 0x09, "only the lower right");
        for pixels in 0..16u8 {
            let diagonal = pixels == UPPER_LEFT | LOWER_RIGHT || pixels == UPPER_RIGHT | LOWER_LEFT;
            assert_eq!(blocks.codes[pixels as usize].is_some(), !diagonal, "{pixels:04b}");
        }
        assert_eq!(cell_of(Position::new(5, 3)), Position::new(2, 1));
    }

    #[test]
    fn petscii_has_every_combination_with_the_diagonals() {
        let font = icy_engine::petscii_font(icy_engine::PetsciiMachine::C64, icy_engine::PetsciiCase::Upper);
        let blocks = QuarterBlocks::of(&font).unwrap();
        assert!(blocks.codes.iter().all(Option::is_some));
        // The C64's quarter blocks, as Petmate's chunky lines use them.
        assert_eq!(blocks.codes[UPPER_LEFT as usize], Some('\u{7E}'));
        assert_eq!(blocks.codes[(UPPER_LEFT | LOWER_RIGHT) as usize], Some('\u{7F}'));
        assert_eq!(blocks.codes[15], Some('\u{A0}'));
        let diagonal = blocks.with_pixel(blocks.with_pixel(' ', Position::new(0, 0), true), Position::new(1, 1), true);
        assert_eq!(diagonal as u32, 0x7F);
        assert!(QuarterBlocks::of(&icy_engine::ATARI_XEP80).is_none(), "7 × 10 characters have no quarters");
    }
}
