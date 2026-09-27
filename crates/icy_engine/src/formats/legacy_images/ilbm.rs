//! Amiga IFF bitmaps: interleaved `FORM ILBM` and Deluxe Paint's chunky `FORM PBM `.
//!
//! Handles `ByteRun1` compression, mask planes, OCS 4-bit palettes, Extra Half-Brite,
//! HAM6/HAM8 and deep 24/32-bit ILBMs.

use image::RgbaImage;

use super::{check_dimensions, Rgb};

const CAMG_EHB: u32 = 0x80;
const CAMG_HAM: u32 = 0x800;
const MASK_PLANE: u8 = 1;

struct Header {
    width: usize,
    height: usize,
    planes: usize,
    masking: u8,
    compression: u8,
}

pub(crate) fn decode(data: &[u8]) -> Result<RgbaImage, String> {
    if data.len() < 12 || &data[0..4] != b"FORM" {
        return Err("not an IFF FORM".to_string());
    }
    let chunky = match &data[8..12] {
        b"ILBM" => false,
        b"PBM " => true,
        // An animation starts with a complete ILBM: show its first frame.
        b"ANIM" if data.get(12..16) == Some(b"FORM") => {
            let length = u32::from_be_bytes([data[16], data[17], data[18], data[19]]) as usize;
            return decode(&data[12..(20usize.saturating_add(length)).min(data.len())]);
        }
        _ => return Err("not an ILBM or PBM picture".to_string()),
    };

    let mut header = None;
    let mut cmap: Vec<Rgb> = Vec::new();
    let mut camg = 0u32;
    let mut body = None;
    let mut offset = 12;
    while offset + 8 <= data.len() {
        let id = &data[offset..offset + 4];
        let length = u32::from_be_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]]) as usize;
        let start = offset + 8;
        // A truncated last chunk still yields what is there.
        let end = start.saturating_add(length).min(data.len());
        let chunk = &data[start..end];
        match id {
            b"BMHD" if chunk.len() >= 20 => {
                header = Some(Header {
                    width: u16::from_be_bytes([chunk[0], chunk[1]]) as usize,
                    height: u16::from_be_bytes([chunk[2], chunk[3]]) as usize,
                    planes: chunk[8] as usize,
                    masking: chunk[9],
                    compression: chunk[10],
                });
            }
            b"CMAP" => cmap = chunk.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
            b"CAMG" if chunk.len() >= 4 => camg = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
            b"BODY" => body = Some(chunk),
            _ => {}
        }
        offset = end + (length & 1);
    }

    let header = header.ok_or("IFF picture has no BMHD chunk")?;
    let body = body.ok_or("IFF picture has no BODY chunk")?;
    let (width, height, planes) = (header.width, header.height, header.planes);
    check_dimensions(width, height)?;
    if !matches!(planes, 1..=8 | 24 | 32) || (chunky && planes != 8) {
        return Err(format!("unsupported IFF depth: {planes} planes"));
    }

    // Chunky PBM rows and each ILBM bitplane row are padded to a 16-bit word.
    let plane_row_bytes = width.div_ceil(16) * 2;
    let has_mask_plane = !chunky && header.masking == MASK_PLANE;
    let row_size = if chunky {
        width + (width & 1)
    } else {
        (planes + usize::from(has_mask_plane)) * plane_row_bytes
    };
    let unpacked = match header.compression {
        0 => {
            let mut raw = body[..body.len().min(row_size * height)].to_vec();
            raw.resize(row_size * height, 0);
            raw
        }
        1 => byte_run1(body, row_size * height),
        other => return Err(format!("unsupported IFF compression {other}")),
    };

    let mut values = vec![0u32; width * height];
    let mut alpha = vec![255u8; width * height];
    for y in 0..height {
        let row = &unpacked[y * row_size..(y + 1) * row_size];
        let out = &mut values[y * width..(y + 1) * width];
        if chunky {
            for (x, value) in out.iter_mut().enumerate() {
                *value = u32::from(row[x]);
            }
            continue;
        }
        for plane in 0..planes {
            let bits = &row[plane * plane_row_bytes..(plane + 1) * plane_row_bytes];
            for (x, value) in out.iter_mut().enumerate() {
                *value |= u32::from((bits[x / 8] >> (7 - x % 8)) & 1) << plane;
            }
        }
        if has_mask_plane {
            let mask = &row[planes * plane_row_bytes..];
            for x in 0..width {
                if (mask[x / 8] >> (7 - x % 8)) & 1 == 0 {
                    alpha[y * width + x] = 0;
                }
            }
        }
    }

    let colors: Vec<Rgb> = if planes >= 24 {
        values.iter().map(|&v| [v as u8, (v >> 8) as u8, (v >> 16) as u8]).collect()
    } else {
        let palette = expand_ocs_palette(cmap);
        if camg & CAMG_HAM != 0 && planes >= 3 {
            decode_ham(&values, width, planes, &palette)
        } else {
            let palette = build_palette(palette, planes, camg & CAMG_EHB != 0);
            values.iter().map(|&v| palette[v as usize]).collect()
        }
    };

    let mut pixels = Vec::with_capacity(width * height * 4);
    for (index, [r, g, b]) in colors.into_iter().enumerate() {
        let a = if planes == 32 { (values[index] >> 24) as u8 } else { alpha[index] };
        pixels.extend_from_slice(&[r, g, b, a]);
    }
    RgbaImage::from_raw(width as u32, height as u32, pixels).ok_or_else(|| "invalid IFF dimensions".to_string())
}

/// Old (OCS) paint programs stored 4-bit guns in the high nibble; stretch them to full range.
fn expand_ocs_palette(mut cmap: Vec<Rgb>) -> Vec<Rgb> {
    if !cmap.is_empty() && cmap.iter().flatten().all(|v| v.trailing_zeros() >= 4) {
        for v in cmap.iter_mut().flatten() {
            *v |= *v >> 4;
        }
    }
    cmap
}

fn build_palette(mut palette: Vec<Rgb>, planes: usize, extra_half_brite: bool) -> Vec<Rgb> {
    let size = 1usize << planes;
    if extra_half_brite {
        palette.resize(32, [0, 0, 0]);
        let half: Vec<Rgb> = palette.iter().map(|[r, g, b]| [r >> 1, g >> 1, b >> 1]).collect();
        palette.extend(half);
    }
    let known = palette.len();
    for index in known..size {
        let v = (index * 255 / (size - 1).max(1)) as u8;
        palette.push([v, v, v]);
    }
    palette.resize(size.max(256), [0, 0, 0]);
    palette
}

/// Hold-And-Modify: the top two bits pick "palette color" or "modify blue/red/green of the
/// previous pixel"; each scanline starts from the background color.
fn decode_ham(values: &[u32], width: usize, planes: usize, palette: &[Rgb]) -> Vec<Rgb> {
    let value_bits = planes - 2;
    let value_mask = (1u32 << value_bits) - 1;
    let modify = |old: u8, value: u32| -> u8 {
        let value = value as u8;
        match value_bits {
            4 => value << 4 | value,
            6 => value << 2 | (old & 0x03),
            bits => ((u32::from(value) * 255) / ((1u32 << bits) - 1)) as u8,
        }
    };
    let background = palette.first().copied().unwrap_or([0, 0, 0]);
    let mut out = Vec::with_capacity(values.len());
    for row in values.chunks(width) {
        let mut color = background;
        for &code in row {
            let value = code & value_mask;
            color = match code >> value_bits {
                0 => palette.get(value as usize).copied().unwrap_or([0, 0, 0]),
                1 => [color[0], color[1], modify(color[2], value)],
                2 => [modify(color[0], value), color[1], color[2]],
                _ => [color[0], modify(color[1], value), color[2]],
            };
            out.push(color);
        }
    }
    out
}

/// `PackBits` as used by IFF; a short stream is padded with zeros.
fn byte_run1(src: &[u8], size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(size);
    let mut i = 0;
    while i < src.len() && out.len() < size {
        let n = src[i] as i8;
        i += 1;
        if n >= 0 {
            let count = (n as usize + 1).min(src.len() - i).min(size - out.len());
            out.extend_from_slice(&src[i..i + count]);
            i += n as usize + 1;
        } else if n != -128 {
            let Some(&value) = src.get(i) else { break };
            i += 1;
            let count = ((1 - i32::from(n)) as usize).min(size - out.len());
            out.resize(out.len() + count, value);
        }
    }
    out.resize(size, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = id.to_vec();
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            out.push(0);
        }
        out
    }

    fn form(kind: &[u8; 4], chunks: &[Vec<u8>]) -> Vec<u8> {
        let mut inner = kind.to_vec();
        for c in chunks {
            inner.extend_from_slice(c);
        }
        let mut out = b"FORM".to_vec();
        out.extend_from_slice(&(inner.len() as u32).to_be_bytes());
        out.extend_from_slice(&inner);
        out
    }

    fn bmhd(width: u16, height: u16, planes: u8, masking: u8, compression: u8) -> Vec<u8> {
        let mut payload = vec![0u8; 20];
        payload[0..2].copy_from_slice(&width.to_be_bytes());
        payload[2..4].copy_from_slice(&height.to_be_bytes());
        payload[8] = planes;
        payload[9] = masking;
        payload[10] = compression;
        chunk(b"BMHD", &payload)
    }

    #[test]
    fn decodes_byterun1_ilbm_with_ocs_palette() {
        let cmap = chunk(b"CMAP", &[0, 0, 0, 0xF0, 0, 0, 0, 0xF0, 0, 0, 0, 0xF0]);
        // Two planes, 3 px wide (rows padded to 2 bytes): plane 0 = 0b010, plane 1 = 0b011.
        // Plane 0 is a two-byte literal; plane 1 a one-byte literal plus a zero run.
        let body = chunk(b"BODY", &[0x01, 0x40, 0x00, 0x00, 0x60, 0xFF, 0x00]);
        let image = decode(&form(b"ILBM", &[bmhd(3, 1, 2, 0, 1), cmap, body])).unwrap();
        assert_eq!(image.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(image.get_pixel(1, 0).0, [0, 0, 0xFF, 255]);
        assert_eq!(image.get_pixel(2, 0).0, [0, 0xFF, 0, 255]);
    }

    #[test]
    fn decodes_pbm_mask_plane_and_ehb() {
        let cmap = chunk(b"CMAP", &[10, 20, 30, 40, 50, 60, 70, 80, 90]);
        let pbm = form(b"PBM ", &[bmhd(3, 1, 8, 0, 0), cmap.clone(), chunk(b"BODY", &[2, 1, 0, 0])]);
        let image = decode(&pbm).unwrap();
        assert_eq!(image.get_pixel(0, 0).0, [70, 80, 90, 255]);
        assert_eq!(image.get_pixel(2, 0).0, [10, 20, 30, 255]);

        // One plane plus mask: pixel 0 set but masked out, pixel 1 set and visible.
        let masked = form(b"ILBM", &[bmhd(2, 1, 1, 1, 0), cmap.clone(), chunk(b"BODY", &[0xC0, 0, 0x40, 0])]);
        let image = decode(&masked).unwrap();
        assert_eq!(image.get_pixel(0, 0).0[3], 0);
        assert_eq!(image.get_pixel(1, 0).0, [40, 50, 60, 255]);

        // Six planes, EHB: index 33 is color 1 at half brightness.
        let mut body = vec![0u8; 12];
        body[0] = 0x80;
        body[10] = 0x80;
        let ehb = form(
            b"ILBM",
            &[bmhd(1, 1, 6, 0, 0), cmap, chunk(b"CAMG", &CAMG_EHB.to_be_bytes()), chunk(b"BODY", &body)],
        );
        assert_eq!(decode(&ehb).unwrap().get_pixel(0, 0).0, [20, 25, 30, 255]);
    }

    #[test]
    fn decodes_ham6() {
        let cmap = chunk(b"CMAP", &[0x10, 0x20, 0x30]);
        // Pixel 0: modify red to 0xF (control 10). Pixel 1: modify green to 0 (control 11).
        let mut body = vec![0u8; 12];
        body[0] = 0x80;
        body[2] = 0x80;
        body[4] = 0x80;
        body[6] = 0x80;
        body[8] = 0x40;
        body[10] = 0xC0;
        let ham = form(
            b"ILBM",
            &[bmhd(2, 1, 6, 0, 0), cmap, chunk(b"CAMG", &CAMG_HAM.to_be_bytes()), chunk(b"BODY", &body)],
        );
        let image = decode(&ham).unwrap();
        // 0x10/0x20/0x30 have zero low nibbles, so the OCS expansion applies to the palette.
        assert_eq!(image.get_pixel(0, 0).0, [0xFF, 0x22, 0x33, 255]);
        assert_eq!(image.get_pixel(1, 0).0, [0xFF, 0x00, 0x33, 255]);
    }

    #[test]
    fn decodes_first_frame_of_anim() {
        let frame = form(b"ILBM", &[bmhd(1, 1, 1, 0, 0), chunk(b"CMAP", &[0, 0, 0, 1, 2, 3]), chunk(b"BODY", &[0x80, 0])]);
        let mut anim = b"FORM".to_vec();
        anim.extend_from_slice(&((4 + frame.len()) as u32).to_be_bytes());
        anim.extend_from_slice(b"ANIM");
        anim.extend_from_slice(&frame);
        assert_eq!(decode(&anim).unwrap().get_pixel(0, 0).0, [1, 2, 3, 255]);
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode(b"FORM\0\0\0\x04ILBM").is_err());
        assert!(decode(b"FORM\0\0\0\x04ANIM").is_err());
        assert!(decode(&form(b"ILBM", &[bmhd(0, 1, 1, 0, 0), chunk(b"BODY", &[])])).is_err());
    }
}
