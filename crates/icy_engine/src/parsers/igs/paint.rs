use std::str::FromStr;

use super::{cmd::IgsCommands, CommandExecutor, IGS_VERSION, LINE_STYLE, RANDOM_PATTERN, SOLID_PATTERN};
use crate::{
    igs::{HATCH_PATTERN, HATCH_WIDE_PATTERN, HOLLOW_PATTERN, TYPE_PATTERN},
    load_atari_fonts, BitFont, Buffer, CallbackAction, Caret, Color, EngineResult, Position, Size, ATARI, IGS_PALETTE, IGS_SYSTEM_PALETTE,
};

#[derive(Default)]
pub enum TerminalResolution {
    /// 320x200
    #[default]
    Low,
    /// 640x200
    Medium,
    /// 640x400  
    High,
}

impl TerminalResolution {
    pub fn resolution_id(&self) -> String {
        match self {
            TerminalResolution::Low => "0".to_string(),
            TerminalResolution::Medium => "1".to_string(),
            TerminalResolution::High => "2".to_string(),
        }
    }

    pub fn get_resolution(&self) -> Size {
        match self {
            TerminalResolution::Low => Size { width: 320, height: 200 },
            TerminalResolution::Medium => Size { width: 640, height: 200 },
            TerminalResolution::High => Size { width: 640, height: 400 },
        }
    }
}
pub enum TextEffects {
    Normal,
    Thickened,
    Ghosted,
    Skewed,
    Underlined,
    Outlined,
}

pub enum TextRotation {
    Right,
    Up,
    Down,
    Left,
    RightReverse,
}

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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DrawingMode {
    Replace,
    Transparent,
    Xor,
    ReverseTransparent,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FillPatternType {
    Hollow,
    Solid,
    Pattern,
    Hatch,
    UserdDefined,
}

pub struct DrawExecutor {
    screen: Vec<u8>,
    terminal_resolution: TerminalResolution,

    cur_position: Position,
    pen_colors: Vec<Color>,
    polymarker_color: u8,
    line_color: u8,
    fill_color: u8,
    text_color: u8,

    text_effects: TextEffects,
    text_size: i32,
    text_rotation: TextRotation,

    polymaker_type: PolymarkerType,
    line_type: LineType,
    drawing_mode: DrawingMode,
    polymarker_size: usize,
    solidline_size: usize,
    user_defined_pattern_number: usize,

    fill_pattern_type: FillPatternType,
    fill_pattern: &'static [u16],
    pattern_index_number: usize,
    draw_border: bool,

    font_7px: BitFont,
    font_9px: BitFont,
    font_16px: BitFont,
    hollow_set: bool,

    screen_memory: Vec<u8>,
    screen_memory_size: Size,

    /// for the G command.
    double_step: f32,

    fonts: Vec<(String, usize, &'static str)>,
}

unsafe impl Send for DrawExecutor {}

unsafe impl Sync for DrawExecutor {}

impl Default for DrawExecutor {
    fn default() -> Self {
        let fonts = load_atari_fonts();
        let font_7px = BitFont::from_str(fonts[0].2).unwrap();
        let font_9px = BitFont::from_str(fonts[1].2).unwrap();
        let font_16px = BitFont::from_str(fonts[2].2).unwrap();

        Self {
            screen: vec![1; 320 * 200],
            terminal_resolution: TerminalResolution::Low,
            pen_colors: IGS_SYSTEM_PALETTE.to_vec(),
            polymarker_color: 0,
            line_color: 0,
            fill_color: 0,
            text_color: 0,
            cur_position: Position::new(0, 0),
            text_effects: TextEffects::Normal,
            text_size: 9,
            text_rotation: TextRotation::Right,
            polymaker_type: PolymarkerType::Point,
            line_type: LineType::Solid,
            drawing_mode: DrawingMode::Replace,
            polymarker_size: 1,
            solidline_size: 1,
            user_defined_pattern_number: 1,
            font_7px,
            font_9px,
            font_16px,
            screen_memory: Vec::new(),
            screen_memory_size: Size::new(0, 0),

            fill_pattern_type: FillPatternType::Solid,
            fill_pattern: &SOLID_PATTERN,
            pattern_index_number: 0,
            draw_border: false,
            hollow_set: false,
            double_step: -1.0,
            fonts,
        }
    }
}

impl DrawExecutor {
    pub fn clear(&mut self, buf: &mut Buffer, caret: &mut Caret) {
        buf.clear_screen(0, caret);
        let res = self.get_resolution();
        self.screen = vec![0; (res.width * res.height) as usize];
    }

    pub fn set_resolution(&mut self, buf: &mut Buffer, caret: &mut Caret) {
        buf.clear_screen(0, caret);
        let res = self.get_resolution();
        self.screen = vec![1; (res.width * res.height) as usize];
    }

    pub fn reset_attributes(&mut self) {
        // TODO
    }

    fn flood_fill(&mut self, x0: i32, y0: i32) {
        let res = self.get_resolution();

        if x0 < 0 || y0 < 0 || x0 >= res.width || y0 >= res.height {
            return;
        }
        let old_px = self.get_pixel(x0, y0);

        let mut vec = vec![Position::new(x0, y0)];
        let col = self.fill_color;
        if old_px == col {
            return;
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
            vec.push(Position::new(pos.x, pos.y - 1));
            vec.push(Position::new(pos.x, pos.y + 1));
        }
    }

    /*
    fn flood_fill(&mut self, x0: i32, y0: i32) {
        let res = self.get_resolution();

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

    fn set_pixel(&mut self, x: i32, y: i32, line_color: u8) {
        let offset = (y * self.get_resolution().width + x) as usize;
        if offset >= self.screen.len() {
            return;
        }
        self.screen[offset] = line_color;
    }

    fn get_pixel(&mut self, x: i32, y: i32) -> u8 {
        let offset = (y * self.get_resolution().width + x) as usize;
        self.screen[offset]
    }

    fn fill_pixel(&mut self, x: i32, y: i32) {
        let w = self.fill_pattern[(y as usize) % self.fill_pattern.len()];
        if w & (0x8000 >> (x as usize % 16)) != 0 {
            self.set_pixel(x, y, self.fill_color);
        }
    }

    fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: u8, mask: usize) {
        let mut line_mask = LINE_STYLE[mask];

        let dx = (x0 - x1).abs();
        let dy = (y0 - y1).abs();

        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;

        let mut x = x0;
        let mut y = y0;
        loop {
            if 1 & line_mask != 0 {
                self.set_pixel(x, y, color);
            }
            line_mask = line_mask.rotate_left(1);

            if x == x1 && y == y1 {
                break;
            }

            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
        }
    }

    fn draw_circle(&mut self, xm: i32, ym: i32, r: i32) {
        let mut x = -r;
        let mut y = 0;
        let mut err = 2 - 2 * r;
        let color = self.line_color;

        while x < 0 {
            self.set_pixel(xm - x, ym + y, color); /*   I. Quadrant */
            self.set_pixel(xm - y, ym - x, color); /*  II. Quadrant */
            self.set_pixel(xm + x, ym - y, color); /* III. Quadrant */
            self.set_pixel(xm + y, ym + x, color); /*  IV. Quadrant */
            let r = err;
            if r <= y {
                y += 1;
                err += y * 2 + 1; /* e_xy+e_y < 0 */
            }
            if r > x || err > y {
                x += 1;
                err += x * 2 + 1; /* e_xy+e_x > 0 or no 2nd y-step */
            }
        }
    }

    fn draw_ellipse(&mut self, xm: i32, ym: i32, a: i32, b: i32) {
        let mut x = -a;
        let mut y = 0; /* II. quadrant from bottom left to top right */
        let e2 = b * b;
        let mut err = x * (2 * e2 + x) + e2; /* error of 1.step */
        let color = self.line_color;

        while x <= 0 {
            self.set_pixel(xm - x, ym + y, color); /*   I. Quadrant */
            self.set_pixel(xm + x, ym + y, color); /*  II. Quadrant */
            self.set_pixel(xm + x, ym - y, color); /* III. Quadrant */
            self.set_pixel(xm - x, ym - y, color); /*  IV. Quadrant */
            let e2 = 2 * err;
            if e2 >= (x * 2 + 1) * b * b {
                /* e_xy+e_x > 0 */
                x += 1;
                err += (x * 2 + 1) * b * b;
            }
            if e2 <= (y * 2 + 1) * a * a {
                /* e_xy+e_y < 0 */
                y += 1;
                err += (y * 2 + 1) * a * a;
            }
        }

        while y < b {
            /* too early stop of flat ellipses a=1, */
            y += 1;
            self.set_pixel(xm, ym + y, color); /* -> finish tip of ellipse */
            self.set_pixel(xm, ym - y, color);
        }
    }

    fn fill_ellipse(&mut self, xm: i32, ym: i32, a: i32, b: i32) {
        let mut x: i32 = -a;
        let mut y = 0; /* II. quadrant from bottom left to top right */
        let e2 = b * b;
        let mut err = x * (2 * e2 + x) + e2; /* error of 1.step */
        let color = self.line_color;

        while x <= 0 {
            self.fill_rect(xm - x, ym + y, xm + x, ym + y); /*  II. Quadrant */
            self.fill_rect(xm + x, ym - y, xm - x, ym - y); /*  IV. Quadrant */
            let e2 = 2 * err;
            if e2 >= (x * 2 + 1) * b * b {
                /* e_xy+e_x > 0 */
                x += 1;
                err += (x * 2 + 1) * b * b;
            }
            if e2 <= (y * 2 + 1) * a * a {
                /* e_xy+e_y < 0 */
                y += 1;
                err += (y * 2 + 1) * a * a;
            }
        }

        while y < b {
            /* too early stop of flat ellipses a=1, */
            y += 1;
            self.set_pixel(xm, ym + y, color); /* -> finish tip of ellipse */
            self.set_pixel(xm, ym - y, color);
        }
    }

    fn fill_rect(&mut self, mut x0: i32, mut y0: i32, mut x1: i32, mut y1: i32) {
        if y0 > y1 {
            std::mem::swap(&mut y0, &mut y1);
        }
        if x0 > x1 {
            std::mem::swap(&mut x0, &mut x1);
        }

        for y in y0..=y1 {
            for x in x0..=x1 {
                self.fill_pixel(x, y);
            }
        }
    }

    fn draw_poly(&mut self, parameters: &[i32]) {
        let mut x = parameters[0];
        let mut y = parameters[1];
        let mask = self.line_type.get_mask();
        let mut i = 2;
        while i < parameters.len() {
            let nx = parameters[i];
            let ny = parameters[i + 1];
            self.draw_line(x, y, nx, ny, self.fill_color, mask);
            x = nx;
            y = ny;
            i += 2;
        }
        // close polygon
        self.draw_line(x, y, parameters[0], parameters[1], self.fill_color, mask);
    }

    fn draw_polyline(&mut self, parameters: &[i32]) {
        let mut x = parameters[0];
        let mut y = parameters[1];
        let mask = self.line_type.get_mask();
        let mut i = 2;
        while i < parameters.len() {
            let nx = parameters[i];
            let ny = parameters[i + 1];
            self.draw_line(x, y, nx, ny, self.fill_color, mask);
            x = nx;
            y = ny;
            i += 2;
        }
    }

    fn fill_poly(&mut self, points: &[i32]) {
        let max_vertices = 512;

        let mut i = 3;
        let mut y_max = points[1];
        let mut y_min = points[1];
        while i < points.len() {
            let y = points[i];
            if y > y_max {
                y_max = y;
            }
            if y < y_min {
                y_min = y;
            }
            i += 2;
        }

        // VDI apparently loops over the scan lines from bottom to top
        for y in (y_min..=y_max).rev() {
            // Set up counter for vector intersections
            let mut intersections = 0;

            // Set up a buffer for storing polygon edges that intersect the scan line
            let mut edge_buffer = Vec::new();

            // Loop over all vertices/points and find the intersections
            let point_cnt = points.len() / 2;
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
                let dy1 = y - y1;
                let dy2 = y - y2;

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
                    if intersections >= max_vertices {
                        break;
                    }

                    intersections += 1;

                    // Add X value for this vector to edge buffer
                    if dx < 0 {
                        edge_buffer.push(((dy2 * dx / dy + 1) >> 1) + x2);
                    } else {
                        edge_buffer.push(((dy1 * dx / dy + 1) >> 1) + x1);
                    }
                }
            }

            // All of the points of intersection have now been found.  If there
            // were none (or one, which I think is impossible), then there is
            // nothing more to do.  Otherwise, sort the list of points of
            // intersection in ascending order.
            // (The list contains only the x-coordinates of the points.)

            if intersections < 2 {
                continue;
            }

            // Sort the X-coordinates, so they are arranged left to right.
            // There are almost always exactly 2, except for weird shapes.
            edge_buffer.sort_by(|a, b| a.partial_cmp(b).unwrap());

            // Loop through all edges in pairs, filling the pixels in between.
            let mut i = intersections / 2;
            let mut j = 0;
            while i > 0 {
                i -= 1;
                /* grab a pair of endpoints */
                let x1 = edge_buffer[j];
                let x2 = edge_buffer[j + 1];
                // Fill in all pixels horizontally from (x1, y) to (x2, y)
                for k in x1..=x2 {
                    self.fill_pixel(k, y);
                }
                j += 2;
            }
        }
    }

    fn write_text(&mut self, text_pos: Position, string_parameter: &str) {
        let mut pos = text_pos;
        println!("text size: {} ", self.text_size);

        let font = if self.text_size < 9 {
            self.font_7px.clone()
        } else if self.text_size > 11 {
            self.font_16px.clone()
        } else {
            self.font_9px.clone()
        };

        let color = self.text_color;

        let font_size = font.size; /*match self.text_size {
                                                                       8 => Size::new(8, 8),
                                   9 => Size::new(8, 8),
                                   // 10 => Size::new(9, 14),
                                   // 16 => Size::new(8, 14),
                                   // 18 => Size::new(8, 16),
                                   // 20 => Size::new(8, 18),
                                   _ => Size::new(8, 8),
                                                                   };*/
        let HIGH_BIT = 1 << (font.size.width - 1);
        for ch in string_parameter.chars() {
            let data = font.get_glyph(ch).unwrap().data.clone();
            for y in 0..font_size.height {
                for x in 0..font_size.width {
                    let iy = y; //(y as f32 / font_size.height as f32 * char_size.height as f32) as i32;
                    let ix = x; // (x as f32 / font_size.width as f32 * char_size.width as f32) as i32;
                    if data[iy as usize] & (HIGH_BIT >> ix) != 0 {
                        let p = pos + Position::new(x, y - font_size.height / 2);
                        self.set_pixel(p.x, p.y, color);
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

    fn blit_screen_to_screen(&mut self, _write_mode: i32, from: Position, to: Position, dest: Position) {
        let width = (to.x - from.x) as usize;
        let height = (to.y - from.y) as usize;
        let line_length = self.get_resolution().width as usize;
        let start = from.x as usize + from.y as usize * line_length;
        let mut offset = start;

        let mut blit = Vec::with_capacity(width as usize * height as usize);

        for _y in 0..height {
            blit.extend_from_slice(&self.screen[offset..offset + width]);
            offset += line_length;
        }

        offset = dest.x as usize + dest.y as usize * line_length;
        let mut blit_offset = 0;
        for _y in 0..height as i32 {
            let mut o = offset;
            for _x in 0..width {
                let s = blit[blit_offset];
                let d = self.screen[o];
                let dest = match _write_mode {
                    0 => 0,
                    1 => s & d,
                    2 => s & !d,
                    3 => s,
                    4 => !s & d,
                    5 => d,
                    6 => s ^ d,
                    7 => s | d,
                    8 => !(s | d),
                    9 => !(s ^ d),
                    10 => !d,
                    11 => s | !d,
                    12 => !s,
                    13 => !s | d,
                    14 => !(s & d),
                    15 => 1,
                    _ => 2,
                };
                self.screen[o] = dest;
                o += 1;
                blit_offset += 1
            }

            offset += line_length;
            if offset >= self.screen.len() {
                break;
            }
        }
    }

    fn blit_memory_to_screen(&mut self, _write_mode: i32, from: Position, to: Position, dest: Position) {
        let width = to.x - from.x;
        let height = to.y - from.y;
        let res = self.get_resolution();

        for y in 0..height {
            let yp = y + from.y;
            if dest.y + y >= res.height {
                break;
            }
            for x in 0..width {
                let xp = x + from.x;

                if dest.x + x >= res.width {
                    break;
                }
                let offset = (yp * width + xp) as usize;
                let color = self.screen_memory[offset];
                self.set_pixel(dest.x + x, dest.y + y, color);
            }
        }
    }

    fn blit_screen_to_memory(&mut self, _write_mode: i32, from: Position, to: Position) {
        let width = to.x - from.x;
        let height = to.y - from.y;

        self.screen_memory_size = Size::new(width, height);
        self.screen_memory.clear();

        for y in from.y..to.y {
            for x in from.x..to.x {
                let color = self.get_pixel(x, y);
                self.screen_memory.push(color);
            }
        }
    }

    fn round_rect(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, parameters: i32) {
        let mut points = Vec::new();

        let x_radius = (self.get_resolution().width >> 6).min((x2 - x1) / 2);
        let y_radius = x_radius.min((y2 - y1) / 2);

        let x_off = [0, 12539 * x_radius / 32767, 23170 * x_radius / 32767, 30273 * x_radius / 32767, x_radius];

        let y_off = [y_radius, 30273 * y_radius / 32767, 23170 * y_radius / 32767, 12539 * y_radius / 32767, 0];
        let xc = x2 - x_radius;
        let yc = y2 - y_radius;

        // upper right
        for i in 0..x_off.len() {
            points.push(xc + x_off[i]);
            points.push(yc + y_off[i]);
        }

        // lower right
        let yc = y1 + y_radius;
        for i in 0..x_off.len() {
            points.push(xc + x_off[4 - i]);
            points.push(yc - y_off[4 - i]);
        }

        // lower left
        let xc = x1 + x_radius;
        for i in 0..x_off.len() {
            points.push(xc - x_off[i]);
            points.push(yc - y_off[i]);
        }

        // upper left
        let yc = y2 - y_radius;
        for i in 0..x_off.len() {
            points.push(xc - x_off[4 - i]);
            points.push(yc + y_off[4 - i]);
        }

        if parameters == 1 {
            self.fill_poly(&points);
        } else {
            self.draw_poly(&points);
        }
    }

    fn draw_poly_maker(&mut self, x0: i32, y0: i32) {
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
        let old_color = self.fill_color;
        let old_type = self.line_type;
        self.line_type = LineType::Solid;
        self.fill_color = self.line_color;
        for _ in 0..num_lines {
            let num_points = points[i] as usize;
            i += 1;
            let mut p = Vec::new();
            p.push(x0);
            p.push(y0);
            for x in 0..num_points {
                p.push(points[i + x * 2] + x0);
                p.push(points[i + x * 2 + 1] + y0);
            }
            self.draw_polyline(&p);
            i += num_points;
        }
        self.line_type = old_type;
        self.fill_color = old_color;
    }
}

impl CommandExecutor for DrawExecutor {
    fn get_resolution(&self) -> Size {
        let s = self.terminal_resolution.get_resolution();
        Size::new(s.width, s.height)
    }

    fn get_picture_data(&mut self) -> Option<(Size, Vec<u8>)> {
        let mut pixels = Vec::new();
        for i in &self.screen {
            let (r, g, b) = self.pen_colors[(*i as usize) & 0xF].get_rgb();

            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            if r == 0 && g == 0 && b == 0 {
                pixels.push(0);
            } else {
                pixels.push(255);
            }
        }
        Some((self.get_resolution(), pixels))
    }

    fn execute_command(
        &mut self,
        buf: &mut Buffer,
        caret: &mut Caret,
        command: IgsCommands,
        parameters: &[i32],
        string_parameter: &str,
    ) -> EngineResult<CallbackAction> {
        // println!("cmd:{:?}", command);
        match command {
            IgsCommands::Initialize => {
                if parameters.len() != 1 {
                    return Err(anyhow::anyhow!("Initialize command requires 1 argument"));
                }
                match parameters[0] {
                    0 => {
                        self.set_resolution(buf, caret);
                        self.pen_colors = IGS_SYSTEM_PALETTE.to_vec();
                        self.reset_attributes();
                    }
                    1 => {
                        self.set_resolution(buf, caret);
                        self.pen_colors = IGS_SYSTEM_PALETTE.to_vec();
                    }
                    2 => {
                        self.reset_attributes();
                    }
                    3 => {
                        self.set_resolution(buf, caret);
                        self.pen_colors = IGS_PALETTE.to_vec();
                    }
                    x => return Err(anyhow::anyhow!("Initialize unknown/unsupported argument: {x}")),
                }
                Ok(CallbackAction::Update)
            }
            IgsCommands::ScreenClear => {
                self.clear(buf, caret);
                Ok(CallbackAction::Update)
            }
            IgsCommands::AskIG => {
                if parameters.len() != 1 {
                    return Err(anyhow::anyhow!("Initialize command requires 1 argument"));
                }
                match parameters[0] {
                    0 => Ok(CallbackAction::SendString(IGS_VERSION.to_string())),
                    3 => Ok(CallbackAction::SendString(self.terminal_resolution.resolution_id() + ":")),
                    x => Err(anyhow::anyhow!("AskIG unknown/unsupported argument: {x}")),
                }
            }
            IgsCommands::Cursor => {
                if parameters.len() != 1 {
                    return Err(anyhow::anyhow!("Cursor command requires 1 argument"));
                }
                match parameters[0] {
                    0 => caret.set_is_visible(false),
                    1 => caret.set_is_visible(true),
                    2 | 3 => {
                        log::warn!("Backspace options not supported.");
                    }
                    x => return Err(anyhow::anyhow!("Cursor unknown/unsupported argument: {x}")),
                }
                Ok(CallbackAction::Update)
            }

            IgsCommands::ColorSet => {
                if parameters.len() != 2 {
                    return Err(anyhow::anyhow!("ColorSet command requires 2 arguments"));
                }
                /*println!("Color Set {}={}", match parameters[0] {
                                    0 => "polymaker",
                                    1 => "line",
                                    2 => "fill",
                                    3 => "text",
                                    _ => "?"
                ,                },  parameters[1]);*/
                match parameters[0] {
                    0 => self.polymarker_color = parameters[1] as u8,
                    1 => self.line_color = parameters[1] as u8,
                    2 => self.fill_color = parameters[1] as u8,
                    3 => self.text_color = parameters[1] as u8,
                    x => return Err(anyhow::anyhow!("ColorSet unknown/unsupported argument: {x}")),
                }
                Ok(CallbackAction::NoUpdate)
            }

            IgsCommands::SetPenColor => {
                if parameters.len() != 4 {
                    return Err(anyhow::anyhow!("SetPenColor command requires 4 arguments"));
                }

                let color = parameters[0];
                if !(0..=15).contains(&color) {
                    return Err(anyhow::anyhow!("ColorSet unknown/unsupported argument: {color}"));
                }
                self.pen_colors[color as usize] = Color::new(
                    (parameters[1] as u8) << 5 | parameters[1] as u8,
                    (parameters[2] as u8) << 5 | parameters[2] as u8,
                    (parameters[3] as u8) << 5 | parameters[3] as u8,
                );
                //println!("Set pen color {} to {}", color, self.pen_colors[color as usize]);
                Ok(CallbackAction::Update)
            }

            IgsCommands::DrawLine => {
                if parameters.len() != 4 {
                    return Err(anyhow::anyhow!("DrawLine command requires 4 arguments"));
                }
                let color = self.line_color;
                self.draw_line(parameters[0], parameters[1], parameters[2], parameters[3], color, self.line_type.get_mask());
                self.cur_position = Position::new(parameters[2], parameters[3]);
                Ok(CallbackAction::Update)
            }
            IgsCommands::PolyFill => {
                if parameters.is_empty() {
                    return Err(anyhow::anyhow!("PolyFill requires minimun 1 arguments"));
                }
                let points: i32 = parameters[0];
                if points * 2 + 1 != parameters.len() as i32 {
                    return Err(anyhow::anyhow!("PolyFill requires {} arguments was {} ", points * 2 + 1, parameters.len()));
                }
                self.fill_poly(&parameters[1..]);
                if self.draw_border {
                    self.draw_poly(&parameters[1..]);
                }
                Ok(CallbackAction::Update)
            }

            IgsCommands::PolyLine => {
                if parameters.is_empty() {
                    return Err(anyhow::anyhow!("PolyLine requires minimun 1 arguments"));
                }
                let points: i32 = parameters[0];
                if points * 2 + 1 != parameters.len() as i32 {
                    return Err(anyhow::anyhow!("PolyLine requires {} arguments was {} ", points * 2 + 1, parameters.len()));
                }
                self.draw_polyline(&parameters[1..]);
                self.cur_position = Position::new(parameters[parameters.len() - 2], parameters[parameters.len() - 1]);

                Ok(CallbackAction::Update)
            }

            IgsCommands::LineDrawTo => {
                if parameters.len() != 2 {
                    return Err(anyhow::anyhow!("LineDrawTo command requires 2 arguments"));
                }
                self.draw_line(
                    self.cur_position.x,
                    self.cur_position.y,
                    parameters[0],
                    parameters[1],
                    self.line_color,
                    self.line_type.get_mask(),
                );
                self.cur_position = Position::new(parameters[0], parameters[1]);
                Ok(CallbackAction::Update)
            }

            IgsCommands::Box => {
                if parameters.len() != 5 {
                    return Err(anyhow::anyhow!("Box command requires 5 arguments"));
                }
                let mut x0 = parameters[0];
                let mut y0 = parameters[1];
                let mut x1 = parameters[2];
                let mut y1 = parameters[3];

                if x0 > x1 {
                    std::mem::swap(&mut x0, &mut x1);
                }

                if y0 > y1 {
                    std::mem::swap(&mut y0, &mut y1);
                }

                self.fill_rect(x0, y0, x1, y1);
                if self.draw_border {
                    let color = self.fill_color;

                    self.draw_line(x0, y0, x0, y1, color, 0);
                    self.draw_line(x1, y0, x1, y1, color, 0);
                    self.draw_line(x0, y0, x1, y0, color, 0);
                    self.draw_line(x0, y1, x1, y1, color, 0);
                }
                Ok(CallbackAction::Update)
            }

            IgsCommands::RoundedRectangles => {
                if parameters.len() != 5 {
                    return Err(anyhow::anyhow!("Box command requires 5 arguments"));
                }
                let x0 = parameters[0];
                let y0 = parameters[1];
                let x1 = parameters[2];
                let y1 = parameters[3];

                self.round_rect(x0, y0, x1, y1, parameters[4]);
                Ok(CallbackAction::Update)
            }

            IgsCommands::HollowSet => {
                if parameters.len() != 1 {
                    return Err(anyhow::anyhow!("HollowSet command requires 1 argument"));
                }
                match parameters[0] {
                    0 => self.hollow_set = false,
                    1 => self.hollow_set = true,
                    x => return Err(anyhow::anyhow!("HollowSet unknown/unsupported argument: {x}")),
                }
                Ok(CallbackAction::NoUpdate)
            }
            IgsCommands::Pieslice => {
                if parameters.len() != 5 {
                    return Err(anyhow::anyhow!("AttributeForFills command requires 3 arguments"));
                }
                println!("Todo pieslice!");
                /*
                let mut pb = PathBuilder::new();
                pb.arc(
                    parameters[0] as f32,
                    parameters[1] as f32,
                    parameters[2] as f32,
                    parameters[3] as f32 / 360.0 * 2.0 * std::f32::consts::PI,
                    parameters[4] as f32 / 360.0 * 2.0 * std::f32::consts::PI,
                );
                let path = pb.finish();

                let (r, g, b) = self.pen_colors[self.fill_color].get_rgb();
                self.screen.fill(&path, &Source::Solid(create_solid_source(r, g, b)), &DrawOptions::new());
                */
                Ok(CallbackAction::Update)
            }

            IgsCommands::Circle => {
                if parameters.len() != 3 {
                    return Err(anyhow::anyhow!("AttributeForFills command requires 3 arguments"));
                }
                self.fill_ellipse(parameters[0], parameters[1], parameters[2], parameters[2]);
                if self.draw_border {
                    self.draw_circle(parameters[0], parameters[1], parameters[2]);
                }
                Ok(CallbackAction::Update)
            }

            IgsCommands::Ellipse => {
                if parameters.len() != 4 {
                    return Err(anyhow::anyhow!("Ellipse command requires 4 arguments"));
                }
                self.fill_ellipse(parameters[0], parameters[1], parameters[2], parameters[3]);
                if self.draw_border {
                    self.draw_ellipse(parameters[0], parameters[1], parameters[2], parameters[3]);
                }
                Ok(CallbackAction::Update)
            }

            IgsCommands::EllipticalArc => {
                if parameters.len() != 6 {
                    return Err(anyhow::anyhow!("EllipticalArc command requires 6 arguments"));
                }
                /*
                let mut pb = PathBuilder::new();
                pb.elliptic_arc(
                    parameters[0] as f32,
                    parameters[1] as f32,
                    parameters[2] as f32,
                    parameters[4] as f32,
                    parameters[5] as f32 / 360.0 * 2.0 * std::f32::consts::PI,
                    parameters[6] as f32 / 360.0 * 2.0 * std::f32::consts::PI,
                );
                let path = pb.finish();
                let (r, g, b) = self.pen_colors[self.fill_color].get_rgb();
                self.screen.fill(&path, &Source::Solid(create_solid_source(r, g, b)), &DrawOptions::new());
                */
                Ok(CallbackAction::Update)
            }

            IgsCommands::QuickPause => {
                if parameters.len() != 1 {
                    return Err(anyhow::anyhow!("QuickPause command requires 1 arguments"));
                }
                match parameters[0] {
                    9995 => {
                        self.double_step = 4.0;
                        Ok(CallbackAction::NoUpdate)
                    }
                    9996 => {
                        self.double_step = 3.0;
                        Ok(CallbackAction::NoUpdate)
                    }
                    9997 => {
                        self.double_step = 2.0;
                        Ok(CallbackAction::NoUpdate)
                    }
                    9998 => {
                        self.double_step = 1.0;
                        Ok(CallbackAction::NoUpdate)
                    }
                    9999 => {
                        self.double_step = -1.0;
                        Ok(CallbackAction::NoUpdate)
                    }
                    p => {
                        if p < 180 {
                            Ok(CallbackAction::Pause((p as f32 * 1000.0 / 60.0) as u32))
                        } else {
                            Err(anyhow::anyhow!("Quick pause invalid {}", p))
                        }
                    }
                }
            }
            IgsCommands::AttributeForFills => {
                if parameters.len() != 3 {
                    return Err(anyhow::anyhow!("AttributeForFills command requires 3 arguments"));
                }
                match parameters[0] {
                    0 => {
                        self.fill_pattern_type = FillPatternType::Hollow;
                        self.fill_pattern = &HOLLOW_PATTERN;
                    }
                    1 => {
                        self.fill_pattern_type = FillPatternType::Solid;
                        self.fill_pattern = &SOLID_PATTERN;
                    }
                    2 => {
                        self.fill_pattern_type = FillPatternType::Pattern;
                        if parameters[1] == 0 {
                            self.fill_pattern = &RANDOM_PATTERN;
                        } else if parameters[1] >= 1 && parameters[1] <= 24 {
                            self.fill_pattern = &TYPE_PATTERN[parameters[1] as usize - 1];
                        } else {
                            log::warn!("AttributeForFills inlvalid type pattern number : {} (valid is 1->24)", parameters[1]);
                            self.fill_pattern = &SOLID_PATTERN;
                        }
                    }
                    3 => {
                        self.fill_pattern_type = FillPatternType::Hatch;
                        if parameters[1] >= 1 && parameters[1] <= 12 {
                            if parameters[1] <= 6 {
                                self.fill_pattern = &HATCH_PATTERN[parameters[1] as usize - 1];
                            } else {
                                self.fill_pattern = &HATCH_WIDE_PATTERN[parameters[1] as usize - 7];
                            }
                        } else {
                            log::warn!("AttributeForFills inlvalid hatch pattern number : {} (valid is 1->12)", parameters[1]);
                            self.fill_pattern = &SOLID_PATTERN;
                        }
                    }
                    4 => {
                        self.fill_pattern_type = FillPatternType::UserdDefined;
                        // TODO
                        self.fill_pattern = &SOLID_PATTERN;
                    }
                    _ => return Err(anyhow::anyhow!("AttributeForFills unknown/unsupported argument: {}", parameters[0])),
                }
                self.pattern_index_number = parameters[1] as usize;
                match parameters[2] {
                    0 => self.draw_border = false,
                    1 => self.draw_border = true,
                    _ => return Err(anyhow::anyhow!("AttributeForFills unknown/unsupported argument: {}", parameters[2])),
                }
                Ok(CallbackAction::NoUpdate)
            }
            IgsCommands::FilledRectangle => {
                if parameters.len() != 4 {
                    return Err(anyhow::anyhow!("FilledRectangle command requires 4 arguments"));
                }
                self.fill_rect(parameters[0], parameters[1], parameters[2], parameters[3]);
                Ok(CallbackAction::Update)
            }

            IgsCommands::TimeAPause => Ok(CallbackAction::Pause(1000 * parameters[0] as u32)),

            IgsCommands::PolymarkerPlot => {
                if parameters.len() != 2 {
                    return Err(anyhow::anyhow!("PolymarkerPlot command requires 2 arguments"));
                }
                self.draw_poly_maker(parameters[0], parameters[1]);
                Ok(CallbackAction::Update)
            }

            IgsCommands::TextEffects => {
                if parameters.len() != 3 {
                    return Err(anyhow::anyhow!("PolymarkerPlot command requires 2 arguments"));
                }
                match parameters[0] {
                    0 => self.text_effects = TextEffects::Normal,
                    1 => self.text_effects = TextEffects::Thickened,
                    2 => self.text_effects = TextEffects::Ghosted,
                    4 => self.text_effects = TextEffects::Skewed,
                    8 => self.text_effects = TextEffects::Underlined,
                    16 => self.text_effects = TextEffects::Outlined,
                    _ => return Err(anyhow::anyhow!("TextEffects unknown/unsupported argument: {}", parameters[0])),
                }

                match parameters[1] {
                    8 | 9 | 10 | 16 | 18 | 20 => self.text_size = parameters[1],
                    _ => return Err(anyhow::anyhow!("TextEffects unknown/unsupported argument: {}", parameters[1])),
                }

                match parameters[2] {
                    0 => self.text_rotation = TextRotation::Right,
                    1 => self.text_rotation = TextRotation::Up,
                    2 => self.text_rotation = TextRotation::Down,
                    3 => self.text_rotation = TextRotation::Left,
                    4 => self.text_rotation = TextRotation::RightReverse,
                    _ => return Err(anyhow::anyhow!("TextEffects unknown/unsupported argument: {}", parameters[2])),
                }
                Ok(CallbackAction::Update)
            }

            IgsCommands::LineMarkerTypes => {
                if parameters.len() != 3 {
                    return Err(anyhow::anyhow!("LineMarkerTypes command requires 3 arguments"));
                }
                if parameters[0] == 1 {
                    match parameters[1] {
                        1 => self.polymaker_type = PolymarkerType::Point,
                        2 => self.polymaker_type = PolymarkerType::Plus,
                        3 => self.polymaker_type = PolymarkerType::Star,
                        4 => self.polymaker_type = PolymarkerType::Square,
                        5 => self.polymaker_type = PolymarkerType::DiagonalCross,
                        6 => self.polymaker_type = PolymarkerType::Diamond,
                        _ => return Err(anyhow::anyhow!("LineMarkerTypes unknown/unsupported argument: {}", parameters[0])),
                    }
                    self.polymarker_size = parameters[2] as usize;
                } else if parameters[0] == 2 {
                    match parameters[1] {
                        1 => self.line_type = LineType::Solid,
                        2 => self.line_type = LineType::LongDash,
                        3 => self.line_type = LineType::DottedLine,
                        4 => self.line_type = LineType::DashDot,
                        5 => self.line_type = LineType::DashedLine,
                        6 => self.line_type = LineType::DashedDotDot,
                        7 => self.line_type = LineType::UserDefined,
                        _ => return Err(anyhow::anyhow!("LineMarkerTypes unknown/unsupported argument: {}", parameters[1])),
                    }
                    if self.line_type == LineType::UserDefined {
                        self.user_defined_pattern_number = parameters[2] as usize;
                    } else {
                        self.solidline_size = parameters[2] as usize;
                    }
                } else {
                    return Err(anyhow::anyhow!("LineMarkerTypes unknown/unsupported argument: {}", parameters[0]));
                }
                Ok(CallbackAction::NoUpdate)
            }

            IgsCommands::DrawingMode => {
                if parameters.len() != 1 {
                    return Err(anyhow::anyhow!("DrawingMode command requires 1 argument"));
                }
                match parameters[0] {
                    1 => self.drawing_mode = DrawingMode::Replace,
                    2 => self.drawing_mode = DrawingMode::Transparent,
                    3 => self.drawing_mode = DrawingMode::Xor,
                    4 => self.drawing_mode = DrawingMode::ReverseTransparent,
                    _ => return Err(anyhow::anyhow!("DrawingMode unknown/unsupported argument: {}", parameters[0])),
                }
                Ok(CallbackAction::NoUpdate)
            }

            IgsCommands::SetResolution => {
                if parameters.len() != 2 {
                    return Err(anyhow::anyhow!("SetResolution command requires 2 argument"));
                }
                match parameters[0] {
                    0 => self.terminal_resolution = TerminalResolution::Low,
                    1 => self.terminal_resolution = TerminalResolution::Medium,
                    _ => return Err(anyhow::anyhow!("SetResolution unknown/unsupported argument: {}", parameters[0])),
                }
                match parameters[1] {
                    0 => { // no change
                    }
                    1 => {
                        // default system colors
                        self.pen_colors = IGS_SYSTEM_PALETTE.to_vec();
                    }
                    2 => {
                        // IG colors
                        self.pen_colors = IGS_PALETTE.to_vec();
                    }
                    _ => return Err(anyhow::anyhow!("SetResolution unknown/unsupported argument: {}", parameters[1])),
                }

                Ok(CallbackAction::NoUpdate)
            }

            IgsCommands::WriteText => {
                if parameters.len() != 3 {
                    return Err(anyhow::anyhow!("WriteText command requires 3 arguments"));
                }
                let text_pos = Position::new(parameters[0], parameters[1]);
                self.write_text(text_pos, string_parameter);
                Ok(CallbackAction::Update)
            }

            IgsCommands::FloodFill => {
                if parameters.len() != 2 {
                    return Err(anyhow::anyhow!("FloodFill command requires 2 arguments"));
                }
                self.flood_fill(parameters[0], parameters[1]);
                Ok(CallbackAction::Pause(100))
            }

            IgsCommands::GrabScreen => {
                if parameters.len() < 2 {
                    return Err(anyhow::anyhow!("GrabScreen command requires > 2 argument"));
                }
                let write_mode = parameters[1];
                println!("grab screen {} - {write_mode}", parameters[0]);
                match parameters[0] {
                    0 => {
                        if parameters.len() != 8 {
                            return Err(anyhow::anyhow!("GrabScreen screen to screen command requires 8 argument"));
                        }
                        let from_start = Position::new(parameters[2], parameters[3]);
                        let from_end = Position::new(parameters[4], parameters[5]);
                        let dest = Position::new(parameters[6], parameters[7]);
                        self.blit_screen_to_screen(write_mode, from_start, from_end, dest);
                    }

                    1 => {
                        if parameters.len() != 6 {
                            return Err(anyhow::anyhow!("GrabScreen screen to memory command requires 6 argument"));
                        }
                        let from_start = Position::new(parameters[2], parameters[3]);
                        let from_end = Position::new(parameters[4], parameters[5]);
                        self.blit_screen_to_memory(write_mode, from_start, from_end);
                    }

                    2 => {
                        if parameters.len() != 4 {
                            return Err(anyhow::anyhow!("GrabScreen memory to screen command requires 4 argument"));
                        }
                        let dest = Position::new(parameters[2], parameters[3]);
                        self.blit_memory_to_screen(
                            write_mode,
                            Position::new(0, 0),
                            Position::new(self.screen_memory_size.width, self.screen_memory_size.height),
                            dest,
                        );
                    }

                    3 => {
                        if parameters.len() != 8 {
                            return Err(anyhow::anyhow!("GrabScreen piece of memory to screen command requires 4 argument"));
                        }
                        let from_start = Position::new(parameters[2], parameters[3]);
                        let from_end = Position::new(parameters[4], parameters[5]);
                        let dest = Position::new(parameters[6], parameters[7]);
                        self.blit_memory_to_screen(write_mode, from_start, from_end, dest);
                    }
                    _ => return Err(anyhow::anyhow!("GrabScreen unknown/unsupported grab screen mode: {}", parameters[0])),
                }

                if self.double_step >= 0.0 {
                    Ok(CallbackAction::Pause((self.double_step * 1000.0 / 60.0) as u32))
                } else {
                    Ok(CallbackAction::Update)
                }
            }

            IgsCommands::VTColor => {
                if parameters.len() != 2 {
                    return Err(anyhow::anyhow!("VTColor command requires 2 argument"));
                }
                if let Some(pen) = REGISTER_TO_PEN.get(parameters[1] as usize) {
                    let color = self.pen_colors[*pen].clone();

                    if parameters[0] == 0 {
                        caret.set_background(buf.palette.insert_color(color));
                    } else if parameters[0] == 1 {
                        caret.set_foreground(buf.palette.insert_color(color));
                    } else {
                        return Err(anyhow::anyhow!("VTColor unknown/unsupported color mode: {}", parameters[0]));
                    }
                    Ok(CallbackAction::NoUpdate)
                } else {
                    Err(anyhow::anyhow!("VTColor unknown/unsupported color selector: {}", parameters[1]))
                }
            }
            IgsCommands::VTPosition => {
                if parameters.len() != 2 {
                    return Err(anyhow::anyhow!("VTPosition command requires 2 argument"));
                }
                caret.set_position(Position::new(parameters[0], parameters[1]));
                Ok(CallbackAction::NoUpdate)
            }
            _ => Err(anyhow::anyhow!("Unimplemented IGS command: {command:?}")),
        }
    }
}

const REGISTER_TO_PEN: &[usize; 17] = &[0, 2, 3, 6, 4, 7, 5, 8, 9, 10, 11, 14, 12, 12, 15, 13, 1];
