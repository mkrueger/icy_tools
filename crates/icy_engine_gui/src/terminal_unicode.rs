//! Unicode text tiles in raw document coordinates, with independent raster density.
//!
//! Block elements and box drawing are drawn by [`builtin`] to fill the cell exactly.
//! Bundled fonts make Latin, Greek, Cyrillic and terminal symbols independent of
//! the host. Missing scripts use installed outline fonts in a stable order.
//! Install suitable system fonts (for example Noto Sans CJK) for additional
//! scripts. Font discovery is local only. Unsupported clusters, including
//! color-only emoji without an outline fallback, produce a box and a log warning.
mod builtin;

use std::{collections::HashMap, sync::OnceLock};

use icy_engine::{Position, Rectangle, RenderOptions, Screen, Size, TextAttribute};
use parking_lot::Mutex;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};
use ttf_parser::OutlineBuilder;
use unicode_width::UnicodeWidthStr;

const TERMINUS: &[u8] = include_bytes!("../fonts/modern-terminus/TerminusTTF-4.46.0.ttf");
const HERMIT: &[u8] = include_bytes!("../fonts/modern-hermit/Hermit-medium.otf");
const HERMIT_BOLD: &[u8] = include_bytes!("../fonts/modern-hermit/Hermit-bold.otf");
const FIRA: &[u8] = include_bytes!("../data/fonts/FiraSans-Regular.ttf");
const FIRA_BOLD: &[u8] = include_bytes!("../data/fonts/FiraSans-SemiBold.ttf");

struct SystemFonts {
    database: fontdb::Database,
    faces: Vec<fontdb::ID>,
}

fn system_fonts() -> &'static SystemFonts {
    static FONTS: OnceLock<SystemFonts> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut database = fontdb::Database::new();
        database.load_system_fonts();
        let mut faces: Vec<_> = database.faces().collect();
        // Never depend on filesystem enumeration order or the user's default font.
        faces.sort_by_key(|face| {
            let family = face.families.first().map_or("", |f| f.0.as_str());
            (
                !family.starts_with("Noto Sans"),
                !face.monospaced,
                face.style != fontdb::Style::Normal,
                face.weight.0.abs_diff(400),
                face.post_script_name.clone(),
                face.index,
            )
        });
        let faces = faces.into_iter().map(|face| face.id).collect();
        SystemFonts { database, faces }
    })
}

/// Load fallback fonts for visible UI characters without blocking the UI thread.
#[cfg(feature = "egui")]
pub fn request_unicode_ui_fonts(context: &egui::Context, characters: &[char]) {
    use std::{collections::HashSet, sync::Arc};

    #[derive(Default)]
    struct Loader {
        checked: HashSet<char>,
        loading: bool,
    }

    fn add_font(context: &egui::Context, name: String, bytes: &[u8], index: u32, missing: &mut Vec<char>) {
        let Ok(face) = ttf_parser::Face::parse(bytes, index) else {
            return;
        };
        let before = missing.len();
        missing.retain(|ch| {
            !face
                .glyph_index(*ch)
                .is_some_and(|glyph| ch.is_whitespace() || face.glyph_bounding_box(glyph).is_some())
        });
        if missing.len() != before {
            let mut data = egui::FontData::from_owned(bytes.to_vec());
            data.index = index;
            context.add_font(egui::epaint::text::FontInsert {
                name,
                data,
                families: vec![egui::epaint::text::InsertFontFamily {
                    family: egui::FontFamily::Monospace,
                    priority: egui::epaint::text::FontPriority::Lowest,
                }],
            });
        }
    }

    let loader = context.data_mut(|data| {
        data.get_temp_mut_or_default::<Arc<Mutex<Loader>>>(egui::Id::unique("unicode-ui-font-loader"))
            .clone()
    });
    let mut state = loader.lock();
    if state.loading {
        return;
    }
    let mut missing = context.fonts_mut(|fonts| {
        characters
            .iter()
            .copied()
            .filter(|ch| !state.checked.contains(ch) && !fonts.has_glyph(&egui::FontId::monospace(20.0), *ch))
            .collect::<Vec<_>>()
    });
    state.checked.extend(characters);
    if missing.is_empty() {
        return;
    }
    state.loading = true;
    drop(state);
    let context = context.clone();
    let worker_loader = loader.clone();
    if let Err(error) = std::thread::Builder::new().name("unicode-ui-fonts".into()).spawn(move || {
        add_font(&context, "unicode-hermit".into(), HERMIT, 0, &mut missing);
        add_font(&context, "unicode-fira".into(), FIRA, 0, &mut missing);
        add_font(&context, "unicode-terminus".into(), TERMINUS, 0, &mut missing);
        if !missing.is_empty() {
            let fonts = system_fonts();
            for &id in &fonts.faces {
                fonts.database.with_face_data(id, |bytes, index| {
                    add_font(&context, format!("unicode-system-{id:?}"), bytes, index, &mut missing);
                });
                if missing.is_empty() {
                    break;
                }
            }
        }
        if !missing.is_empty() {
            log::warn!("No installed UI font covers Unicode characters {missing:?}; using replacement glyphs");
        }
        worker_loader.lock().loading = false;
        context.request_repaint();
    }) {
        loader.lock().loading = false;
        log::error!("Unable to start Unicode UI font loader: {error}");
    }
}

#[derive(Default)]
pub(crate) struct TerminalUnicodeCache {
    // Keyed by size first, so hits look up `&str` without allocating per cell.
    masks: HashMap<(u32, u32, bool), HashMap<String, Vec<u8>>>,
    count: usize,
}

impl TerminalUnicodeCache {
    fn mask(&mut self, text: &str, width: u32, height: u32, bold: bool) -> &[u8] {
        let key = (width, height, bold);
        if !self.masks.get(&key).is_some_and(|masks| masks.contains_key(text)) {
            // Bound memory during long terminal sessions containing many unique clusters.
            if self.count >= 4096 {
                self.masks.clear();
                self.count = 0;
            }
            let mask = Self::rasterize_with_fallback(text, width, height, bold);
            self.masks.entry(key).or_default().insert(text.to_owned(), mask);
            self.count += 1;
        }
        &self.masks[&key][text]
    }

    fn rasterize_with_fallback(text: &str, width: u32, height: u32, bold: bool) -> Vec<u8> {
        if let Some(mask) = builtin::glyph(text, width, height) {
            return mask;
        }
        for bytes in [if bold { HERMIT_BOLD } else { HERMIT }, if bold { FIRA_BOLD } else { FIRA }, TERMINUS] {
            if let Some(mask) = rasterize(bytes, 0, text, width, height, bold) {
                return mask;
            }
        }
        let fonts = system_fonts();
        for &id in &fonts.faces {
            if let Some(Some(mask)) = fonts
                .database
                .with_face_data(id, |bytes, index| rasterize(bytes, index, text, width, height, bold))
            {
                return mask;
            }
        }
        log::warn!("No installed outline font covers Unicode grapheme {text:?}; drawing a missing-glyph box");
        missing_glyph(width, height)
    }
}

fn missing_glyph(width: u32, height: u32) -> Vec<u8> {
    let mut mask = vec![0; (width * height) as usize];
    for y in 1..height.saturating_sub(1) {
        for x in 1..width.saturating_sub(1) {
            if x == 1 || x == width - 2 || y == 1 || y == height - 2 {
                mask[(y * width + x) as usize] = 255;
            }
        }
    }
    mask
}

fn rasterize(bytes: &[u8], index: u32, text: &str, width: u32, height: u32, bold: bool) -> Option<Vec<u8>> {
    let face = rustybuzz::Face::from_slice(bytes, index)?;
    if !text
        .chars()
        .all(|ch| matches!(ch, '\u{200c}' | '\u{200d}' | '\u{fe00}'..='\u{fe0f}' | '\u{e0100}'..='\u{e01ef}') || face.glyph_index(ch).is_some())
    {
        return None;
    }
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    // A terminal cluster is positioned by its cell, never by bidi reordering.
    buffer.set_direction(rustybuzz::Direction::LeftToRight);
    let shaped = rustybuzz::shape(&face, &[], buffer);
    let advance: i32 = shaped.glyph_positions().iter().map(|p| p.x_advance).sum();
    let line_height = (face.ascender() as f32 - face.descender() as f32).max(1.0);
    let scale = (height as f32 / line_height).min(width as f32 / (advance as f32).max(1.0));
    let x_origin = (width as f32 - advance as f32 * scale) / 2.0;
    let baseline = face.ascender() as f32 * scale + (height as f32 - line_height * scale) / 2.0;
    let mut pixmap = Pixmap::new(width, height)?;
    let mut paint = Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    let mut pen_x = 0;
    let mut outlined = false;
    for (glyph, pos) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
        let mut outline = Outline(PathBuilder::new());
        if face.outline_glyph(ttf_parser::GlyphId(glyph.glyph_id as u16), &mut outline).is_some() {
            if let Some(path) = outline.0.finish() {
                let transform = Transform::from_row(
                    scale,
                    0.0,
                    0.0,
                    -scale,
                    x_origin + (pen_x + pos.x_offset) as f32 * scale,
                    baseline - pos.y_offset as f32 * scale,
                );
                pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
                if bold {
                    let mut bold_transform = transform;
                    bold_transform.tx += 0.5;
                    pixmap.fill_path(&path, &paint, FillRule::Winding, bold_transform, None);
                }
                outlined = true;
            }
        }
        pen_x += pos.x_advance;
    }
    // Bitmap/color-only emoji cannot be outlined: try another font, then show a box.
    if !outlined && !text.chars().all(char::is_whitespace) {
        return None;
    }
    Some(pixmap.data().chunks_exact(4).map(|p| p[3]).collect())
}

struct Outline(PathBuilder);
impl OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.quad_to(x1, y1, x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.cubic_to(x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        self.0.close();
    }
}

fn colors(screen: &dyn Screen, attribute: TextAttribute, pos: Position, options: &RenderOptions) -> ([u8; 4], [u8; 4]) {
    let palette = |index, default| {
        let (r, g, b) = screen.palette().rgb(if index < screen.palette().len() as u32 { index } else { default });
        [r, g, b, 255]
    };
    let rgb = |(r, g, b)| [r, g, b, 255];
    let extended = |index: u8| rgb(icy_engine::XTERM_256_PALETTE[index as usize].1.rgb());
    let bg = if attribute.is_background_rgb() {
        rgb(attribute.background_rgb())
    } else if attribute.is_background_ext() {
        extended(attribute.background_ext())
    } else {
        palette(attribute.background(), 0)
    };
    let fg = if attribute.is_blinking() && !options.blink_on {
        bg
    } else if attribute.is_foreground_rgb() {
        rgb(attribute.foreground_rgb())
    } else if attribute.is_foreground_ext() {
        extended(attribute.foreground_ext())
    } else {
        let index = attribute.foreground();
        palette(if attribute.is_bold() && index < 8 { index + 8 } else { index }, 7)
    };
    if options.selection.is_some_and(|selection| selection.is_inside(pos)) {
        if let Some((fg, bg)) = options.selection_fg.as_ref().zip(options.selection_bg.as_ref()) {
            (rgb(fg.rgb()), rgb(bg.rgb()))
        } else {
            (bg, fg)
        }
    } else {
        let mut fg = fg;
        let mut bg = bg;
        if attribute.is_foreground_transparent() {
            fg[3] = 0;
        }
        if attribute.is_background_transparent() {
            bg[3] = 0;
        }
        (fg, bg)
    }
}

pub(crate) fn render_region(screen: &dyn Screen, region: Rectangle, options: &RenderOptions, cache: &Mutex<TerminalUnicodeCache>) -> (Size, Vec<u8>) {
    render_region_scaled(screen, region, options, cache, 1)
}

pub(crate) fn render_region_scaled(
    screen: &dyn Screen,
    region: Rectangle,
    options: &RenderOptions,
    cache: &Mutex<TerminalUnicodeCache>,
    raster_scale: u32,
) -> (Size, Vec<u8>) {
    let scaled_region = || {
        let scale = i32::try_from(raster_scale).ok().filter(|scale| *scale > 0)?;
        Some((
            scale,
            Rectangle::from(
                region.start.x.checked_mul(scale)?,
                region.start.y.checked_mul(scale)?,
                region.size.width.checked_mul(scale)?,
                region.size.height.checked_mul(scale)?,
            ),
        ))
    };
    let Some((scale, region)) = scaled_region() else {
        log::error!("Unicode raster scale or region exceeds the render coordinate limits");
        return (Size::new(0, 0), Vec::new());
    };
    let width = region.size.width.max(0) as usize;
    let height = region.size.height.max(0) as usize;
    if width.checked_mul(height).is_none_or(|pixels| pixels > 100_000_000) || width == 0 || height == 0 {
        return (Size::new(0, 0), Vec::new());
    }
    let dims = screen.font_dimensions();
    let cell_w = dims.width.max(1) * scale;
    let raw_cell_h = screen.font(0).map_or(dims.height, |font| font.size().height).max(1);
    let cell_h = raw_cell_h * scale;
    let scan = if options.override_scan_lines.unwrap_or(screen.scan_lines()) { 2 } else { 1 };
    let line_h = cell_h * scan;
    let mut pixels = vec![0; width * height * 4];
    let first_col = region.start.x.div_euclid(cell_w).max(0);
    let last_col = (region.start.x + region.size.width - 1).div_euclid(cell_w).min(screen.width() - 1);
    let first_row = region.start.y.div_euclid(line_h).max(0);
    let last_row = (region.start.y + region.size.height - 1).div_euclid(line_h).min(screen.height() - 1);
    let mut cache = cache.lock();
    for row in first_row..=last_row {
        let mut next_col = 0;
        // Start one cell early so a wide grapheme crossing a horizontal crop is included.
        for col in (first_col - 1).max(0)..=last_col {
            let pos = Position::new(col, row);
            if col < next_col || screen.is_grapheme_continuation(pos) {
                continue;
            }
            let ch = screen.char_at(pos);
            let mut scalar_buffer = [0; 4];
            let scalar: &str = ch.ch.encode_utf8(&mut scalar_buffer);
            let (text, cells) = screen.grapheme_at(pos).unwrap_or((scalar, scalar.width().clamp(1, 2)));
            let cells = cells.clamp(1, 2);
            next_col = col + cells as i32;
            let glyph_w = cell_w * cells as i32;
            let mut cell_colors = [([0; 4], [0; 4]); 2];
            for (offset, colors_out) in cell_colors.iter_mut().enumerate().take(cells) {
                let cell_pos = Position::new(col + offset as i32, row);
                *colors_out = colors(screen, screen.char_at(cell_pos).attribute, cell_pos, options);
            }
            let cell_colors = &cell_colors[..cells];
            let mask = if text == "\0" || text.chars().all(char::is_whitespace) {
                None
            } else {
                Some(cache.mask(text, glyph_w as u32, cell_h as u32, ch.attribute.is_bold()))
            };
            let attribute = ch.attribute;
            let glyph_x = col * cell_w - region.start.x;
            let visible_w = glyph_w.min((screen.width() - col) * cell_w);
            let x_start = (-glyph_x).max(0);
            let x_end = visible_w.min(width as i32 - glyph_x);
            if x_start >= x_end {
                continue;
            }
            for y in 0..line_h {
                let py = row * line_h + y - region.start.y;
                if py < 0 || py >= height as i32 {
                    continue;
                }
                let gy = y / scan;
                let logical_y = gy / scale;
                let decoration = (attribute.is_underlined() && logical_y == raw_cell_h - 2)
                    || (attribute.is_double_underlined() && logical_y == raw_cell_h - 4)
                    || (attribute.is_overlined() && logical_y == 0)
                    || (attribute.is_crossed_out() && logical_y == raw_cell_h / 2);
                let mask_row = mask.map(|m| &m[(gy * glyph_w) as usize..((gy + 1) * glyph_w) as usize]);
                let row_out = &mut pixels[py as usize * width * 4..(py as usize + 1) * width * 4];
                // Selection and background can differ on the two halves of a wide cell.
                for (half, &(fg, bg)) in cell_colors.iter().enumerate() {
                    let start = x_start.max(half as i32 * cell_w);
                    let end = x_end.min((half as i32 + 1) * cell_w);
                    let background = if bg[3] == 0 { [0; 4] } else { bg };
                    for x in start..end {
                        let coverage = if decoration { 255 } else { mask_row.map_or(0, |m| m[x as usize]) };
                        let out = &mut row_out[(glyph_x + x) as usize * 4..][..4];
                        if coverage == 0 {
                            out.copy_from_slice(&background);
                            continue;
                        }
                        if coverage == 255 && fg[3] == 255 {
                            out.copy_from_slice(&fg);
                            continue;
                        }
                        let alpha = coverage as u32 * fg[3] as u32 / 255;
                        let out_alpha = alpha + bg[3] as u32 * (255 - alpha) / 255;
                        for c in 0..3 {
                            out[c] = if out_alpha == 0 {
                                0
                            } else {
                                ((fg[c] as u32 * alpha + bg[c] as u32 * bg[3] as u32 * (255 - alpha) / 255) / out_alpha) as u8
                            };
                        }
                        out[3] = out_alpha as u8;
                    }
                }
            }
        }
    }
    (region.size, pixels)
}

/// Render Unicode text in raw pixels, with the CRT path's bundled/system fallback.
/// Graphics overlays are separate, just as in the CRT text tile pipeline.
pub fn render_unicode_region_to_rgba(screen: &dyn Screen, region: Rectangle, options: &RenderOptions) -> (Size, Vec<u8>) {
    render_region(screen, region, options, cpu_cache())
}

fn cpu_cache() -> &'static Mutex<TerminalUnicodeCache> {
    static CACHE: OnceLock<Mutex<TerminalUnicodeCache>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Render Unicode screen text, graphics and display aspect ratio.
/// This CPU entry point is suitable for image exports and thumbnails; unlike the
/// region entry point its result is in display rather than raw texture pixels.
/// `options.rect` is a cell-space crop clamped to the screen before allocating.
/// The default selection means the entire screen; an explicit rectangular 1x1
/// selection still crops to one cell.
pub fn render_unicode_screen_to_rgba(screen: &dyn Screen, options: &RenderOptions) -> (Size, Vec<u8>) {
    let (left, top, right, bottom) = if options.rect == icy_engine::Selection::default() {
        (0, 0, screen.width().max(0), screen.height().max(0))
    } else {
        let min = options.rect.min();
        let max = options.rect.max();
        let (left, right) = if options.rect.shape == icy_engine::Shape::Lines && min.y != max.y {
            (0, screen.width().max(0))
        } else {
            (min.x.clamp(0, screen.width().max(0)), max.x.saturating_add(1).clamp(0, screen.width().max(0)))
        };
        (
            left,
            min.y.clamp(0, screen.height().max(0)),
            right,
            max.y.saturating_add(1).clamp(0, screen.height().max(0)),
        )
    };
    if left >= right || top >= bottom {
        return (Size::new(0, 0), Vec::new());
    }
    let dimensions = screen.font_dimensions();
    let raw_h = screen.font(0).map_or(dimensions.height, |font| font.size().height);
    let scan = if options.override_scan_lines.unwrap_or(screen.scan_lines()) { 2 } else { 1 };
    let checked_dimensions = || {
        let x = left.checked_mul(dimensions.width)?;
        let y = top.checked_mul(raw_h)?.checked_mul(scan)?;
        let width = (right - left).checked_mul(dimensions.width)?;
        let height = (bottom - top).checked_mul(raw_h)?.checked_mul(scan)?;
        let display_height = if screen.use_aspect_ratio() {
            (bottom - top).checked_mul(dimensions.height)?.checked_mul(scan)?
        } else {
            height
        };
        if width <= 0 || height <= 0 || display_height <= 0 || width as u64 * height.max(display_height) as u64 > 100_000_000 {
            return None;
        }
        Some((Rectangle::from(x, y, width, height), display_height))
    };
    let Some((region, display_height)) = checked_dimensions() else {
        log::error!("Unicode preview dimensions exceed the render allocation limit");
        return (Size::new(0, 0), Vec::new());
    };
    let (size, mut pixels) = render_unicode_region_to_rgba(screen, region, options);
    if pixels.is_empty() {
        return (size, pixels);
    }
    let (overlay_size, overlay) = screen.render_graphics_region_to_rgba_raw(region, options);
    if overlay_size == size {
        for (base, over) in pixels.chunks_exact_mut(4).zip(overlay.chunks_exact(4)) {
            let alpha = over[3] as u32;
            let out_alpha = alpha + base[3] as u32 * (255 - alpha) / 255;
            for c in 0..3 {
                base[c] = if out_alpha == 0 {
                    0
                } else {
                    ((over[c] as u32 * alpha + base[c] as u32 * base[3] as u32 * (255 - alpha) / 255) / out_alpha) as u8
                };
            }
            base[3] = out_alpha as u8;
        }
    }
    if screen.use_aspect_ratio() && display_height != size.height {
        if let Some(image) = image::RgbaImage::from_raw(size.width as u32, size.height as u32, pixels) {
            let image = image::imageops::resize(&image, size.width as u32, display_height as u32, image::imageops::FilterType::Nearest);
            return (Size::new(size.width, display_height), image.into_raw());
        }
        return (Size::new(0, 0), Vec::new());
    }
    (size, pixels)
}

/// Transparent RGBA8 grapheme preview using the CRT's font fallback.
/// `cell_size` is one cell in pixels; a wide grapheme returns twice that width.
/// Pass the result to egui's `ColorImage::from_rgba_unmultiplied`.
pub fn render_unicode_glyph_to_rgba(text: &str, cell_size: Size, foreground: [u8; 4]) -> (Size, Vec<u8>) {
    let Some(width) = cell_size.width.checked_mul(text.width().clamp(1, 2) as i32) else {
        return (Size::new(0, 0), Vec::new());
    };
    let height = cell_size.height;
    if width <= 0 || height <= 0 || width as u64 * height as u64 > 100_000_000 {
        return (Size::new(0, 0), Vec::new());
    }
    let size = Size::new(width, height);
    let mut pixels = vec![0; width as usize * height as usize * 4];
    if text.is_empty() || text == "\0" || text.chars().all(char::is_whitespace) {
        return (size, pixels);
    }
    let mut cache = cpu_cache().lock();
    let mask = cache.mask(text, width as u32, height as u32, false);
    for (pixel, &alpha) in pixels.chunks_exact_mut(4).zip(mask) {
        pixel[..3].copy_from_slice(&foreground[..3]);
        pixel[3] = (alpha as u32 * foreground[3] as u32 / 255) as u8;
    }
    (size, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "egui")]
    #[test]
    fn ui_fallback_fonts_load_on_demand_and_preserve_existing_families() {
        let context = egui::Context::default();
        let mut definitions = egui::FontDefinitions::default();
        definitions.font_data.insert("test-fira".into(), egui::FontData::from_static(FIRA).into());
        definitions.families.insert(egui::FontFamily::Monospace, vec!["test-fira".into()]);
        let proportional = definitions.families[&egui::FontFamily::Proportional].clone();
        context.set_fonts(definitions);
        context.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
        let font = egui::FontId::monospace(20.0);
        let character = '\u{2801}';
        assert!(!context.fonts_mut(|fonts| fonts.has_glyph(&font, character)));
        request_unicode_ui_fonts(&context, &[character]);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !context.fonts_mut(|fonts| fonts.has_glyph(&font, character)) {
            assert!(std::time::Instant::now() < deadline, "Fallback font did not become available");
            std::thread::sleep(std::time::Duration::from_millis(1));
            context.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
        }
        let count = context.fonts(|fonts| {
            assert_eq!(fonts.definitions().families[&egui::FontFamily::Proportional], proportional);
            assert!(fonts.definitions().font_data.contains_key("unicode-hermit") || fonts.definitions().font_data.contains_key("unicode-terminus"));
            fonts.definitions().font_data.len()
        });
        request_unicode_ui_fonts(&context, &[character]);
        context.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
        assert_eq!(context.fonts(|fonts| fonts.definitions().font_data.len()), count);
    }

    #[test]
    fn bundled_fonts_cover_terminal_scripts_and_combining_clusters() {
        for text in ["A", "Ω", "Ж", "╬", "e\u{301}"] {
            let mask = [HERMIT, TERMINUS, FIRA].into_iter().find_map(|bytes| rasterize(bytes, 0, text, 8, 16, false));
            assert!(mask.is_some_and(|mask| mask.iter().any(|&alpha| alpha > 0)), "bundled glyph is missing: {text}");
        }
    }

    #[test]
    fn scalable_monospace_font_rasterizes_diagonals_at_the_requested_resolution() {
        assert!(
            rasterize(HERMIT, 0, "X", 32, 64, false).is_some(),
            "primary monospace font must provide scalable Latin outlines"
        );
        for text in ["X", "\u{3C7}"] {
            let (font, small) = [HERMIT, FIRA]
                .into_iter()
                .find_map(|font| rasterize(font, 0, text, 8, 16, false).map(|small| (font, small)))
                .expect("bundled outline fonts must cover Latin and Greek");
            let large = rasterize(font, 0, text, 32, 64, false).unwrap();
            assert!(
                (0..64).any(|y| (0..32).any(|x| large[y * 32 + x] != small[y / 4 * 8 + x / 4])),
                "high-resolution text must be rasterized from outlines, not magnified from 8x16 pixels"
            );
        }
    }

    #[test]
    fn terminal_blocks_and_lines_keep_their_cell_edges_at_high_resolution() {
        let mut cache = TerminalUnicodeCache::default();
        for (width, height) in [(8, 16), (32, 64)] {
            assert!(
                cache.mask("\u{2588}", width, height, false).iter().all(|alpha| *alpha == 255),
                "full block must fill its entire cell"
            );
            let line = cache.mask("\u{2500}", width, height, false);
            for x in [0, width - 1] {
                assert!(
                    (0..height).any(|y| line[(y * width + x) as usize] > 0),
                    "box-drawing lines must reach adjacent cells"
                );
            }
        }
    }

    #[test]
    fn installed_cjk_outline_font_is_used_instead_of_a_missing_glyph_box() {
        let fonts = system_fonts();
        let expected = fonts.faces.iter().find_map(|&id| {
            fonts
                .database
                .with_face_data(id, |bytes, index| rasterize(bytes, index, "界", 16, 16, false))
                .flatten()
        });
        // System scripts are optional; missing fonts must still have a visible fallback.
        let mut cache = TerminalUnicodeCache::default();
        let actual = cache.mask("界", 16, 16, false);
        assert!(actual.iter().any(|&alpha| alpha > 0));
        if let Some(expected) = expected {
            assert_eq!(actual, expected);
            assert_ne!(actual, missing_glyph(16, 16));
        } else {
            assert_eq!(actual, missing_glyph(16, 16));
        }
    }
}
