//! Decoders for DOS and Amiga raster formats the `image` crate does not read.
//!
//! Every decoder turns the file into RGBA pixels and never panics on malformed input.
//! The format coverage follows kaleidotron's decoders (MIT, Copyright (c) 2026 Rick Christy),
//! extended with the planar EGA/CGA PCX modes, transparent/deep ILBM and the exact VGA palette.

pub(crate) mod bsave;
pub(crate) mod ilbm;
pub(crate) mod pcx;

use image::RgbaImage;

/// Largest image any of these decoders will allocate (per side).
const MAX_DIMENSION: usize = 16_384;
/// Largest pixel count any of these decoders will allocate.
const MAX_PIXELS: usize = 64 * 1024 * 1024;

type Rgb = [u8; 3];

fn check_dimensions(width: usize, height: usize) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("zero-sized image".to_string());
    }
    if width > MAX_DIMENSION || height > MAX_DIMENSION || width * height > MAX_PIXELS {
        return Err(format!("image too large ({width}×{height})"));
    }
    Ok(())
}

/// Builds an RGBA image from palette indices; an index outside the palette renders black.
fn indexed_to_rgba(width: usize, height: usize, indices: &[u8], palette: &[Rgb], transparent: Option<u8>) -> Result<RgbaImage, String> {
    let mut pixels = Vec::with_capacity(width * height * 4);
    for &index in indices.iter().take(width * height) {
        let [r, g, b] = palette.get(index as usize).copied().unwrap_or([0, 0, 0]);
        let alpha = if transparent == Some(index) { 0 } else { 255 };
        pixels.extend_from_slice(&[r, g, b, alpha]);
    }
    pixels.resize(width * height * 4, 0);
    RgbaImage::from_raw(width as u32, height as u32, pixels).ok_or_else(|| "invalid image dimensions".to_string())
}

/// The 16 EGA/CGA text colors, as the hardware showed them.
const EGA_16: [Rgb; 16] = [
    [0x00, 0x00, 0x00],
    [0x00, 0x00, 0xAA],
    [0x00, 0xAA, 0x00],
    [0x00, 0xAA, 0xAA],
    [0xAA, 0x00, 0x00],
    [0xAA, 0x00, 0xAA],
    [0xAA, 0x55, 0x00],
    [0xAA, 0xAA, 0xAA],
    [0x55, 0x55, 0x55],
    [0x55, 0x55, 0xFF],
    [0x55, 0xFF, 0x55],
    [0x55, 0xFF, 0xFF],
    [0xFF, 0x55, 0x55],
    [0xFF, 0x55, 0xFF],
    [0xFF, 0xFF, 0x55],
    [0xFF, 0xFF, 0xFF],
];

/// CGA mode 4, palette 1 high intensity (black, cyan, magenta, white): what most CGA art used.
const CGA_4: [Rgb; 4] = [[0x00, 0x00, 0x00], [0x55, 0xFF, 0xFF], [0xFF, 0x55, 0xFF], [0xFF, 0xFF, 0xFF]];

fn dac(value: u8) -> u8 {
    ((u16::from(value.min(63)) * 255 + 31) / 63) as u8
}

/// The VGA DAC's power-on palette for mode 13h: EGA colors, a grey ramp, then nine 24-hue
/// rings (three intensities × three saturations) and eight blacks.
fn vga_default_palette() -> Vec<Rgb> {
    const GREYS: [u8; 16] = [0, 5, 8, 11, 14, 17, 20, 24, 28, 32, 36, 40, 45, 50, 56, 63];
    const RINGS: [[u8; 5]; 9] = [
        [0, 16, 31, 47, 63],
        [31, 39, 47, 55, 63],
        [45, 49, 54, 58, 63],
        [0, 7, 14, 21, 28],
        [14, 17, 21, 24, 28],
        [20, 22, 24, 26, 28],
        [0, 4, 8, 12, 16],
        [8, 10, 12, 14, 16],
        [11, 12, 13, 15, 16],
    ];
    let mut palette = EGA_16.to_vec();
    palette.extend(GREYS.iter().map(|&v| [dac(v), dac(v), dac(v)]));
    for s in RINGS {
        let (lo, hi) = (s[0], s[4]);
        // Blue → magenta → red → yellow → green → cyan → back towards blue.
        let hues = [
            [lo, lo, hi],
            [s[1], lo, hi],
            [s[2], lo, hi],
            [s[3], lo, hi],
            [hi, lo, hi],
            [hi, lo, s[3]],
            [hi, lo, s[2]],
            [hi, lo, s[1]],
            [hi, lo, lo],
            [hi, s[1], lo],
            [hi, s[2], lo],
            [hi, s[3], lo],
            [hi, hi, lo],
            [s[3], hi, lo],
            [s[2], hi, lo],
            [s[1], hi, lo],
            [lo, hi, lo],
            [lo, hi, s[1]],
            [lo, hi, s[2]],
            [lo, hi, s[3]],
            [lo, hi, hi],
            [lo, s[3], hi],
            [lo, s[2], hi],
            [lo, s[1], hi],
        ];
        palette.extend(hues.iter().map(|[r, g, b]| [dac(*r), dac(*g), dac(*b)]));
    }
    palette.resize(256, [0, 0, 0]);
    palette
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vga_palette_matches_known_entries() {
        let palette = vga_default_palette();
        assert_eq!(palette.len(), 256);
        assert_eq!(palette[15], [0xFF, 0xFF, 0xFF]);
        assert_eq!(palette[16], [0, 0, 0]);
        assert_eq!(palette[31], [0xFF, 0xFF, 0xFF]);
        assert_eq!(palette[32], [0, 0, 0xFF]);
        assert_eq!(palette[40], [0xFF, 0, 0]);
        assert_eq!(palette[48], [0, 0xFF, 0]);
        assert_eq!(palette[247], [dac(11), dac(12), dac(16)]);
        assert_eq!(palette[248], [0, 0, 0]);
    }
}
