use std::collections::HashSet;
use std::mem::swap;

use icy_parser_core::{DrawingMode, LineKind, PatternType, PenType, PolymarkerKind, TextEffects, TextRotation};

use super::{HATCH_PATTERN, HATCH_WIDE_PATTERN, HOLLOW_PATTERN, TYPE_PATTERN};
use super::{
    RANDOM_PATTERN, SOLID_PATTERN,
    vdi::{TWOPI, gdp_curve},
};
use crate::igs::load_atari_font;
use crate::palette_screen_buffer::igs::TerminalResolution;
use crate::{EditableScreen, Position, Size, palette_screen_buffer::igs::vdi::blit_px};

pub struct DrawExecutor {
    terminal_resolution: TerminalResolution,

    cur_position: Position,
    polymarker_color: u8,
    pub line_color: u8,
    pub fill_color: u8,
    pub text_color: u8,

    pub text_effects: TextEffects,
    pub text_size: i32,
    pub text_rotation: TextRotation,

    pub polymarker_type: PolymarkerKind,
    pub line_kind: LineKind,
    drawing_mode: DrawingMode,
    polymarker_size: usize,
    solidline_size: usize,
    user_mask: u16,

    fill_pattern_type: PatternType,
    fill_pattern: &'static [u16],
    draw_border: bool,

    pub hollow_set: bool,

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

impl DrawExecutor {
    pub fn new(terminal_resolution: TerminalResolution) -> Self {
        let default_color = terminal_resolution.default_fg_color();
        Self {
            terminal_resolution,
            polymarker_color: default_color,
            line_color: default_color,
            fill_color: default_color,
            text_color: default_color,
            cur_position: Position::new(0, 0),
            text_effects: TextEffects::NORMAL,
            text_size: 9,
            text_rotation: TextRotation::Degrees0,
            polymarker_type: PolymarkerKind::Point,
            line_kind: LineKind::Solid,
            drawing_mode: DrawingMode::Replace,
            polymarker_size: 1,
            solidline_size: 1,
            user_mask: 0b1010_1010_1010_1010,

            fill_pattern_type: PatternType::Solid,
            fill_pattern: &SOLID_PATTERN,
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
    }

    pub fn get_terminal_resolution(&self) -> TerminalResolution {
        self.terminal_resolution
    }

    /// Apply rotation transformation to character pixel coordinates
    /// For 90° and 270° rotations, also flip in Y direction (in char coordinates)
    #[inline]
    fn apply_rotation(&self, x: i32, y: i32, font_size: Size, skew_offset: i32, y_offset: i32) -> (i32, i32) {
        match self.text_rotation {
            TextRotation::Degrees0 => (x + skew_offset, y - y_offset),
            TextRotation::Degrees90 => (y - y_offset, -1 + font_size.width - (x + skew_offset)),
            TextRotation::Degrees180 => (font_size.width - (x + skew_offset) - 1, -y + y_offset),
            TextRotation::Degrees270 => (-y + y_offset, x + skew_offset),
        }
    }
    #[inline]
    fn apply_underline_rotation(&self, x: i32, y: i32, font_size: Size, skew_offset: i32, y_offset: i32) -> (i32, i32) {
        match self.text_rotation {
            TextRotation::Degrees0 => (x + skew_offset, y - y_offset),
            TextRotation::Degrees90 => (y - y_offset, font_size.width - (x + skew_offset)),
            TextRotation::Degrees180 => (font_size.width - (x + skew_offset) - 1, -y + y_offset),
            TextRotation::Degrees270 => (-y + y_offset - 1, (x + skew_offset) - 1),
        }
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
        let tmp = self.fill_color;
        self.fill_color = col;
        let mut visited = HashSet::new();
        while let Some(pos) = vec.pop() {
            if pos.x < 0 || pos.y < 0 || pos.x >= res.width || pos.y >= res.height {
                continue;
            }

            let cp = self.get_pixel(buf, pos.x, pos.y);
            if cp != old_px || visited.contains(&pos) {
                continue;
            }
            self.fill_pixel(buf, pos.x, pos.y);
            visited.insert(pos);

            vec.push(Position::new(pos.x - 1, pos.y));
            vec.push(Position::new(pos.x + 1, pos.y));
            vec.push(Position::new(pos.x, pos.y - 1));
            vec.push(Position::new(pos.x, pos.y + 1));
        }
        self.fill_color = tmp;
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
        let res = buf.get_resolution();
        let px = x;
        // In IGS medium/high Auflösungen sind die Pattern immer 16‑Pixel breit
        // und werden unabhängig von der aktuellen X‑Position wiederholt.
        // Damit das Brick‑Muster in CARD.ig sichtbar wird, verwenden wir
        // x modulo 16 statt der absoluten X‑Koordinate.
        if px < 0 || px >= res.width {
            return;
        }

        let w = self.fill_pattern[(y as usize) % self.fill_pattern.len()];
        let mask = w & (0x8000 >> (px as usize % 16)) != 0;
        match self.drawing_mode {
            DrawingMode::Replace => {
                // In Replace mode, pattern 0-bits are filled with palette index 0 (background color)
                // Pattern 1-bits are filled with fill_color
                // This creates a proper pattern effect where the gaps show the background
                if mask {
                    self.set_pixel(buf, x, y, self.fill_color);
                } else {
                    // Use palette index 0 as background for pattern gaps
                    self.set_pixel(buf, x, y, 0);
                }
            }
            DrawingMode::Transparent => {
                // In Transparent mode, only pattern 1-bits are drawn
                // Pattern 0-bits leave the existing pixels unchanged (transparent)
                if mask {
                    self.set_pixel(buf, x, y, self.fill_color);
                }
            }
            DrawingMode::Xor => {
                let s = if mask { 0xFF } else { 0x00 };
                let d = self.get_pixel(buf, x, y);
                let new_color = (s ^ d) & 0x0F;
                self.set_pixel(buf, x, y, new_color);
            }
            DrawingMode::ReverseTransparent => {
                if !mask {
                    self.set_pixel(buf, x, y, self.fill_color);
                }
            }
        }
    }

    fn draw_vline(&mut self, buf: &mut dyn EditableScreen, x: i32, mut y0: i32, mut y1: i32, color: u8, mask: u16) {
        if y1 < y0 {
            swap(&mut y0, &mut y1);
        }
        let mut line_mask = mask;
        for y in y0..=y1 {
            line_mask = line_mask.rotate_left(1);
            if 1 & line_mask != 0 {
                self.set_pixel(buf, x, y, color);
            }
        }
    }

    fn draw_hline(&mut self, buf: &mut dyn EditableScreen, y: i32, x0: i32, x1: i32, color: u8, mask: u16) {
        let mut line_mask = mask;
        line_mask = line_mask.rotate_left((x0 & 0x0f) as u32);
        for x in x0..=x1 {
            line_mask = line_mask.rotate_left(1);
            if 1 & line_mask != 0 {
                self.set_pixel(buf, x, y, color);
            }
        }
    }

    pub fn draw_line(&mut self, buf: &mut dyn EditableScreen, mut x0: i32, mut y0: i32, mut x1: i32, mut y1: i32, color: u8, mask: u16) {
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
        let mut line_mask = mask;

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
        let y_rad = self.calc_circle_y_rad(r);
        let points: Vec<i32> = gdp_curve(xm, ym, r, y_rad, 0, TWOPI as i32);
        self.fill_poly(buf, &points);
    }

    pub fn draw_circle(&mut self, buf: &mut dyn EditableScreen, xm: i32, ym: i32, r: i32, color: u8) {
        let y_rad = self.calc_circle_y_rad(r);
        let points: Vec<i32> = gdp_curve(xm, ym, r, y_rad, 0, TWOPI as i32);
        self.draw_poly(buf, &points, color, false);
    }

    pub fn draw_ellipse(&mut self, buf: &mut dyn EditableScreen, xm: i32, ym: i32, a: i32, b: i32, color: u8) {
        let b = self.calc_circle_y_rad(b);
        let points: Vec<i32> = gdp_curve(xm, ym, a, b, 0, TWOPI as i32);
        self.draw_poly(buf, &points, color, false);
    }

    pub fn draw_elliptical_pieslice(&mut self, buf: &mut dyn EditableScreen, xm: i32, ym: i32, xr: i32, yr: i32, beg_ang: i32, end_ang: i32) {
        let yr = self.calc_circle_y_rad(yr);
        let mut points = gdp_curve(xm, ym, xr, yr, beg_ang * 10, end_ang * 10);
        points.extend_from_slice(&[xm, ym]);
        self.draw_poly(buf, &points, self.fill_color, true);
    }

    pub fn fill_elliptical_pieslice(&mut self, buf: &mut dyn EditableScreen, xm: i32, ym: i32, xr: i32, yr: i32, beg_ang: i32, end_ang: i32) {
        let yr = self.calc_circle_y_rad(yr);
        let mut points = gdp_curve(xm, ym, xr, yr, beg_ang * 10, end_ang * 10);
        points.extend_from_slice(&[xm, ym]);
        self.fill_poly(buf, &points);
    }

    pub fn draw_pieslice(&mut self, buf: &mut dyn EditableScreen, xm: i32, ym: i32, radius: i32, beg_ang: i32, end_ang: i32) {
        let yr = self.calc_circle_y_rad(radius);
        let mut points = gdp_curve(xm, ym, radius, yr, beg_ang * 10, end_ang * 10);
        points.extend_from_slice(&[xm, ym]);
        self.draw_poly(buf, &points, self.fill_color, true);
    }

    pub fn fill_pieslice(&mut self, buf: &mut dyn EditableScreen, xm: i32, ym: i32, radius: i32, beg_ang: i32, end_ang: i32) {
        let yr = self.calc_circle_y_rad(radius);
        let mut points = gdp_curve(xm, ym, radius, yr, beg_ang * 10, end_ang * 10);
        points.extend_from_slice(&[xm, ym]);
        self.fill_poly(buf, &points);
    }

    pub fn fill_ellipse(&mut self, buf: &mut dyn EditableScreen, xm: i32, ym: i32, a: i32, b: i32) {
        let b = self.calc_circle_y_rad(b);
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

        // If hollow pattern is set, don't fill the rectangle
        if self.hollow_set {
            return;
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
        let mask = self.line_kind.get_mask(self.user_mask);
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
        let mask = self.line_kind.get_mask(self.user_mask);
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

                for k in x1..=x2 {
                    self.fill_pixel(buf, k, y);
                }
            }
        }
        if matches!(self.fill_pattern_type, PatternType::Solid) {
            self.draw_poly(buf, &points, self.fill_color, true);
        }
    }

    pub fn write_text(&mut self, buf: &mut dyn EditableScreen, text_pos: Position, string_parameter: &[u8]) {
        let (metrics, font) = load_atari_font(self.text_size);
        let is_outlined = self.text_effects.contains(TextEffects::OUTLINED);
        let outline_thickness = if is_outlined { 1 } else { 0 };

        // For outlined text, the position represents where the outline starts
        // (not the character itself, which is 1 pixel inward)
        let mut pos = text_pos;

        //println!("write_text {string_parameter} {text_pos} size:{} effect:{:?} rot:{:?}", self.text_size, self.text_effects, self.text_rotation);

        let color = self.text_color;
        let bg_color = 0; // Background color for outlined text
        let font_size = font.size();

        // Adjust starting position for rotated text
        // For 90° rotation, text grows upward, so start at the bottom
        // For 270° rotation, text grows downward from right
        match self.text_rotation {
            TextRotation::Degrees90 => {
                // Text grows upward, so start position needs to be at the bottom of where the char will be
                pos.y -= font_size.height - 1;
            }
            TextRotation::Degrees180 => {
                // Text grows to the left
                pos.x -= font_size.width - 1;
            }
            _ => {}
        }

        let mut draw_mask: u16 = if self.text_effects.contains(TextEffects::GHOSTED) { 0x5555 } else { 0xFFFF };

        for ch in string_parameter {
            let glyph = font.get_glyph(*ch as char).unwrap();

            // For outlined text, we need to:
            // 1. Draw the outline (border) in text color
            // 2. Fill the character itself with background color
            if is_outlined {
                // First pass: draw the outline border (1 pixel around character)
                for y in 0..font_size.height {
                    for x in 0..font_size.width {
                        let iy = y;
                        let ix = x;
                        let pixel_set = if iy < glyph.bitmap.pixels.len() as i32 && ix < glyph.bitmap.pixels[iy as usize].len() as i32 {
                            glyph.bitmap.pixels[iy as usize][ix as usize]
                        } else {
                            false
                        };
                        draw_mask = draw_mask.rotate_left(1);
                        if pixel_set && (1 & draw_mask) != 0 {
                            // Apply rotation transformation for outline first pass
                            let (rx, ry) = self.apply_rotation(x, y, font_size, 0, metrics.y_off);
                            // Draw outline pixels around this character pixel
                            for dy in -1..=1 {
                                for dx in -1..=1 {
                                    let p = pos + Position::new(rx + dx, ry + dy);
                                    self.set_pixel(buf, p.x, p.y, color);
                                }
                            }
                        }
                    }
                }
                let mut draw_mask: u16 = if self.text_effects.contains(TextEffects::GHOSTED) { 0x5555 } else { 0xFFFF };

                // Second pass: fill character interior with background color
                for y in 0..font_size.height {
                    for x in 0..font_size.width {
                        let iy = y;
                        let ix = x;
                        let pixel_set = if iy < glyph.bitmap.pixels.len() as i32 && ix < glyph.bitmap.pixels[iy as usize].len() as i32 {
                            glyph.bitmap.pixels[iy as usize][ix as usize]
                        } else {
                            false
                        };
                        draw_mask = draw_mask.rotate_left(1);

                        if pixel_set && (1 & draw_mask) != 0 {
                            // Apply rotation transformation for outline second pass
                            let (rx, ry) = self.apply_rotation(x, y, font_size, 0, metrics.y_off);
                            let p = pos + Position::new(rx, ry);
                            self.set_pixel(buf, p.x, p.y, bg_color);
                        }
                    }
                }
            } else {
                // Normal text rendering (non-outlined)
                for y in 0..font_size.height {
                    draw_mask = draw_mask.rotate_left(1);

                    for x in 0..font_size.width {
                        let iy = y;
                        let ix = x;
                        // Check pixel in bitmap.pixels[row][col]
                        let pixel_set = if iy < glyph.bitmap.pixels.len() as i32 && ix < glyph.bitmap.pixels[iy as usize].len() as i32 {
                            glyph.bitmap.pixels[iy as usize][ix as usize]
                        } else {
                            false
                        };
                        if pixel_set {
                            if 1 & draw_mask != 0 {
                                // For skewed text, add horizontal offset based on vertical position
                                // Atari VDI style: skew decreases from top to bottom (right-leaning italic)
                                // Top of character has maximum offset, bottom has zero offset
                                let skew_offset = if self.text_effects.contains(TextEffects::SKEWED) {
                                    (font_size.height - 1 - y) / 2 - (y % 2)
                                } else {
                                    0
                                };

                                // Apply rotation transformation for normal text
                                let (rx, ry) = self.apply_rotation(x, y, font_size, skew_offset, metrics.y_off);
                                let p = pos + Position::new(rx, ry);
                                self.set_pixel(buf, p.x, p.y, color);

                                // THICKENED: Draw additional pixels to the right
                                // Only for Degrees0 rotation (horizontal text)
                                if self.text_effects.contains(TextEffects::THICKENED) {
                                    match self.text_rotation {
                                        TextRotation::Degrees0 => {
                                            for t in 1..=metrics.thicken {
                                                self.set_pixel(buf, p.x + t, p.y, color);
                                            }
                                        }
                                        TextRotation::Degrees90 => {
                                            for t in 1..=metrics.thicken {
                                                self.set_pixel(buf, p.x, p.y - t, color);
                                            }
                                        }
                                        TextRotation::Degrees180 => {
                                            for t in 1..=metrics.thicken {
                                                self.set_pixel(buf, p.x - t, p.y, color);
                                            }
                                        }
                                        TextRotation::Degrees270 => {
                                            for t in 1..=metrics.thicken {
                                                self.set_pixel(buf, p.x, p.y + t, color);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        draw_mask = draw_mask.rotate_left(1);
                    }
                }
            }
            if self.text_effects.contains(TextEffects::UNDERLINED) {
                // Atari VDI: underline with rotation support
                // Continue using the same draw_mask pattern from text rendering
                let mut underline_mask: u16 = if self.text_effects.contains(TextEffects::GHOSTED) { 0x5555 } else { 0xFFFF };
                let underline_width = if is_outlined {
                    metrics.underline_width + 2 * outline_thickness
                } else {
                    metrics.underline_width
                };
                for y2 in 0..metrics.underline_height {
                    for x in 0..underline_width {
                        underline_mask = underline_mask.rotate_left(1);

                        if 1 & underline_mask != 0 {
                            // Calculate skew offset for underline position (same logic as text)
                            let underline_y = metrics.underline_pos + y2;
                            let skew_offset = if self.text_effects.contains(TextEffects::SKEWED) {
                                (font_size.height - 1 - underline_y) / 2 - (underline_y % 2)
                            } else {
                                0
                            };

                            // Apply rotation to underline coordinates
                            let (rx, ry) = self.apply_underline_rotation(x, underline_y, font_size, skew_offset, metrics.y_off);
                            let p = pos + Position::new(rx, ry);
                            self.set_pixel(buf, p.x, p.y, color);
                        }
                    }
                    // Note: No extra rotate_left at end of underline row, unlike character rows
                }
            }
            // Calculate character advance based on rotation
            // For outlined text, add 2 pixels (1 left + 1 right for the border)
            // For thickened text, do NOT extend character width - thickening overlaps into next cell
            let base_width = if is_outlined {
                font_size.width + 2 * outline_thickness
            } else {
                font_size.width
            };

            // Note: Character width stays the same even for THICKENED text
            // The thickening pixels extend into the spacing between characters
            let char_width = base_width;

            // Advance position based on rotation
            // When rotated 90° or 270°, width becomes vertical advance
            match self.text_rotation {
                TextRotation::Degrees0 => pos.x += char_width,
                TextRotation::Degrees90 => pos.y -= char_width,  // Width becomes vertical advance
                TextRotation::Degrees270 => pos.y += char_width, // Width becomes vertical advance
                TextRotation::Degrees180 => pos.x -= char_width,
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
        let max_colors = self.terminal_resolution.get_max_colors();
        for y in 0..height {
            for x in 0..width {
                let color = self.get_pixel(buf, start_x + x, start_y + y) % max_colors as u8;
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

        let x_radius = ((buf.get_resolution().width >> 6).min((x2 - x1) / 2) - 1).max(0);

        // This is a hack I've fixed it visually
        let y_radius = match self.terminal_resolution {
            TerminalResolution::Low => self.calc_circle_y_rad(x_radius).min((y1 - y2) / 2),
            TerminalResolution::Medium => x_radius,
            TerminalResolution::High => self.calc_circle_y_rad(x_radius).min((y1 - y2) / 2),
        };

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
            self.draw_poly(buf, &points, self.fill_color, false);
        }
    }

    pub fn draw_poly_maker(&mut self, buf: &mut dyn EditableScreen, x0: i32, y0: i32) {
        let points = match self.polymarker_type {
            PolymarkerKind::Point => vec![1i32, 2, 0, 0, 0, 0],
            PolymarkerKind::Plus => vec![2, 2, 0, -3, 0, 3, 2, -4, 0, 4, 0],
            PolymarkerKind::Star => vec![3, 2, 0, -3, 0, 3, 2, 3, 2, -3, -2, 2, 3, -2, -3, 2],
            PolymarkerKind::Square => vec![1, 5, -4, -3, 4, -3, 4, 3, -4, 3, -4, -3],
            PolymarkerKind::DiagonalCross => vec![2, 2, -4, -3, 4, 3, 2, -4, 3, 4, -3],
            PolymarkerKind::Diamond => vec![1, 5, -4, 0, 0, -3, 4, 0, 0, 3, -4, 0],
        };
        let num_lines = points[0];
        let mut i = 1;
        let old_type = self.line_kind;
        let scale = 1;
        self.line_kind = LineKind::Solid;
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
        self.line_kind = old_type;
    }

    fn calc_circle_y_rad(&self, rad: i32) -> i32 {
        let (xsize, ysize) = match self.terminal_resolution {
            TerminalResolution::Low => (338, 372),
            TerminalResolution::Medium => (440, 1000),
            TerminalResolution::High => (372, 372),
        };

        (rad * xsize) / ysize
    }
}

pub const REGISTER_TO_PEN: &[usize; 17] = &[0, 2, 3, 6, 4, 7, 5, 8, 9, 10, 11, 14, 12, 12, 15, 13, 1];

// Public wrapper methods for IGS command handling
impl DrawExecutor {
    pub fn draw_rect(&mut self, buf: &mut dyn crate::EditableScreen, x1: i32, y1: i32, x2: i32, y2: i32) {
        self.fill_rect(buf, x1, y1, x2, y2);
        if self.draw_border {
            // Use fill_color for borders on filled rectangles (GEM VDI behavior)
            let color = self.fill_color;
            let mask = self.line_kind.get_mask(self.user_mask);
            self.draw_line(buf, x1, y1, x1, y2, color, mask);
            self.draw_line(buf, x2, y1, x2, y2, color, mask);
            self.draw_line(buf, x1, y1, x2, y1, color, mask);
            self.draw_line(buf, x1, y2, x2, y2, color, mask);
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
        let mask = self.line_kind.get_mask(self.user_mask);
        self.draw_line(buf, x1, y1, x2, y2, color, mask);
    }

    pub fn draw_circle_pub(&mut self, buf: &mut dyn crate::EditableScreen, x: i32, y: i32, radius: i32) {
        self.fill_circle(buf, x, y, radius);
        if self.draw_border {
            self.draw_circle(buf, x, y, radius, self.fill_color);
        }
    }

    pub fn draw_ellipse_pub(&mut self, buf: &mut dyn crate::EditableScreen, x: i32, y: i32, x_radius: i32, y_radius: i32) {
        self.fill_ellipse(buf, x, y, x_radius, y_radius);
        if self.draw_border {
            self.draw_ellipse(buf, x, y, x_radius, y_radius, self.fill_color);
        }
    }

    pub fn draw_arc_pub(&mut self, buf: &mut dyn crate::EditableScreen, x: i32, y: i32, start_angle: i32, end_angle: i32, radius: i32) {
        self.draw_arc(buf, x, y, radius, radius, start_angle, end_angle);
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

    pub fn set_fill_pattern(&mut self, pattern_type: PatternType) {
        self.fill_pattern_type = pattern_type;
        self.fill_pattern = match pattern_type {
            PatternType::Hollow => &HOLLOW_PATTERN,
            PatternType::Solid => &SOLID_PATTERN,
            PatternType::Pattern(idx) => {
                if idx == 0 {
                    &RANDOM_PATTERN
                } else if idx >= 1 && idx <= 24 {
                    &TYPE_PATTERN[idx as usize - 1]
                } else {
                    &SOLID_PATTERN
                }
            }
            PatternType::Hatch(idx) => {
                if idx >= 1 && idx <= 6 {
                    &HATCH_PATTERN[idx as usize - 1]
                } else if idx >= 7 && idx <= 12 {
                    &HATCH_WIDE_PATTERN[idx as usize - 7]
                } else {
                    &SOLID_PATTERN
                }
            }
            PatternType::UserDefined(_) => &RANDOM_PATTERN,
        };
        self.hollow_set = matches!(pattern_type, PatternType::Hollow);
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

    pub fn set_line_thickness(&mut self, thickness: usize) {
        self.solidline_size = thickness;
    }

    pub fn get_cur_position(&self) -> crate::Position {
        self.cur_position
    }

    pub fn set_cur_position(&mut self, x: i32, y: i32) {
        self.cur_position = crate::Position::new(x, y);
    }

    pub fn set_polymarker_size(&mut self, size: usize) {
        self.polymarker_size = size;
    }
}
