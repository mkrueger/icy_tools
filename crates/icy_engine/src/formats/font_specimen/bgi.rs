//! Borland BGI stroke fonts (`.chr`), rasterized at their design size.

use image::RgbaImage;

use super::{render, Face, Glyph};
use crate::palette_screen_buffer::bgi::{Font, StrokeType};

pub(crate) fn decode(data: &[u8]) -> Result<RgbaImage, String> {
    if !data.starts_with(b"PK\x08\x08") {
        return Err("not a BGI stroke font".to_string());
    }
    let font = Font::load(data).map_err(|error| format!("invalid BGI font: {error}"))?;
    let top = i32::from(font.org_to_cap);
    let bottom = i32::from(font.org_to_dec).min(0);
    // One pixel of slack keeps strokes that touch the outer design lines inside the bitmap.
    let height = (top - bottom + 3).clamp(1, 512) as usize;
    let mut face = Face::new(
        format!("{} stroke font", font.name.trim()),
        format!("Borland BGI vector font, {} units high", top - bottom),
        height,
    );
    for code in 0..=255u8 {
        let Some(character) = font.character(code) else { continue };
        if character.strokes.is_empty() && character.width <= 0 {
            continue;
        }
        let width = (character.width.clamp(0, 512) + 2) as usize;
        let mut pixels = vec![0u8; width * height];
        let map = |x: i32, y: i32| (x + 1, top + 1 - y);
        let mut pen = map(0, 0);
        for stroke in &character.strokes {
            let target = map(stroke.x, stroke.y);
            if matches!(stroke.stype, StrokeType::LineTo) {
                line(&mut pixels, width, height, pen, target);
            }
            pen = target;
        }
        face.glyphs.insert(
            u32::from(code),
            Glyph {
                width,
                advance: character.width.max(0),
                offset: 0,
                pixels,
            },
        );
    }
    render(&[face])
}

fn line(pixels: &mut [u8], width: usize, height: usize, (mut x0, mut y0): (i32, i32), (x1, y1): (i32, i32)) {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let mut error = dx + dy;
    loop {
        if x0 >= 0 && y0 >= 0 && (x0 as usize) < width && (y0 as usize) < height {
            pixels[y0 as usize * width + x0 as usize] = 1;
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * error;
        if e2 >= dy {
            error += dy;
            x0 += sx;
        }
        if e2 <= dx {
            error += dx;
            y0 += sy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_bundled_stroke_fonts() {
        for name in ["TRIP", "SANS", "LITT", "GOTH"] {
            let path = format!("{}/src/palette_screen_buffer/bgi/fonts/{name}.CHR", env!("CARGO_MANIFEST_DIR"));
            let image = decode(&std::fs::read(path).unwrap()).unwrap();
            assert!(image.width() > 200 && image.height() > 100, "{name}: {:?}", image.dimensions());
        }
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode(b"PK\x08\x08").is_err());
        assert!(decode(b"PK\x08\x08BGI \x1a\xff\xff").is_err());
        assert!(decode(b"hello").is_err());
    }
}
