use std::collections::HashMap;
use std::mem::swap;
use std::sync::Mutex;

use icy_parser_core::PenType;

use super::{HATCH_PATTERN, HATCH_WIDE_PATTERN, HOLLOW_PATTERN, TYPE_PATTERN};
use super::{
    LINE_STYLE, RANDOM_PATTERN, SOLID_PATTERN,
    vdi::{TWOPI, color_idx_to_pixel_val, gdp_curve, pixel_val_to_color_idx},
};
use crate::palette_screen_buffer::igs::TerminalResolution;
use crate::{BitFont, EditableScreen, Position, Size, palette_screen_buffer::igs::vdi::blit_px};

#[derive(Debug, PartialEq)]
pub enum TextEffects {
    Normal,
    Thickened,
    Ghosted,
    Skewed,
    Underlined,
    Outlined,
}

#[derive(Debug)]
pub enum TextRotation {
    /// 0 degree
    Right,
    /// 90 degree
    Up,
    /// 180 degree
    Left,
    /// 270 degree
    Down,
    /// 360 degree
    RightReverse,
}

#[derive(Debug)]
pub enum PolymarkerType {
    Point,
    Plus,
    Star,
    Square,
    DiagonalCross,
    Diamond,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LineType {
    Solid,
    LongDash,
    DottedLine,
    DashDot,
    DashedLine,
    DashedDotDot,
    UserDefined,
}
impl LineType {
    fn get_mask(self) -> usize {
        match self {
            LineType::Solid => 0,
            LineType::LongDash => 1,
            LineType::DottedLine => 2,
            LineType::DashDot => 3,
            LineType::DashedLine => 4,
            LineType::DashedDotDot => 5,
            LineType::UserDefined => 6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawingMode {
    /// new = (fore AND mask) OR (back AND NOT mask)
    Replace,

    /// Transparent mode affects only the pixels where the mask is 1.
    /// new = (fore AND mask) OR (old AND NOT mask)
    Transparent,

    /// XOR mode reverses the bits representing the color. T
    /// new = mask XOR old
    Xor,

    /// Reverse transparent mode affects only the pixels where the mask is 0,
    /// changing them to the fore value.
    /// new = (old AND mask) OR (fore AND NOT mask)
    ReverseTransparent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillPatternType {
    Hollow,
    Solid,
    Pattern,
    Hatch,
    UserdDefined,
}

pub struct DrawExecutor {
    terminal_resolution: TerminalResolution,

    cur_position: Position,
    polymarker_color: u8,
    pub line_color: u8,
    pub fill_color: u8,
    pub text_color: u8,

    text_effects: TextEffects,
    text_size: i32,
    text_rotation: TextRotation,

    polymaker_type: PolymarkerType,
    line_type: LineType,
    drawing_mode: DrawingMode,
    polymarker_size: usize,
    solidline_size: usize,
    _user_defined_pattern_number: usize,

    fill_pattern_type: FillPatternType,
    fill_pattern: &'static [u16],
    pattern_index_number: usize,
    draw_border: bool,

    hollow_set: bool,

    // Screen memory for blit operations
    screen_memory: Vec<u8>,
    pub screen_memory_size: Size,
}

unsafe impl Send for DrawExecutor {}

unsafe impl Sync for DrawExecutor {}

impl Default for DrawExecutor {
    fn default() -> Self {
        DrawExecutor::new(TerminalResolution::Low)
    }
}
lazy_static::lazy_static! {
    pub static ref ATARI_ST_FONT_6x6: BitFont = BitFont::from_bytes("Atari ST 6x6", include_bytes!("../../../data/fonts/Atari/atari-st-6x6.yaff")).unwrap();
    pub static ref ATARI_ST_FONT_8x8: BitFont = BitFont::from_bytes("Atari ST 8x8", include_bytes!("../../../data/fonts/Atari/atari-st-8x8.yaff")).unwrap();

    pub static ref ATARI_ST_FONT_12x12: BitFont = {
        ATARI_ST_FONT_6x6.double_size()
    };

    pub static ref ATARI_ST_FONT_16x16: BitFont = {
        ATARI_ST_FONT_8x8.double_size()
    };

    pub static ref ATARI_ST_FONT_7x11: BitFont = {
        ATARI_ST_FONT_8x16.scale_to_height(11).unwrap()
    };

    pub static ref ATARI_ST_FONT_14x22: BitFont = {
        ATARI_ST_FONT_7x11.double_size()
    };

    pub static ref ATARI_ST_FONT_8x16: BitFont = BitFont::from_bytes("Atari ST 8x8", include_bytes!("../../../data/fonts/Atari/atari-st-8x16.yaff")).unwrap();


    static ref ATARI_DYNAMIC_FONTS: Mutex<HashMap<i32, &'static BitFont>> = Mutex::new(HashMap::new());
}

fn load_atari_font(text_size: i32) -> (i32, &'static BitFont) {
    if text_size <= 8 {
        return (3, &ATARI_ST_FONT_6x6);
    }
    if text_size == 9 {
        return (6, &ATARI_ST_FONT_8x8);
    }

    if text_size <= 15 {
        // 7x11 Font
        return (11, &ATARI_ST_FONT_7x11);
    }

    if text_size <= 17 {
        // 12x12 Font (upscaled 6x6)
        return (8, &ATARI_ST_FONT_12x12);
    }

    if text_size <= 19 {
        // 16x16 Font (upscaled 8x8)
        return (12, &ATARI_ST_FONT_16x16);
    }

    (28, &ATARI_ST_FONT_14x22)
}

impl DrawExecutor {
    pub fn new(terminal_resolution: TerminalResolution) -> Self {
        Self {
            terminal_resolution,
            polymarker_color: 1,
            line_color: 1,
            fill_color: 1,
            text_color: 1,
            cur_position: Position::new(0, 0),
            text_effects: TextEffects::Normal,
            text_size: 9,
            text_rotation: TextRotation::Right,
            polymaker_type: PolymarkerType::Point,
            line_type: LineType::Solid,
            drawing_mode: DrawingMode::Replace,
            polymarker_size: 1,
            solidline_size: 1,
            _user_defined_pattern_number: 1,

            fill_pattern_type: FillPatternType::Solid,
            fill_pattern: &SOLID_PATTERN,
            pattern_index_number: 0,
            draw_border: false,
            hollow_set: false,
            screen_memory: Vec::new(),
            screen_memory_size: Size::new(0, 0),
        }
    }

    pub fn scroll(&mut self, buf: &mut dyn EditableScreen, amount: i32) {
        if amount == 0 {
            return;
        }
        let res = buf.get_resolution();
        if amount < 0 {
            buf.screen_mut().splice(0..0, vec![1; res.width as usize * amount.abs() as usize]);
            buf.screen_mut().truncate(res.width as usize * res.height as usize);
        } else {
            buf.screen_mut().splice(0..res.width as usize * amount.abs() as usize, vec![]);
            buf.screen_mut().extend(vec![1; res.width as usize * amount.abs() as usize]);
        }
    }

    pub fn set_resolution(&mut self, res: TerminalResolution) {
        self.terminal_resolution = res;
        // let res = buf.get_resolution();
        // buf.screen_mut() = vec![1; (res.width * res.height) as usize];
    }

    pub fn init_resolution(&mut self, buf: &mut dyn EditableScreen) {
        buf.clear_screen();
        // TODO?
    }

    pub fn reset_attributes(&mut self) {
        // TODO
    }

    pub fn flood_fill(&mut self, buf: &mut dyn EditableScreen, x0: i32, y0: i32) {
        let res = buf.get_resolution();

        if x0 < 0 || y0 < 0 || x0 >= res.width || y0 >= res.height {
            return;
        }
        let old_px = self.get_pixel(buf, x0, y0);

        let mut vec = vec![Position::new(x0, y0)];
        let col = self.fill_color;
        if old_px == col {
            return;
        }

        while let Some(pos) = vec.pop() {
            if pos.x < 0 || pos.y < 0 || pos.x >= res.width || pos.y >= res.height {
                continue;
            }

            let cp = self.get_pixel(buf, pos.x, pos.y);
            if cp != old_px {
                continue;
            }
            self.set_pixel(buf, pos.x, pos.y, col);

            vec.push(Position::new(pos.x - 1, pos.y));
            vec.push(Position::new(pos.x + 1, pos.y));
            vec.push(Position::new(pos.x, pos.y - 1));
            vec.push(Position::new(pos.x, pos.y + 1));
        }
    }

    /*
    fn flood_fill(&mut self, x0: i32, y0: i32) {
        let res = buf.get_resolution();

        if x0 < 0 || y0 < 0 || x0 >= res.width || y0 >= res.height {
            return;
        }
        let old_px = self.get_pixel(x0, y0);

        let mut vec = vec![Position::new(x0, y0)];
        let col = self.fill_color;
        if old_px == col {
            return;
        }

        let mut y = y0 - 1;
        while y >= 0 && self.get_pixel(x0, y) == old_px {
            vec.push(Position::new(x0, y));
            y -= 1;
        }

        let mut y = y0 + 1;
        while y < res.height && self.get_pixel(x0, y) == old_px {
            vec.push(Position::new(x0, y));
            y += 1;
        }

        while let Some(pos) = vec.pop() {
            if pos.x < 0 || pos.y < 0 || pos.x >= res.width || pos.y >= res.height {
                continue;
            }

            let cp = self.get_pixel(pos.x, pos.y);
            if cp != old_px {
                continue;
            }
            self.set_pixel(pos.x, pos.y, col);

            vec.push(Position::new(pos.x - 1, pos.y));
            vec.push(Position::new(pos.x + 1, pos.y));
        }
    }*/

    pub fn set_pixel(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, line_color: u8) {
        let res = buf.get_resolution();
        if x < 0 || y < 0 || x >= res.width || y >= res.height {
            return;
        }
        let offset = (y * res.width + x) as usize;
        buf.screen_mut()[offset] = line_color;
    }

    pub fn get_pixel(&mut self, buf: &dyn EditableScreen, x: i32, y: i32) -> u8 {
        let offset = (y * buf.get_resolution().width + x) as usize;
        buf.screen()[offset]
    }

    fn fill_pixel(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32) {
        let w = self.fill_pattern[(y as usize) % self.fill_pattern.len()];

        let mask = w & (0x8000 >> (x as usize % 16)) != 0;
        match self.drawing_mode {
            DrawingMode::Replace => {
                if mask {
                    self.set_pixel(buf, x, y, self.fill_color);
                }
            }
            DrawingMode::Transparent => {
                if mask {
                    self.set_pixel(buf, x, y, self.fill_color);
                }
            }
            DrawingMode::Xor => {
                let s = if mask { 0xFF } else { 0x00 };
                let d = color_idx_to_pixel_val(buf.palette().len(), self.get_pixel(buf, x, y));
                let new_color = pixel_val_to_color_idx(buf.palette().len(), (s ^ d) & 0x0F);
                self.set_pixel(buf, x, y, new_color);
            }
            DrawingMode::ReverseTransparent => {
                if !mask {
                    self.set_pixel(buf, x, y, self.fill_color);
                }
            }
        }
    }

    fn draw_vline(&mut self, buf: &mut dyn EditableScreen, x: i32, mut y0: i32, mut y1: i32, color: u8, mask: usize) {
        if y1 < y0 {
            swap(&mut y0, &mut y1);
        }
        let mut line_mask = LINE_STYLE[mask];
        for y in y0..=y1 {
            line_mask = line_mask.rotate_left(1);
            if 1 & line_mask != 0 {
                self.set_pixel(buf, x, y, color);
            }
        }
    }

    fn draw_hline(&mut self, buf: &mut dyn EditableScreen, y: i32, x0: i32, x1: i32, color: u8, mask: usize) {
        let mut line_mask = LINE_STYLE[mask];
        line_mask = line_mask.rotate_left((x0 & 0x0f) as u32);
        for x in x0..=x1 {
            line_mask = line_mask.rotate_left(1);
            if 1 & line_mask != 0 {
                self.set_pixel(buf, x, y, color);
            }
        }
    }

    pub fn draw_line(&mut self, buf: &mut dyn EditableScreen, mut x0: i32, mut y0: i32, mut x1: i32, mut y1: i32, color: u8, mask: usize) {
        if x1 < x0 {
            swap(&mut x0, &mut x1);
            swap(&mut y0, &mut y1);
        }
        if x0 == x1 {
            self.draw_vline(buf, x0, y0, y1, color, mask);
            return;
        }
        if y0 == y1 {
            self.draw_hline(buf, y0, x0, x1, color, mask);
            return;
        }
        let mut line_mask = LINE_STYLE[mask];

        let mut dx = x1 - x0;
        let mut dy = y1 - y0;

        let xinc = 1;

        let yinc;
        if dy < 0 {
            dy = -dy;
            yinc = -1;
        } else {
            yinc = 1;
        }

        let mut x = x0;
        let mut y = y0;

        if dx >= dy {
            let mut eps = -dx;
            let e1 = 2 * dy;
            let e2 = 2 * dx;
            while dx >= 0 {
                line_mask = line_mask.rotate_left(1);
                if 1 & line_mask != 0 {
                    self.set_pixel(buf, x, y, color);
                }
                x += xinc;
                eps += e1;
                if eps >= 0 {
                    eps -= e2;
                    y += yinc;
                }
                dx -= 1;
            }
        } else {
            let mut eps = -dy;
            let e1 = 2 * dx;
            let e2 = 2 * dy;
            while dy >= 0 {
                line_mask = line_mask.rotate_left(1);
                if 1 & line_mask != 0 {
                    self.set_pixel(buf, x, y, color);
                }
                y += yinc;

                eps += e1;
                if eps >= 0 {
                    eps -= e2;
                    x += xinc;
                }
                dy -= 1;
            }
        }
    }

    pub fn fill_circle(&mut self, buf: &mut dyn EditableScreen, xm: i32, ym: i32, r: i32) {
        let y_rad = self.calc_circle_y_rad(r).max(1);
        self.fill_ellipse(buf, xm, ym, r, y_rad);
    }

    pub fn draw_circle(&mut self, buf: &mut dyn EditableScreen, xm: i32, ym: i32, r: i32) {
        let y_rad = self.calc_circle_y_rad(r);
        let points: Vec<i32> = gdp_curve(xm, ym, r, y_rad, 0, TWOPI as i32);
        self.draw_poly(buf, &points, self.line_color, false);
    }

    pub fn draw_ellipse(&mut self, buf: &mut dyn EditableScreen, xm: i32, ym: i32, a: i32, b: i32) {
        let points: Vec<i32> = gdp_curve(xm, ym, a, b, 0, TWOPI as i32);
        self.draw_poly(buf, &points, self.line_color, false);
    }

    pub fn draw_elliptical_pieslice(&mut self, buf: &mut dyn EditableScreen, xm: i32, ym: i32, xr: i32, yr: i32, beg_ang: i32, end_ang: i32) {
        let mut points = gdp_curve(xm, ym, xr, yr, beg_ang * 10, end_ang * 10);
        points.extend_from_slice(&[xm, ym]);
        self.draw_poly(buf, &points, self.line_color, true);
    }

    pub fn fill_elliptical_pieslice(&mut self, buf: &mut dyn EditableScreen, xm: i32, ym: i32, xr: i32, yr: i32, beg_ang: i32, end_ang: i32) {
        let mut points = gdp_curve(xm, ym, xr, yr, beg_ang * 10, end_ang * 10);
        points.extend_from_slice(&[xm, ym]);
        self.fill_poly(buf, &points);
    }

    pub fn draw_pieslice(&mut self, buf: &mut dyn EditableScreen, xm: i32, ym: i32, radius: i32, beg_ang: i32, end_ang: i32) {
        let yr = self.calc_circle_y_rad(radius);
        let mut points = gdp_curve(xm, ym, radius, yr, beg_ang * 10, end_ang * 10);
        points.extend_from_slice(&[xm, ym]);
        self.draw_poly(buf, &points, self.line_color, true);
    }

    pub fn fill_pieslice(&mut self, buf: &mut dyn EditableScreen, xm: i32, ym: i32, radius: i32, beg_ang: i32, end_ang: i32) {
        let yr = self.calc_circle_y_rad(radius);
        let mut points = gdp_curve(xm, ym, radius, yr, beg_ang * 10, end_ang * 10);
        points.extend_from_slice(&[xm, ym]);
        self.fill_poly(buf, &points);
    }

    pub fn fill_ellipse(&mut self, buf: &mut dyn EditableScreen, xm: i32, ym: i32, a: i32, b: i32) {
        let points: Vec<i32> = gdp_curve(xm, ym, a, b, 0, TWOPI as i32);
        self.fill_poly(buf, &points);
    }

    pub fn fill_rect(&mut self, buf: &mut dyn EditableScreen, mut x0: i32, mut y0: i32, mut x1: i32, mut y1: i32) {
        if y0 > y1 {
            std::mem::swap(&mut y0, &mut y1);
        }
        if x0 > x1 {
            std::mem::swap(&mut x0, &mut x1);
        }

        for y in y0..=y1 {
            for x in x0..=x1 {
                self.fill_pixel(buf, x, y);
            }
        }
    }

    pub fn draw_arc(&mut self, buf: &mut dyn EditableScreen, xm: i32, ym: i32, a: i32, b: i32, beg_ang: i32, end_ang: i32) {
        let points = gdp_curve(xm, ym, a, b, beg_ang * 10, end_ang * 10);
        self.draw_poly(buf, &points, self.line_color, false);
    }

    fn draw_poly(&mut self, buf: &mut dyn EditableScreen, parameters: &[i32], color: u8, close: bool) {
        let mut x = parameters[0];
        let mut y = parameters[1];
        let mask = self.line_type.get_mask();
        let mut i = 2;
        while i < parameters.len() {
            let nx = parameters[i];
            let ny = parameters[i + 1];
            self.draw_line(buf, x, y, nx, ny, color, mask);
            x = nx;
            y = ny;
            i += 2;
        }
        if close {
            // close polygon
            self.draw_line(buf, x, y, parameters[0], parameters[1], color, mask);
        }
    }

    pub fn draw_polyline(&mut self, buf: &mut dyn EditableScreen, color: u8, parameters: &[i32]) {
        let mut x = parameters[0];
        let mut y = parameters[1];
        let mask = self.line_type.get_mask();
        let mut i = 2;
        while i < parameters.len() {
            let nx = parameters[i];
            let ny = parameters[i + 1];
            self.draw_line(buf, x, y, nx, ny, color, mask);
            x = nx;
            y = ny;
            i += 2;
        }
    }

    pub fn fill_poly(&mut self, buf: &mut dyn EditableScreen, points: &[i32]) {
        if self.hollow_set {
            self.draw_poly(buf, points, self.fill_color, true);
            return;
        }
        let max_vertices = 512;
        let mut y_max = points[1];
        let mut y_min = points[1];

        let mut i = 3;
        while i < points.len() - 1 {
            let y = points[i];
            y_max = y_max.max(y);
            y_min = y_min.min(y);
            i += 2;
        }

        let point_cnt = points.len() / 2;
        // VDI apparently loops over the scan lines from bottom to top
        for y in (y_min + 1..=y_max).rev() {
            // Set up a buffer for storing polygon edges that intersect the scan line
            let mut edge_buffer = Vec::new();

            // Loop over all vertices/points and find the intersections
            for i in 0..point_cnt {
                // Account for fact that final point connects to the first point
                let mut next_point = i + 1;
                if next_point >= point_cnt {
                    next_point = 0;
                }

                // Convenience variables for endpoints

                let y1 = points[i * 2 + 1]; // Get Y-coord of 1st endpoint.
                let y2 = points[next_point * 2 + 1]; // Get Y-coord of 2nd endpoint.

                // Get Y delta of current vector/segment/edge
                let dy = y2 - y1;

                // If the current vector is horizontal (0), ignore it.
                // Calculate deltas of each endpoint with current scan line.
                let dy1 = (y - y1) as i32;
                let dy2 = (y - y2) as i32;

                // Determine whether the current vector intersects with
                // the scan line by comparing the Y-deltas we calculated
                // of the two endpoints from the scan line.
                //
                // If both deltas have the same sign, then the line does
                // not intersect and can be ignored.  The origin for this
                // test is found in Newman and Sproull.
                if (dy1 ^ dy2) < 0 {
                    let x1 = points[i * 2]; // Get X-coord of 1st endpoint.
                    let x2 = points[next_point * 2]; // Get X-coord of 2nd endpoint.

                    // Calculate X delta of current vector
                    let dx = (x2 - x1) << 1; // Left shift so we can round by adding 1 below

                    // Stop if we have reached the max number of verticies allowed (512)
                    if edge_buffer.len() >= max_vertices {
                        break;
                    }

                    // Add X value for this vector to edge buffer
                    let a = if dx < 0 {
                        ((dy2 * dx / dy + 1) >> 1) + x2
                    } else {
                        ((dy1 * dx / dy + 1) >> 1) + x1
                    };
                    edge_buffer.push(a);
                }
            }

            // All of the points of intersection have now been found.  If there
            // were none (or one, which I think is impossible), then there is
            // nothing more to do.  Otherwise, sort the list of points of
            // intersection in ascending order.
            // (The list contains only the x-coordinates of the points.)

            if edge_buffer.len() < 2 {
                continue;
            }

            // Sort the X-coordinates, so they are arranged left to right.
            // There are almost always exactly 2, except for weird shapes.
            edge_buffer.sort_by(|a, b| a.partial_cmp(b).unwrap());
            // Loop through all edges in pairs, filling the pixels in between.
            let mut j = 0;
            while j < edge_buffer.len() {
                /* grab a pair of endpoints */
                let x1 = edge_buffer[j] as i32;
                j += 1;
                let x2 = edge_buffer[j] as i32;
                j += 1;

                // Fill in all pixels horizontally from (x1, y) to (x2, y)
                for k in x1..=x2 {
                    self.fill_pixel(buf, k, y);
                }
            }
        }
        if self.fill_pattern_type == FillPatternType::Solid {
            self.draw_poly(buf, &points, self.fill_color, true);
        }
    }

    pub fn write_text(&mut self, buf: &mut dyn EditableScreen, text_pos: Position, string_parameter: &str) {
        let mut pos = text_pos;
        let (y_off, font) = load_atari_font(self.text_size);
        pos.y -= y_off;
        // println!("write_text {string_parameter} {text_pos} size:{} effect:{:?} rot:{:?}", self.text_size, self.text_effects, self.text_rotation);

        let color = self.text_color;
        let font_size = font.size();
        let mut draw_mask: u16 = if self.text_effects == TextEffects::Ghosted { 0x5555 } else { 0xFFFF };
        for ch in string_parameter.chars() {
            let glyph = font.get_glyph(ch).unwrap();
            for y in 0..font_size.height {
                for x in 0..font_size.width {
                    let iy = y; //(y as f32 / font_size.height as f32 * char_size.height as f32) as i32;
                    let ix = x; // (x as f32 / font_size.width as f32 * char_size.width as f32) as i32;
                    draw_mask = draw_mask.rotate_left(1);
                    // Check pixel in bitmap.pixels[row][col]
                    let pixel_set = if iy < glyph.bitmap.pixels.len() as i32 && ix < glyph.bitmap.pixels[iy as usize].len() as i32 {
                        glyph.bitmap.pixels[iy as usize][ix as usize]
                    } else {
                        false
                    };
                    if pixel_set {
                        if 1 & draw_mask != 0 {
                            let p = pos + Position::new(x, y);
                            self.set_pixel(buf, p.x, p.y, color);
                            if self.text_effects == TextEffects::Thickened {
                                self.set_pixel(buf, p.x + 1, p.y, color);
                            }
                        }
                    }
                }
                draw_mask = draw_mask.rotate_left(1);
            }
            if self.text_effects == TextEffects::Underlined {
                let y = font_size.height - 1;
                for x in 0..font_size.width {
                    let p = pos + Position::new(x, y);
                    self.set_pixel(buf, p.x, p.y, color);
                    if self.text_effects == TextEffects::Thickened {
                        self.set_pixel(buf, p.x + 1, p.y, color);
                    }
                }
            }
            match self.text_rotation {
                TextRotation::RightReverse | TextRotation::Right => pos.x += font_size.width,
                TextRotation::Up => pos.y -= font_size.height,
                TextRotation::Down => pos.y += font_size.height,
                TextRotation::Left => pos.x -= font_size.width,
            }
        }
    }

    pub fn blit_screen_to_screen(&mut self, buf: &mut dyn EditableScreen, write_mode: i32, from: Position, to: Position, mut dest: Position) {
        let mut width = (to.x - from.x).abs() as usize + 1;
        let mut height = (to.y - from.y).abs() as usize + 1;

        if dest.x < 0 {
            if width < dest.x.abs() as usize {
                return;
            }
            width -= dest.x.abs() as usize;
            dest.x = 0;
        }

        if dest.y < 0 {
            if height < dest.y.abs() as usize {
                return;
            }
            height -= dest.y.abs() as usize;
            dest.y = 0;
        }

        let start_x = to.x.min(from.x) as usize;
        let start_y = to.y.min(from.y) as usize;

        let res = buf.get_resolution();

        if res.width < dest.x as i32 || res.height < dest.y as i32 {
            return;
        }
        let width = width.min(res.width as usize - start_x);
        let height = height.min(res.height as usize - start_y);

        let line_length = res.width as usize;
        let start = start_x + start_y as usize * line_length;

        let mut offset: usize = start;

        let mut blit = Vec::with_capacity(width as usize * height as usize);

        for _y in 0..height {
            blit.extend_from_slice(&buf.screen_mut()[offset..offset + width]);
            offset += line_length;
        }

        offset = dest.x as usize + dest.y as usize * line_length;
        let mut blit_offset = 0;
        if res.width < dest.x as i32 || res.height < dest.y as i32 {
            return;
        }
        let width = width.min(res.width as usize - dest.x as usize);
        let height = height.min(res.height as usize - dest.y as usize);

        for _y in 0..height as i32 {
            let mut o = offset;
            for _x in 0..width {
                buf.screen_mut()[o] = blit_px(write_mode, buf.palette().len(), blit[blit_offset], buf.screen_mut()[o]);
                o += 1;
                blit_offset += 1;
            }
            offset += line_length;
        }
    }

    pub fn blit_memory_to_screen(&mut self, buf: &mut dyn EditableScreen, write_mode: i32, from: Position, to: Position, mut dest: Position) {
        let mut width = (to.x - from.x).abs() as usize + 1;
        let mut height = (to.y - from.y).abs() as usize + 1;

        if dest.x < 0 {
            if width < dest.x.abs() as usize {
                return;
            }
            width -= dest.x.abs() as usize;
            dest.x = 0;
        }

        if dest.y < 0 {
            if height < dest.y.abs() as usize {
                return;
            }
            height -= dest.y.abs() as usize;
            dest.y = 0;
        }

        let start_x = to.x.min(from.x) as usize;
        let start_y = to.y.min(from.y) as usize;

        if self.screen_memory_size.width < start_x as i32 || self.screen_memory_size.height < start_y as i32 {
            return;
        }
        let width = width.min(self.screen_memory_size.width as usize - start_x);
        let height = height.min(self.screen_memory_size.height as usize - start_y);

        let res = buf.get_resolution();
        for y in 0..height {
            let mut offset = (start_y + y) * self.screen_memory_size.width as usize + start_x;
            let mut screen_offset = (dest.y as usize + y) * res.width as usize + dest.x as usize;
            if screen_offset >= buf.screen().len() {
                break;
            }
            for _x in 0..width {
                if dest.x + _x as i32 >= res.width {
                    break;
                }
                let color = self.screen_memory[offset];
                offset += 1;
                if screen_offset >= buf.screen_mut().len() {
                    break;
                }
                let px = buf.screen_mut()[screen_offset];
                buf.screen_mut()[screen_offset] = blit_px(write_mode, buf.palette().len(), color, px);
                screen_offset += 1;
            }
        }
    }

    pub fn blit_screen_to_memory(&mut self, buf: &mut dyn EditableScreen, _write_mode: i32, from: Position, to: Position) {
        let width = (to.x - from.x).abs() + 1;
        let height = (to.y - from.y).abs() + 1;

        let start_x = to.x.min(from.x);
        let start_y = to.y.min(from.y);

        self.screen_memory_size = Size::new(width, height);
        self.screen_memory.clear();

        for y in 0..height {
            for x in 0..width {
                let color = self.get_pixel(buf, start_x + x, start_y + y);
                self.screen_memory.push(color);
            }
        }
    }

    pub fn round_rect(&mut self, buf: &mut dyn EditableScreen, mut x1: i32, mut y1: i32, mut x2: i32, mut y2: i32, filled: bool) {
        let mut points = Vec::new();
        if x1 > x2 {
            swap(&mut x1, &mut x2);
        }
        if y1 < y2 {
            swap(&mut y1, &mut y2);
        }

        let x_radius = (buf.get_resolution().width >> 6).min((x2 - x1) / 2) - 1;
        let y_radius = self.calc_circle_y_rad(x_radius).min((y1 - y2) / 2);

        const ISIN225: i32 = 12539;
        const ISIN450: i32 = 23170;
        const ISIN675: i32 = 30273;
        const ICOS225: i32 = ISIN675;
        const ICOS450: i32 = ISIN450;
        const ICOS675: i32 = ISIN225;

        let x_off = [
            0,
            (ICOS675 * x_radius) / 32767,
            (ICOS450 * x_radius) / 32767,
            (ICOS225 * x_radius) / 32767,
            x_radius,
        ];

        let y_off = [
            y_radius,
            (ISIN675 * y_radius) / 32767,
            (ISIN450 * y_radius) / 32767,
            (ISIN225 * y_radius) / 32767,
            0,
        ];
        let xc = x2 - x_radius;
        let yc = y2 + y_radius;

        // upper right
        for i in 0..x_off.len() {
            points.push(xc + x_off[i]);
            points.push(yc - y_off[i]);
        }

        // lower right
        let yc = y1 - y_radius;
        for i in 0..x_off.len() {
            points.push(xc + x_off[4 - i]);
            points.push(yc + y_off[4 - i]);
        }

        // lower left
        let xc = x1 + x_radius;
        for i in 0..x_off.len() {
            points.push(xc - x_off[i]);
            points.push(yc + y_off[i]);
        }

        // upper left
        let yc = y2 + y_radius;
        for i in 0..x_off.len() {
            points.push(xc - x_off[4 - i]);
            points.push(yc - y_off[4 - i]);
        }
        points.push(points[0]);
        points.push(points[1]);

        if filled {
            self.fill_poly(buf, &points);
        } else {
            self.draw_poly(buf, &points, self.line_color, false);
        }
    }

    pub fn draw_poly_maker(&mut self, buf: &mut dyn EditableScreen, x0: i32, y0: i32) {
        let points = match self.polymaker_type {
            PolymarkerType::Point => vec![1i32, 2, 0, 0, 0, 0],
            PolymarkerType::Plus => vec![2, 2, 0, -3, 0, 3, 2, -4, 0, 4, 0],
            PolymarkerType::Star => vec![3, 2, 0, -3, 0, 3, 2, 3, 2, -3, -2, 2, 3, -2, -3, 2],
            PolymarkerType::Square => vec![1, 5, -4, -3, 4, -3, 4, 3, -4, 3, -4, -3],
            PolymarkerType::DiagonalCross => vec![2, 2, -4, -3, 4, 3, 2, -4, 3, 4, -3],
            PolymarkerType::Diamond => vec![1, 5, -4, 0, 0, -3, 4, 0, 0, 3, -4, 0],
        };
        let num_lines = points[0];
        let mut i = 1;
        let old_type = self.line_type;
        let scale = 1;
        self.line_type = LineType::Solid;
        for _ in 0..num_lines {
            let num_points = points[i] as usize;
            i += 1;
            let mut p = Vec::new();
            for _x in 0..num_points {
                p.push(scale * points[i] + x0);
                i += 1;
                p.push(scale * points[i] + y0);
                i += 1;
            }
            self.draw_polyline(buf, self.polymarker_color, &p);
        }
        self.line_type = old_type;
    }

    fn calc_circle_y_rad(&self, xrad: i32) -> i32 {
        // st med 169, st low 338, st high 372, height == 372
        let x_size = match self.terminal_resolution {
            TerminalResolution::Low => 338,
            TerminalResolution::Medium => 169,
            TerminalResolution::High => 372,
        };
        xrad * x_size / 372
    }
}

pub const REGISTER_TO_PEN: &[usize; 17] = &[0, 2, 3, 6, 4, 7, 5, 8, 9, 10, 11, 14, 12, 12, 15, 13, 1];

// Public wrapper methods for IGS command handling
impl DrawExecutor {
    pub fn draw_rect(&mut self, buf: &mut dyn crate::EditableScreen, x1: i32, y1: i32, x2: i32, y2: i32) {
        self.fill_rect(buf, x1, y1, x2, y2);
        if self.draw_border {
            let color = self.line_color;
            self.draw_line(buf, x1, y1, x1, y2, color, 0);
            self.draw_line(buf, x2, y1, x2, y2, color, 0);
            self.draw_line(buf, x1, y1, x2, y1, color, 0);
            self.draw_line(buf, x1, y2, x2, y2, color, 0);
        }
    }

    pub fn draw_rounded_rect(&mut self, buf: &mut dyn crate::EditableScreen, x1: i32, y1: i32, x2: i32, y2: i32) {
        self.round_rect(buf, x1, y1, x2, y2, true);
        if self.draw_border {
            self.round_rect(buf, x1, y1, x2, y2, false);
        }
    }

    pub fn draw_line_pub(&mut self, buf: &mut dyn crate::EditableScreen, x1: i32, y1: i32, x2: i32, y2: i32) {
        let color = self.line_color;
        let mask = self.line_type.get_mask();
        self.draw_line(buf, x1, y1, x2, y2, color, mask);
    }

    pub fn draw_circle_pub(&mut self, buf: &mut dyn crate::EditableScreen, x: i32, y: i32, radius: i32) {
        self.fill_circle(buf, x, y, radius);
        if self.draw_border {
            self.draw_circle(buf, x, y, radius);
        }
    }

    pub fn draw_ellipse_pub(&mut self, buf: &mut dyn crate::EditableScreen, x: i32, y: i32, x_radius: i32, y_radius: i32) {
        self.fill_ellipse(buf, x, y, x_radius, y_radius);
        if self.draw_border {
            self.draw_ellipse(buf, x, y, x_radius, y_radius);
        }
    }

    pub fn draw_arc_pub(&mut self, buf: &mut dyn crate::EditableScreen, x: i32, y: i32, start_angle: i32, end_angle: i32, radius: i32) {
        let y_radius = self.calc_circle_y_rad(radius);
        self.draw_arc(buf, x, y, radius, y_radius, start_angle, end_angle);
    }

    pub fn draw_pieslice_pub(&mut self, buf: &mut dyn crate::EditableScreen, x: i32, y: i32, radius: i32, start_angle: i32, end_angle: i32) {
        self.fill_pieslice(buf, x, y, radius, start_angle, end_angle);
        if self.draw_border {
            self.draw_pieslice(buf, x, y, radius, start_angle, end_angle);
        }
    }

    pub fn draw_elliptical_pieslice_pub(
        &mut self,
        buf: &mut dyn crate::EditableScreen,
        x: i32,
        y: i32,
        x_radius: i32,
        y_radius: i32,
        start_angle: i32,
        end_angle: i32,
    ) {
        self.fill_elliptical_pieslice(buf, x, y, x_radius, y_radius, start_angle, end_angle);
        if self.draw_border {
            self.draw_elliptical_pieslice(buf, x, y, x_radius, y_radius, start_angle, end_angle);
        }
    }

    pub fn set_color(&mut self, pen: PenType, color: u8) {
        match pen {
            PenType::Polymarker => self.polymarker_color = color,
            PenType::Line => self.line_color = color,
            PenType::Fill => self.fill_color = color,
            PenType::Text => self.text_color = color,
        }
    }

    pub fn set_fill_pattern(&mut self, pattern_type: u8, pattern_index: u8) {
        self.fill_pattern_type = match pattern_type {
            0 => FillPatternType::Hollow,
            1 => FillPatternType::Solid,
            2 => FillPatternType::Pattern,
            3 => FillPatternType::Hatch,
            4 => FillPatternType::UserdDefined,
            _ => FillPatternType::Solid,
        };

        self.fill_pattern = match self.fill_pattern_type {
            FillPatternType::Hollow => &HOLLOW_PATTERN,
            FillPatternType::Solid => &SOLID_PATTERN,
            FillPatternType::Pattern => {
                if pattern_index == 0 {
                    &RANDOM_PATTERN
                } else if pattern_index >= 1 && pattern_index <= 24 {
                    &TYPE_PATTERN[pattern_index as usize - 1]
                } else {
                    &SOLID_PATTERN
                }
            }
            FillPatternType::Hatch => {
                if pattern_index >= 1 && pattern_index <= 6 {
                    &HATCH_PATTERN[pattern_index as usize - 1]
                } else if pattern_index >= 7 && pattern_index <= 12 {
                    &HATCH_WIDE_PATTERN[pattern_index as usize - 7]
                } else {
                    &SOLID_PATTERN
                }
            }
            FillPatternType::UserdDefined => &RANDOM_PATTERN,
        };

        self.pattern_index_number = pattern_index as usize;
        self.hollow_set = self.fill_pattern_type == FillPatternType::Hollow;
    }

    pub fn set_draw_border(&mut self, border: bool) {
        self.draw_border = border;
    }

    pub fn set_drawing_mode(&mut self, mode: DrawingMode) {
        self.drawing_mode = mode;
    }

    pub fn get_screen_memory_size(&self) -> Size {
        self.screen_memory_size
    }

    pub fn set_line_style_pub(&mut self, style: u8) {
        self.line_type = match style {
            0 | 1 => LineType::Solid,
            2 => LineType::LongDash,
            3 => LineType::DottedLine,
            4 => LineType::DashDot,
            5 => LineType::DashedLine,
            6 => LineType::DashedDotDot,
            _ => LineType::UserDefined,
        };
    }

    pub fn set_line_thickness(&mut self, thickness: usize) {
        self.solidline_size = thickness;
    }

    pub fn set_text_effects_pub(&mut self, effects: u8) {
        self.text_effects = match effects {
            0 => TextEffects::Normal,
            1 => TextEffects::Thickened,
            2 => TextEffects::Ghosted,
            4 => TextEffects::Skewed,
            8 => TextEffects::Underlined,
            16 => TextEffects::Outlined,
            _ => TextEffects::Normal,
        };
    }

    pub fn set_text_size(&mut self, size: i32) {
        self.text_size = size;
    }

    pub fn set_text_rotation_pub(&mut self, rotation: u8) {
        self.text_rotation = match rotation {
            0 => TextRotation::Right,
            1 => TextRotation::Up,
            2 => TextRotation::Left,
            3 => TextRotation::Down,
            _ => TextRotation::RightReverse,
        };
    }

    pub fn get_cur_position(&self) -> crate::Position {
        self.cur_position
    }

    pub fn set_cur_position(&mut self, x: i32, y: i32) {
        self.cur_position = crate::Position::new(x, y);
    }

    pub fn set_polymarker_type(&mut self, marker_type: u8) {
        self.polymaker_type = match marker_type {
            1 => PolymarkerType::Point,
            2 => PolymarkerType::Plus,
            3 => PolymarkerType::Star,
            4 => PolymarkerType::Square,
            5 => PolymarkerType::DiagonalCross,
            6 => PolymarkerType::Diamond,
            _ => PolymarkerType::Point,
        };
    }

    pub fn set_polymarker_size(&mut self, size: usize) {
        self.polymarker_size = size;
    }
}
