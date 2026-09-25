//! BASIC `BSAVE` screen dumps (`.bsv`).
//!
//! The 7-byte header (`0xFD`, segment, offset, length) records where the bytes came from but
//! not the video mode, so the mode is inferred from the length: 16000/16384 bytes are CGA
//! 320×200×4 with interleaved scanline banks, 64000 bytes are VGA mode 13h. Neither stores a
//! palette, so the hardware defaults are used.

use image::RgbaImage;

use super::{indexed_to_rgba, vga_default_palette, CGA_4};

const HEADER_SIZE: usize = 7;
const WIDTH: usize = 320;
const HEIGHT: usize = 200;

pub(crate) fn decode(data: &[u8]) -> Result<RgbaImage, String> {
    if data.len() < HEADER_SIZE || data[0] != 0xFD {
        return Err("not a BSAVE file".to_string());
    }
    let length = u16::from_le_bytes([data[5], data[6]]) as usize;
    let body = &data[HEADER_SIZE..];
    match length {
        64000 => {
            let mut indices = body[..body.len().min(WIDTH * HEIGHT)].to_vec();
            indices.resize(WIDTH * HEIGHT, 0);
            indexed_to_rgba(WIDTH, HEIGHT, &indices, &vga_default_palette(), None)
        }
        // A memory dump keeps the odd bank at 0x2000; packed dumps put it right after the even one.
        16384 => decode_cga(body, 0x2000),
        16000 => decode_cga(body, 8000),
        other => Err(format!("unsupported BSAVE length {other} (expected a CGA or mode 13h screen)")),
    }
}

fn decode_cga(body: &[u8], odd_bank: usize) -> Result<RgbaImage, String> {
    const ROW_BYTES: usize = WIDTH / 4;
    let mut indices = Vec::with_capacity(WIDTH * HEIGHT);
    for y in 0..HEIGHT {
        let row = if y % 2 == 0 { 0 } else { odd_bank } + (y / 2) * ROW_BYTES;
        for x in 0..WIDTH {
            let byte = body.get(row + x / 4).copied().unwrap_or(0);
            indices.push((byte >> (6 - (x % 4) * 2)) & 0x03);
        }
    }
    indexed_to_rgba(WIDTH, HEIGHT, &indices, &CGA_4, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bsave(length: u16, body: &[u8]) -> Vec<u8> {
        let mut data = vec![0xFD, 0x00, 0xB8, 0x00, 0x00];
        data.extend_from_slice(&length.to_le_bytes());
        data.extend_from_slice(body);
        data
    }

    #[test]
    fn decodes_interleaved_cga() {
        let mut body = vec![0u8; 16384];
        body[0] = 0b01_00_00_11;
        body[0x2000] = 0b10_00_00_00;
        let image = decode(&bsave(16384, &body)).unwrap();
        assert_eq!(image.dimensions(), (320, 200));
        assert_eq!(image.get_pixel(0, 0).0, [0x55, 0xFF, 0xFF, 255]);
        assert_eq!(image.get_pixel(3, 0).0, [0xFF, 0xFF, 0xFF, 255]);
        assert_eq!(image.get_pixel(0, 1).0, [0xFF, 0x55, 0xFF, 255]);
    }

    #[test]
    fn decodes_mode_13h_with_vga_palette_and_short_body() {
        let image = decode(&bsave(64000, &[4, 40])).unwrap();
        assert_eq!(image.get_pixel(0, 0).0, [0xAA, 0, 0, 255]);
        assert_eq!(image.get_pixel(1, 0).0, [0xFF, 0, 0, 255]);
        assert_eq!(image.get_pixel(319, 199).0, [0, 0, 0, 255]);
    }

    #[test]
    fn rejects_unknown_lengths() {
        assert!(decode(&bsave(4000, &[0; 4000])).is_err());
        assert!(decode(&[0xFD, 0]).is_err());
    }
}
