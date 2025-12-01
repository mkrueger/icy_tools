use crate::UnicodeGlyphCache;
use icy_engine::{Position, Screen};
use parking_lot::Mutex;
use std::sync::Arc;

pub struct RenderUnicodeOptions {
    pub selection: Option<icy_engine::Selection>,
    pub selection_fg: Option<icy_engine::Color>,
    pub selection_bg: Option<icy_engine::Color>,
    pub blink_on: bool,
    pub font_px_size: Option<f32>,
    pub glyph_cache: Arc<Mutex<Option<UnicodeGlyphCache>>>, // Arc<Mutex> for interior mutability
}

pub fn render_unicode_to_rgba(buf: &dyn Screen, opts: &RenderUnicodeOptions) -> (icy_engine::Size, Vec<u8>) {
    let width = buf.get_width();
    let height = buf.get_height();

    let size = buf.get_font_dimensions();
    let (cell_w, cell_h) = (size.width as usize, size.height as usize);

    //static FONT_REGULAR: &[u8] = include_bytes!("../fonts/modern-fixedsys-excelsior/FSEX301-L2.ttf");
    // static FONT_REGULAR: &[u8] = include_bytes!("../fonts/1985-ibm-pc-vga/PxPlus_IBM_VGA8.ttf");
    static FONT_REGULAR: &[u8] = include_bytes!("../fonts/modern-pro-font-win-tweaked/ProFontWindows.ttf");
    static FONT_BOLD_OPT: Option<&[u8]> = None;

    // Lock and get or create the cache
    let mut cache_guard = opts.glyph_cache.lock();
    let glyph_cache = cache_guard.get_or_insert_with(|| UnicodeGlyphCache::new(FONT_REGULAR, FONT_BOLD_OPT));

    let px_size = opts.font_px_size.unwrap_or((cell_h as f32) * 0.90);

    let px_w = width as usize * cell_w;
    let px_h = height as usize * cell_h;
    let mut rgba = vec![0u8; px_w * px_h * 4];

    // Palette cache
    let mut palette_cache = [(0u8, 0u8, 0u8); 256];
    for i in 0..buf.palette().len() {
        palette_cache[i] = buf.palette().get_rgb(i as u32);
    }

    let explicit_sel = opts.selection_fg.as_ref().zip(opts.selection_bg.as_ref()).map(|(fg, bg)| {
        let (fr, fg_, fb) = fg.get_rgb();
        let (br, bg_, bb) = bg.get_rgb();
        (fr, fg_, fb, br, bg_, bb)
    });

    for row in 0..height {
        for col in 0..width {
            let pos = Position::new(col, row);
            let ch_attr = buf.get_char(pos);
            let ch = ch_attr.ch;

            let mut fg_idx = ch_attr.attribute.get_foreground();
            let mut bg_idx = ch_attr.attribute.get_background();
            let is_bold = ch_attr.attribute.is_bold();

            // Clamp indices to valid palette range
            let palette_len = buf.palette().len().min(256) as u32;
            if fg_idx >= palette_len {
                fg_idx = 7; // Default to white
            }
            if bg_idx >= palette_len {
                bg_idx = 0; // Default to black
            }

            if ch_attr.attribute.is_blinking() && !opts.blink_on {
                fg_idx = bg_idx;
            }
            // Simulated bold brightening only if no bold face
            if is_bold && fg_idx < 8 && !glyph_cache.has_bold() {
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

            let cell_px_x = col as usize * cell_w;
            let cell_px_y = row as usize * cell_h;

            // Background fill
            unsafe {
                for cy in 0..cell_h {
                    let off = (cell_px_y + cy) * (px_w * 4) + cell_px_x * 4;
                    let slice = rgba.get_unchecked_mut(off..off + cell_w * 4);
                    for px in slice.chunks_exact_mut(4) {
                        px[0] = br;
                        px[1] = bg;
                        px[2] = bb;
                        px[3] = 0xFF;
                    }
                }
            }

            if ch == '\0' || ch == ' ' {
                continue;
            }

            // let units_em = glyph_cache.units_per_em() as f32;
            // let scale = px_size / units_em;
            // let ascender_px = glyph_cache.ascender() as f32 * scale;
            // let descender_px = (-glyph_cache.descender()) as f32 * scale;

            // Use the cached glyph cache - it's mutable through the Mutex
            if let Some(rg) = glyph_cache.get(ch, px_size, is_bold) {
                let gw = rg.width as i32;
                let gh = rg.height as i32;
                if gw == 0 || gh == 0 {
                    continue;
                }

                // Horizontal (unchanged center logic)
                let mut off_x = cell_px_x as i32 + (cell_w as i32 - gw) / 2;
                if off_x < cell_px_x as i32 {
                    off_x = cell_px_x as i32;
                }
                if off_x + gw > (cell_px_x + cell_w) as i32 {
                    off_x = (cell_px_x + cell_w) as i32 - gw;
                }

                // Vertical placement - position baseline in upper portion of cell
                // Most terminal fonts expect baseline around 70-80% from top
                let baseline_from_top = (cell_h as f32 * 0.75).round();
                let baseline_y = cell_px_y as f32 + baseline_from_top;

                // The glyph's ymin is distance from baseline to top (negative for chars above baseline)
                // In our flipped coordinate system, we need to account for this
                let glyph_top_y = baseline_y - (rg.height as f32);
                let mut off_y = glyph_top_y as i32;

                // Adjust for lowercase characters - they should sit slightly lower
                if ch.is_lowercase() && !matches!(ch, 'g' | 'j' | 'p' | 'q' | 'y') {
                    off_y += 1;
                }

                // For characters with descenders, allow them to go below baseline
                if matches!(ch, 'g' | 'j' | 'p' | 'q' | 'y') {
                    off_y += 2; // Allow descenders to extend lower
                }

                // Ensure we don't go above the cell
                if off_y < cell_px_y as i32 {
                    off_y = cell_px_y as i32;
                }

                // For most characters, ensure they don't extend too far down
                // But allow descenders to go a bit past the cell bottom if needed
                let max_y = if matches!(ch, 'g' | 'j' | 'p' | 'q' | 'y') {
                    (cell_px_y + cell_h) as i32 + 2 // Allow slight overflow for descenders
                } else {
                    (cell_px_y + cell_h) as i32
                };

                if off_y + gh > max_y {
                    off_y = max_y - gh;
                    if off_y < cell_px_y as i32 {
                        off_y = cell_px_y as i32;
                    }
                }

                const MIN_COV: u8 = 8;
                const SOLID_COV: u8 = 220;

                for yy in 0..gh {
                    let py = off_y + yy;
                    // Allow slight overflow for descenders
                    if py < cell_px_y as i32 || py >= (cell_px_y + cell_h) as i32 + 2 {
                        continue;
                    }
                    if py >= px_h as i32 {
                        continue;
                    } // But don't go past buffer
                    let row_off = py as usize * (px_w * 4);
                    for xx in 0..gw {
                        let px = off_x + xx;
                        if px < cell_px_x as i32 || px >= (cell_px_x + cell_w) as i32 {
                            continue;
                        }
                        let cov = rg.pixels[(yy * gw + xx) as usize];
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
        }
    }

    (icy_engine::Size::new(px_w as i32, px_h as i32), rgba)
}
