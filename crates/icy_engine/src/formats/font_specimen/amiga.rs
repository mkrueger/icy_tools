//! Amiga disk fonts: size files (hunk executables holding a `TextFont`, including 8-plane
//! `ColorFonts`) and the `.font` contents file that lists a family's sizes.

use std::fmt::Write;
use std::path::Path;

use image::RgbaImage;

use super::{nul_terminated, render, Face, Glyph, INK};

const HUNK_HEADER: u32 = 0x3F3;
const HUNK_CODE: u32 = 0x3E9;
const DFH_ID: u16 = 0x0F80;
const FCH_ID: u16 = 0x0F00;
const TFCH_ID: u16 = 0x0F02;
const FSF_COLORFONT: u8 = 0x40;
const FPF_PROPORTIONAL: u8 = 0x20;
const CONTENTS_RECORD: usize = 260;
const MAX_SIZES: usize = 32;

fn u16_at(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*data.get(offset)?, *data.get(offset + 1)?]))
}

fn u32_at(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(offset..offset + 4)?.try_into().ok()?))
}

/// True if `data` looks like an Amiga font size file; they usually have no extension.
pub(crate) fn is_size_file(data: &[u8]) -> bool {
    code_segment(data).is_some_and(|segment| u16_at(data, segment + 18) == Some(DFH_ID))
}

pub(crate) fn is_contents_file(data: &[u8]) -> bool {
    matches!(u16_at(data, 0), Some(FCH_ID | TFCH_ID))
}

pub(crate) fn decode(data: &[u8]) -> Result<RgbaImage, String> {
    if is_contents_file(data) {
        let sizes = contents(data).into_iter().map(|(_, size)| size.to_string()).collect::<Vec<_>>().join(", ");
        return Err(format!("Amiga font contents file (sizes {sizes}); the glyphs are in its size files"));
    }
    let mut face = parse_size_file(data)?;
    if face.name.is_empty() {
        face.name = "Amiga font".to_string();
    }
    render(&[face])
}

/// Like [`decode`], but a `.font` contents file is rendered from the size files next to it.
pub(crate) fn decode_path(data: &[u8], path: &Path) -> Result<RgbaImage, String> {
    let directory = path.parent().unwrap_or(Path::new(""));
    if !is_contents_file(data) {
        // Size files live in a directory named after the family (`topaz/8`).
        let mut face = parse_size_file(data)?;
        if face.name.is_empty() {
            face.name = directory.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        }
        return render(&[face]);
    }
    let family = path.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut faces: Vec<(u16, Face)> = contents(data)
        .into_iter()
        .filter_map(|(file, size)| {
            let bytes = std::fs::read(resolve_case_insensitive(directory, &file)?).ok()?;
            let mut face = parse_size_file(&bytes).ok()?;
            if face.name.is_empty() {
                face.name.clone_from(&family);
            }
            Some((size, face))
        })
        .collect();
    if faces.is_empty() {
        return decode(data);
    }
    faces.sort_by_key(|(size, _)| *size);
    render(&faces.into_iter().map(|(_, face)| face).collect::<Vec<_>>())
}

/// `AmigaOS` file names are case-insensitive, and contents files rarely match the case on disk.
fn resolve_case_insensitive(directory: &Path, relative: &str) -> Option<std::path::PathBuf> {
    let mut path = directory.to_path_buf();
    for part in relative.split('/').filter(|part| !part.is_empty()) {
        let exact = path.join(part);
        path = if exact.exists() {
            exact
        } else {
            std::fs::read_dir(&path)
                .ok()?
                .filter_map(Result::ok)
                .find(|entry| entry.file_name().to_string_lossy().eq_ignore_ascii_case(part))?
                .path()
        };
    }
    Some(path)
}

/// The (relative path, height) records of a `.font` contents file.
fn contents(data: &[u8]) -> Vec<(String, u16)> {
    let count = u16_at(data, 2).unwrap_or(0) as usize;
    (0..count.min(MAX_SIZES))
        .filter_map(|i| {
            let record = 4 + i * CONTENTS_RECORD;
            let file = nul_terminated(data.get(record..record + 256)?);
            let size = u16_at(data, record + 256)?;
            // Paths are relative to the contents file; never let them escape its directory.
            let safe = !file.is_empty() && !file.contains(':') && !file.starts_with('/') && !file.split('/').any(|part| part == "..");
            safe.then_some((file, size))
        })
        .collect()
}

/// Offset of the first code hunk's contents: every pointer in the font is relative to it.
fn code_segment(data: &[u8]) -> Option<usize> {
    if u32_at(data, 0)? != HUNK_HEADER {
        return None;
    }
    let mut offset = 4;
    loop {
        let name_longs = u32_at(data, offset)? as usize;
        offset += 4;
        if name_longs == 0 {
            break;
        }
        offset = offset.checked_add(name_longs.checked_mul(4)?)?;
    }
    let (first, last) = (u32_at(data, offset + 4)?, u32_at(data, offset + 8)?);
    if last < first || last - first > 64 {
        return None;
    }
    offset += 12 + (last - first + 1) as usize * 4;
    (u32_at(data, offset)? & 0x3FFF_FFFF == HUNK_CODE).then_some(offset + 8)
}

fn parse_size_file(data: &[u8]) -> Result<Face, String> {
    let segment = code_segment(data).ok_or("not an Amiga font file")?;
    if u16_at(data, segment + 18) != Some(DFH_ID) {
        return Err("Amiga hunk file without a DiskFontHeader".to_string());
    }
    let seg = &data[segment..];
    let name = seg.get(26..58).map(nul_terminated).unwrap_or_default();
    let tf = 58;
    let truncated = || "truncated Amiga TextFont".to_string();
    let height = u16_at(seg, tf + 20).ok_or_else(truncated)? as usize;
    let style = *seg.get(tf + 22).ok_or_else(truncated)?;
    let flags = *seg.get(tf + 23).ok_or_else(truncated)?;
    let x_size = u16_at(seg, tf + 24).ok_or_else(truncated)? as i32;
    let (lo, hi) = (*seg.get(tf + 32).ok_or_else(truncated)?, *seg.get(tf + 33).ok_or_else(truncated)?);
    let char_data = u32_at(seg, tf + 34).ok_or_else(truncated)? as usize;
    let modulo = u16_at(seg, tf + 38).ok_or_else(truncated)? as usize;
    let char_loc = u32_at(seg, tf + 40).ok_or_else(truncated)? as usize;
    let char_space = u32_at(seg, tf + 44).ok_or_else(truncated)? as usize;
    let char_kern = u32_at(seg, tf + 48).ok_or_else(truncated)? as usize;
    if height == 0 || height > 512 || hi < lo || modulo == 0 {
        return Err("implausible Amiga font metrics".to_string());
    }

    let color = style & FSF_COLORFONT != 0;
    let (planes, palette) = if color {
        let ctf = tf + 52;
        let depth = *seg.get(ctf + 2).ok_or_else(truncated)? as usize;
        if !(1..=8).contains(&depth) {
            return Err("implausible ColorFont depth".to_string());
        }
        let planes = (0..depth)
            .map(|i| u32_at(seg, ctf + 12 + 4 * i).map(|p| p as usize))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(truncated)?;
        (planes, color_table(seg, u32_at(seg, ctf + 8).unwrap_or(0) as usize, depth))
    } else {
        (vec![char_data], vec![[0, 0, 0], INK])
    };

    let mut detail = format!("Amiga font, {height} px high, ");
    detail.push_str(if flags & FPF_PROPORTIONAL != 0 { "proportional" } else { "fixed width" });
    if color {
        let _ = write!(detail, ", {} colors", palette.len());
    }
    let mut face = Face::new(name, detail, height);
    face.palette = palette;

    let signed = |table: usize, index: usize| -> Option<i32> { (table != 0).then(|| u16_at(seg, table + index * 2).map(|v| v as i16 as i32)).flatten() };
    for index in 0..=(hi - lo) as usize {
        let (Some(bit_offset), Some(width)) = (u16_at(seg, char_loc + index * 4), u16_at(seg, char_loc + index * 4 + 2)) else {
            break;
        };
        let (bit_offset, width) = (bit_offset as usize, (width as usize).min(1024));
        let kern = signed(char_kern, index).unwrap_or(0);
        let space = signed(char_space, index).unwrap_or(x_size);
        if width == 0 && space <= 0 {
            continue;
        }
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let bit = bit_offset + x;
                let value = planes.iter().enumerate().fold(0u8, |acc, (plane, &base)| {
                    let byte = seg.get(base + y * modulo + bit / 8).copied().unwrap_or(0);
                    acc | (((byte >> (7 - bit % 8)) & 1) << plane)
                });
                pixels.push(value);
            }
        }
        face.glyphs.insert(
            u32::from(lo) + index as u32,
            Glyph {
                width,
                advance: kern + space,
                offset: kern,
                pixels,
            },
        );
    }
    Ok(face)
}

/// A `ColorFontColors` table of 0x0RGB words; a missing one falls back to a grey ramp.
fn color_table(seg: &[u8], colors: usize, depth: usize) -> Vec<[u8; 3]> {
    let size = 1usize << depth;
    let mut palette = Vec::new();
    if colors != 0 {
        let count = u16_at(seg, colors + 2).unwrap_or(0) as usize;
        let table = u32_at(seg, colors + 4).unwrap_or(0) as usize;
        for i in 0..count.min(256) {
            let Some(word) = u16_at(seg, table + i * 2) else { break };
            palette.push([((word >> 8) & 0xF) as u8 * 17, ((word >> 4) & 0xF) as u8 * 17, (word & 0xF) as u8 * 17]);
        }
    }
    if palette.len() < size {
        palette = (0..size).map(|i| (i * 255 / (size - 1).max(1)) as u8).map(|v| [v, v, v]).collect();
    }
    palette
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a size file around a code segment (pointers are segment relative).
    fn size_file(segment: &[u8]) -> Vec<u8> {
        let mut data = Vec::new();
        for long in [HUNK_HEADER, 0, 1, 0, 0, (segment.len() / 4) as u32, HUNK_CODE, (segment.len() / 4) as u32] {
            data.extend_from_slice(&long.to_be_bytes());
        }
        data.extend_from_slice(segment);
        data
    }

    fn put16(seg: &mut [u8], at: usize, v: u16) {
        seg[at..at + 2].copy_from_slice(&v.to_be_bytes());
    }

    fn put32(seg: &mut [u8], at: usize, v: u32) {
        seg[at..at + 4].copy_from_slice(&v.to_be_bytes());
    }

    /// Two glyphs 'A' (3 px) and 'B' (2 px) side by side in a 1-byte-modulo, 2-row bitmap.
    fn segment(color: bool) -> Vec<u8> {
        let mut seg = vec![0u8; 256];
        put16(&mut seg, 18, DFH_ID);
        seg[26..30].copy_from_slice(b"test");
        let tf = 58;
        put16(&mut seg, tf + 20, 2);
        seg[tf + 22] = if color { FSF_COLORFONT } else { 0 };
        seg[tf + 23] = FPF_PROPORTIONAL;
        put16(&mut seg, tf + 24, 3);
        seg[tf + 32] = b'A';
        seg[tf + 33] = b'B';
        put32(&mut seg, tf + 34, 160);
        put16(&mut seg, tf + 38, 1);
        put32(&mut seg, tf + 40, 170);
        put32(&mut seg, tf + 44, 180);
        // Bitmap rows: A = 101 / 010, B = 11 / 01.
        seg[160] = 0b1011_1000;
        seg[161] = 0b0100_1000;
        // CharLoc (offset, width) and CharSpace.
        for (i, (offset, width, space)) in [(0u16, 3u16, 4u16), (3, 2, 3)].into_iter().enumerate() {
            put16(&mut seg, 170 + i * 4, offset);
            put16(&mut seg, 172 + i * 4, width);
            put16(&mut seg, 180 + i * 2, space);
        }
        if color {
            let ctf = tf + 52;
            seg[ctf + 2] = 2;
            put32(&mut seg, ctf + 8, 200);
            put32(&mut seg, ctf + 12, 160);
            put32(&mut seg, ctf + 16, 164);
            // Second plane: only the top-left pixel of 'A'.
            seg[164] = 0x80;
            put16(&mut seg, 202, 4);
            put32(&mut seg, 204, 210);
            for (i, word) in [0x000u16, 0xF00, 0x0F0, 0x00F].into_iter().enumerate() {
                put16(&mut seg, 210 + i * 2, word);
            }
        }
        seg
    }

    #[test]
    fn parses_mono_proportional_size_file() {
        let data = size_file(&segment(false));
        assert!(is_size_file(&data));
        let face = parse_size_file(&data).unwrap();
        assert_eq!(face.name, "test");
        let a = &face.glyphs[&('A' as u32)];
        assert_eq!((a.width, a.advance), (3, 4));
        assert_eq!(a.pixels, vec![1, 0, 1, 0, 1, 0]);
        assert_eq!(face.glyphs[&('B' as u32)].pixels, vec![1, 1, 0, 1]);
        assert!(decode(&data).is_ok());
    }

    #[test]
    fn parses_color_font_planes_and_palette() {
        let face = parse_size_file(&size_file(&segment(true))).unwrap();
        assert_eq!(face.palette[1], [0xFF, 0, 0]);
        assert_eq!(face.palette[3], [0, 0, 0xFF]);
        assert_eq!(face.glyphs[&('A' as u32)].pixels, vec![3, 0, 1, 0, 1, 0]);
    }

    #[test]
    fn contents_file_paths_stay_inside_their_directory() {
        let mut data = vec![0u8; 4 + 3 * CONTENTS_RECORD];
        data[0..2].copy_from_slice(&FCH_ID.to_be_bytes());
        data[2..4].copy_from_slice(&3u16.to_be_bytes());
        for (i, name) in [&b"test/8"[..], b"../evil/8", b"/abs/8"].into_iter().enumerate() {
            let record = 4 + i * CONTENTS_RECORD;
            data[record..record + name.len()].copy_from_slice(name);
            data[record + 256..record + 258].copy_from_slice(&8u16.to_be_bytes());
        }
        assert_eq!(contents(&data), vec![("test/8".to_string(), 8)]);
        assert!(decode(&data).is_err());

        let directory = std::env::temp_dir().join(format!("icy_amiga_font_{}", std::process::id()));
        std::fs::create_dir_all(directory.join("TEST")).unwrap();
        std::fs::write(directory.join("TEST/8"), size_file(&segment(false))).unwrap();
        let result = decode_path(&data, &directory.join("test.font"));
        std::fs::remove_dir_all(&directory).unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode(&[0, 0, 3, 0xF3, 0xFF, 0xFF, 0xFF, 0xFF]).is_err());
        assert!(!is_size_file(b"hello"));
        let mut broken = segment(false);
        broken[58 + 20] = 0;
        broken[58 + 21] = 0;
        assert!(decode(&size_file(&broken)).is_err());
    }
}
