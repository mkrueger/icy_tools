//! Compact bitmap glyph representation for efficient font rendering.
//!
//! This module provides a memory-efficient glyph representation that stores
//! bitmap data as packed bytes instead of Vec<Vec<bool>>. Each row is stored
//! as a single byte (for widths up to 8 pixels), supporting fonts up to 8x32.

/// Maximum glyph height in pixels (32 rows × 1 byte per row = 32 bytes)
pub const MAX_GLYPH_HEIGHT: usize = 32;

/// A compact bitmap glyph stored as packed bytes.
///
/// Each row is stored as a single byte with MSB-first bit ordering:
/// - Bit 7 = leftmost pixel
/// - Bit 0 = rightmost pixel (for 8px wide fonts)
///
/// Supports fonts up to 8 pixels wide and 32 pixels tall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactGlyph {
    /// Bitmap data: one byte per row, MSB = leftmost pixel
    pub data: [u8; MAX_GLYPH_HEIGHT],
    /// Glyph width in pixels (1-8)
    pub width: u8,
    /// Glyph height in pixels (1-32)
    pub height: u8,
}

impl Default for CompactGlyph {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl CompactGlyph {
    /// An empty glyph (8x16, all zeros)
    pub const EMPTY: Self = Self {
        data: [0; MAX_GLYPH_HEIGHT],
        width: 8,
        height: 16,
    };

    /// Create a new glyph with the given dimensions.
    /// All pixels are initially off.
    #[inline]
    pub const fn new(width: u8, height: u8) -> Self {
        Self {
            data: [0; MAX_GLYPH_HEIGHT],
            width,
            height,
        }
    }

    /// Create a glyph from raw row data.
    ///
    /// # Arguments
    /// * `width` - Glyph width in pixels (1-8)
    /// * `height` - Glyph height in pixels (1-32)
    /// * `rows` - Raw byte data, one byte per row (MSB = leftmost pixel)
    #[inline]
    pub fn from_rows(width: u8, height: u8, rows: &[u8]) -> Self {
        let mut data = [0u8; MAX_GLYPH_HEIGHT];
        let copy_len = rows.len().min(MAX_GLYPH_HEIGHT).min(height as usize);
        data[..copy_len].copy_from_slice(&rows[..copy_len]);
        Self { data, width, height }
    }

    /// Get a pixel value at the given position.
    ///
    /// Returns `false` if coordinates are out of bounds.
    #[inline]
    pub fn get_pixel(&self, x: usize, y: usize) -> bool {
        if x >= self.width as usize || y >= self.height as usize {
            return false;
        }
        // MSB-first: bit 7 = x=0, bit 6 = x=1, etc.
        (self.data[y] & (0x80 >> x)) != 0
    }

    /// Set a pixel value at the given position.
    ///
    /// Does nothing if coordinates are out of bounds.
    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, value: bool) {
        if x >= self.width as usize || y >= self.height as usize {
            return;
        }
        let mask = 0x80 >> x;
        if value {
            self.data[y] |= mask;
        } else {
            self.data[y] &= !mask;
        }
    }

    /// Get a row as a byte (MSB = leftmost pixel).
    #[inline]
    pub fn get_row(&self, y: usize) -> u8 {
        if y >= self.height as usize {
            return 0;
        }
        self.data[y]
    }

    /// Set a row from a byte (MSB = leftmost pixel).
    #[inline]
    pub fn set_row(&mut self, y: usize, value: u8) {
        if y < self.height as usize {
            self.data[y] = value;
        }
    }

    /// Check if the glyph is empty (all pixels off).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data[..self.height as usize].iter().all(|&b| b == 0)
    }

    /// Flip the glyph horizontally (left-right mirror).
    pub fn flip_x(&mut self) {
        for y in 0..self.height as usize {
            self.data[y] = reverse_bits(self.data[y], self.width);
        }
    }

    /// Flip the glyph vertically (top-bottom mirror).
    pub fn flip_y(&mut self) {
        let h = self.height as usize;
        for y in 0..h / 2 {
            self.data.swap(y, h - 1 - y);
        }
    }

    /// Convert from libyaff's Vec<Vec<bool>> bitmap format.
    pub fn from_bitmap_pixels(pixels: &[Vec<bool>], width: usize, height: usize) -> Self {
        let w = width.min(8) as u8;
        let h = height.min(MAX_GLYPH_HEIGHT) as u8;
        let mut glyph = Self::new(w, h);

        for (y, row) in pixels.iter().enumerate().take(h as usize) {
            let mut byte = 0u8;
            for (x, &pixel) in row.iter().enumerate().take(w as usize) {
                if pixel {
                    byte |= 0x80 >> x;
                }
            }
            glyph.data[y] = byte;
        }

        glyph
    }

    /// Convert to a Vec<Vec<bool>> for compatibility with libyaff.
    pub fn to_bitmap_pixels(&self) -> Vec<Vec<bool>> {
        let mut pixels = Vec::with_capacity(self.height as usize);
        for y in 0..self.height as usize {
            let mut row = Vec::with_capacity(self.width as usize);
            for x in 0..self.width as usize {
                row.push(self.get_pixel(x, y));
            }
            pixels.push(row);
        }
        pixels
    }

    /// Extend the glyph width from 8 to 9 pixels for VGA letter spacing mode.
    /// For box-drawing characters, the 8th pixel is extended to the 9th.
    ///
    /// Note: This returns a new representation since `CompactGlyph` only supports up to 8px width.
    /// The caller should handle 9px fonts separately.
    pub fn extend_to_9px(&self, extend_8th_pixel: bool) -> [u16; MAX_GLYPH_HEIGHT] {
        let mut result = [0u16; MAX_GLYPH_HEIGHT];
        for y in 0..self.height as usize {
            let byte = self.data[y];
            // Shift left by 1 to make room for the 9th pixel at position 6
            let mut word = (byte as u16) << 1;
            if extend_8th_pixel && (byte & 0x01) != 0 {
                // Copy the 8th pixel (bit 0) to the 9th position
                word |= 0x01;
            }
            result[y] = word;
        }
        result
    }
}

/// Reverse the bits in a byte, considering only `width` significant bits.
#[inline]
fn reverse_bits(byte: u8, width: u8) -> u8 {
    let mut result = 0u8;
    for i in 0..width {
        if byte & (0x80 >> i) != 0 {
            result |= 0x80 >> (width - 1 - i);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_glyph() {
        let glyph = CompactGlyph::new(8, 16);
        assert_eq!(glyph.width, 8);
        assert_eq!(glyph.height, 16);
        assert!(glyph.is_empty());
    }

    #[test]
    fn test_get_set_pixel() {
        let mut glyph = CompactGlyph::new(8, 16);

        // Set pixel at (0, 0) - should be bit 7
        glyph.set_pixel(0, 0, true);
        assert!(glyph.get_pixel(0, 0));
        assert_eq!(glyph.data[0], 0x80);

        // Set pixel at (7, 0) - should be bit 0
        glyph.set_pixel(7, 0, true);
        assert!(glyph.get_pixel(7, 0));
        assert_eq!(glyph.data[0], 0x81);

        // Clear pixel at (0, 0)
        glyph.set_pixel(0, 0, false);
        assert!(!glyph.get_pixel(0, 0));
        assert_eq!(glyph.data[0], 0x01);
    }

    #[test]
    fn test_from_rows() {
        let rows = [0xFF, 0x81, 0x81, 0xFF];
        let glyph = CompactGlyph::from_rows(8, 4, &rows);

        assert_eq!(glyph.width, 8);
        assert_eq!(glyph.height, 4);
        assert_eq!(glyph.data[0], 0xFF);
        assert_eq!(glyph.data[1], 0x81);
        assert_eq!(glyph.data[2], 0x81);
        assert_eq!(glyph.data[3], 0xFF);
    }

    #[test]
    fn test_flip_x() {
        let rows = [0x80]; // Leftmost pixel only
        let mut glyph = CompactGlyph::from_rows(8, 1, &rows);

        glyph.flip_x();
        assert_eq!(glyph.data[0], 0x01); // Rightmost pixel only
    }

    #[test]
    fn test_flip_y() {
        let rows = [0xFF, 0x00];
        let mut glyph = CompactGlyph::from_rows(8, 2, &rows);

        glyph.flip_y();
        assert_eq!(glyph.data[0], 0x00);
        assert_eq!(glyph.data[1], 0xFF);
    }

    #[test]
    fn test_from_bitmap_pixels() {
        let pixels = vec![
            vec![true, false, false, false, false, false, false, true], // 0x81
            vec![false, true, true, true, true, true, true, false],     // 0x7E
        ];

        let glyph = CompactGlyph::from_bitmap_pixels(&pixels, 8, 2);

        assert_eq!(glyph.data[0], 0x81);
        assert_eq!(glyph.data[1], 0x7E);
    }

    #[test]
    fn test_to_bitmap_pixels() {
        let mut glyph = CompactGlyph::new(8, 2);
        glyph.data[0] = 0x81;
        glyph.data[1] = 0x7E;

        let pixels = glyph.to_bitmap_pixels();

        assert_eq!(pixels.len(), 2);
        assert_eq!(pixels[0], vec![true, false, false, false, false, false, false, true]);
        assert_eq!(pixels[1], vec![false, true, true, true, true, true, true, false]);
    }

    #[test]
    fn test_reverse_bits() {
        assert_eq!(reverse_bits(0x80, 8), 0x01);
        assert_eq!(reverse_bits(0x01, 8), 0x80);
        assert_eq!(reverse_bits(0xF0, 8), 0x0F);
        assert_eq!(reverse_bits(0x81, 8), 0x81); // Symmetric
    }
}
