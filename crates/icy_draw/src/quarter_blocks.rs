//! Drawing with 2 × 2 pixels per character, using ATASCII's quarter blocks and their inverse.
//!
//! Fourteen of the sixteen combinations exist; the two diagonal pairs do not, so a pixel that
//! would make one keeps only the pixels of its own row.

use icy_engine::Position;

const UPPER_LEFT: u8 = 1;
const UPPER_RIGHT: u8 = 2;
const LOWER_LEFT: u8 = 4;
const LOWER_RIGHT: u8 = 8;

/// ATASCII codes and the pixels they set; codes from 128 are inverse.
const BLOCKS: [(u8, u8); 14] = [
    (0x20, 0),
    (0x0C, UPPER_LEFT),
    (0x0B, UPPER_RIGHT),
    (0x95, UPPER_LEFT | UPPER_RIGHT),
    (0x0F, LOWER_LEFT),
    (0x19, UPPER_LEFT | LOWER_LEFT),
    (0x89, UPPER_LEFT | UPPER_RIGHT | LOWER_LEFT),
    (0x09, LOWER_RIGHT),
    (0x99, UPPER_RIGHT | LOWER_RIGHT),
    (0x8F, UPPER_LEFT | UPPER_RIGHT | LOWER_RIGHT),
    (0x15, LOWER_LEFT | LOWER_RIGHT),
    (0x8B, UPPER_LEFT | LOWER_LEFT | LOWER_RIGHT),
    (0x8C, UPPER_RIGHT | LOWER_LEFT | LOWER_RIGHT),
    (0xA0, UPPER_LEFT | UPPER_RIGHT | LOWER_LEFT | LOWER_RIGHT),
];

/// The pixels a character sets, `None` for characters that are not quarter blocks.
pub fn pixels_of(code: char) -> Option<u8> {
    BLOCKS
        .iter()
        .find(|(candidate, _)| u32::from(*candidate) == code as u32)
        .map(|(_, pixels)| *pixels)
}

fn code_for(pixels: u8) -> Option<char> {
    BLOCKS.iter().find(|(_, candidate)| *candidate == pixels).map(|(code, _)| char::from(*code))
}

/// The pixel of a character at the pixel position `pixel` (in pixels, two per character).
fn bit(pixel: Position) -> u8 {
    1 << ((pixel.x.rem_euclid(2)) + 2 * pixel.y.rem_euclid(2))
}

/// The character cell of the pixel position `pixel`.
pub fn cell_of(pixel: Position) -> Position {
    Position::new(pixel.x.div_euclid(2), pixel.y.div_euclid(2))
}

/// The character `code` with the pixel at `pixel` set or cleared. Other characters count as empty.
pub fn with_pixel(code: char, pixel: Position, set: bool) -> char {
    let bit = bit(pixel);
    let pixels = pixels_of(code).unwrap_or(0);
    let pixels = if set { pixels | bit } else { pixels & !bit };
    let row = if bit & (UPPER_LEFT | UPPER_RIGHT) != 0 {
        UPPER_LEFT | UPPER_RIGHT
    } else {
        LOWER_LEFT | LOWER_RIGHT
    };
    code_for(pixels).or_else(|| code_for(pixels & row)).unwrap_or(' ')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_build_up_to_a_full_block_and_back() {
        let mut code = ' ';
        for pixel in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            code = with_pixel(code, Position::new(pixel.0, pixel.1), true);
        }
        assert_eq!(code as u32, 0xA0);
        code = with_pixel(code, Position::new(1, 1), false);
        assert_eq!(code as u32, 0x89, "all but the lower right is the inverse ▗");
        for pixel in [(0, 0), (1, 0), (0, 1)] {
            code = with_pixel(code, Position::new(pixel.0, pixel.1), false);
        }
        assert_eq!(code, ' ');
    }

    #[test]
    fn diagonals_keep_the_row_of_the_new_pixel() {
        let upper_left = with_pixel(' ', Position::new(0, 0), true);
        assert_eq!(with_pixel(upper_left, Position::new(1, 1), true) as u32, 0x09, "only the lower right");
        let three = char::from(0x89u8);
        assert_eq!(with_pixel(three, Position::new(0, 0), false) as u32, 0x0B, "the upper right stays");
    }

    #[test]
    fn every_combination_but_the_diagonals_has_a_character() {
        for pixels in 0..16u8 {
            let diagonal = pixels == UPPER_LEFT | LOWER_RIGHT || pixels == UPPER_RIGHT | LOWER_LEFT;
            assert_eq!(code_for(pixels).is_some(), !diagonal, "{pixels:04b}");
        }
        assert_eq!(cell_of(Position::new(5, 3)), Position::new(2, 1));
    }
}
