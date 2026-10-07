//! Bounded local image matching and rendered feedback for character-canvas drafts.

use std::{collections::HashMap, io::Cursor};

use icy_engine::{AttributeColor, AttributedChar, BufferType, IceMode, Position, Rectangle, TextAttribute, TextPane};
use image::{imageops, ImageFormat, RgbaImage};
use serde_json::{json, Value};

use super::super::image_attachment::{self, ReferenceImage};
use super::{Draft, MAX_WRITE_CELLS};

#[path = "ai_raster_metrics.rs"]
mod metrics;
#[path = "ai_raster_style.rs"]
mod style;

const MAX_PIXELS: u64 = 4 * 1024 * 1024;
const MAX_MATCH_WORK: u64 = 240_000_000;
const MAX_PASSES: usize = 3;

#[derive(Clone)]
pub(super) struct Review {
    source: ReferenceImage,
    layer: usize,
    original: Vec<(Position, AttributedChar)>,
    expected: Vec<(Position, AttributedChar)>,
    options: Value,
    metrics: metrics::Metrics,
    previewed: bool,
}

struct Conversion {
    source: ReferenceImage,
    layer: usize,
    original: Vec<(Position, AttributedChar)>,
    cells: Vec<(Position, AttributedChar)>,
    options: Value,
    metrics: metrics::Metrics,
    coherent_cells: usize,
}

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
        if let Some(review) = &mut self.conversion_review {
            review.previewed = self.buffer.layers.get(review.layer).is_some_and(|layer| {
                let offset = layer.offset();
                review.expected.iter().all(|(position, _)| {
                    let px = i64::from(position.x) + i64::from(offset.x);
                    let py = i64::from(position.y) + i64::from(offset.y);
                    px >= i64::from(x) && py >= i64::from(y) && px < i64::from(x + w) && py < i64::from(y + h)
                })
            });
        }
        Ok(preview)
    }

    pub fn convert_image(&mut self, args: &Value) -> Result<String, String> {
        let conversion = self.build_conversion(args, None)?;
        let changed = conversion
            .cells
            .iter()
            .filter(|(pos, cell)| self.buffer.layers[conversion.layer].char_at(*pos) != *cell)
            .count();
        let report = json!({
            "changed_cells": changed, "target": {
                "layer": conversion.layer, "x": conversion.options["x"], "y": conversion.options["y"],
                "width": conversion.options["width"], "height": conversion.options["height"],
            },
            "mode": conversion.options["mode"], "preset": conversion.options["preset"],
            "fit": conversion.options["fit"], "dither": conversion.options["dither"],
            "effective_options": conversion.options, "metrics": conversion.metrics,
            "coherence_changed_cells": conversion.coherent_cells,
            "remaining_trials": MAX_PASSES - self.conversion_calls - 1,
            "transparency": "Source transparency and contain margins use the original cell background, or palette 0 for transparent cells; retro uses the shared screen background.",
            "note": "Draft only. Inspect with icy_preview_canvas, then use icy_refine_reference_image with a visual critique and parameter changes. Refinement retains only a measured improvement; user Apply is still required.",
        });
        self.install_conversion(conversion);
        self.conversion_calls += 1;
        Ok(report.to_string())
    }

    fn install_conversion(&mut self, conversion: Conversion) {
        for &(position, cell) in &conversion.cells {
            self.buffer.layers[conversion.layer].set_char(position, cell);
        }
        self.conversion_review = Some(Review {
            source: conversion.source,
            layer: conversion.layer,
            original: conversion.original,
            expected: conversion.cells,
            options: conversion.options,
            metrics: conversion.metrics,
            previewed: false,
        });
    }

    pub fn image_feedback(&self) -> Value {
        self.conversion_review.as_ref().map_or(Value::Null, |review| json!({
            "available": self.can_refine_image(),
            "metrics": review.metrics, "effective_options": review.options,
            "remaining_trials": MAX_PASSES.saturating_sub(self.conversion_calls),
            "previewed": review.previewed,
            "objective": "Lower is better: 0.5*color_error + 0.5*detail_error + 0.25*edge_error + excess_texture. Fixed Oklab measurements of actual glyph colors against the original fitted source, at cell and quadrant scales. Not recognition or artistic-quality scores.",
        }))
    }

    pub fn can_refine_image(&self) -> bool {
        self.preview_enabled
            && self.buffer.buffer_type == BufferType::CP437
            && self.conversion_review.as_ref().is_some_and(|review| review.options["preset"] != "faithful")
    }

    pub fn refine_image(&mut self, args: &Value) -> Result<String, String> {
        if self.conversion_calls >= MAX_PASSES {
            return Err("Conversion/refinement limit reached (3 trials per turn); keep the best draft or continue in another turn".into());
        }
        if !self.preview_enabled {
            return Err("Image critique requires an image-capable model".into());
        }
        let review = self.conversion_review.as_ref().ok_or("Convert a reference image before refining it")?;
        if self.buffer.buffer_type != BufferType::CP437 || review.options["preset"] == "faithful" {
            return Err("Critique refinement requires a CP437 scene, toon or pixel_art conversion; faithful and retro remain literal".into());
        }
        if self.reference_image.as_ref() != Some(&review.source) {
            return Err("The reference image changed; start a new conversion".into());
        }
        let target = self.buffer.layers.get(review.layer).ok_or("The conversion layer was removed")?;
        if review.expected.iter().any(|(position, cell)| target.char_at(*position) != *cell) {
            return Err("The converted region was edited since the last trial; start a new conversion rather than overwriting edits".into());
        }
        if !review.previewed {
            return Err("Inspect the entire converted region with icy_preview_canvas before each critique trial".into());
        }
        let object = args.as_object().ok_or("Refinement arguments must be an object")?;
        let critique = args["critique"]
            .as_str()
            .filter(|s| !s.trim().is_empty() && s.len() <= 2000)
            .ok_or("critique must describe the visible defect in 1..2000 bytes")?;
        let allowed = style::Tuning::schema();
        let mut options = review.options.clone();
        let mut changed = false;
        for (key, value) in object {
            if key == "critique" {
                continue;
            }
            if allowed.get(key).is_none() {
                return Err(format!("Cannot refine {key}; target, source, preset, fit, dither and glyph mode stay fixed"));
            }
            changed |= options[key] != *value;
            options[key] = value.clone();
        }
        if !changed {
            return Err("Change at least one tuning parameter based on the preview critique".into());
        }
        let before = review.metrics;
        let candidate = self.build_conversion(&options, Some(&review.original))?;
        let accepted = candidate.metrics.improves(before);
        let report = json!({
            "accepted": accepted, "critique": critique, "previous_metrics": before,
            "candidate_metrics": candidate.metrics,
            "retained_metrics": if accepted { candidate.metrics } else { before },
            "candidate_options": candidate.options,
            "retained_options": if accepted { &candidate.options } else { &review.options },
            "coherence_changed_cells": candidate.coherent_cells,
            "remaining_trials": MAX_PASSES - self.conversion_calls - 1,
            "note": if accepted {
                "Measured improvement retained in draft only. Preview again and check recognition, contours and colors; the score is not a semantic quality guarantee."
            } else {
                "Trial did not improve the fixed source-relative objective. Kept the previous draft and settings unchanged. Preview before trying a different hypothesis."
            },
        });
        if accepted {
            self.install_conversion(candidate);
        } else if let Some(review) = &mut self.conversion_review {
            review.previewed = false;
        }
        self.conversion_calls += 1;
        Ok(report.to_string())
    }

    fn build_conversion(&self, args: &Value, baseline: Option<&[(Position, AttributedChar)]>) -> Result<Conversion, String> {
        if self.conversion_calls >= MAX_PASSES {
            return Err("Conversion limit reached (3 per turn). Refine this draft or continue in another turn.".into());
        }
        let image = self.reference_image.as_ref().ok_or("Attach a reference picture before converting it")?;
        let encoding = self.buffer.buffer_type;
        if !matches!(encoding, BufferType::CP437 | BufferType::Atascii | BufferType::Petscii | BufferType::AtariSt) {
            return Err("Local conversion supports CP437, ATASCII, PETSCII and Atari ST byte-font canvases, not Unicode canvases".into());
        }
        let preset_name = choice(args, "preset", "faithful", &["faithful", "scene", "toon", "pixel_art"])?;
        let preset = style::Preset::parse(preset_name)?;
        let tuning = style::Tuning::parse(args, preset)?;
        if preset == style::Preset::Faithful
            && style::Tuning::schema()
                .as_object()
                .is_some_and(|fields| fields.keys().any(|key| args.get(key).is_some()))
        {
            return Err("Tuning parameters require scene, toon or pixel_art; faithful conversion preserves literal matching".into());
        }
        if encoding != BufferType::CP437 && preset != style::Preset::Faithful {
            return Err("Style presets require CP437; native retro conversion requires preset='faithful'".into());
        }
        let mode = choice(
            args,
            "mode",
            if encoding == BufferType::CP437 { preset.default_mode() } else { "full" },
            &["half_blocks", "half_pixels", "blocks", "full", "ascii"],
        )?;
        if encoding != BufferType::CP437 && mode != "full" {
            return Err("Native retro conversion requires mode='full'; CP437 block/ASCII codes do not apply".into());
        }
        if mode == "half_pixels" && preset == style::Preset::Faithful {
            return Err("Half-pixel conversion requires scene, toon or pixel_art; faithful retains actual-glyph fitting".into());
        }
        let fit = choice(args, "fit", "contain", &["contain", "crop", "stretch"])?;
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
            "half_blocks" | "half_pixels" => vec![32, 219, 223, 220],
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
        let blend_work = if preset == style::Preset::Scene {
            codes.iter().filter(|&code| (176..=178).contains(code)).count() * fg_count * bg_count * 3
        } else {
            0
        };
        let work = w as u64 * h as u64 * (codes.len() as u64 * (u64::from(cw * ch) + color_work as u64) + blend_work as u64);
        if work > MAX_MATCH_WORK {
            return Err("Glyph matching budget exceeded; use half_blocks/blocks or a smaller region".into());
        }
        let filter = if preset == style::Preset::PixelArt {
            imageops::FilterType::Nearest
        } else {
            imageops::FilterType::Triangle
        };
        let source = image.pixels()?;
        let raster = fit_image_with_filter(&source, width, height, aspect, fit, filter);
        let half_pixels = (mode == "half_pixels").then(|| {
            let grid = style::half_grid(if fit == "stretch" { &source } else { &raster }, w as u32, h as u32);
            if tuning.local_contrast > 0.0 {
                style::enhance_detail(&grid, tuning.local_contrast, 3.0)
            } else {
                grid
            }
        });
        let detailed = (half_pixels.is_none() && tuning.local_contrast > 0.0).then(|| style::enhance_detail(&raster, tuning.local_contrast, cw as f32 * 3.0));
        let palette: Vec<[f64; 3]> = (0..palette_len)
            .map(|i| {
                let (r, g, b) = self.buffer.palette.rgb(i as u32);
                [f64::from(r), f64::from(g), f64::from(b)]
            })
            .collect();
        let matcher = (preset != style::Preset::Faithful).then(|| {
            let mut matcher = style::Matcher::new(preset, &palette);
            matcher.tuning = tuning;
            matcher
        });
        let mut styled = Vec::new();
        let mut source_samples = Vec::new();
        let mut original = Vec::new();
        let mut masks = HashMap::new();
        let mut staged = Vec::with_capacity((w * h) as usize);
        for row in 0..h {
            for column in 0..w {
                let position = Position::new(x + column, y + row);
                let base = match baseline {
                    Some(cells) => cells
                        .get((row * w + column) as usize)
                        .filter(|(pos, _)| *pos == position)
                        .map(|(_, cell)| *cell)
                        .ok_or("Conversion baseline does not match the target region")?,
                    None => target.char_at(position),
                };
                original.push((position, base));
                let old = self.writable_cell(base)?;
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
                    if half_pixels.is_some() {
                        let valid = glyphs.iter().all(|(code, on)| match code {
                            32 => on.is_empty(),
                            219 => on.len() == (cw * ch) as usize,
                            223 => !on.is_empty() && on.len() < (cw * ch) as usize && on.len() % cw as usize == 0 && on.iter().copied().eq(0..on.len()),
                            220 => {
                                !on.is_empty()
                                    && on.len() < (cw * ch) as usize
                                    && on.len() % cw as usize == 0
                                    && on.iter().copied().eq((cw * ch) as usize - on.len()..(cw * ch) as usize)
                            }
                            _ => false,
                        });
                        if !valid {
                            return Err("Half-pixel conversion requires a CP437 font with solid horizontal half-block glyphs".into());
                        }
                    }
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
                let mut reference_samples = Vec::with_capacity((cw * ch) as usize);
                for py in 0..ch {
                    for px in 0..cw {
                        let gx = column as u32 * cw + px;
                        let gy = row as u32 * ch + py;
                        let pixel = raster.get_pixel(gx, gy).0;
                        let alpha = f64::from(pixel[3]) / 255.0;
                        let mut rgb = [0.0; 3];
                        let offset = if dither && half_pixels.is_none() {
                            const BAYER: [[f64; 4]; 4] = [[0., 8., 2., 10.], [12., 4., 14., 6.], [3., 11., 1., 9.], [15., 7., 13., 5.]];
                            if mode == "half_blocks" {
                                let level = (f64::from(pixel[0]) + f64::from(pixel[1]) + f64::from(pixel[2])) / (3.0 * 255.0);
                                let half_row = row as usize * 2 + usize::from(py >= ch / 2);
                                (BAYER[half_row % 4][column as usize % 4] - 7.5) * 6.0 * (4.0 * level * (1.0 - level))
                            } else {
                                (BAYER[gy as usize % 4][gx as usize % 4] - 7.5) * 2.0
                            }
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
                        reference_samples.push(rgb);
                        if let Some(detailed) = &detailed {
                            let enhanced = detailed.get_pixel(gx, gy);
                            for c in 0..3 {
                                rgb[c] = (f64::from(enhanced[c]) + offset).clamp(0.0, 255.0) * alpha + matte[c] * (1.0 - alpha);
                            }
                        }
                        samples.push(rgb);
                    }
                }
                source_samples.push(metrics::Sample::new(&reference_samples, cw as usize, ch as usize));
                if let Some(matcher) = &matcher {
                    let cell = if let Some(grid) = &half_pixels {
                        let rgb = std::array::from_fn(|half| {
                            let pixel = grid.get_pixel(column as u32, row as u32 * 2 + half as u32);
                            let alpha = f64::from(pixel[3]) / 255.0;
                            std::array::from_fn(|c| f64::from(pixel[c]) * alpha + matte[c] * (1.0 - alpha))
                        });
                        matcher.solve_halves(rgb, &foreground, &background, column as usize, row as usize, dither)
                    } else {
                        matcher.solve(&samples, &masks[&page], &foreground, &background)
                    };
                    let best = cell.candidates[0];
                    let mut attr = TextAttribute::from_colors(AttributeColor::Palette(best.foreground as u8), AttributeColor::Palette(best.background as u8));
                    attr.set_font_page(page);
                    staged.push((position, AttributedChar::new(char::from(best.code), attr)));
                    styled.push(cell);
                    continue;
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
        let mut coherent_cells = 0;
        if matcher.is_some() {
            for ((_, cell), best) in staged.iter_mut().zip(style::refine(&styled, w as usize, tuning.coherence)) {
                let previous = *cell;
                cell.ch = char::from(best.code);
                cell.attribute.set_foreground(best.foreground as u32);
                cell.attribute.set_background(best.background as u32);
                coherent_cells += usize::from(*cell != previous);
            }
        }
        let mut rendered = Vec::with_capacity(staged.len());
        for (_, cell) in &staged {
            let on = &masks[&cell.attribute.font_page()]
                .iter()
                .find(|(code, _)| u32::from(*code) == cell.ch as u32)
                .ok_or("Converted glyph is outside the selected vocabulary")?
                .1;
            let mut pixels = vec![palette[cell.attribute.background() as usize]; (cw * ch) as usize];
            for &index in on {
                pixels[index] = palette[cell.attribute.foreground() as usize];
            }
            rendered.push(metrics::Sample::new(&pixels, cw as usize, ch as usize));
        }
        let mut options = json!({"layer": layer, "x": x, "y": y, "width": w, "height": h,
            "preset": preset_name, "mode": mode, "fit": fit, "dither": dither});
        if preset != style::Preset::Faithful {
            let fields = serde_json::to_value(tuning).map_err(|error| format!("Cannot encode conversion tuning: {error}"))?;
            options
                .as_object_mut()
                .ok_or("Conversion options must be an object")?
                .extend(fields.as_object().ok_or("Conversion tuning must be an object")?.clone());
        }
        Ok(Conversion {
            source: image.clone(),
            layer,
            original,
            cells: staged,
            options,
            metrics: metrics::Metrics::measure(&source_samples, &rendered, w as usize),
            coherent_cells,
        })
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

#[cfg(test)]
fn fit_image(source: &RgbaImage, width: u32, height: u32, aspect: f32, fit: &str) -> RgbaImage {
    fit_image_with_filter(source, width, height, aspect, fit, imageops::FilterType::Triangle)
}

fn fit_image_with_filter(source: &RgbaImage, width: u32, height: u32, aspect: f32, fit: &str, filter: imageops::FilterType) -> RgbaImage {
    if fit == "stretch" {
        return imageops::resize(source, width, height, filter);
    }
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
        return imageops::resize(&crop, width, height, filter);
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
    let fitted = imageops::resize(source, w, h, filter);
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
    conversion["mode"] = json!({"type": "string", "enum": ["half_blocks", "half_pixels", "blocks", "full", "ascii"]});
    conversion["preset"] = json!({"type": "string", "enum": ["faithful", "scene", "toon", "pixel_art"]});
    conversion["fit"] = json!({"type": "string", "enum": ["contain", "crop", "stretch"]});
    conversion["dither"] = json!({"type": "boolean"});
    conversion
        .as_object_mut()
        .expect("conversion schema")
        .extend(style::Tuning::schema().as_object().expect("tuning schema").clone());
    let mut refinement = style::Tuning::schema();
    refinement["critique"] = json!({"type": "string", "minLength": 1, "maxLength": 2000, "description": "Visible defect in the preceding preview and why these parameter changes should help"});
    vec![
        ("icy_preview_canvas",
         "Return a rendered PNG of the combined draft, using actual fonts, palette, letter spacing and display aspect ratio. Coordinates are document cells; default whole canvas. At most 8000 cells/4 megapixels and 3 previews per turn. Inspect before claiming success; images require a vision model.",
         json!({"type": "object", "properties": region})),
        ("icy_convert_reference_image",
         "Locally convert the most recently attached picture into draft cells. No paths or downloads. Read canvas_info first. Explicit target x/y/width/height are layer-relative; defaults to whole layer. Replaces only that region. Preset: faithful (default, original pixel matching), scene (perceptual blended shades and coherent colors), toon (simplified lightness and coherent flat regions), pixel_art (nearest resize and perceptual pixel matching). Styled presets require CP437 and accept optional tuning parameters, including hue_families to preserve chromatic hue families. Modes: half_blocks fits actual glyph masks; half_pixels uses two independent area-sampled pixels per cell, requires a styled preset and horizontal block font; blocks includes blocks/shades; full includes all glyphs; ascii is printable ASCII only. Scene defaults to blocks; other CP437 presets to half_blocks. Retro requires faithful/full. Fit contain (default), center crop, or stretch (distorts proportions). Dither defaults false; half_pixels dithers lightness gently at the half-cell scale. Returns metrics and effective options. Preserves font pages, retro shared colors and legal ANSI blink/iCE colors. Max 8000 cells, bounded matching work, 3 conversion/refinement trials per turn combined. Preview then critique with icy_refine_reference_image instead of reconverting over the best draft; user Apply required.",
         json!({"type": "object", "properties": conversion})),
        ("icy_refine_reference_image",
         "After icy_convert_reference_image and a PNG preview of the entire target, critique a visible defect and submit tuning changes. Only CP437 styled presets. Omitted tuning values retain the best settings; source, target, original transparency background, glyph mode, fit and preset stay fixed. A candidate is installed only if the fixed source-relative objective improves; rejected trials keep the best draft and count against the shared 3-trial limit. Requires a new preview before each trial and refuses intervening target cell edits. Returns before/candidate/retained metrics and settings. Preview again; stop if acceptable or budget exhausted. Metrics do not measure semantic recognition. Apply remains the user's decision.",
         json!({"type": "object", "properties": refinement, "required": ["critique"], "additionalProperties": false})),
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
    fn half_block_dither_survives_sampling_and_preserves_black_and_white() {
        let render = |value, dither| {
            let mut draft = draft(8, 4, RgbaImage::from_pixel(64, 64, image::Rgba([value, value, value, 255])));
            draft.buffer.ice_mode = IceMode::Ice;
            draft
                .convert_image(&json!({"preset": "scene", "mode": "half_pixels", "dither": dither, "coherence": 0.0}))
                .unwrap();
            for y in 0..4 {
                for x in 0..8 {
                    assert!([32, 219, 220, 223].contains(&(draft.buffer.char_at(Position::new(x, y)).ch as u32)));
                }
            }
            draft.buffer.render_to_rgba(&Rectangle::from(0, 0, 8, 4).into(), false).1
        };
        for value in [0, 255] {
            assert_eq!(render(value, true), render(value, false));
        }
        let mean = |pixels: Vec<u8>| pixels.chunks_exact(4).map(|p| style::linear([f64::from(p[0]); 3])[0]).sum::<f64>() / (pixels.len() / 4) as f64;
        let expected = style::linear([128.0; 3])[0];
        let literal = (mean(render(128, false)) - expected).abs();
        let dithered = (mean(render(128, true)) - expected).abs();
        assert!(
            dithered < literal * 0.75,
            "half-pixel dithering must improve average tone: {dithered} vs {literal}"
        );
    }

    #[test]
    fn detail_contrast_metrics_still_measure_the_unprocessed_reference() {
        let source = RgbaImage::from_fn(24, 16, |x, y| {
            let value = (70 + x * 4 + y) as u8;
            image::Rgba([value, value, value, 255])
        });
        let mut draft = draft(3, 1, source.clone());
        draft.convert_image(&json!({"preset": "scene", "local_contrast": 1.0})).unwrap();
        let pixels = draft.buffer.render_to_rgba(&Rectangle::from(0, 0, 3, 1).into(), false).1;
        let rendered = RgbaImage::from_raw(24, 16, pixels).unwrap();
        let samples = |image: &RgbaImage| {
            (0..3)
                .map(|cell| {
                    let rgb: Vec<_> = (0..16)
                        .flat_map(|y| {
                            (0..8).map(move |x| {
                                let pixel = image.get_pixel(cell * 8 + x, y);
                                [pixel[0], pixel[1], pixel[2]].map(f64::from)
                            })
                        })
                        .collect();
                    metrics::Sample::new(&rgb, 8, 16)
                })
                .collect::<Vec<_>>()
        };
        let expected = metrics::Metrics::measure(&samples(&source), &samples(&rendered), 3);
        assert!((draft.conversion_review.as_ref().unwrap().metrics.objective - expected.objective).abs() < 1e-12);
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
            json!({"preset": "unknown"}),
            json!({"preset": 1}),
            json!({"fit": "unknown"}),
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
        for preset in ["faithful", "scene", "toon", "pixel_art"] {
            let mut draft = draft(1, 1, source.clone());
            draft.buffer.set_font(1, font.clone());
            let mut cell = AttributedChar::invisible();
            cell.attribute.set_font_page(1);
            draft.buffer.layers[0].set_char(Position::new(0, 0), cell);
            draft.convert_image(&json!({"mode": "full", "preset": preset})).unwrap();
            let cell = draft.buffer.layers[0].char_at(Position::new(0, 0));
            assert_eq!(cell.ch as u32, 42, "{preset}");
            assert_eq!(cell.attribute.font_page(), 1, "{preset}");
        }
    }

    #[test]
    fn scene_improves_rendered_midtone_blends_over_literal_matching() {
        let source = RgbaImage::from_fn(96, 16, |x, _| {
            let value = (40 + x / 8 * 15) as u8;
            image::Rgba([value, value, value, 255])
        });
        let score = |preset| {
            let mut draft = draft(12, 1, source.clone());
            draft.convert_image(&json!({"preset": preset, "mode": "blocks"})).unwrap();
            let (_, pixels) = draft.buffer.render_to_rgba(&Rectangle::from(0, 0, 12, 1).into(), false);
            let mut error = 0.0;
            for cell in 0..12 {
                let mut mean = [0.0; 3];
                for y in 0..16 {
                    for x in 0..8 {
                        let start = (y * 96 + cell * 8 + x) * 4;
                        let rgb = std::array::from_fn(|c| f64::from(pixels[start + c]));
                        for (channel, value) in mean.iter_mut().zip(style::linear(rgb)) {
                            *channel += value / 128.0;
                        }
                    }
                }
                let expected = style::linear([f64::from(40 + cell as u32 * 15); 3]);
                error += mean.into_iter().zip(expected).map(|(a, b)| (a - b).powi(2)).sum::<f64>();
            }
            (error, draft)
        };
        let (faithful, _) = score("faithful");
        let (scene, converted) = score("scene");
        eprintln!("Rendered midtone blend error: faithful={faithful:.6}, scene={scene:.6}");
        assert!(scene < faithful * 0.6, "scene blend error {scene} must be <60% of faithful {faithful}");
        assert!(converted.changes().iter().any(|(_, _, cell)| (176..=178).contains(&(cell.ch as u32))));
    }

    #[test]
    fn presets_are_deterministic_scoped_and_obey_explicit_glyph_and_color_constraints() {
        let source = RgbaImage::from_fn(16, 16, |x, y| image::Rgba([(x * 16) as u8, (y * 16) as u8, 192, 255]));
        for preset in ["faithful", "scene", "toon", "pixel_art"] {
            for mode in ["half_blocks", "half_pixels", "blocks", "full", "ascii"] {
                for ice in [IceMode::Blink, IceMode::Ice] {
                    let mut draft = draft(4, 1, source.clone());
                    draft.buffer.ice_mode = ice;
                    draft.buffer.set_use_letter_spacing(true);
                    let initial_buffer = draft.buffer.clone();
                    let args = json!({"preset": preset, "mode": mode, "x": 1, "width": 2});
                    if preset == "faithful" && mode == "half_pixels" {
                        assert!(draft.convert_image(&args).unwrap_err().contains("requires"));
                        assert!(draft.changes().is_empty());
                        continue;
                    }
                    let result: Value = serde_json::from_str(&draft.convert_image(&args).unwrap()).unwrap();
                    assert_eq!(result["preset"], preset);
                    let first = draft.changes();
                    assert!(!first.is_empty());
                    for (_, position, cell) in &first {
                        assert!((1..=2).contains(&position.x));
                        assert!(cell.attribute.foreground() < 16);
                        assert!(cell.attribute.background() < if ice == IceMode::Blink { 8 } else { 16 });
                        assert!(!cell.attribute.is_blinking());
                        let code = cell.ch as u32;
                        match mode {
                            "ascii" => assert!((32..=126).contains(&code)),
                            "half_blocks" | "half_pixels" => assert!([32, 219, 223, 220].contains(&code)),
                            "blocks" => assert!([32, 219, 223, 220, 221, 222, 176, 177, 178].contains(&code)),
                            _ => assert!(code <= 255),
                        }
                    }
                    // Contain margins use the existing background; repeat from identical input.
                    draft.buffer = initial_buffer;
                    draft.convert_image(&args).unwrap();
                    assert_eq!(draft.changes(), first, "{preset}/{mode}/{ice:?}");
                }
            }
        }
    }

    #[test]
    fn styles_reject_retro_and_late_font_errors_without_partial_writes() {
        let source = reference(RgbaImage::from_pixel(16, 16, image::Rgba([128, 90, 75, 255])));
        let buffer = icy_draw::screen_profile::petscii_buffer(icy_engine::PetsciiMachine::C64, icy_engine::PetsciiCase::Upper);
        let mut retro = Draft::new("PETSCII", buffer, 0, None);
        retro.begin_turn(Some(source));
        for preset in ["scene", "toon", "pixel_art"] {
            assert!(retro
                .convert_image(&json!({"preset": preset, "width": 1, "height": 1}))
                .unwrap_err()
                .contains("CP437"));
            assert!(retro.changes().is_empty());
            let mut draft = draft(2, 1, RgbaImage::new(16, 16));
            let mut missing = AttributedChar::invisible();
            missing.attribute.set_font_page(254);
            draft.buffer.layers[0].set_char(Position::new(1, 0), missing);
            let before = draft.buffer.layers[0].char_at(Position::new(0, 0));
            assert!(draft.convert_image(&json!({"preset": preset})).unwrap_err().contains("missing"));
            assert_eq!(draft.buffer.layers[0].char_at(Position::new(0, 0)), before);
        }
    }

    #[test]
    fn half_pixels_reject_non_block_fonts_without_writing_cells() {
        let mut draft = draft(2, 1, RgbaImage::from_pixel(16, 16, image::Rgba([170, 85, 0, 255])));
        let mut font = draft.buffer.font_for_render(0).unwrap().clone();
        font.glyphs[223] = font.glyphs[65].clone();
        draft.buffer.set_font(0, font);
        assert!(draft
            .convert_image(&json!({"preset": "scene", "mode": "half_pixels"}))
            .unwrap_err()
            .contains("horizontal half-block"));
        assert!(draft.changes().is_empty());
    }

    #[test]
    fn pixel_art_resizing_does_not_introduce_interpolated_colors() {
        let source = RgbaImage::from_fn(2, 1, |x, _| image::Rgba(if x == 0 { [0, 0, 0, 255] } else { [255; 4] }));
        for fit in ["contain", "crop"] {
            let resized = fit_image_with_filter(&source, 16, 16, 1.0, fit, imageops::FilterType::Nearest);
            assert!(resized.pixels().all(|pixel| pixel[0] == 0 || pixel[0] == 255));
        }
    }

    #[test]
    fn scene_converts_full_screen_with_blended_shades_and_repeatable_output() {
        let source = RgbaImage::from_fn(640, 400, |x, y| {
            let inside = (i64::from(x) - 320).pow(2) + (i64::from(y) - 200).pow(2) < 140 * 140;
            let value = (60 + x * 140 / 640) as u8;
            image::Rgba(if inside { [value, value, value, 255] } else { [0, 0, 170, 255] })
        });
        let mut draft = draft(80, 25, source);
        let initial = draft.buffer.clone();
        let start = std::time::Instant::now();
        let result: Value = serde_json::from_str(&draft.convert_image(&json!({"preset": "scene"})).unwrap()).unwrap();
        eprintln!("80x25 Scene conversion: {:?}", start.elapsed());
        assert_eq!(result["mode"], "blocks");
        assert_eq!(result["target"]["width"], 80);
        assert_eq!(result["target"]["height"], 25);
        let first = draft.changes();
        assert_eq!(first.len(), 2000);
        assert!(first.iter().filter(|(_, _, cell)| (176..=178).contains(&(cell.ch as u32))).count() > 100);
        let (_, rendered) = draft.buffer.render_to_rgba(&Rectangle::from(0, 0, 80, 25).into(), false);
        assert!(rendered[..640 * 4].chunks_exact(4).all(|pixel| pixel == [0, 0, 170, 255]));
        draft.buffer = initial;
        draft.convert_image(&json!({"preset": "scene"})).unwrap();
        assert_eq!(draft.changes(), first);
    }

    #[test]
    fn faithful_default_matches_explicit_preset_and_schema_exposes_styles() {
        let source = RgbaImage::from_fn(16, 16, |x, y| image::Rgba([(x * 13) as u8, (y * 15) as u8, 120, 255]));
        let mut implicit = draft(2, 1, source.clone());
        let mut explicit = draft(2, 1, source);
        implicit.convert_image(&json!({})).unwrap();
        explicit.convert_image(&json!({"preset": "faithful"})).unwrap();
        assert_eq!(implicit.changes(), explicit.changes());
        let specs = tool_specs();
        let (_, _, schema) = specs.iter().find(|(name, _, _)| *name == "icy_convert_reference_image").unwrap();
        assert_eq!(schema["properties"]["preset"]["enum"], json!(["faithful", "scene", "toon", "pixel_art"]));
    }

    #[test]
    fn critique_accepts_better_colors_rejects_regression_and_shares_trial_budget() {
        let mut draft = draft(2, 1, RgbaImage::from_pixel(16, 16, image::Rgba([170, 0, 0, 255])));
        let original = draft.original.layers[0].char_at(Position::new(0, 0));
        draft
            .call("icy_convert_reference_image", &json!({"preset": "scene", "saturation": 0.0}))
            .unwrap();
        let initial = draft.image_feedback()["metrics"]["objective"].as_f64().unwrap();
        assert!(initial > 0.001);
        draft.preview_image(&json!({})).unwrap();
        let accepted: Value = serde_json::from_str(
            &draft
                .call(
                    "icy_refine_reference_image",
                    &json!({
                        "critique": "The red source has become gray; restore chroma.", "saturation": 1.0
                    }),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(accepted["accepted"], true);
        assert_eq!(accepted["remaining_trials"], 1);
        assert!(accepted["retained_metrics"]["objective"].as_f64().unwrap() < initial * 0.01);
        let best = draft.changes();
        let options = draft.image_feedback()["effective_options"].clone();
        draft.preview_image(&json!({})).unwrap();
        let rejected: Value = serde_json::from_str(
            &draft
                .call(
                    "icy_refine_reference_image",
                    &json!({
                        "critique": "Try desaturating the red region.", "saturation": 0.0
                    }),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(rejected["accepted"], false);
        assert_eq!(rejected["remaining_trials"], 0);
        assert_eq!(draft.changes(), best);
        assert_eq!(draft.image_feedback()["effective_options"], options);
        assert_eq!(rejected["previous_metrics"], rejected["retained_metrics"]);
        assert!(draft
            .refine_image(&json!({"critique": "Again", "contrast": 0.9}))
            .unwrap_err()
            .contains("limit"));
        assert!(draft.convert_image(&json!({"preset": "scene"})).unwrap_err().contains("limit"));
        assert_eq!(draft.original.layers[0].char_at(Position::new(0, 0)), original);
        assert_ne!(draft.buffer.layers[0].char_at(Position::new(0, 0)), original);
    }

    #[test]
    fn critique_requires_full_preview_valid_controls_and_unedited_target() {
        let mut draft = draft(2, 1, RgbaImage::from_pixel(16, 16, image::Rgba([120, 160, 60, 255])));
        assert!(draft.refine_image(&json!({})).unwrap_err().contains("Convert"));
        draft.convert_image(&json!({"preset": "scene"})).unwrap();
        let change = json!({"critique": "Increase contrast.", "contrast": 1.1});
        assert!(draft.refine_image(&change).unwrap_err().contains("Inspect"));
        draft.preview_image(&json!({"width": 1})).unwrap();
        assert!(draft.refine_image(&change).unwrap_err().contains("Inspect"));
        draft.preview_image(&json!({})).unwrap();
        let before = draft.changes();
        for args in [
            json!({"contrast": 1.1}),
            json!({"critique": "", "contrast": 1.1}),
            json!({"critique": "No delta."}),
            json!({"critique": "Invalid", "contrast": 9}),
            json!({"critique": "Invalid", "brightness": "bright"}),
            json!({"critique": "Invalid", "saturation": -0.5}),
            json!({"critique": "Invalid", "lightness_levels": 1}),
            json!({"critique": "Invalid", "lightness_levels": 2.5}),
            json!({"critique": "Invalid", "coherence": -1}),
            json!({"critique": "Invalid", "shade_penalty": 2.1}),
            json!({"critique": "Invalid", "local_contrast": -0.1}),
            json!({"critique": "Move", "x": 1}),
            json!({"critique": "Change source", "fit": "crop"}),
            json!({"critique": "Change target", "mode": "full"}),
            json!({"critique": "Change style", "preset": "toon"}),
        ] {
            assert!(draft.refine_image(&args).is_err(), "{args}");
            assert_eq!(draft.changes(), before);
            assert_eq!(draft.conversion_calls, 1);
        }
        draft.preview_enabled = false;
        assert!(draft.refine_image(&change).unwrap_err().contains("image-capable"));
        draft.preview_enabled = true;
        draft.buffer.layers[0].properties.is_locked = true;
        assert!(draft.refine_image(&change).is_err());
        draft.buffer.layers[0].properties.is_locked = false;
        draft.buffer.layers[0].set_char(Position::new(0, 0), AttributedChar::from_char('X'));
        assert!(draft.refine_image(&change).unwrap_err().contains("edited"));
        assert_eq!(draft.buffer.layers[0].char_at(Position::new(0, 0)).ch, 'X');
    }

    #[test]
    fn critique_preserves_original_transparency_background_and_unrelated_cells() {
        let mut draft = draft(3, 1, RgbaImage::new(8, 16));
        let original = AttributedChar::new(' ', TextAttribute::from_colors(AttributeColor::Palette(7), AttributeColor::Palette(1)));
        draft.buffer.layers[0].set_char(Position::new(1, 0), original);
        draft.convert_image(&json!({"preset": "scene", "x": 1, "width": 1, "saturation": 0.0})).unwrap();
        draft.buffer.layers[0].set_char(Position::new(0, 0), AttributedChar::from_char('X'));
        draft.preview_image(&json!({})).unwrap();
        let result: Value = serde_json::from_str(
            &draft
                .refine_image(&json!({
                    "critique": "The transparent area should preserve its blue background.", "saturation": 1.0
                }))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result["accepted"], true);
        let (_, rgba) = draft.buffer.render_to_rgba(&Rectangle::from(1, 0, 1, 1).into(), false);
        assert!(rgba.chunks_exact(4).all(|pixel| pixel == [0, 0, 170, 255]));
        assert_eq!(draft.buffer.layers[0].char_at(Position::new(0, 0)).ch, 'X');
        let image = draft.reference_image.clone();
        draft.begin_turn(image);
        assert!(draft.conversion_review.is_some(), "continuing the same image keeps the best candidate");
        assert_eq!(draft.conversion_calls, 0);
        draft.begin_turn(Some(reference(RgbaImage::from_pixel(8, 16, image::Rgba([255; 4])))));
        assert!(draft.conversion_review.is_none());
        assert!(draft
            .refine_image(&json!({"critique": "New picture", "contrast": 1.1}))
            .unwrap_err()
            .contains("Convert"));
    }

    #[test]
    fn conversion_really_applies_neighbor_consistency_after_solving_cells() {
        let source = RgbaImage::from_fn(128, 128, |x, y| {
            let value = (90 + ((x / 8 + y / 16 * 16) * 7) % 43) as u8;
            image::Rgba([value, value, value, 255])
        });
        let mut draft = draft(16, 8, source);
        let initial = draft.buffer.clone();
        draft.convert_image(&json!({"preset": "scene", "coherence": 0.0})).unwrap();
        let independent = draft.changes();
        draft.buffer = initial;
        let report: Value = serde_json::from_str(&draft.convert_image(&json!({"preset": "scene", "coherence": 0.02})).unwrap()).unwrap();
        assert!(report["coherence_changed_cells"].as_u64().unwrap() > 0, "{report}");
        assert_ne!(
            draft.changes(),
            independent,
            "testing the helper alone does not establish that the pipeline invokes it"
        );
    }

    #[test]
    fn reported_measurements_match_actual_engine_render_including_ninth_column() {
        for spacing in [false, true] {
            let width = if spacing { 9 } else { 8 };
            let source = RgbaImage::from_fn(width, 16, |x, y| image::Rgba([(70 + x * 12) as u8, (70 + y * 7) as u8, 150, 255]));
            let mut draft = draft(1, 1, source.clone());
            draft.buffer.set_use_letter_spacing(spacing);
            draft.convert_image(&json!({"preset": "scene"})).unwrap();
            let (_, rgba) = draft.buffer.render_to_rgba(&Rectangle::from(0, 0, 1, 1).into(), false);
            let sample = |bytes: &[u8]| {
                let rgb: Vec<_> = bytes.chunks_exact(4).map(|pixel| std::array::from_fn(|c| f64::from(pixel[c]))).collect();
                metrics::Sample::new(&rgb, width as usize, 16)
            };
            let actual = metrics::Metrics::measure(&[sample(source.as_raw())], &[sample(&rgba)], 1);
            assert!((actual.objective - draft.image_feedback()["metrics"]["objective"].as_f64().unwrap()).abs() < 1e-12);
        }
    }
}
