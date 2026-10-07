//! Bounded local image matching and rendered feedback for character-canvas drafts.

use std::{collections::HashMap, io::Cursor};

use icy_engine::{AttributeColor, AttributedChar, BufferType, IceMode, Position, Rectangle, TextAttribute, TextPane};
use image::{imageops, ImageFormat, RgbaImage};
use serde_json::{json, Value};

use super::super::image_attachment::{self, ReferenceImage};
use super::{Draft, MAX_WRITE_CELLS};

const MAX_PIXELS: u64 = 4 * 1024 * 1024;
const MAX_MATCH_WORK: u64 = 160_000_000;
const MAX_PASSES: usize = 3;

fn integer(args: &Value, key: &str, default: i32) -> Result<i32, String> {
    match args.get(key) {
        None => Ok(default),
        Some(value) => value
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| format!("{key} must be a 32-bit integer")),
    }
}

fn choice<'a>(args: &'a Value, key: &str, default: &'a str, choices: &[&str]) -> Result<&'a str, String> {
    let value = match args.get(key) {
        None => default,
        Some(value) => value.as_str().ok_or_else(|| format!("{key} must be a string"))?,
    };
    if !choices.contains(&value) {
        return Err(format!("{key} must be one of {}", choices.join(", ")));
    }
    Ok(value)
}

fn region(args: &Value, width: i32, height: i32) -> Result<(i32, i32, i32, i32), String> {
    let x = integer(args, "x", 0)?;
    let y = integer(args, "y", 0)?;
    let w = integer(args, "width", width)?;
    let h = integer(args, "height", height)?;
    if x < 0 || y < 0 || w <= 0 || h <= 0 || i64::from(x) + i64::from(w) > i64::from(width) || i64::from(y) + i64::from(h) > i64::from(height) {
        return Err("Rectangle must be positive and entirely inside the target; use a smaller explicit region".into());
    }
    if i64::from(w) * i64::from(h) > MAX_WRITE_CELLS as i64 {
        return Err(format!("Use at most {MAX_WRITE_CELLS} cells per conversion or preview"));
    }
    Ok((x, y, w, h))
}

fn geometry(draft: &Draft) -> Result<(u32, u32, f32), String> {
    let font = draft.buffer.font_for_render(0).ok_or("Document has no renderable font")?;
    if !(1..=8).contains(&font.width) || !(1..=32).contains(&font.height) {
        return Err("Canvas image tools require fonts of 1..8 by 1..32 pixels".into());
    }
    let width = u32::from(font.width) + u32::from(draft.buffer.use_letter_spacing() && font.width == 8);
    let aspect = if draft.buffer.use_aspect_ratio() {
        draft.buffer.get_aspect_ratio_stretch_factor()
    } else {
        1.0
    };
    Ok((width, u32::from(font.height), if aspect > 0.0 { aspect } else { 1.0 }))
}

impl Draft {
    pub fn preview_image(&mut self, args: &Value) -> Result<ReferenceImage, String> {
        if !self.preview_enabled {
            return Err("The selected model does not support image feedback. Choose an image-capable model.".into());
        }
        if self.preview_calls >= MAX_PASSES {
            return Err("Preview limit reached (3 per turn). Finish this draft or continue in another turn.".into());
        }
        let (x, y, w, h) = region(args, self.buffer.width(), self.buffer.height())?;
        let (cw, ch, aspect) = geometry(self)?;
        let pixels = w as u64 * h as u64 * u64::from(cw) * u64::from(ch);
        if pixels > MAX_PIXELS || pixels as f64 * f64::from(aspect.max(1.0)) > MAX_PIXELS as f64 {
            return Err("Preview exceeds 4 megapixels; select a smaller region".into());
        }
        let (size, rgba) = self.buffer.render_to_rgba(&Rectangle::from(x, y, w, h).into(), false);
        let raw = RgbaImage::from_raw(size.width.max(0) as u32, size.height.max(0) as u32, rgba)
            .filter(|image| image.width() > 0 && image.height() > 0)
            .ok_or("Cannot render the draft")?;
        let display_height = (raw.height() as f32 * aspect).round().max(1.0) as u32;
        let corrected = imageops::resize(&raw, raw.width(), display_height, imageops::FilterType::Nearest);
        let mut png = Cursor::new(Vec::new());
        corrected
            .write_to(&mut png, ImageFormat::Png)
            .map_err(|error| format!("Cannot encode draft preview: {error}"))?;
        let preview = image_attachment::prepare("draft-preview.png", &png.into_inner())?;
        self.preview_calls += 1;
        Ok(preview)
    }

    pub fn convert_image(&mut self, args: &Value) -> Result<String, String> {
        if self.conversion_calls >= MAX_PASSES {
            return Err("Conversion limit reached (3 per turn). Refine this draft or continue in another turn.".into());
        }
        let image = self.reference_image.as_ref().ok_or("Attach a reference picture before converting it")?;
        let encoding = self.buffer.buffer_type;
        if !matches!(encoding, BufferType::CP437 | BufferType::Atascii | BufferType::Petscii | BufferType::AtariSt) {
            return Err("Local conversion supports CP437, ATASCII, PETSCII and Atari ST byte-font canvases, not Unicode canvases".into());
        }
        let mode = choice(
            args,
            "mode",
            if encoding == BufferType::CP437 { "half_blocks" } else { "full" },
            &["half_blocks", "blocks", "full", "ascii"],
        )?;
        if encoding != BufferType::CP437 && mode != "full" {
            return Err("Native retro conversion requires mode='full'; CP437 block/ASCII codes do not apply".into());
        }
        let fit = choice(args, "fit", "contain", &["contain", "crop"])?;
        let dither = match args.get("dither") {
            None => false,
            Some(value) => value.as_bool().ok_or("dither must be a boolean")?,
        };
        let layer = self.layer_index(args, true)?;
        let target = &self.buffer.layers[layer];
        let (x, y, w, h) = region(args, target.width(), target.height())?;
        let (cw, ch, aspect) = geometry(self)?;
        let width = w as u32 * cw;
        let height = h as u32 * ch;
        if u64::from(width) * u64::from(height) > MAX_PIXELS {
            return Err("Conversion exceeds 4 megapixels; use a smaller target region".into());
        }
        let codes: Vec<u8> = match mode {
            "half_blocks" => vec![32, 219, 223, 220],
            "blocks" => vec![32, 219, 223, 220, 221, 222, 176, 177, 178],
            "ascii" => (32..=126).collect(),
            _ => (0..=255).collect(),
        };
        let palette_len = self.buffer.palette.len().min(256);
        if palette_len == 0 {
            return Err("Document palette is empty".into());
        }
        let fg_count = match encoding {
            BufferType::CP437 => palette_len.min(16),
            BufferType::Petscii => palette_len.min(icy_engine::petscii_charset(&self.buffer).0.text_colors() as usize),
            _ => palette_len,
        };
        let bg_count = if encoding == BufferType::CP437 {
            palette_len.min(if self.buffer.ice_mode == IceMode::Blink { 8 } else { 16 })
        } else {
            palette_len
        };
        let color_work = match encoding {
            BufferType::Atascii => 2,
            BufferType::Petscii => fg_count + 1,
            _ => fg_count + bg_count,
        };
        let work = w as u64 * h as u64 * codes.len() as u64 * (u64::from(cw * ch) + color_work as u64);
        if work > MAX_MATCH_WORK {
            return Err("Glyph matching budget exceeded; use half_blocks/blocks or a smaller region".into());
        }
        let raster = fit_image(&image.pixels()?, width, height, aspect, fit);
        let palette: Vec<[f64; 3]> = (0..palette_len)
            .map(|i| {
                let (r, g, b) = self.buffer.palette.rgb(i as u32);
                [f64::from(r), f64::from(g), f64::from(b)]
            })
            .collect();
        let mut masks = HashMap::new();
        let mut staged = Vec::with_capacity((w * h) as usize);
        for row in 0..h {
            for column in 0..w {
                let position = Position::new(x + column, y + row);
                let old = self.writable_cell(target.char_at(position))?;
                let page = old.attribute.font_page();
                if !masks.contains_key(&page) {
                    let font = self.buffer.font_for_render(page).ok_or_else(|| format!("Font page {page} is missing"))?;
                    if u32::from(font.height) != ch || u32::from(font.width) != cw.min(8) {
                        return Err("Mixed font dimensions are not supported by local conversion".into());
                    }
                    let glyphs: Vec<_> = codes
                        .iter()
                        .map(|&code| {
                            let glyph = &font.glyphs[usize::from(code)];
                            let mut on = Vec::new();
                            for py in 0..ch {
                                for px in 0..cw {
                                    let set = if px == 8 {
                                        (0xC0..=0xDF).contains(&code) && glyph.get_pixel(7, py as usize)
                                    } else {
                                        glyph.get_pixel(px as usize, py as usize)
                                    };
                                    if set {
                                        on.push((py * cw + px) as usize);
                                    }
                                }
                            }
                            (code, on)
                        })
                        .collect();
                    masks.insert(page, glyphs);
                }
                let fixed_color = |foreground: bool| -> Result<usize, String> {
                    let color = if foreground {
                        old.attribute.foreground_color()
                    } else {
                        old.attribute.background_color()
                    };
                    match color {
                        icy_engine::AttributeColor::Palette(index) if usize::from(index) < palette_len => Ok(usize::from(index)),
                        _ => Err("Shared screen colors must be valid document palette indices".into()),
                    }
                };
                let foreground = if encoding == BufferType::Atascii {
                    vec![fixed_color(true)?]
                } else {
                    (0..fg_count).collect()
                };
                let background = match encoding {
                    BufferType::Atascii => vec![fixed_color(false)?],
                    BufferType::Petscii => {
                        let bg = icy_engine::petscii_background(&self.buffer) as usize;
                        if bg >= palette_len {
                            return Err("PETSCII background is outside the palette".into());
                        }
                        vec![bg]
                    }
                    _ => (0..bg_count).collect(),
                };
                let matte = match encoding {
                    BufferType::Atascii | BufferType::Petscii => palette[background[0]],
                    _ => match old.attribute.background_color() {
                        icy_engine::AttributeColor::Palette(index) if usize::from(index) < palette_len => palette[usize::from(index)],
                        icy_engine::AttributeColor::Rgb(r, g, b) => [f64::from(r), f64::from(g), f64::from(b)],
                        icy_engine::AttributeColor::ExtendedPalette(index) => {
                            let internal = icy_engine::ansi_to_internal_palette_index(u32::from(index)) as usize;
                            if internal < palette_len {
                                palette[internal]
                            } else {
                                let (r, g, b) = icy_engine::XTERM_256_PALETTE[usize::from(index)].1.rgb();
                                [f64::from(r), f64::from(g), f64::from(b)]
                            }
                        }
                        icy_engine::AttributeColor::Transparent => palette[0],
                        _ => return Err("Cell background is outside the document palette".into()),
                    },
                };
                let mut samples = Vec::with_capacity((cw * ch) as usize);
                for py in 0..ch {
                    for px in 0..cw {
                        let gx = column as u32 * cw + px;
                        let gy = row as u32 * ch + py;
                        let pixel = raster.get_pixel(gx, gy).0;
                        let alpha = f64::from(pixel[3]) / 255.0;
                        let mut rgb = [0.0; 3];
                        let offset = if dither {
                            const BAYER: [[f64; 4]; 4] = [[0., 8., 2., 10.], [12., 4., 14., 6.], [3., 11., 1., 9.], [15., 7., 13., 5.]];
                            (BAYER[gy as usize % 4][gx as usize % 4] - 7.5) * 2.0
                        } else {
                            0.0
                        };
                        if encoding == BufferType::Atascii {
                            let luma = ((0.2126 * f64::from(pixel[0]) + 0.7152 * f64::from(pixel[1]) + 0.0722 * f64::from(pixel[2]) + offset) / 255.0)
                                .clamp(0.0, 1.0)
                                * alpha;
                            for c in 0..3 {
                                rgb[c] = matte[c] + luma * (palette[foreground[0]][c] - matte[c]);
                            }
                        } else {
                            for c in 0..3 {
                                rgb[c] = ((f64::from(pixel[c]) + offset).clamp(0.0, 255.0) * alpha) + matte[c] * (1.0 - alpha);
                            }
                        }
                        samples.push(rgb);
                    }
                }
                let total = sum(&samples);
                let mut best = (f64::INFINITY, 32u8, foreground[0], background[0]);
                for (code, on) in &masks[&page] {
                    let mut lit = [0.0; 3];
                    for &i in on {
                        for c in 0..3 {
                            lit[c] += samples[i][c];
                        }
                    }
                    let unlit = [total[0] - lit[0], total[1] - lit[1], total[2] - lit[2]];
                    let (fe, fg) = closest(lit, on.len(), &foreground, &palette);
                    let (be, bg) = closest(unlit, samples.len() - on.len(), &background, &palette);
                    if fe + be < best.0 {
                        best = (fe + be, *code, fg, bg);
                    }
                }
                let mut attr = TextAttribute::from_colors(AttributeColor::Palette(best.2 as u8), AttributeColor::Palette(best.3 as u8));
                attr.set_font_page(page);
                staged.push((position, AttributedChar::new(char::from(best.1), attr)));
            }
        }
        let changed = staged.iter().filter(|(position, cell)| target.char_at(*position) != *cell).count();
        for (position, cell) in staged {
            self.buffer.layers[layer].set_char(position, cell);
        }
        self.conversion_calls += 1;
        Ok(json!({
            "changed_cells": changed, "target": {"layer": layer, "x": x, "y": y, "width": w, "height": h},
            "mode": mode, "fit": fit, "dither": dither,
            "transparency": "Source transparency and contain margins use the existing cell background, or palette 0 for transparent cells; retro uses the shared screen background.",
            "note": "Converted locally into the draft. Use icy_preview_canvas to inspect and refine. Nothing is applied until the user accepts.",
        })
        .to_string())
    }
}

fn sum(samples: &[[f64; 3]]) -> [f64; 3] {
    let mut total = [0.0; 3];
    for sample in samples {
        for c in 0..3 {
            total[c] += sample[c];
        }
    }
    total
}

// Squared error without the constant sum of source squares; optimal colors can be chosen independently.
fn closest(sum: [f64; 3], count: usize, choices: &[usize], palette: &[[f64; 3]]) -> (f64, usize) {
    let mut best = (f64::INFINITY, choices[0]);
    for &index in choices {
        let color = palette[index];
        let error: f64 = (0..3).map(|c| count as f64 * color[c] * color[c] - 2.0 * color[c] * sum[c]).sum();
        if error < best.0 {
            best = (error, index);
        }
    }
    best
}

fn fit_image(source: &RgbaImage, width: u32, height: u32, aspect: f32, fit: &str) -> RgbaImage {
    let target_ratio = f64::from(width) / (f64::from(height) * f64::from(aspect));
    let source_ratio = f64::from(source.width()) / f64::from(source.height());
    if fit == "crop" {
        let (w, h) = if source_ratio > target_ratio {
            (
                (f64::from(source.height()) * target_ratio).round().clamp(1.0, f64::from(source.width())) as u32,
                source.height(),
            )
        } else {
            (
                source.width(),
                (f64::from(source.width()) / target_ratio).round().clamp(1.0, f64::from(source.height())) as u32,
            )
        };
        let crop = imageops::crop_imm(source, (source.width() - w) / 2, (source.height() - h) / 2, w, h).to_image();
        return imageops::resize(&crop, width, height, imageops::FilterType::Triangle);
    }
    let (w, h) = if source_ratio > target_ratio {
        (
            width,
            (f64::from(width) / source_ratio / f64::from(aspect)).round().clamp(1.0, f64::from(height)) as u32,
        )
    } else {
        (
            (f64::from(height) * f64::from(aspect) * source_ratio).round().clamp(1.0, f64::from(width)) as u32,
            height,
        )
    };
    let fitted = imageops::resize(source, w, h, imageops::FilterType::Triangle);
    let mut result = RgbaImage::new(width, height);
    imageops::replace(&mut result, &fitted, i64::from((width - w) / 2), i64::from((height - h) / 2));
    result
}

pub(super) fn tool_specs() -> Vec<(&'static str, &'static str, Value)> {
    let region = json!({
        "x": {"type": "integer", "minimum": 0}, "y": {"type": "integer", "minimum": 0},
        "width": {"type": "integer", "minimum": 1}, "height": {"type": "integer", "minimum": 1},
    });
    let mut conversion = region.clone();
    conversion["layer"] = json!({"type": "integer", "minimum": 0});
    conversion["mode"] = json!({"type": "string", "enum": ["half_blocks", "blocks", "full", "ascii"]});
    conversion["fit"] = json!({"type": "string", "enum": ["contain", "crop"]});
    conversion["dither"] = json!({"type": "boolean"});
    vec![
        ("icy_preview_canvas",
         "Return a rendered PNG of the combined draft, using actual fonts, palette, letter spacing and display aspect ratio. Coordinates are document cells; default whole canvas. At most 8000 cells/4 megapixels and 3 previews per turn. Inspect before claiming success; images require a vision model.",
         json!({"type": "object", "properties": region})),
        ("icy_convert_reference_image",
         "Locally convert the most recently attached picture into draft cells. No paths or downloads. Read canvas_info first. Explicit target x/y/width/height are layer-relative; defaults to whole layer. Replaces only that region. CP437 modes: half_blocks (default), blocks, full, ascii; retro requires full. Fit contain (default) or center crop. Dither defaults false. Preserves font pages, retro shared colors and legal ANSI blink/iCE colors. Max 8000 cells, bounded matching work, 3 conversions/turn. Preview then refine; requires user Apply.",
         json!({"type": "object", "properties": conversion})),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::{Size, TextBuffer};

    fn reference(image: RgbaImage) -> ReferenceImage {
        let mut png = Cursor::new(Vec::new());
        image.write_to(&mut png, ImageFormat::Png).unwrap();
        image_attachment::prepare("test.png", &png.into_inner()).unwrap()
    }

    fn draft(width: i32, height: i32, image: RgbaImage) -> Draft {
        let mut buffer = TextBuffer::new((width, height));
        buffer.ice_mode = IceMode::Blink;
        let mut draft = Draft::new("ANSI/ASCII", buffer, 0, None);
        draft.begin_turn(Some(reference(image)));
        draft
    }

    #[test]
    fn half_blocks_reconstruct_palette_image_without_touching_original() {
        let font = TextBuffer::new((2, 1)).font_for_render(0).unwrap().clone();
        let source = RgbaImage::from_fn(16, 16, |x, y| {
            if font.glyphs[223].get_pixel(x as usize % 8, y as usize) {
                image::Rgba([170, 0, 0, 255])
            } else {
                image::Rgba([0, 0, 170, 255])
            }
        });
        let mut draft = draft(2, 1, source.clone());
        let result: Value = serde_json::from_str(&draft.convert_image(&json!({})).unwrap()).unwrap();
        assert_eq!(result["changed_cells"], 2);
        assert!(draft.original.layers[0].char_at(Position::new(0, 0)).ch != char::from(223));
        let (size, rendered) = draft.buffer.render_to_rgba(&Rectangle::from(0, 0, 2, 1).into(), false);
        assert_eq!(size, Size::new(16, 16));
        assert!(
            rendered == source.into_raw(),
            "conversion must match the rendered pixels, not just write some cells"
        );
        for x in 0..2 {
            let cell = draft.buffer.layers[0].char_at(Position::new(x, 0));
            assert!(cell.attribute.background() < 8);
            assert!(!cell.attribute.is_blinking());
        }
    }

    #[test]
    fn native_matching_preserves_shared_colors_and_font_pages() {
        for mut buffer in [
            icy_draw::screen_profile::atascii_buffer(icy_draw::screen_profile::AtasciiMode::Antic),
            icy_draw::screen_profile::atascii_buffer(icy_draw::screen_profile::AtasciiMode::Xep80),
            icy_draw::screen_profile::petscii_buffer(icy_engine::PetsciiMachine::C64, icy_engine::PetsciiCase::Upper),
            icy_draw::screen_profile::atari_st_buffer(icy_engine::TerminalResolution::Medium),
        ] {
            buffer.set_use_aspect_ratio(false);
            let old = buffer.layers[0].char_at(Position::new(0, 0));
            let font = buffer.font_for_render(0).unwrap();
            let glyph = font.glyphs[193];
            let source = RgbaImage::from_fn(u32::from(font.width), u32::from(font.height), |x, y| {
                let v = if glyph.get_pixel(x as usize, y as usize) { 255 } else { 0 };
                image::Rgba([v, v, v, 255])
            });
            let atascii = buffer.buffer_type == BufferType::Atascii;
            let petscii = buffer.buffer_type == BufferType::Petscii;
            let mut draft = Draft::new("retro", buffer, 0, None);
            draft.begin_turn(Some(reference(source)));
            draft.convert_image(&json!({"width": 1, "height": 1, "mode": "full"})).unwrap();
            let cell = draft.buffer.layers[0].char_at(Position::new(0, 0));
            assert_eq!(cell.attribute.font_page(), old.attribute.font_page());
            if atascii {
                assert_eq!(cell.attribute.foreground_color(), AttributeColor::Palette(7));
                assert_eq!(cell.attribute.background_color(), AttributeColor::Palette(0));
                assert_eq!(draft.buffer.font_for_render(0).unwrap().glyph(cell.ch), &glyph);
                let (_, pixels) = draft.buffer.render_to_rgba(&Rectangle::from(0, 0, 1, 1).into(), false);
                assert!(pixels.chunks_exact(4).all(|pixel| pixel[3] == 255));
                let foreground = draft.buffer.palette.rgb(7);
                assert!(pixels.chunks_exact(4).any(|pixel| (pixel[0], pixel[1], pixel[2]) == foreground));
            }
            if petscii {
                assert_eq!(cell.attribute.background(), icy_engine::petscii_background(&draft.buffer));
            }
        }
    }

    #[test]
    fn modes_and_ice_constraints_are_real_output_constraints() {
        let source = RgbaImage::from_fn(16, 16, |x, y| image::Rgba([(x * 16) as u8, (y * 16) as u8, 255, 255]));
        for mode in ["half_blocks", "blocks", "full", "ascii"] {
            for ice in [IceMode::Blink, IceMode::Ice] {
                let mut draft = draft(2, 1, source.clone());
                draft.buffer.ice_mode = ice;
                draft.convert_image(&json!({"mode": mode, "dither": true})).unwrap();
                for x in 0..2 {
                    let cell = draft.buffer.layers[0].char_at(Position::new(x, 0));
                    assert!(cell.attribute.foreground() < 16);
                    assert!(cell.attribute.background() < if ice == IceMode::Blink { 8 } else { 16 });
                    assert!(!cell.attribute.is_blinking() && !cell.attribute.is_bold());
                    if mode == "ascii" {
                        assert!((32..=126).contains(&(cell.ch as u32)));
                    }
                    if mode == "half_blocks" {
                        assert!([32, 219, 223, 220].contains(&(cell.ch as u32)));
                    }
                    if mode == "blocks" {
                        assert!([32, 219, 223, 220, 221, 222, 176, 177, 178].contains(&(cell.ch as u32)));
                    }
                }
            }
        }
    }

    #[test]
    fn conversion_is_atomic_bounded_and_scoped() {
        let mut draft = draft(3, 1, RgbaImage::from_pixel(8, 16, image::Rgba([255; 4])));
        let original = draft.buffer.layers[0].char_at(Position::new(0, 0));
        for args in [
            json!({"width": 4}),
            json!({"x": -1}),
            json!({"width": 0}),
            json!({"width": 1.5}),
            json!({"mode": "unknown"}),
            json!({"fit": "stretch"}),
            json!({"dither": "yes"}),
            json!({"layer": 500}),
            json!({"layer": -1}),
            json!({"layer": "0"}),
        ] {
            assert!(draft.convert_image(&args).is_err(), "{args}");
            assert!(draft.changes().is_empty());
        }
        draft.buffer.layers[0].properties.is_locked = true;
        assert!(draft.convert_image(&json!({})).is_err());
        draft.buffer.layers[0].properties.is_locked = false;
        let mut missing = original;
        missing.attribute.set_font_page(254);
        draft.buffer.layers[0].set_char(Position::new(2, 0), missing);
        assert!(draft.convert_image(&json!({})).unwrap_err().contains("missing"));
        assert_eq!(
            draft.buffer.layers[0].char_at(Position::new(0, 0)),
            original,
            "late failure cannot partially write"
        );
        draft.buffer.layers[0].set_char(Position::new(2, 0), original);
        for _ in 0..3 {
            draft.convert_image(&json!({"x": 1, "width": 1, "height": 1})).unwrap();
        }
        assert!(draft.convert_image(&json!({})).unwrap_err().contains("limit"));
        assert_eq!(draft.buffer.layers[0].char_at(Position::new(0, 0)), original);
        assert_eq!(draft.buffer.layers[0].char_at(Position::new(2, 0)), original);
        draft.begin_turn(None);
        assert!(draft.convert_image(&json!({})).unwrap_err().contains("Attach"));
        let huge = self::draft(8001, 1, RgbaImage::new(1, 1));
        assert!(region(&json!({}), huge.buffer.width(), 1).is_err());
    }

    #[test]
    fn previews_are_bounded_read_only_and_aspect_corrected() {
        let mut draft = draft(2, 1, RgbaImage::new(1, 1));
        draft.buffer.set_use_aspect_ratio(true);
        let (_, ch, aspect) = geometry(&draft).unwrap();
        for _ in 0..3 {
            let preview = draft.preview_image(&json!({})).unwrap();
            assert_eq!(preview.width, 16);
            assert_eq!(preview.height, (ch as f32 * aspect).round() as u32);
            assert!(draft.changes().is_empty());
        }
        assert!(draft.preview_image(&json!({})).unwrap_err().contains("limit"));
        draft.begin_turn(None);
        assert!(draft.preview_image(&json!({"width": 3})).is_err());
        draft.preview_enabled = false;
        assert!(draft.preview_image(&json!({})).unwrap_err().contains("image-capable"));
    }

    #[test]
    fn contain_preserves_full_picture_and_crop_fills_target() {
        let source = RgbaImage::from_pixel(40, 10, image::Rgba([255; 4]));
        let contained = fit_image(&source, 20, 20, 1.0, "contain");
        assert_eq!(contained.pixels().filter(|pixel| pixel[3] == 255).count(), 20 * 5);
        let cropped = fit_image(&source, 20, 20, 1.0, "crop");
        assert!(cropped.pixels().all(|pixel| pixel[3] == 255));
        let corrected = fit_image(&source, 20, 20, 1.25, "contain");
        assert_eq!(corrected.pixels().filter(|pixel| pixel[3] == 255).count(), 20 * 4);
    }

    #[test]
    fn full_screen_conversion_is_deterministic_and_improves_pixel_error() {
        let source = RgbaImage::from_fn(640, 400, |x, y| {
            let head = (i64::from(x) - 320).pow(2) * 3 + (i64::from(y) - 195).pow(2) * 4 < 65_000;
            let eye = (y > 140 && y < 165) && ((x > 240 && x < 275) || (x > 365 && x < 400));
            let color = if eye {
                [255, 255, 255, 255]
            } else if head {
                [170, 85, 0, 255]
            } else {
                [0, 0, 170, 255]
            };
            image::Rgba(color)
        });
        let mut draft = draft(80, 25, source.clone());
        let start = std::time::Instant::now();
        draft.convert_image(&json!({"mode": "full"})).unwrap();
        eprintln!("80x25 full-glyph conversion: {:?}", start.elapsed());
        let first = draft.changes();
        assert_eq!(first.len(), 2000);
        let (_, rendered) = draft.buffer.render_to_rgba(&Rectangle::from(0, 0, 80, 25).into(), false);
        let error: u64 = source
            .as_raw()
            .iter()
            .zip(&rendered)
            .enumerate()
            .filter(|(i, _)| i % 4 != 3)
            .map(|(_, (&a, &b))| i64::from(a).abs_diff(i64::from(b)).pow(2))
            .sum();
        let blank_error: u64 = source
            .pixels()
            .map(|pixel| pixel.0[..3].iter().map(|&c| u64::from(c).pow(2)).sum::<u64>())
            .sum();
        assert!(error * 10 < blank_error, "rendered error {error} must be <10% of blank error {blank_error}");
        draft.convert_image(&json!({"mode": "full"})).unwrap();
        assert_eq!(draft.changes(), first, "same picture and target must produce identical cells");
    }

    #[test]
    fn matching_budget_rejects_expensive_regions_without_writing() {
        let mut draft = draft(160, 50, RgbaImage::new(1, 1));
        assert!(draft.convert_image(&json!({"mode": "full"})).unwrap_err().contains("budget"));
        assert!(draft.changes().is_empty());
    }

    #[test]
    fn ice_bright_backgrounds_render_without_blink_and_spacing_matches() {
        let mut expected = TextBuffer::new((2, 1));
        expected.set_use_letter_spacing(true);
        expected.ice_mode = IceMode::Ice;
        for x in 0..2 {
            expected.layers[0].set_char(
                Position::new(x, 0),
                AttributedChar::new(
                    char::from(223),
                    TextAttribute::from_colors(AttributeColor::Palette(15), AttributeColor::Palette(12)),
                ),
            );
        }
        let (size, pixels) = expected.render_to_rgba(&Rectangle::from(0, 0, 2, 1).into(), false);
        let source = RgbaImage::from_raw(size.width as u32, size.height as u32, pixels.clone()).unwrap();
        let mut draft = draft(2, 1, source);
        draft.buffer.set_use_letter_spacing(true);
        draft.buffer.ice_mode = IceMode::Ice;
        draft.convert_image(&json!({})).unwrap();
        let (_, result) = draft.buffer.render_to_rgba(&Rectangle::from(0, 0, 2, 1).into(), false);
        assert!(result == pixels, "bright-background conversion must render exactly, including the ninth pixel");
        assert!(draft.buffer.layers[0].char_at(Position::new(0, 0)).attribute.background() >= 8);
        assert!(!draft.buffer.layers[0].char_at(Position::new(0, 0)).attribute.is_blinking());
    }

    #[test]
    fn actual_custom_font_page_is_used_instead_of_standard_cp437_shapes() {
        let mut font = icy_engine::BitFont::create_8("Custom", 8, 16, &[]);
        for y in 0..16 {
            font.glyphs[42].set_pixel(y % 8, y, true);
        }
        let source = RgbaImage::from_fn(8, 16, |x, y| {
            let v = if font.glyphs[42].get_pixel(x as usize, y as usize) { 255 } else { 0 };
            image::Rgba([v, v, v, 255])
        });
        let mut draft = draft(1, 1, source);
        draft.buffer.set_font(1, font);
        let mut cell = AttributedChar::invisible();
        cell.attribute.set_font_page(1);
        draft.buffer.layers[0].set_char(Position::new(0, 0), cell);
        draft.convert_image(&json!({"mode": "full"})).unwrap();
        let cell = draft.buffer.layers[0].char_at(Position::new(0, 0));
        assert_eq!(cell.ch as u32, 42);
        assert_eq!(cell.attribute.font_page(), 1);
    }
}
