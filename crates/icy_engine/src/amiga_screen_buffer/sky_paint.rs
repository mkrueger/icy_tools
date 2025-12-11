//! SkyPaint - Minimal graphics engine for Skypix protocol

use std::collections::HashSet;

use crate::{EditableScreen, Position, Rectangle};
use icy_parser_core::FillMode;

/// Image data structure for brush operations
pub struct Image {
    pub width: i32,
    pub height: i32,
    pub data: Vec<u8>,
}

/// SkyPaint graphics state - minimal for Skypix protocol
pub struct SkyPaint {
    pen_pos: Position,
    viewport: Rectangle,
    pub rip_image: Option<Image>,
}

impl Default for SkyPaint {
    fn default() -> Self {
        Self::new()
    }
}

impl SkyPaint {
    pub fn new() -> Self {
        Self {
            pen_pos: Position::new(0, 0),
            viewport: Rectangle::from(0, 0, 640, 200),
            rip_image: None,
        }
    }

    pub fn init_viewport(&mut self, width: i32, height: i32) {
        self.viewport = Rectangle::from(0, 0, width, height);
    }

    pub fn move_pen(&mut self, x: i32, y: i32) {
        self.pen_pos = Position::new(x, y);
    }

    pub fn pen_pos(&self) -> Position {
        self.pen_pos
    }

    pub fn put_pixel(&self, buf: &mut dyn EditableScreen, x: i32, y: i32, col: u8) {
        if !self.viewport.contains(x, y) {
            return;
        }
        let width = buf.resolution().width;
        let offset = (y * width + x) as usize;
        let screen = buf.screen_mut();
        if offset < screen.len() {
            // Apply color mask for 8/16 color mode
            screen[offset] = col;
        }
    }

    pub fn pixel(&self, buf: &dyn EditableScreen, x: i32, y: i32) -> u8 {
        let width = buf.resolution().width;
        let offset = (y * width + x) as usize;
        let screen = buf.screen();
        if offset < screen.len() { screen[offset] } else { 0 }
    }

    pub fn line(&mut self, buf: &mut dyn EditableScreen, x0: i32, y0: i32, x1: i32, y1: i32, color: u8) {
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;
        let mut x = x0;
        let mut y = y0;

        loop {
            self.put_pixel(buf, x, y, color);
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

    pub fn line_to(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, color: u8) {
        self.line(buf, self.pen_pos.x, self.pen_pos.y, x, y, color);
        self.move_pen(x, y);
    }

    /// Flood fill starting at (x, y) using the specified fill mode.
    ///
    /// # Fill Modes (from Amiga graphics.library)
    ///
    /// - `FillMode::Outline`: Fill stops at pixels matching the outline color (pen_a).
    ///   All pixels NOT of the outline color are filled.
    /// - `FillMode::Color`: Fill replaces all connected pixels that are the
    ///   SAME color as the starting pixel (x, y).
    pub fn flood_fill(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, mode: FillMode, fill_color: u8) {
        let res = buf.resolution();

        if x < 0 || y < 0 || x >= res.width || y >= res.height {
            return;
        }

        let start_color = self.pixel(buf, x, y);

        match mode {
            FillMode::Outline => {
                // Outline mode: fill all pixels that are NOT the outline color (fill_color)
                // The fill stops when it encounters pixels matching fill_color
                let outline_color = fill_color;
                if start_color == outline_color {
                    return; // Starting point is on the outline, nothing to fill
                }
                self.flood_fill_outline(buf, x, y, outline_color, fill_color, res.width, res.height);
            }
            FillMode::Color => {
                // Color mode: replace all connected pixels of the same color as start
                if start_color == fill_color {
                    return; // Already the fill color, nothing to do
                }
                self.flood_fill_color(buf, x, y, start_color, fill_color, res.width, res.height);
            }
        }
    }

    /// Outline mode flood fill - fills all pixels that are NOT the outline color
    fn flood_fill_outline(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, outline_color: u8, fill_color: u8, width: i32, height: i32) {
        let mut stack = vec![Position::new(x, y)];
        let mut visited = HashSet::new();

        while let Some(pos) = stack.pop() {
            if pos.x < 0 || pos.x >= width || pos.y < 0 || pos.y >= height {
                continue;
            }
            if visited.contains(&pos) {
                continue;
            }
            visited.insert(pos);

            let pixel = self.pixel(buf, pos.x, pos.y);
            if pixel == outline_color {
                continue; // Stop at outline
            }

            self.put_pixel(buf, pos.x, pos.y, fill_color);

            stack.push(Position::new(pos.x + 1, pos.y));
            stack.push(Position::new(pos.x - 1, pos.y));
            stack.push(Position::new(pos.x, pos.y + 1));
            stack.push(Position::new(pos.x, pos.y - 1));
        }
    }

    /// Color mode flood fill - replaces all connected pixels of the target color
    fn flood_fill_color(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, target_color: u8, fill_color: u8, width: i32, height: i32) {
        let mut stack = vec![Position::new(x, y)];

        while let Some(pos) = stack.pop() {
            if pos.x < 0 || pos.x >= width || pos.y < 0 || pos.y >= height {
                continue;
            }

            let pixel = self.pixel(buf, pos.x, pos.y);
            if pixel != target_color {
                continue;
            }

            self.put_pixel(buf, pos.x, pos.y, fill_color);

            stack.push(Position::new(pos.x + 1, pos.y));
            stack.push(Position::new(pos.x - 1, pos.y));
            stack.push(Position::new(pos.x, pos.y + 1));
            stack.push(Position::new(pos.x, pos.y - 1));
        }
    }

    pub fn bar(&mut self, buf: &mut dyn EditableScreen, left: i32, top: i32, right: i32, bottom: i32, color: u8) {
        let rect = Rectangle::from(left, top, right - left + 1, bottom - top + 1).intersect(&self.viewport);
        if rect.width() <= 0 || rect.height() <= 0 {
            return;
        }
        let width = buf.resolution().width;
        let screen = buf.screen_mut();
        for y in rect.top()..rect.bottom() {
            let start = (y * width + rect.left()) as usize;
            let end = start + rect.width() as usize;
            if end <= screen.len() {
                screen[start..end].fill(color);
            }
        }
    }

    pub fn ellipse(&mut self, buf: &mut dyn EditableScreen, cx: i32, cy: i32, rx: i32, ry: i32, color: u8) {
        if rx <= 0 || ry <= 0 {
            return;
        }
        let mut x = 0i32;
        let mut y = ry;
        let rx2 = (rx as i64) * (rx as i64);
        let ry2 = (ry as i64) * (ry as i64);
        let mut px = 0i64;
        let mut py = 2 * rx2 * y as i64;

        let mut p = ry2 - rx2 * ry as i64 + rx2 / 4;
        while px < py {
            self.put_pixel(buf, cx + x, cy + y, color);
            self.put_pixel(buf, cx - x, cy + y, color);
            self.put_pixel(buf, cx + x, cy - y, color);
            self.put_pixel(buf, cx - x, cy - y, color);
            x += 1;
            px += 2 * ry2;
            if p < 0 {
                p += ry2 + px;
            } else {
                y -= 1;
                py -= 2 * rx2;
                p += ry2 + px - py;
            }
        }

        p = ry2 * (x as i64 * 2 + 1) * (x as i64 * 2 + 1) / 4 + rx2 * (y as i64 - 1) * (y as i64 - 1) - rx2 * ry2;
        while y >= 0 {
            self.put_pixel(buf, cx + x, cy + y, color);
            self.put_pixel(buf, cx - x, cy + y, color);
            self.put_pixel(buf, cx + x, cy - y, color);
            self.put_pixel(buf, cx - x, cy - y, color);
            y -= 1;
            py -= 2 * rx2;
            if p > 0 {
                p += rx2 - py;
            } else {
                x += 1;
                px += 2 * ry2;
                p += rx2 - py + px;
            }
        }
    }

    pub fn fill_ellipse(&mut self, buf: &mut dyn EditableScreen, cx: i32, cy: i32, rx: i32, ry: i32, color: u8) {
        if rx <= 0 || ry <= 0 {
            return;
        }
        for y in -ry..=ry {
            let y_ratio = (y as f64) / (ry as f64);
            let x_extent = ((1.0 - y_ratio * y_ratio).sqrt() * rx as f64).round() as i32;
            for x in -x_extent..=x_extent {
                self.put_pixel(buf, cx + x, cy + y, color);
            }
        }
        self.ellipse(buf, cx, cy, rx, ry, color);
    }

    pub fn image(&self, buf: &dyn EditableScreen, x0: i32, y0: i32, x1: i32, y1: i32) -> Image {
        let mut data = Vec::new();
        for y in y0..y1 {
            for x in x0..x1 {
                data.push(self.pixel(buf, x, y));
            }
        }
        Image {
            width: x1 - x0,
            height: y1 - y0,
            data,
        }
    }

    pub fn put_image2(&mut self, buf: &mut dyn EditableScreen, src_x: i32, src_y: i32, width: i32, height: i32, dst_x: i32, dst_y: i32, image: &Image) {
        for iy in 0..height {
            if src_y + iy >= image.height {
                break;
            }
            for ix in 0..width {
                if src_x + ix >= image.width {
                    break;
                }
                let o = (src_x + ix + (src_y + iy) * image.width) as usize;
                if o < image.data.len() {
                    self.put_pixel(buf, dst_x + ix, dst_y + iy, image.data[o]);
                }
            }
        }
    }
}
