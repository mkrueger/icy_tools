use fontdue::{Font, Metrics};
use icy_engine::{Position, TextPane};
use std::collections::HashMap;

pub struct UnicodeGlyphCache {
    font: Font,
    font_bold: Font,
    glyphs: HashMap<(char, bool), (Metrics, Vec<u8>)>, // Key includes bold flag
}

impl UnicodeGlyphCache {
    fn new(font_bytes: &'static [u8], font_bold_bytes: &'static [u8], _cell_w: usize, _cell_h: usize) -> Self {
        let font = Font::from_bytes(font_bytes, fontdue::FontSettings::default()).expect("valid font");
        let font_bold = Font::from_bytes(font_bold_bytes, fontdue::FontSettings::default()).expect("valid bold font");
        Self {
            font,
            font_bold,
            glyphs: HashMap::new(),
        }
    }

    fn get(&mut self, ch: char, px_size: f32, is_bold: bool) -> (Metrics, &[u8]) {
        let key = (ch, is_bold);
        if !self.glyphs.contains_key(&key) {
            let font = if is_bold { &self.font_bold } else { &self.font };
            let (metrics, bitmap) = font.rasterize(ch, px_size);
            self.glyphs.insert(key, (metrics, bitmap));
        }
        let (m, bm) = self.glyphs.get(&key).unwrap();
        (*m, bm.as_slice())
    }
}

pub struct RenderUnicodeOptions<'a> {
    pub buffer: &'a icy_engine::Buffer,
    pub selection: Option<icy_engine::Selection>,
    pub selection_fg: Option<icy_engine::Color>,
    pub selection_bg: Option<icy_engine::Color>,
    pub blink_on: bool,
    pub font_px_size: Option<f32>, // Allow override
}

const TIGHT_MODE: bool = true;

pub fn render_unicode_to_rgba(opts: &RenderUnicodeOptions<'_>) -> (icy_engine::Size, Vec<u8>) {
    let buf = opts.buffer;
    let width = buf.get_width();
    let height = buf.get_height();

    // Prefer buffer font cell size if available; do NOT hard-code doubled dimensions:
    let (cell_w, cell_h) = (8 * 2, 16 * 2);

    static FONT_REGULAR: &[u8] = include_bytes!("../fonts/CascadiaMono-Regular.ttf");
    static FONT_BOLD: &[u8] = include_bytes!("../fonts/CascadiaMono-Bold.ttf");

    let mut glyph_cache = UnicodeGlyphCache::new(FONT_REGULAR, FONT_BOLD, cell_w, cell_h);

    // Base px size guess
    let mut px_size = opts.font_px_size.unwrap_or(cell_h as f32);

    // Sample a canonical wide glyph (use 'M'); fallback to 'W' or first printable
    let sample_char = 'M';
    let (sm_metrics, _) = glyph_cache.get(sample_char, px_size, false);

    // Vertical line metrics
    if let Some(lm) = glyph_cache.font.horizontal_line_metrics(px_size) {
        let vert_core = lm.ascent - lm.descent;
        let vert_scale = (cell_h as f32 - 2.0).max(4.0) / vert_core;

        // Horizontal target uses advance width (better than bitmap width)
        let horiz_scale = (cell_w as f32) / sm_metrics.advance_width.max(1.0);

        // Choose smaller scale to avoid overflow; clamp to reasonable range
        let applied_scale = vert_scale.min(horiz_scale).clamp(0.5, 1.5);
        px_size *= applied_scale;
    }

    // Recompute line metrics at final px_size
    // Recompute line metrics at final px_size
    let lm2 = glyph_cache.font.horizontal_line_metrics(px_size);
    let (ascent, descent) = lm2
        .map(|lm| (lm.ascent, lm.descent)) // descent is negative
        .unwrap_or((px_size * 0.78, -px_size * 0.22));
    let core_h = ascent - descent; // positive usable vertical ink
    let free_space = (cell_h as f32 - core_h).max(0.0);

    // Raise baseline: instead of equal top/bottom padding, give less top padding
    // Tunables:
    const TOP_PAD_FACTOR: f32 = 0.28; // 0.25–0.30 lifts glyphs slightly
    const DESCENT_LIFT_FACTOR: f32 = 0.18; // lift a bit more if descent is large

    let top_pad = free_space * TOP_PAD_FACTOR;
    let extra_descent_bias = (-descent).max(0.0) * DESCENT_LIFT_FACTOR;
    let baseline = (top_pad + ascent + extra_descent_bias).clamp(0.0, cell_h as f32) as i32;

    let px_w = width as usize * cell_w;
    let px_h = height as usize * cell_h;
    let mut rgba = vec![0u8; px_w * px_h * 4];

    // Palette cache
    let mut palette_cache = [(0u8, 0u8, 0u8); 256];
    for i in 0..buf.palette.len() {
        palette_cache[i] = buf.palette.get_rgb(i as u32);
    }

    let explicit_sel = opts.selection_fg.as_ref().zip(opts.selection_bg.as_ref()).map(|(fg, bg)| {
        let (fr, fg_, fb) = fg.get_rgb();
        let (br, bg_, bb) = bg.get_rgb();
        (fr, fg_, fb, br, bg_, bb)
    });

    for y in 0..height {
        for x in 0..width {
            let pos = Position::new(x, y);
            let ch_attr = buf.get_char(pos);
            let ch = ch_attr.ch;

            let mut fg_idx = ch_attr.attribute.get_foreground();
            let bg_idx = ch_attr.attribute.get_background();
            let is_bold = ch_attr.attribute.is_bold();

            if ch_attr.attribute.is_blinking() && !opts.blink_on {
                fg_idx = bg_idx;
            }
            if is_bold && fg_idx < 8 {
                fg_idx += 8;
            }

            let is_sel = opts.selection.as_ref().map_or(false, |s| s.is_inside(pos));
            let (fr, fg, fb, br, bg, bb) = if is_sel {
                if let Some((efr, efg, efb, ebr, ebg, ebb)) = explicit_sel {
                    (efr, efg, efb, ebr, ebg, ebb)
                } else {
                    let (fr_, fg_, fb_) = palette_cache[bg_idx as usize];
                    let (br_, bg_, bb_) = palette_cache[fg_idx as usize];
                    (fr_, fg_, fb_, br_, bg_, bb_)
                }
            } else {
                let (fr_, fg_, fb_) = palette_cache[fg_idx as usize];
                let (br_, bg_, bb_) = palette_cache[bg_idx as usize];
                (fr_, fg_, fb_, br_, bg_, bb_)
            };

            let cell_px_x = x as usize * cell_w;
            let cell_px_y = y as usize * cell_h;

            // Background fill
            unsafe {
                for cy in 0..cell_h {
                    let row_off = (cell_px_y + cy) * (px_w * 4) + cell_px_x * 4;
                    let slice = rgba.get_unchecked_mut(row_off..row_off + cell_w * 4);
                    for p in slice.chunks_exact_mut(4) {
                        p[0] = br;
                        p[1] = bg;
                        p[2] = bb;
                        p[3] = 0xFF;
                    }
                }
            }

            // Glyph render
            if ch != '\0' && ch != ' ' {
                let (metrics, bitmap) = glyph_cache.get(ch, px_size, is_bold);

                let gw = metrics.width;
                let gh = metrics.height;
                let advance = metrics.advance_width;
                let xmin = metrics.xmin;

                // Horizontal positioning: left-align by advance, not centering trimmed bitmap.
                // We shift so glyph ink starts at cell left + optional padding.
                let hpad = if TIGHT_MODE {
                    0
                } else {
                    ((cell_w as f32 - advance).max(0.0) / 2.0).floor() as i32
                };
                let off_x = (cell_px_x as i32 + hpad - xmin).max(cell_px_x as i32);

                // Vertical placement using adjusted baseline & ymin
                let glyph_top = baseline + metrics.ymin;
                let mut off_y = cell_px_y as i32 + glyph_top;
                // Prevent clipping above
                if off_y < cell_px_y as i32 {
                    off_y = cell_px_y as i32;
                }
                // Prevent overflow below
                if off_y + gh as i32 > (cell_px_y + cell_h) as i32 {
                    off_y = (cell_px_y + cell_h).saturating_sub(gh) as i32;
                }

                // Coverage thresholds tuned for sharper look
                const MIN_COV: u8 = 30;
                const SOLID_COV: u8 = 200;

                for gy in 0..gh {
                    let py = off_y + gy as i32;
                    if py < cell_px_y as i32 || py >= (cell_px_y + cell_h) as i32 || py >= px_h as i32 {
                        continue;
                    }
                    let row_off = py as usize * (px_w * 4);

                    for gx in 0..gw {
                        let px = off_x + gx as i32;
                        if px < cell_px_x as i32 || px >= (cell_px_x + cell_w) as i32 || px >= px_w as i32 {
                            continue;
                        }
                        let cov = bitmap[gy * gw + gx];
                        if cov < MIN_COV {
                            continue;
                        }
                        let o = row_off + px as usize * 4;
                        if cov >= SOLID_COV {
                            rgba[o] = fr;
                            rgba[o + 1] = fg;
                            rgba[o + 2] = fb;
                        } else {
                            let alpha = (cov - MIN_COV) as f32 / (SOLID_COV - MIN_COV) as f32;
                            let inv = 1.0 - alpha;
                            rgba[o] = (fr as f32 * alpha + br as f32 * inv) as u8;
                            rgba[o + 1] = (fg as f32 * alpha + bg as f32 * inv) as u8;
                            rgba[o + 2] = (fb as f32 * alpha + bb as f32 * inv) as u8;
                        }
                    }
                }
            }

            // Attributes (unchanged except using current vars)
            if ch_attr.attribute.is_underlined() {
                let rows: &[usize] = if ch_attr.attribute.is_double_underlined() {
                    &[cell_h.saturating_sub(3), cell_h.saturating_sub(1)]
                } else {
                    &[cell_h.saturating_sub(2)]
                };
                for r in rows {
                    if *r >= cell_h {
                        continue;
                    }
                    let py = cell_px_y + *r;
                    let row_off = py * (px_w * 4) + cell_px_x * 4;
                    for cx in 0..cell_w {
                        let o = row_off + cx * 4;
                        rgba[o] = fr;
                        rgba[o + 1] = fg;
                        rgba[o + 2] = fb;
                    }
                }
            }
            if ch_attr.attribute.is_overlined() {
                let py = cell_px_y;
                let row_off = py * (px_w * 4) + cell_px_x * 4;
                for cx in 0..cell_w {
                    let o = row_off + cx * 4;
                    rgba[o] = fr;
                    rgba[o + 1] = fg;
                    rgba[o + 2] = fb;
                }
            }
            if ch_attr.attribute.is_crossed_out() {
                let py = cell_px_y + cell_h / 2;
                let row_off = py * (px_w * 4) + cell_px_x * 4;
                for cx in 0..cell_w {
                    let o = row_off + cx * 4;
                    rgba[o] = fr;
                    rgba[o + 1] = fg;
                    rgba[o + 2] = fb;
                }
            }
        }
    }

    (icy_engine::Size::new(px_w as i32, px_h as i32), rgba)
}
