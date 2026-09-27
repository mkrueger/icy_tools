//! Windows raster fonts: bare `.fnt` resources and `.fon` NE libraries that bundle several.

use std::fmt::Write;

use image::RgbaImage;

use super::{nul_terminated, render, Face, Glyph};

const RT_FONT: u16 = 0x8008;
const FNT_HEADER_V1: usize = 0x75;
const FNT_HEADER_V2: usize = 0x76;
const FNT_HEADER_V3: usize = 0x94;
/// dfType bit 0: a vector font, which has no bitmaps to show.
const TYPE_VECTOR: u16 = 1;

pub(crate) fn decode(data: &[u8]) -> Result<RgbaImage, String> {
    let faces: Vec<Face> = if data.starts_with(b"MZ") {
        font_resources(data)?.into_iter().filter_map(parse_fnt).collect()
    } else {
        // DOS programs used `.fnt` for raw 8×N VGA fonts as well.
        parse_fnt(data).or_else(|| parse_raw_dos_font(data)).into_iter().collect()
    };
    if faces.is_empty() {
        return Err("no raster font found (vector and TrueType fonts are not supported)".to_string());
    }
    render(&faces)
}

fn u16_at(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*data.get(offset)?, *data.get(offset + 1)?]))
}

fn u32_at(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(offset..offset + 4)?.try_into().ok()?))
}

/// Walks the NE resource table of a `.fon` and returns every `RT_FONT` resource.
fn font_resources(data: &[u8]) -> Result<Vec<&[u8]>, String> {
    let ne = u32_at(data, 0x3C).ok_or("truncated MZ header")? as usize;
    if data.get(ne..ne + 2) != Some(b"NE") {
        return Err("not a 16-bit Windows font library (no NE header)".to_string());
    }
    let mut offset = ne + u16_at(data, ne + 0x24).ok_or("truncated NE header")? as usize;
    let shift = u16_at(data, offset).ok_or("truncated resource table")?;
    if shift > 16 {
        return Err("invalid resource alignment".to_string());
    }
    offset += 2;
    let mut fonts = Vec::new();
    while let Some(type_id) = u16_at(data, offset) {
        if type_id == 0 {
            break;
        }
        let count = u16_at(data, offset + 2).ok_or("truncated resource table")? as usize;
        offset += 8;
        for entry in 0..count {
            let at = offset + entry * 12;
            let (Some(start), Some(length)) = (u16_at(data, at), u16_at(data, at + 2)) else {
                break;
            };
            let start = (start as usize) << shift;
            let end = start + ((length as usize) << shift);
            if type_id == RT_FONT {
                if let Some(resource) = data.get(start..end.min(data.len())) {
                    fonts.push(resource);
                }
            }
        }
        offset += count * 12;
    }
    Ok(fonts)
}

fn parse_fnt(data: &[u8]) -> Option<Face> {
    let version = u16_at(data, 0)?;
    if !matches!(version, 0x100 | 0x200 | 0x300) || data.len() < FNT_HEADER_V1 {
        return None;
    }
    let font_type = u16_at(data, 0x42)?;
    if font_type & TYPE_VECTOR != 0 {
        return None;
    }
    let points = u16_at(data, 0x44)?;
    let italic = data[0x50] != 0;
    let weight = u16_at(data, 0x53)?;
    let pixel_width = u16_at(data, 0x56)? as usize;
    let height = u16_at(data, 0x58)? as usize;
    let (first, last) = (data[0x5F], data[0x60]);
    if height == 0 || height > 256 || last < first {
        return None;
    }
    let name = u32_at(data, 0x69)
        .and_then(|at| data.get(at as usize..))
        .map(nul_terminated)
        .filter(|n| !n.is_empty());

    let mut detail = format!("{points} pt, {height} px high, ");
    if pixel_width == 0 {
        detail.push_str("proportional");
    } else {
        let _ = write!(detail, "{pixel_width} px wide");
    }
    if weight >= 700 {
        detail.push_str(", bold");
    }
    if italic {
        detail.push_str(", italic");
    }
    let mut face = Face::new(name.unwrap_or_else(|| "Windows font".to_string()), detail, height);

    let count = (last - first) as usize + 1;
    if version == 0x100 {
        // Version 1 stores one row-major bitmap strip; proportional fonts carry bit offsets.
        let row_bytes = u16_at(data, 0x63)? as usize;
        let bits = data.get(u32_at(data, 0x71)? as usize..)?;
        for index in 0..count {
            let (start, width) = if pixel_width == 0 {
                let start = u16_at(data, FNT_HEADER_V1 + index * 2)? as usize;
                (start, (u16_at(data, FNT_HEADER_V1 + index * 2 + 2)? as usize).saturating_sub(start))
            } else {
                (index * pixel_width, pixel_width)
            };
            let mut pixels = Vec::with_capacity(width * height);
            for y in 0..height {
                for x in start..start + width {
                    let byte = bits.get(y * row_bytes + x / 8).copied().unwrap_or(0);
                    pixels.push((byte >> (7 - x % 8)) & 1);
                }
            }
            insert(&mut face, first as u32 + index as u32, width, pixels);
        }
    } else {
        let (table, entry_size) = if version == 0x300 { (FNT_HEADER_V3, 6) } else { (FNT_HEADER_V2, 4) };
        for index in 0..count {
            let entry = table + index * entry_size;
            let width = u16_at(data, entry)? as usize;
            let offset = if version == 0x300 {
                u32_at(data, entry + 2)? as usize
            } else {
                u16_at(data, entry + 2)? as usize
            };
            if width > 256 {
                continue;
            }
            // Glyphs are stored as columns of 8-pixel-wide strips, each `height` bytes tall.
            let mut pixels = Vec::with_capacity(width * height);
            for y in 0..height {
                for x in 0..width {
                    let byte = data.get(offset + (x / 8) * height + y).copied().unwrap_or(0);
                    pixels.push((byte >> (7 - x % 8)) & 1);
                }
            }
            insert(&mut face, first as u32 + index as u32, width, pixels);
        }
    }
    Some(face)
}

/// A headerless VGA font: 256 glyphs, 8 pixels wide, one byte per row.
fn parse_raw_dos_font(data: &[u8]) -> Option<Face> {
    let height = data.len() / 256;
    if !(1..=32).contains(&height) || data.len() % 256 > 8 {
        return None;
    }
    let mut face = Face::new("DOS bitmap font".to_string(), format!("8x{height} px, 256 glyphs, no header"), height);
    for (code, rows) in data.chunks_exact(height).take(256).enumerate() {
        let pixels = rows.iter().flat_map(|row| (0..8).map(move |x| (row >> (7 - x)) & 1)).collect();
        insert(&mut face, code as u32, 8, pixels);
    }
    Some(face)
}

fn insert(face: &mut Face, code: u32, width: usize, pixels: Vec<u8>) {
    let glyph = Glyph {
        width,
        advance: width as i32,
        offset: 0,
        pixels,
    };
    face.glyphs.insert(code, glyph);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A version 2 FNT with glyphs 'A' (3 px wide) and 'B' (10 px wide, two strips), 2 px high.
    pub(crate) fn sample_fnt() -> Vec<u8> {
        let mut data = vec![0u8; FNT_HEADER_V2 + 3 * 4];
        data[0..2].copy_from_slice(&0x200u16.to_le_bytes());
        data[0x44..0x46].copy_from_slice(&10u16.to_le_bytes());
        data[0x53..0x55].copy_from_slice(&400u16.to_le_bytes());
        data[0x58..0x5A].copy_from_slice(&2u16.to_le_bytes());
        data[0x5F] = b'A';
        data[0x60] = b'B';
        let glyph_a = data.len();
        data.extend_from_slice(&[0b1010_0000, 0b0100_0000]);
        let glyph_b = data.len();
        data.extend_from_slice(&[0xFF, 0x00, 0b1100_0000, 0b0100_0000]);
        let name = data.len() as u32;
        data.extend_from_slice(b"Tiny\0");
        data[0x69..0x6D].copy_from_slice(&name.to_le_bytes());
        for (i, (width, offset)) in [(3u16, glyph_a), (10, glyph_b)].into_iter().enumerate() {
            let entry = FNT_HEADER_V2 + i * 4;
            data[entry..entry + 2].copy_from_slice(&width.to_le_bytes());
            data[entry + 2..entry + 4].copy_from_slice(&(offset as u16).to_le_bytes());
        }
        data
    }

    fn fon(resources: &[Vec<u8>]) -> Vec<u8> {
        let mut data = vec![0u8; 0x40];
        data[0..2].copy_from_slice(b"MZ");
        data[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        let ne = data.len();
        data.resize(ne + 0x40, 0);
        data[ne..ne + 2].copy_from_slice(b"NE");
        data[ne + 0x24..ne + 0x26].copy_from_slice(&0x40u16.to_le_bytes());
        // Resource table: shift 4, one RT_FONT type entry, terminator.
        let table = data.len();
        data.extend_from_slice(&4u16.to_le_bytes());
        data.extend_from_slice(&RT_FONT.to_le_bytes());
        data.extend_from_slice(&(resources.len() as u16).to_le_bytes());
        data.extend_from_slice(&[0; 4]);
        let entries = data.len();
        data.resize(entries + resources.len() * 12 + 2, 0);
        for (i, resource) in resources.iter().enumerate() {
            data.resize(data.len().div_ceil(16) * 16, 0);
            let start = data.len();
            data.extend_from_slice(resource);
            let entry = entries + i * 12;
            data[entry..entry + 2].copy_from_slice(&((start >> 4) as u16).to_le_bytes());
            data[entry + 2..entry + 4].copy_from_slice(&(resource.len().div_ceil(16) as u16).to_le_bytes());
        }
        assert_eq!(u16_at(&data, table), Some(4));
        data
    }

    #[test]
    fn parses_v2_glyph_strips() {
        let face = parse_fnt(&sample_fnt()).unwrap();
        assert_eq!(face.name, "Tiny");
        assert_eq!(face.glyphs[&('A' as u32)].pixels, vec![1, 0, 1, 0, 1, 0]);
        let b = &face.glyphs[&('B' as u32)];
        assert_eq!(b.width, 10);
        assert_eq!(&b.pixels[0..10], &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1]);
        assert_eq!(&b.pixels[10..20], &[0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn decodes_fon_libraries_and_rejects_garbage() {
        let image = decode(&fon(&[sample_fnt(), sample_fnt()])).unwrap();
        assert!(image.height() > 100);
        assert!(decode(&sample_fnt()).is_ok());
        assert!(decode(b"MZ").is_err());
        assert!(decode(&[0u8; 300]).is_err());
        assert!(decode(&[0x18u8; 256 * 14]).is_ok(), "raw 8×14 DOS font");
        let mut vector = sample_fnt();
        vector[0x42] = 1;
        assert!(decode(&vector).is_err());
    }
}
