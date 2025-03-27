use std::{mem::swap, str::FromStr};

use super::{
    IGS_VERSION, LINE_STYLE, RANDOM_PATTERN, SOLID_PATTERN,
    cmd::IgsCommands,
    sound::SOUND_DATA,
    vdi::{TWOPI, color_idx_to_pixel_val, gdp_curve, pixel_val_to_color_idx},
};
use crate::{
    BitFont, Buffer, CallbackAction, Caret, Color, EngineResult, IGS_PALETTE, IGS_SYSTEM_PALETTE, Position, Size,
    igs::{HATCH_PATTERN, HATCH_WIDE_PATTERN, HOLLOW_PATTERN, TYPE_PATTERN, vdi::blit_px},
    load_atari_fonts,
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
    screen: Vec<u8>,
    terminal_resolution: TerminalResolution,

    cur_position: Position,
    pen_colors: Vec<Color>,
    polymarker_color: u8,
    line_color: u8,
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
}

unsafe impl Send for DrawExecutor {}

unsafe impl Sync for DrawExecutor {}

pub enum ClearCommand {
    /// Clear screen home cursor.
    ClearScreen,
    /// Clear from home to cursor.
    ClearFromHomeToCursor,
    /// Clear from cursor to bottom of screen.
    ClearFromCursorToBottom,
}

impl Default for DrawExecutor {
    fn default() -> Self {
        DrawExecutor::new(TerminalResolution::Low)
    }
}

impl DrawExecutor {
    pub fn new(terminal_resolution: TerminalResolution) -> Self {
        let fonts = load_atari_fonts();
        let font_7px = BitFont::from_str(fonts[0].2).unwrap();
        let font_9px = BitFont::from_str(fonts[1].2).unwrap();
        let font_16px = BitFont::from_str(fonts[2].2).unwrap();
        let res = terminal_resolution.get_resolution();
        Self {
            screen: vec![1; res.width as usize * res.height as usize],
            terminal_resolution,
            pen_colors: IGS_SYSTEM_PALETTE.to_vec(),
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
        }
    }
    pub fn clear(&mut self, _cmd: ClearCommand, caret: &mut Caret) {
        // TODO: Clear command
        caret.set_position(Position::default());
        self.screen.fill(1);
    }

    pub fn scroll(&mut self, amount: i32) {
        if amount == 0 {
            return;
        }
        let res = self.get_resolution();
        if amount < 0 {
            self.screen.splice(0..0, vec![1; res.width as usize * amount.abs() as usize]);
            self.screen.truncate(res.width as usize * res.height as usize);
        } else {
            self.screen.splice(0..res.width as usize * amount.abs() as usize, vec![]);
            self.screen.extend(vec![1; res.width as usize * amount.abs() as usize]);
        }
    }

    pub fn set_resolution(&mut self, res: TerminalResolution) {
        self.terminal_resolution = res;
        // let res = self.get_resolution();
        // self.screen = vec![1; (res.width * res.height) as usize];
    }

    pub fn init_resolution(&mut self, buf: &mut Buffer, caret: &mut Caret) {
        buf.clear_screen(0, caret);
        let res = self.get_resolution();
        self.screen = vec![1; (res.width * res.height) as usize];
    }

    pub fn get_char_resolution(&self) -> Size {
        let res = self.get_resolution();
        Size::new(res.width / 8, res.height / 8)
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
        let res = self.get_resolution();
        if x < 0 || y < 0 || x >= res.width || y >= res.height {
            return;
        }
        let offset = (y * res.width + x) as usize;
        self.screen[offset] = line_color;
    }

    fn get_pixel(&mut self, x: i32, y: i32) -> u8 {
        let offset = (y * self.get_resolution().width + x) as usize;
        self.screen[offset]
    }

    fn fill_pixel(&mut self, x: i32, y: i32) {
        let w = self.fill_pattern[(y as usize) % self.fill_pattern.len()];

        let mask = w & (0x8000 >> (x as usize % 16)) != 0;
        match self.drawing_mode {
            DrawingMode::Replace => {
                if mask {
                    self.set_pixel(x, y, self.fill_color);
                }
            }
            DrawingMode::Transparent => {
                if mask {
                    self.set_pixel(x, y, self.fill_color);
                }
            }
            DrawingMode::Xor => {
                let s = if mask { 0xFF } else { 0x00 };
                let d = color_idx_to_pixel_val(self.pen_colors.len(), self.get_pixel(x, y));
                let new_color = pixel_val_to_color_idx(self.pen_colors.len(), (s ^ d) & 0x0F);
                self.set_pixel(x, y, new_color);
            }
            DrawingMode::ReverseTransparent => {
                if !mask {
                    self.set_pixel(x, y, self.fill_color);
                }
            }
        }
    }

    fn draw_vline(&mut self, x: i32, mut y0: i32, mut y1: i32, color: u8, mask: usize) {
        if y1 < y0 {
            swap(&mut y0, &mut y1);
        }
        let mut line_mask = LINE_STYLE[mask];
        line_mask = line_mask.rotate_left((y0 & 0x0f) as u32);
        for y in y0..=y1 {
            line_mask = line_mask.rotate_left(1);
            if 1 & line_mask != 0 {
                self.set_pixel(x, y, color);
            }
        }
    }

    fn draw_hline(&mut self, y: i32, x0: i32, x1: i32, color: u8, mask: usize) {
        let mut line_mask = LINE_STYLE[mask];
        line_mask = line_mask.rotate_left((x0 & 0x0f) as u32);
        for x in x0..=x1 {
            line_mask = line_mask.rotate_left(1);
            if 1 & line_mask != 0 {
                self.set_pixel(x, y, color);
            }
        }
    }

    fn draw_line(&mut self, mut x0: i32, mut y0: i32, mut x1: i32, mut y1: i32, color: u8, mask: usize) {
        if x1 < x0 {
            swap(&mut x0, &mut x1);
            swap(&mut y0, &mut y1);
        }
        if x0 == x1 {
            self.draw_vline(x0, y0, y1, color, mask);
            return;
        }
        if y0 == y1 {
            self.draw_hline(y0, x0, x1, color, mask);
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
                    self.set_pixel(x, y, color);
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
                    self.set_pixel(x, y, color);
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

    fn fill_circle(&mut self, xm: i32, ym: i32, r: i32) {
        let y_rad = self.calc_circle_y_rad(r).max(1);
        self.fill_ellipse(xm, ym, r, y_rad);
    }

    fn draw_circle(&mut self, xm: i32, ym: i32, r: i32) {
        let y_rad = self.calc_circle_y_rad(r);
        let points = gdp_curve(xm, ym, r, y_rad, 0, TWOPI as i32);
        self.draw_poly(&points, self.line_color, false);
    }

    fn draw_ellipse(&mut self, xm: i32, ym: i32, a: i32, b: i32) {
        let points = gdp_curve(xm, ym, a, b, 0, TWOPI as i32);
        self.draw_poly(&points, self.line_color, false);
    }

    fn draw_elliptical_pieslice(&mut self, xm: i32, ym: i32, xr: i32, yr: i32, beg_ang: i32, end_ang: i32) {
        let mut points = gdp_curve(xm, ym, xr, yr, beg_ang * 10, end_ang * 10);
        points.extend_from_slice(&[xm, ym]);
        self.draw_poly(&points, self.line_color, true);
    }

    fn fill_elliptical_pieslice(&mut self, xm: i32, ym: i32, xr: i32, yr: i32, beg_ang: i32, end_ang: i32) {
        let mut points = gdp_curve(xm, ym, xr, yr, beg_ang * 10, end_ang * 10);
        points.extend_from_slice(&[xm, ym]);
        self.fill_poly(&points);
    }

    fn fill_ellipse(&mut self, xm: i32, ym: i32, a: i32, b: i32) {
        let points = gdp_curve(xm, ym, a, b, 0, TWOPI as i32);
        self.fill_poly(&points);
    }

    pub fn fill_rect(&mut self, mut x0: i32, mut y0: i32, mut x1: i32, mut y1: i32) {
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

    fn draw_arc(&mut self, xm: i32, ym: i32, a: i32, b: i32, beg_ang: i32, end_ang: i32) {
        let points = gdp_curve(xm, ym, a, b, beg_ang * 10, end_ang * 10);
        self.draw_poly(&points, self.line_color, false);
    }

    fn draw_poly(&mut self, parameters: &[i32], color: u8, close: bool) {
        let mut x = parameters[0];
        let mut y = parameters[1];
        let mask = self.line_type.get_mask();
        let mut i = 2;
        while i < parameters.len() {
            let nx = parameters[i];
            let ny = parameters[i + 1];
            self.draw_line(x, y, nx, ny, color, mask);
            x = nx;
            y = ny;
            i += 2;
        }
        if close {
            // close polygon
            self.draw_line(x, y, parameters[0], parameters[1], color, mask);
        }
    }

    fn draw_polyline(&mut self, color: u8, parameters: &[i32]) {
        let mut x = parameters[0];
        let mut y = parameters[1];
        let mask = self.line_type.get_mask();
        let mut i = 2;
        while i < parameters.len() {
            let nx = parameters[i];
            let ny = parameters[i + 1];
            self.draw_line(x, y, nx, ny, color, mask);
            x = nx;
            y = ny;
            i += 2;
        }
    }

    fn fill_poly(&mut self, points: &[i32]) {
        if self.hollow_set {
            self.draw_poly(points, self.fill_color, true);
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
                    self.fill_pixel(k, y);
                }
            }
        }
        if self.fill_pattern_type == FillPatternType::Solid {
            self.draw_poly(&points, self.fill_color, true);
        }
    }

    pub fn write_text(&mut self, text_pos: Position, string_parameter: &str) {
        let mut pos = text_pos;
        let y_off;
        let font = match self.text_size {
            8 => {
                y_off = -4;
                self.font_7px.clone()
            }
            9 => {
                y_off = -6;
                self.font_9px.clone()
            }
            16 => {
                y_off = -11;
                self.font_16px.clone()
            }
            _ => {
                y_off = -13;
                self.font_16px.clone()
            }
        };
        pos.y += y_off;
        // println!("write_text {string_parameter} {text_pos} size:{} effect:{:?} rot:{:?}", self.text_size, self.text_effects, self.text_rotation);

        let color = self.text_color;
        let font_size = font.size;
        let high_bit = 1 << (font.size.width - 1);
        let mut draw_mask: u16 = if self.text_effects == TextEffects::Ghosted { 0x5555 } else { 0xFFFF };
        for ch in string_parameter.chars() {
            let data = font.get_glyph(ch).unwrap().data.clone();
            for y in 0..font_size.height {
                for x in 0..font_size.width {
                    let iy = y; //(y as f32 / font_size.height as f32 * char_size.height as f32) as i32;
                    let ix = x; // (x as f32 / font_size.width as f32 * char_size.width as f32) as i32;
                    draw_mask = draw_mask.rotate_left(1);
                    if data[iy as usize] & (high_bit >> ix) != 0 {
                        if 1 & draw_mask != 0 {
                            let p = pos + Position::new(x, y);
                            self.set_pixel(p.x, p.y, color);
                            if self.text_effects == TextEffects::Thickened {
                                self.set_pixel(p.x + 1, p.y, color);
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
                    self.set_pixel(p.x, p.y, color);
                    if self.text_effects == TextEffects::Thickened {
                        self.set_pixel(p.x + 1, p.y, color);
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

    fn blit_screen_to_screen(&mut self, write_mode: i32, from: Position, to: Position, mut dest: Position) {
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

        let res = self.get_resolution();

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
            blit.extend_from_slice(&self.screen[offset..offset + width]);
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
                self.screen[o] = blit_px(write_mode, self.pen_colors.len(), blit[blit_offset], self.screen[o]);
                o += 1;
                blit_offset += 1;
            }
            offset += line_length;
        }
    }

    fn blit_memory_to_screen(&mut self, write_mode: i32, from: Position, to: Position, mut dest: Position) {
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

        let res = self.get_resolution();
        for y in 0..height {
            let mut offset = (start_y + y) * self.screen_memory_size.width as usize + start_x;
            let mut screen_offset = (dest.y as usize + y) * res.width as usize + dest.x as usize;
            if screen_offset >= self.screen.len() {
                break;
            }
            for _x in 0..width {
                if dest.x + _x as i32 >= res.width {
                    break;
                }
                let color = self.screen_memory[offset];
                offset += 1;
                if screen_offset >= self.screen.len() {
                    break;
                }
                let px = self.screen[screen_offset];
                self.screen[screen_offset] = blit_px(write_mode, self.pen_colors.len(), color, px);
                screen_offset += 1;
            }
        }
    }

    fn blit_screen_to_memory(&mut self, _write_mode: i32, from: Position, to: Position) {
        let width = (to.x - from.x).abs() + 1;
        let height = (to.y - from.y).abs() + 1;

        let start_x = to.x.min(from.x);
        let start_y = to.y.min(from.y);

        self.screen_memory_size = Size::new(width, height);
        self.screen_memory.clear();

        for y in 0..height {
            for x in 0..width {
                let color = self.get_pixel(start_x + x, start_y + y);
                self.screen_memory.push(color);
            }
        }
    }

    fn round_rect(&mut self, mut x1: i32, mut y1: i32, mut x2: i32, mut y2: i32, parameters: i32) {
        let mut points = Vec::new();
        if x1 > x2 {
            swap(&mut x1, &mut x2);
        }
        if y1 < y2 {
            swap(&mut y1, &mut y2);
        }

        let x_radius = (self.get_resolution().width >> 6).min((x2 - x1) / 2);
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

        if parameters == 1 {
            self.fill_poly(&points);
        } else {
            self.draw_poly(&points, self.line_color, false);
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
            self.draw_polyline(self.polymarker_color, &p);
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

    pub fn get_resolution(&self) -> Size {
        let s = self.terminal_resolution.get_resolution();
        Size::new(s.width, s.height)
    }

    pub fn get_picture_data(&mut self) -> Option<(Size, Vec<u8>)> {
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

    pub fn execute_command(
        &mut self,
        buf: &mut Buffer,
        caret: &mut Caret,
        command: IgsCommands,
        parameters: &[i32],
        string_parameter: &str,
    ) -> EngineResult<CallbackAction> {
        //println!("cmd:{:?} arguments: {:?}", command, parameters);
        match command {
            IgsCommands::Initialize => {
                if parameters.len() < 1 {
                    return Err(anyhow::anyhow!("Initialize command requires 1 argument"));
                }
                match parameters[0] {
                    0 => {
                        self.init_resolution(buf, caret);
                        self.pen_colors = IGS_SYSTEM_PALETTE.to_vec();
                        self.reset_attributes();
                    }
                    1 => {
                        self.init_resolution(buf, caret);
                        self.pen_colors = IGS_SYSTEM_PALETTE.to_vec();
                    }
                    2 => {
                        self.reset_attributes();
                    }
                    3 => {
                        self.init_resolution(buf, caret);
                        self.pen_colors = IGS_PALETTE.to_vec();
                    }
                    x => return Err(anyhow::anyhow!("Initialize unknown/unsupported argument: {x}")),
                }
                Ok(CallbackAction::Update)
            }
            IgsCommands::ScreenClear => {
                let cmd = match parameters[0] {
                    0 => ClearCommand::ClearScreen,
                    1 => ClearCommand::ClearFromHomeToCursor,
                    2 => ClearCommand::ClearFromCursorToBottom,
                    3 => ClearCommand::ClearScreen,
                    4 => ClearCommand::ClearScreen,
                    5 => ClearCommand::ClearScreen,
                    _ => return Err(anyhow::anyhow!("ScreenClear unknown/unsupported argument: {}", parameters[0])),
                };
                self.clear(cmd, caret);
                Ok(CallbackAction::Update)
            }
            IgsCommands::AskIG => {
                if parameters.len() < 1 {
                    return Err(anyhow::anyhow!("Initialize command requires 1 argument"));
                }
                match parameters[0] {
                    0 => Ok(CallbackAction::SendString(IGS_VERSION.to_string())),
                    3 => Ok(CallbackAction::SendString(self.terminal_resolution.resolution_id() + ":")),
                    x => Err(anyhow::anyhow!("AskIG unknown/unsupported argument: {x}")),
                }
            }
            IgsCommands::Cursor => {
                if parameters.len() < 1 {
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
                if parameters.len() < 2 {
                    return Err(anyhow::anyhow!("ColorSet command requires 2 arguments"));
                }

                /*                println!(
                    "Color Set {}={}",
                    match parameters[0] {
                        0 => "polymaker",
                        1 => "line",
                        2 => "fill",
                        3 => "text",
                        _ => "?",
                    },
                    parameters[1]
                );*/
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
                if parameters.len() < 4 {
                    return Err(anyhow::anyhow!("SetPenColor command requires 4 arguments"));
                }

                let color = parameters[0];
                if !(0..=15).contains(&color) {
                    return Err(anyhow::anyhow!("ColorSet unknown/unsupported argument: {color}"));
                }
                //println!("Set pen color {} to {:b}", color, parameters[1]);
                self.pen_colors[color as usize] = Color::new((parameters[1] * 0x22) as u8, (parameters[2] * 0x22) as u8, (parameters[3] * 0x22) as u8);
                //println!("Set pen color {} to {}", color, self.pen_colors[color as usize]);
                Ok(CallbackAction::Update)
            }

            IgsCommands::DrawLine => {
                if parameters.len() < 4 {
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
                self.draw_polyline(self.line_color, &parameters[1..]);
                self.cur_position = Position::new(parameters[parameters.len() - 2], parameters[parameters.len() - 1]);

                Ok(CallbackAction::Update)
            }

            IgsCommands::LineDrawTo => {
                if parameters.len() < 2 {
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
                if parameters.len() < 5 {
                    return Err(anyhow::anyhow!("Box command requires 5 arguments"));
                }
                let mut x0 = parameters[0];
                let mut y0 = parameters[1];
                let mut x1 = parameters[2];
                let mut y1 = parameters[3];
                let round_corners = parameters[4] != 0;

                if x0 > x1 {
                    std::mem::swap(&mut x0, &mut x1);
                }

                if y0 > y1 {
                    std::mem::swap(&mut y0, &mut y1);
                }

                if round_corners {
                    self.round_rect(x0, y0, x1, y1, 1);
                } else {
                    self.fill_rect(x0, y0, x1, y1);
                }
                if self.draw_border {
                    if round_corners {
                        self.round_rect(x0, y0, x1, y1, 0);
                    } else {
                        let color = self.fill_color;
                        self.draw_line(x0, y0, x0, y1, color, 0);
                        self.draw_line(x1, y0, x1, y1, color, 0);
                        self.draw_line(x0, y0, x1, y0, color, 0);
                        self.draw_line(x0, y1, x1, y1, color, 0);
                    }
                }
                Ok(CallbackAction::Update)
            }

            IgsCommands::RoundedRectangles => {
                if parameters.len() < 5 {
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
                if parameters.len() < 1 {
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
                if parameters.len() < 5 {
                    return Err(anyhow::anyhow!("Pieslice command requires 5 arguments"));
                }
                let xrad = parameters[2];
                let yrad = self.calc_circle_y_rad(xrad);
                self.fill_elliptical_pieslice(parameters[0], parameters[1], xrad, yrad, parameters[3], parameters[4]);
                if self.draw_border {
                    self.draw_elliptical_pieslice(parameters[0], parameters[1], xrad, yrad, parameters[3], parameters[4]);
                }
                Ok(CallbackAction::Update)
            }

            IgsCommands::EllipticalPieslice => {
                if parameters.len() < 6 {
                    return Err(anyhow::anyhow!("EllipticalPieslice command requires 6 arguments"));
                }

                self.fill_elliptical_pieslice(parameters[0], parameters[1], parameters[2], parameters[3], parameters[4], parameters[5]);
                if self.draw_border {
                    self.draw_elliptical_pieslice(parameters[0], parameters[1], parameters[2], parameters[3], parameters[4], parameters[5]);
                }
                Ok(CallbackAction::Update)
            }

            IgsCommands::Circle => {
                if parameters.len() < 3 {
                    return Err(anyhow::anyhow!("AttributeForFills command requires 3 arguments"));
                }
                let r = parameters[2];
                self.fill_circle(parameters[0], parameters[1], r);
                if self.draw_border {
                    self.draw_circle(parameters[0], parameters[1], r);
                }
                Ok(CallbackAction::Update)
            }

            IgsCommands::Arc => {
                if parameters.len() < 5 {
                    return Err(anyhow::anyhow!("EllipticalArc command requires 5 arguments"));
                }

                let xrad = parameters[2];
                self.draw_arc(parameters[0], parameters[1], xrad, self.calc_circle_y_rad(xrad), parameters[3], parameters[4]);
                Ok(CallbackAction::Update)
            }

            IgsCommands::Ellipse => {
                if parameters.len() < 4 {
                    return Err(anyhow::anyhow!("Ellipse command requires 4 arguments"));
                }
                self.fill_ellipse(parameters[0], parameters[1], parameters[2], parameters[3]);
                if self.draw_border {
                    self.draw_ellipse(parameters[0], parameters[1], parameters[2], parameters[3]);
                }
                Ok(CallbackAction::Update)
            }

            IgsCommands::EllipticalArc => {
                if parameters.len() < 6 {
                    return Err(anyhow::anyhow!("EllipticalArc command requires 6 arguments"));
                }

                self.draw_arc(parameters[0], parameters[1], parameters[2], parameters[3], parameters[4], parameters[5]);
                Ok(CallbackAction::Update)
            }

            IgsCommands::QuickPause => {
                if parameters.len() < 1 {
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
                if parameters.len() < 3 {
                    return Err(anyhow::anyhow!("AttributeForFills command requires 3 arguments"));
                }
                //println!("AttributeForFills {:?}", parameters);
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
                if parameters.len() < 4 {
                    return Err(anyhow::anyhow!("FilledRectangle command requires 4 arguments"));
                }
                self.fill_rect(parameters[0], parameters[1], parameters[2], parameters[3]);
                Ok(CallbackAction::Update)
            }

            IgsCommands::TimeAPause => Ok(CallbackAction::Pause(1000 * parameters[0] as u32)),

            IgsCommands::PolymarkerPlot => {
                if parameters.len() < 2 {
                    return Err(anyhow::anyhow!("PolymarkerPlot command requires 2 arguments"));
                }
                self.draw_poly_maker(parameters[0], parameters[1]);
                Ok(CallbackAction::Update)
            }

            IgsCommands::TextEffects => {
                if parameters.len() < 3 {
                    return Err(anyhow::anyhow!("PolymarkerPlot command requires 2 arguments"));
                }
                //println!("text effect {}", parameters[0]);
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
                    2 => self.text_rotation = TextRotation::Left,
                    3 => self.text_rotation = TextRotation::Down,
                    4 => self.text_rotation = TextRotation::RightReverse,
                    _ => return Err(anyhow::anyhow!("TextEffects unknown/unsupported argument: {}", parameters[2])),
                }
                Ok(CallbackAction::Update)
            }

            IgsCommands::LineMarkerTypes => {
                if parameters.len() < 3 {
                    return Err(anyhow::anyhow!("LineMarkerTypes command requires 3 arguments"));
                }
                if parameters[0] == 1 {
                    match parameters[1] {
                        0 => {} // no change
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
                        0 => {} // no change
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
                if parameters.len() < 1 {
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
                if parameters.len() < 2 {
                    return Err(anyhow::anyhow!("SetResolution command requires 2 argument"));
                }
                match parameters[0] {
                    0 => self.set_resolution(TerminalResolution::Low),
                    1 => self.set_resolution(TerminalResolution::Medium),
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
                if parameters.len() < 3 {
                    return Err(anyhow::anyhow!("WriteText command requires 3 arguments"));
                }
                let text_pos = Position::new(parameters[0], parameters[1]);
                self.write_text(text_pos, string_parameter);
                Ok(CallbackAction::Update)
            }

            IgsCommands::FloodFill => {
                if parameters.len() < 2 {
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
                match parameters[0] {
                    0 => {
                        if parameters.len() < 8 {
                            return Err(anyhow::anyhow!("GrabScreen screen to screen command requires 8 argument"));
                        }
                        let from_start = Position::new(parameters[2], parameters[3]);
                        let from_end = Position::new(parameters[4], parameters[5]);
                        let dest = Position::new(parameters[6], parameters[7]);
                        self.blit_screen_to_screen(write_mode, from_start, from_end, dest);
                    }

                    1 => {
                        if parameters.len() < 6 {
                            return Err(anyhow::anyhow!("GrabScreen screen to memory command requires 6 argument"));
                        }
                        let from_start = Position::new(parameters[2], parameters[3]);
                        let from_end = Position::new(parameters[4], parameters[5]);
                        self.blit_screen_to_memory(write_mode, from_start, from_end);
                    }

                    2 => {
                        if parameters.len() < 4 {
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
                        if parameters.len() < 8 {
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
                if parameters.len() < 2 {
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
                if parameters.len() < 2 {
                    return Err(anyhow::anyhow!("VTPosition command requires 2 argument"));
                }
                caret.set_position(Position::new(parameters[0], parameters[1]));
                Ok(CallbackAction::NoUpdate)
            }
            IgsCommands::BellsAndWhistles => {
                let effect = parameters[0] as usize;
                const CLK_TCK: usize = 200;
                match effect {
                    0..=4 => unsafe {
                        let data = SOUND_DATA[effect].to_vec();
                        let mut out_data = Vec::new();
                        (0..5 * CLK_TCK).for_each(|_| out_data.extend_from_slice(&data));
                        Ok(CallbackAction::PlayGISTSound(out_data.to_vec()))
                    },
                    5..=19 => unsafe { Ok(CallbackAction::PlayGISTSound(SOUND_DATA[effect].to_vec())) },
                    20 => unsafe {
                        // b 20,play_flag,snd_num,element_num,negative_flag,thousands,ones
                        let play_flag = parameters[1];
                        let snd_num = parameters[2].clamp(0, 19);
                        let element_num = parameters[3].clamp(0, 55);
                        let negative_flag = if parameters[4] != 0 { -1 } else { 1 };
                        let thousands = parameters[5];
                        let ones = parameters[6];
                        SOUND_DATA[snd_num as usize][element_num as usize] = negative_flag * (thousands * 1000 * ones) as i16;
                        if play_flag != 0 {
                            Ok(CallbackAction::PlayGISTSound(SOUND_DATA[snd_num as usize].to_vec()))
                        } else {
                            Ok(CallbackAction::NoUpdate)
                        }
                    },
                    21 => {
                        //STOP sound
                        Ok(CallbackAction::NoUpdate)
                    }
                    22 => unsafe {
                        // b 22, snd_num
                        let snd_num = parameters[1].clamp(0, 19);
                        Ok(CallbackAction::PlayGISTSound(SOUND_DATA[snd_num as usize].to_vec()))
                    },
                    _ => Err(anyhow::anyhow!("BellsAndWhistles unknown/unsupported effect: {}", effect)),
                }
            }
            IgsCommands::ExtendedCommands => {
                match parameters[0] {
                    11 => {
                        // 11 LOAD or WIPE SCREEN BITBLIT MEMORY
                        if parameters.len() < 4 {
                            return Err(anyhow::anyhow!("ExtCmd WipeScreenBitBlitMemory command requires 3 argument"));
                        }
                        let sub_cmd = parameters[1];
                        let target = parameters[2];
                        let value = parameters[3];

                        match sub_cmd {
                            0 => {
                                // WIPE BitBlit Memory
                                if target == 0 {
                                    // all
                                    if value < 256 {
                                        self.screen_memory.fill(value as u8);
                                    } else {
                                        // todo: fill with random
                                    }
                                } else {
                                    // specific line
                                    if target < self.screen_memory_size.height {
                                        for x in 0..self.screen_memory_size.width {
                                            self.screen_memory[(x as usize) + (target as usize) * self.screen_memory_size.width as usize] = value as u8;
                                        }
                                    } else {
                                        log::warn!("WipeScreenBitBlitMemory invalid target line: {}", target);
                                    }
                                }
                            }
                            1 => { // LOAD and SHOW BitBlit Memory
                            }
                            2 => { //  LOAD BitBlit Memory
                            }
                            _ => {
                                println!("Unimplemented IGS LOAD or WIPE SCREEN BITBLIT MEMORY command: {parameters:?}");
                            }
                        }
                    }
                    _ => {
                        println!("Unimplemented IGS extended command: {parameters:?}");
                    }
                }

                Ok(CallbackAction::NoUpdate)
            }
            IgsCommands::InputCommand => {
                log::warn!("InputCommand not implemented - parameters: {:?}", parameters);
                Ok(CallbackAction::NoUpdate)
            }
            _ => Err(anyhow::anyhow!("Unimplemented IGS command: {command:?}")),
        }
    }
}

const REGISTER_TO_PEN: &[usize; 17] = &[0, 2, 3, 6, 4, 7, 5, 8, 9, 10, 11, 14, 12, 12, 15, 13, 1];
