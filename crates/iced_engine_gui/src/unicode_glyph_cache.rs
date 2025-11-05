use rustybuzz::{Face as HbFace, GlyphBuffer, UnicodeBuffer};
use std::collections::HashMap;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

pub struct RasterizedGlyph {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>, // alpha mask
    pub xmin: i32,
    pub ymin: i32,
}

pub struct UnicodeGlyphCache {
    face: Face<'static>,
    hb_face: HbFace<'static>,
    bold_face: Option<Face<'static>>,
    hb_bold_face: Option<HbFace<'static>>,
    glyphs: HashMap<(char, bool, u32), RasterizedGlyph>, // (char, bold?, rounded_px_size)
}

impl UnicodeGlyphCache {
    pub fn new(font_bytes: &'static [u8], bold_bytes: Option<&'static [u8]>) -> Self {
        let face = Face::parse(font_bytes, 0).expect("valid font");
        let hb_face = HbFace::from_slice(font_bytes, 0).expect("valid hb font");

        let (bold_face, hb_bold_face) = if let Some(b) = bold_bytes {
            let f = Face::parse(b, 0).expect("valid bold font");
            let hbf = HbFace::from_slice(b, 0).expect("valid bold hb face");
            (Some(f), Some(hbf))
        } else {
            (None, None)
        };

        Self {
            face,
            hb_face,
            bold_face,
            hb_bold_face,
            glyphs: HashMap::new(),
        }
    }

    pub fn ascender(&self) -> i16 {
        self.face.ascender()
    }
    pub fn descender(&self) -> i16 {
        self.face.descender()
    }
    pub fn units_per_em(&self) -> u16 {
        self.face.units_per_em()
    }

    pub fn has_bold(&self) -> bool {
        self.bold_face.is_some()
    }

    fn shape_single(&self, ch: char, use_bold: bool) -> Option<GlyphId> {
        let mut buf = UnicodeBuffer::new();
        // rustybuzz expects strings; single char works
        buf.push_str(&ch.to_string());
        let face = if use_bold && self.hb_bold_face.is_some() {
            self.hb_bold_face.as_ref().unwrap()
        } else {
            &self.hb_face
        };
        let shaped: GlyphBuffer = rustybuzz::shape(face, &[], buf);
        if shaped.len() == 0 {
            return None;
        }
        let gid_u32 = shaped.glyph_infos()[0].glyph_id;
        Some(GlyphId(gid_u32 as u16))
    }

    pub fn get(&mut self, ch: char, px_size: f32, bold: bool) -> Option<&RasterizedGlyph> {
        let size_key = (px_size.round() as u32).max(1);
        let key = (ch, bold, size_key);

        if !self.glyphs.contains_key(&key) {
            // Early escapes
            if ch == ' ' || ch == '\0' {
                self.glyphs.insert(
                    key,
                    RasterizedGlyph {
                        width: 0,
                        height: 0,
                        pixels: Vec::new(),
                        xmin: 0,
                        ymin: 0,
                    },
                );
                return self.glyphs.get(&key);
            }

            let gid = match self.shape_single(ch, bold) {
                Some(g) => g,
                None => {
                    // Fallback: treat as empty
                    self.glyphs.insert(
                        key,
                        RasterizedGlyph {
                            width: 0,
                            height: 0,
                            pixels: Vec::new(),
                            xmin: 0,
                            ymin: 0,
                        },
                    );
                    return self.glyphs.get(&key);
                }
            };

            // Collect outline
            let mut pb = PathBuilder::new();
            let mut collector = PathCollector::new(&mut pb);
            let outlined = self.face.outline_glyph(gid, &mut collector);

            if outlined.is_none() || collector.is_empty() {
                // No drawable outline
                self.glyphs.insert(
                    key,
                    RasterizedGlyph {
                        width: 0,
                        height: 0,
                        pixels: Vec::new(),
                        xmin: 0,
                        ymin: 0,
                    },
                );
                return self.glyphs.get(&key);
            }

            let xmin = collector.xmin;
            let ymin = collector.ymin;
            let xmax = collector.xmax;
            let ymax = collector.ymax;
            let gw_raw = (xmax - xmin).max(0) as u32;
            let gh_raw = (ymax - ymin).max(0) as u32;

            if gw_raw == 0 || gh_raw == 0 {
                self.glyphs.insert(
                    key,
                    RasterizedGlyph {
                        width: 0,
                        height: 0,
                        pixels: Vec::new(),
                        xmin: 0,
                        ymin: 0,
                    },
                );
                return self.glyphs.get(&key);
            }

            let units_per_em = self.face.units_per_em() as f32;
            let scale = px_size / units_per_em;
            let scaled_w = (gw_raw as f32 * scale).ceil() as u32;
            let scaled_h = (gh_raw as f32 * scale).ceil() as u32;

            let mut pixmap = Pixmap::new(scaled_w.max(1), scaled_h.max(1)).unwrap();

            let mut path = pb.finish().unwrap_or_else(|| {
                // Empty path fallback
                PathBuilder::new().finish().unwrap()
            });

            // Translate to origin
            let translate = Transform::from_translate(-(xmin as f32), -(ymin as f32));
            if let Some(p) = path.clone().transform(translate) {
                path = p;
            }
            // Scale with Y-axis flip (negative scale on Y)
            let scale_tf = Transform::from_scale(scale, -scale);
            if let Some(p) = path.clone().transform(scale_tf) {
                path = p;
            }
            // Move back down since we flipped
            let move_down = Transform::from_translate(0.0, scaled_h as f32);
            if let Some(p) = path.clone().transform(move_down) {
                path = p;
            }

            let mut paint = Paint::default();
            paint.set_color(Color::from_rgba(1.0, 1.0, 1.0, 1.0).unwrap());

            // First fill (base)
            pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

            // Synthetic bold: draw a second pass offset slightly if bold requested but no bold face
            if bold && !self.bold_face.is_some() {
                // Translate by 1px right
                if let Some(p2) = path.transform(Transform::from_translate(1.0, 0.0)) {
                    pixmap.fill_path(&p2, &paint, FillRule::Winding, Transform::identity(), None);
                }
            }

            // Extract alpha
            let data = pixmap.data();
            let mut alpha = Vec::with_capacity((scaled_w * scaled_h) as usize);
            for chunk in data.chunks_exact(4) {
                alpha.push(chunk[3]);
            }

            self.glyphs.insert(
                key,
                RasterizedGlyph {
                    width: scaled_w,
                    height: scaled_h,
                    pixels: alpha,
                    xmin,
                    ymin,
                },
            );
        }

        self.glyphs.get(&key)
    }
}

// Outline collector implementing ttf-parser's OutlineBuilder trait
struct PathCollector<'a> {
    pb: &'a mut PathBuilder,
    pub xmin: i32,
    pub ymin: i32,
    pub xmax: i32,
    pub ymax: i32,
    had_any: bool,
}

impl<'a> PathCollector<'a> {
    fn new(pb: &'a mut PathBuilder) -> Self {
        Self {
            pb,
            xmin: i32::MAX,
            ymin: i32::MAX,
            xmax: i32::MIN,
            ymax: i32::MIN,
            had_any: false,
        }
    }

    fn update_bounds(&mut self, x: f32, y: f32) {
        let xi = x as i32;
        let yi = y as i32;
        self.xmin = self.xmin.min(xi);
        self.ymin = self.ymin.min(yi);
        self.xmax = self.xmax.max(xi);
        self.ymax = self.ymax.max(yi);
        self.had_any = true;
    }

    fn is_empty(&self) -> bool {
        !self.had_any
    }
}

impl<'a> OutlineBuilder for PathCollector<'a> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.update_bounds(x, y);
        self.pb.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.update_bounds(x, y);
        self.pb.line_to(x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.update_bounds(x1, y1);
        self.update_bounds(x, y);
        self.pb.quad_to(x1, y1, x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.update_bounds(x1, y1);
        self.update_bounds(x2, y2);
        self.update_bounds(x, y);
        self.pb.cubic_to(x1, y1, x2, y2, x, y);
    }

    fn close(&mut self) {
        self.pb.close();
    }
}
