//! `ZSoft` PC Paintbrush (PCX).
//!
//! Reads every layout PC Paintbrush and its clones wrote: monochrome, CGA 2-bit, EGA
//! planar (1 bit × 2–4 planes), packed 4-bit, VGA 8-bit with the trailing 256-color
//! palette, and 24/32-bit plane-per-scanline truecolor.

use image::RgbaImage;

use super::{check_dimensions, indexed_to_rgba, Rgb, CGA_4, EGA_16};

const HEADER_SIZE: usize = 128;
/// Paintbrush 2.8 without palette information: the header palette must be ignored.
const VERSION_NO_PALETTE: u8 = 3;

pub(crate) fn decode(data: &[u8]) -> Result<RgbaImage, String> {
    if data.len() < HEADER_SIZE || data[0] != 0x0A {
        return Err("not a PCX file".to_string());
    }
    let u16_at = |offset: usize| u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
    let version = data[1];
    let encoding = data[2];
    let bits_per_pixel = data[3] as usize;
    let (x_min, y_min, x_max, y_max) = (u16_at(4), u16_at(6), u16_at(8), u16_at(10));
    let planes = data[65] as usize;
    let bytes_per_line = u16_at(66);

    if x_max < x_min || y_max < y_min {
        return Err("invalid PCX window".to_string());
    }
    let width = x_max - x_min + 1;
    let height = y_max - y_min + 1;
    check_dimensions(width, height)?;
    let supported = matches!((bits_per_pixel, planes), (1, 1..=4) | (2 | 4 | 8, 1) | (8, 3 | 4));
    if !supported {
        return Err(format!("unsupported PCX layout: {bits_per_pixel} bpp × {planes} planes"));
    }
    if bytes_per_line * 8 < width * bits_per_pixel {
        return Err("PCX scanline is shorter than the image width".to_string());
    }

    let line_size = planes * bytes_per_line;
    let scanlines = unpack(&data[HEADER_SIZE..], encoding, line_size * height);
    let line = |y: usize| &scanlines[y * line_size..(y + 1) * line_size];

    if bits_per_pixel == 8 && planes >= 3 {
        let mut pixels = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            let row = line(y);
            for x in 0..width {
                let alpha = if planes == 4 { row[3 * bytes_per_line + x] } else { 255 };
                pixels.extend_from_slice(&[row[x], row[bytes_per_line + x], row[2 * bytes_per_line + x], alpha]);
            }
        }
        return RgbaImage::from_raw(width as u32, height as u32, pixels).ok_or_else(|| "invalid PCX dimensions".to_string());
    }

    let mut indices = Vec::with_capacity(width * height);
    for y in 0..height {
        let row = line(y);
        for x in 0..width {
            let index = if planes == 1 {
                let bit = x * bits_per_pixel;
                let shift = 8 - bits_per_pixel - bit % 8;
                (row[bit / 8] >> shift) & ((1u16 << bits_per_pixel) - 1) as u8
            } else {
                (0..planes).fold(0u8, |acc, plane| {
                    let bit = (row[plane * bytes_per_line + x / 8] >> (7 - x % 8)) & 1;
                    acc | (bit << plane)
                })
            };
            indices.push(index);
        }
    }

    let palette = palette(data, version, bits_per_pixel * planes);
    indexed_to_rgba(width, height, &indices, &palette, None)
}

/// Expands the RLE body. A truncated stream leaves the rest of the picture black instead of failing,
/// so partially downloaded files still show what they contain.
fn unpack(body: &[u8], encoding: u8, size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(size);
    if encoding == 0 {
        out.extend_from_slice(&body[..body.len().min(size)]);
    } else {
        let mut i = 0;
        while out.len() < size && i < body.len() {
            let byte = body[i];
            i += 1;
            if byte & 0xC0 == 0xC0 {
                let Some(&value) = body.get(i) else { break };
                i += 1;
                let count = ((byte & 0x3F) as usize).min(size - out.len());
                out.resize(out.len() + count, value);
            } else {
                out.push(byte);
            }
        }
    }
    out.resize(size, 0);
    out
}

fn palette(data: &[u8], version: u8, depth: usize) -> Vec<Rgb> {
    let header: Vec<Rgb> = data[16..64].chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
    let header_usable = version != VERSION_NO_PALETTE && header.iter().any(|c| *c != [0, 0, 0]);
    match depth {
        1 => vec![[0, 0, 0], [0xFF, 0xFF, 0xFF]],
        2 if header_usable => header[..4].to_vec(),
        2 => CGA_4.to_vec(),
        3 | 4 if header_usable => header,
        3 | 4 => EGA_16.to_vec(),
        _ => {
            let trailer = data.len().checked_sub(769).filter(|&start| start >= HEADER_SIZE && data[start] == 0x0C);
            match trailer {
                Some(start) => data[start + 1..].chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
                None => (0..=255).map(|v| [v, v, v]).collect(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(bits_per_pixel: u8, planes: u8, width: u16, height: u16, bytes_per_line: u16) -> Vec<u8> {
        let mut data = vec![0u8; HEADER_SIZE];
        data[0] = 0x0A;
        data[1] = 5;
        data[2] = 1;
        data[3] = bits_per_pixel;
        data[8..10].copy_from_slice(&(width - 1).to_le_bytes());
        data[10..12].copy_from_slice(&(height - 1).to_le_bytes());
        data[65] = planes;
        data[66..68].copy_from_slice(&bytes_per_line.to_le_bytes());
        data
    }

    #[test]
    fn decodes_vga_8bit_with_rle_and_trailing_palette() {
        let mut data = header(8, 1, 4, 2, 4);
        // Row 0: a run of four 1s. Row 1: literal 2, then 0xC0 must be escaped as a run of one.
        data.extend_from_slice(&[0xC4, 1, 2, 0xC1, 0xC0, 0xC2, 3]);
        data.push(0x0C);
        let mut palette = vec![0u8; 768];
        palette[3..6].copy_from_slice(&[10, 20, 30]);
        palette[6..9].copy_from_slice(&[40, 50, 60]);
        palette[0xC0 * 3..0xC0 * 3 + 3].copy_from_slice(&[1, 2, 3]);
        palette[9..12].copy_from_slice(&[70, 80, 90]);
        data.extend_from_slice(&palette);

        let image = decode(&data).unwrap();
        assert_eq!(image.dimensions(), (4, 2));
        assert_eq!(image.get_pixel(3, 0).0, [10, 20, 30, 255]);
        assert_eq!(image.get_pixel(0, 1).0, [40, 50, 60, 255]);
        assert_eq!(image.get_pixel(1, 1).0, [1, 2, 3, 255]);
        assert_eq!(image.get_pixel(3, 1).0, [70, 80, 90, 255]);
    }

    #[test]
    fn decodes_ega_planar_with_default_palette_when_header_is_empty() {
        let mut data = header(1, 4, 8, 1, 1);
        // Pixel 0 gets planes 0 and 3 (index 9), pixel 7 gets plane 1 (index 2).
        data.extend_from_slice(&[0x80, 0x01, 0x00, 0x80]);
        let image = decode(&data).unwrap();
        assert_eq!(image.get_pixel(0, 0).0, [0x55, 0x55, 0xFF, 255]);
        assert_eq!(image.get_pixel(7, 0).0, [0x00, 0xAA, 0x00, 255]);
        assert_eq!(image.get_pixel(3, 0).0, [0, 0, 0, 255]);
    }

    #[test]
    fn decodes_cga_and_truecolor() {
        let mut cga = header(2, 1, 4, 1, 1);
        cga.push(0b00_01_10_11);
        let image = decode(&cga).unwrap();
        assert_eq!(image.get_pixel(1, 0).0, [0x55, 0xFF, 0xFF, 255]);
        assert_eq!(image.get_pixel(3, 0).0, [0xFF, 0xFF, 0xFF, 255]);

        let mut rgb = header(8, 3, 2, 1, 2);
        rgb.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        let image = decode(&rgb).unwrap();
        assert_eq!(image.get_pixel(0, 0).0, [1, 3, 5, 255]);
        assert_eq!(image.get_pixel(1, 0).0, [2, 4, 6, 255]);
    }

    #[test]
    fn rejects_garbage_and_tolerates_truncation() {
        assert!(decode(&[0x0A; 20]).is_err());
        let mut data = header(8, 1, 100, 100, 100);
        data.extend_from_slice(&[0xFF, 7]);
        let image = decode(&data).unwrap();
        assert_eq!(image.dimensions(), (100, 100));
        let mut bad = header(7, 2, 4, 4, 4);
        bad.push(0);
        assert!(decode(&bad).is_err());
    }
}
