//! Renders bitmap and stroke fonts that don't fit [`crate::BitFont`] (proportional, wider than
//! 8 pixels, colored or vector) as a specimen sheet: name, sample text and a glyph table.
//!
//! The parsers follow kaleidotron's font viewers (MIT, Copyright (c) 2026 Rick Christy).

pub(crate) mod amiga;
pub(crate) mod bgi;
pub(crate) mod windows;

use std::collections::BTreeMap;

use image::RgbaImage;

use crate::BitFont;

type Rgb = [u8; 3];

const BACKGROUND: [u8; 4] = [0x1C, 0x1C, 0x22, 0xFF];
const CELL: [u8; 4] = [0x2A, 0x2A, 0x33, 0xFF];
const TITLE: [u8; 4] = [0xFF, 0xD0, 0x60, 0xFF];
const DETAIL: [u8; 4] = [0x9A, 0x9A, 0xA8, 0xFF];
pub(crate) const INK: Rgb = [0xF0, 0xF0, 0xF0];

const MARGIN: usize = 12;
const GAP: usize = 6;
const LABEL_HEIGHT: usize = 16;
const MAX_WIDTH: usize = 4096;
const MAX_HEIGHT: usize = 16_384;
const MAX_FACES: usize = 32;
const SAMPLES: [&str; 3] = [
    "The quick brown fox jumps over the lazy dog.",
    "ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789",
    "abcdefghijklmnopqrstuvwxyz !?&@#$%*()",
];

/// One glyph; `pixels` holds `width × face.height` palette indices where 0 is transparent.
pub(crate) struct Glyph {
    pub width: usize,
    /// Horizontal distance to the next glyph in running text.
    pub advance: i32,
    /// Offset of the bitmap from the pen position (Amiga kerning).
    pub offset: i32,
    pub pixels: Vec<u8>,
}

pub(crate) struct Face {
    pub name: String,
    pub detail: String,
    pub height: usize,
    /// Colors for pixel values 1..; index 0 is transparent and never read.
    pub palette: Vec<Rgb>,
    pub glyphs: BTreeMap<u32, Glyph>,
}

impl Face {
    pub(crate) fn new(name: String, detail: String, height: usize) -> Self {
        Self {
            name,
            detail,
            height,
            palette: vec![[0, 0, 0], INK],
            glyphs: BTreeMap::new(),
        }
    }

    fn scale(&self) -> usize {
        24usize.div_ceil(self.height.max(1)).clamp(1, 4)
    }

    fn text_width(&self, text: &str) -> usize {
        let scale = self.scale() as i32;
        let width: i32 = text.chars().map(|ch| self.glyph_for(ch).map_or(0, |g| g.advance.max(0) * scale)).sum();
        width.max(0) as usize
    }

    fn glyph_for(&self, ch: char) -> Option<&Glyph> {
        self.glyphs.get(&(ch as u32)).or_else(|| self.glyphs.get(&(ch.to_ascii_uppercase() as u32)))
    }

    fn cell_size(&self) -> (usize, usize) {
        let scale = self.scale();
        let widest = self.glyphs.values().map(|g| g.width).max().unwrap_or(1).max(1);
        (widest * scale + GAP, self.height * scale + GAP)
    }

    fn grid_rows(&self) -> Vec<u32> {
        let mut rows: Vec<u32> = self.glyphs.keys().map(|code| code / 16).collect();
        rows.dedup();
        rows
    }

    fn block_size(&self) -> (usize, usize) {
        let scale = self.scale();
        let (cell_w, cell_h) = self.cell_size();
        let samples_width = SAMPLES.iter().map(|s| self.text_width(s)).max().unwrap_or(0);
        let label_width = (self.name.chars().count().max(self.detail.chars().count())) * 8;
        let width = samples_width.max(16 * cell_w).max(label_width);
        let height = 2 * LABEL_HEIGHT + GAP + SAMPLES.len() * (self.height * scale + GAP) + GAP + self.grid_rows().len() * cell_h;
        (width, height)
    }
}

pub(crate) fn render(faces: &[Face]) -> Result<RgbaImage, String> {
    if faces.is_empty() || faces.iter().all(|face| face.glyphs.is_empty()) {
        return Err("font contains no glyphs".to_string());
    }
    let mut blocks = Vec::new();
    let mut height = MARGIN;
    let mut width = 0;
    for face in faces.iter().filter(|face| !face.glyphs.is_empty()).take(MAX_FACES) {
        let (w, h) = face.block_size();
        if height + h + MARGIN > MAX_HEIGHT && !blocks.is_empty() {
            break;
        }
        blocks.push((face, height));
        height += h + 2 * MARGIN;
        width = width.max(w);
    }
    let width = (width + 2 * MARGIN).min(MAX_WIDTH);
    let height = height.min(MAX_HEIGHT);

    let mut canvas = Canvas::new(width, height);
    let label_font = BitFont::default();
    for (face, top) in blocks {
        let scale = face.scale();
        let mut y = top;
        canvas.label(&label_font, MARGIN, y, &face.name, TITLE);
        y += LABEL_HEIGHT;
        canvas.label(&label_font, MARGIN, y, &face.detail, DETAIL);
        y += LABEL_HEIGHT + GAP;
        for sample in SAMPLES {
            let mut x = MARGIN as i32;
            for ch in sample.chars() {
                if let Some(glyph) = face.glyph_for(ch) {
                    canvas.glyph(face, glyph, x + glyph.offset * scale as i32, y as i32, scale);
                    x += glyph.advance * scale as i32;
                }
            }
            y += face.height * scale + GAP;
        }
        y += GAP;
        let (cell_w, cell_h) = face.cell_size();
        for (row_index, row) in face.grid_rows().into_iter().enumerate() {
            for column in 0..16 {
                let Some(glyph) = face.glyphs.get(&(row * 16 + column)) else { continue };
                let x = MARGIN + column as usize * cell_w;
                let cell_y = y + row_index * cell_h;
                canvas.fill(x, cell_y, cell_w - 2, cell_h - 2, CELL);
                canvas.glyph(face, glyph, (x + GAP / 2 - 1) as i32, (cell_y + GAP / 2 - 1) as i32, scale);
            }
        }
    }
    RgbaImage::from_raw(width as u32, height as u32, canvas.pixels).ok_or_else(|| "invalid specimen size".to_string())
}

struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: BACKGROUND.repeat(width * height),
        }
    }

    fn put(&mut self, x: i32, y: i32, color: [u8; 4]) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            let offset = (y as usize * self.width + x as usize) * 4;
            self.pixels[offset..offset + 4].copy_from_slice(&color);
        }
    }

    fn fill(&mut self, x: usize, y: usize, width: usize, height: usize, color: [u8; 4]) {
        for dy in 0..height {
            for dx in 0..width {
                self.put((x + dx) as i32, (y + dy) as i32, color);
            }
        }
    }

    fn label(&mut self, font: &BitFont, x: usize, y: usize, text: &str, color: [u8; 4]) {
        for (i, ch) in text.chars().enumerate() {
            let glyph = font.glyph(if ch.is_ascii() { ch } else { '?' });
            for gy in 0..16 {
                for gx in 0..8 {
                    if glyph.get_pixel(gx, gy) {
                        self.put((x + i * 8 + gx) as i32, (y + gy) as i32, color);
                    }
                }
            }
        }
    }

    fn glyph(&mut self, face: &Face, glyph: &Glyph, x: i32, y: i32, scale: usize) {
        for gy in 0..face.height {
            for gx in 0..glyph.width {
                let value = glyph.pixels.get(gy * glyph.width + gx).copied().unwrap_or(0) as usize;
                if value == 0 {
                    continue;
                }
                let [r, g, b] = face.palette.get(value).copied().unwrap_or(INK);
                for sy in 0..scale {
                    for sx in 0..scale {
                        self.put(x + (gx * scale + sx) as i32, y + (gy * scale + sy) as i32, [r, g, b, 0xFF]);
                    }
                }
            }
        }
    }
}

fn nul_terminated(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_face_with_proportional_glyphs() {
        let mut face = Face::new("Test".to_string(), "2 px".to_string(), 2);
        face.glyphs.insert(
            'A' as u32,
            Glyph {
                width: 2,
                advance: 3,
                offset: 0,
                pixels: vec![1, 0, 0, 1],
            },
        );
        let image = render(&[face]).unwrap();
        assert!(image.width() as usize >= 16 * (2 * 4 + GAP));
        let ink = image.pixels().filter(|p| p.0 == [INK[0], INK[1], INK[2], 0xFF]).count();
        // One 'a'/'A' per sample line (lowercase falls back to uppercase) plus the grid cell, at 4× scale.
        assert_eq!(ink, 4 * 2 * 16);
        assert!(render(&[]).is_err());
    }
}
