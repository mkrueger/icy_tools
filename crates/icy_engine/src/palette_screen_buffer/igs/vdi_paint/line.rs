use std::mem::swap;

use icy_parser_core::{ArrowEnd, LineKind, PatternType, TerminalResolution};

use super::VdiPaint;
use crate::EditableScreen;

/// `a * b / c`, rounded the way VDI's `mult_div` rounds.
fn mult_div(a: i32, b: i32, c: i32) -> i32 {
    (a * b * 2 / c + 1) / 2
}

/// The offsets of a wide line's pen: `offsets[k]` is the half width of row `k` above and below
/// the center, averaged over the rows that a square pixel circle maps to (VDI's `cl_circ`).
fn quarter_circle(width: i32, xsize: i32, ysize: i32) -> Vec<i32> {
    let rows = (width * xsize / ysize) / 2 + 1;
    let radius = (width + 1) / 2;
    let mut circle = vec![0; (radius + 2).max(rows) as usize * 4];
    let (mut x, mut y) = (0, radius);
    let mut d = 3 - 2 * y;
    while x < y {
        circle[y as usize] = x;
        circle[x as usize] = y;
        if d < 0 {
            d += 4 * x + 6;
        } else {
            d += 4 * (x - y) + 10;
            y -= 1;
        }
        x += 1;
    }
    if x == y {
        circle[x as usize] = x;
    }
    let mut low = 0;
    let mut offsets = Vec::with_capacity(rows as usize);
    for i in 0..rows {
        let high = (((2 * i + 1) * ysize / xsize) / 2).max(low);
        let sum: i32 = (low..=high).map(|j| circle.get(j as usize).copied().unwrap_or(0)).sum();
        offsets.push(sum / (high - low + 1));
        low = high + 1;
    }
    offsets
}

fn quad_transform(quad: u8, x: i32, y: i32) -> (i32, i32) {
    (if matches!(quad, 1 | 4) { x } else { -x }, if matches!(quad, 1 | 2) { y } else { -y })
}

/// The point on the pen's quarter circle closest to the direction `(x, y)` (VDI's `perp_off`).
fn perpendicular_offset(x: i32, y: i32, offsets: &[i32]) -> (i32, i32) {
    let quad = match (x >= 0, y >= 0) {
        (true, true) => 1,
        (false, true) => 2,
        (false, false) => 3,
        (true, false) => 4,
    };
    let (x, y) = quad_transform(quad, x, y);
    let last = offsets.len() as i32 - 1;
    let (mut u, mut v, mut index) = (offsets[0], 0, 0);
    let (mut best, mut x_value, mut y_value): (i32, i32, i32) = (i32::MAX, 0, 0);
    loop {
        let magnitude = (u * y - v * x).abs();
        if magnitude < best || (magnitude == best && (x_value - y_value).abs() > (u - v).abs()) {
            (best, x_value, y_value) = (magnitude, u, v);
        } else {
            break;
        }
        if v == last {
            if u <= 1 {
                break;
            }
            u -= 1;
        } else if offsets[index + 1] >= u - 1 {
            v += 1;
            index += 1;
            u = offsets[index];
        } else {
            u -= 1;
        }
    }
    quad_transform(quad, x_value, y_value)
}

impl VdiPaint {
    /// Pixel width and height in micrometers, as VDI reports them for the resolution.
    fn pixel_size(&self) -> (i32, i32) {
        match self.terminal_resolution {
            TerminalResolution::Low => (338, 372),
            TerminalResolution::Medium => (169, 372),
            TerminalResolution::High => (372, 372),
        }
    }

    /// Fills a polygon and its outline in a solid `color`, as VDI fills wide lines and arrow
    /// heads.
    pub(super) fn fill_solid(&mut self, buf: &mut dyn EditableScreen, points: &[i32], color: u8) {
        if points.len() < 6 {
            return;
        }
        let saved = (self.fill_pattern_type, self.fill_color, self.fill_draw_border, self.line_kind);
        self.fill_pattern_type = PatternType::Solid;
        self.fill_color = color;
        self.fill_draw_border = true;
        self.line_kind = LineKind::Solid;
        self.fill_poly(buf, points);
        (self.fill_pattern_type, self.fill_color, self.fill_draw_border, self.line_kind) = saved;
    }

    /// The round pen of a wide line at `(x, y)` (VDI's `do_circ`).
    fn draw_pen(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, offsets: &[i32], color: u8) {
        for (row, offset) in offsets.iter().enumerate() {
            let row = row as i32;
            for y in if row == 0 { vec![y] } else { vec![y - row, y + row] } {
                for x in x - offset..=x + offset {
                    self.set_pixel_with_mode(buf, x, y, color, true);
                }
            }
        }
    }

    /// Draws connected lines with the line attributes: type, width (`vsl_width`) and end
    /// styles (`vsl_ends`).
    pub fn draw_styled_polyline(&mut self, buf: &mut dyn EditableScreen, parameters: &[i32], color: u8) {
        let mut points: Vec<(i32, i32)> = parameters.chunks_exact(2).map(|point| (point[0], point[1])).collect();
        if points.len() < 2 {
            return;
        }
        // VDI only draws solid lines wide, and only in odd widths.
        let width = if self.line_kind == LineKind::Solid {
            (self.line_thickness.max(1) - 1) / 2 * 2 + 1
        } else {
            1
        };
        let (start, end) = self.line_ends;
        if start == ArrowEnd::Arrow {
            self.draw_arrow_head(buf, &mut points, true, width, color);
        }
        if end == ArrowEnd::Arrow {
            self.draw_arrow_head(buf, &mut points, false, width, color);
        }
        if width <= 1 {
            let flat: Vec<i32> = points.iter().flat_map(|&(x, y)| [x, y]).collect();
            self.draw_polyline(buf, color, &flat);
            return;
        }
        self.draw_wide_polyline(buf, &points, width, color);
    }

    /// Each segment as a band filled between the pen offsets, with the pen drawn at joints and
    /// at ends that are not square (VDI's `wideline`).
    fn draw_wide_polyline(&mut self, buf: &mut dyn EditableScreen, points: &[(i32, i32)], width: i32, color: u8) {
        let (xsize, ysize) = self.pixel_size();
        let offsets = quarter_circle(width, xsize, ysize);
        let (start, end) = self.line_ends;
        let (mut x1, mut y1) = points[0];
        if start != ArrowEnd::Square {
            self.draw_pen(buf, x1, y1, &offsets, color);
        }
        for (index, &(x2, y2)) in points.iter().enumerate().skip(1) {
            let (mut vx, mut vy) = (x2 - x1, y2 - y1);
            if vx == 0 && vy == 0 {
                continue;
            }
            if vx == 0 {
                (vx, vy) = (offsets[0], 0);
            } else if vy == 0 {
                (vx, vy) = (0, offsets.len() as i32 - 1);
            } else {
                let perpendicular_x = mult_div(-vy, ysize, xsize);
                vy = mult_div(vx, xsize, ysize);
                (vx, vy) = perpendicular_offset(perpendicular_x, vy, &offsets);
            }
            let band = [x1 + vx, y1 + vy, x1 - vx, y1 - vy, x2 - vx, y2 - vy, x2 + vx, y2 + vy];
            self.fill_solid(buf, &band, color);
            if index < points.len() - 1 || end != ArrowEnd::Square {
                self.draw_pen(buf, x2, y2, &offsets, color);
            }
            (x1, y1) = (x2, y2);
        }
    }

    /// Draws an arrow head at the start or end of `points` and moves that end back to the
    /// base of the head, the way VDI draws arrow line ends.
    fn draw_arrow_head(&mut self, buf: &mut dyn EditableScreen, points: &mut [(i32, i32)], at_start: bool, width: i32, color: u8) {
        let (xsize, ysize) = self.pixel_size();
        let arrow_length = if width < 4 { 8 } else { 3 * width - 1 };
        let arrow_width = arrow_length / 2;
        let last = points.len() - 1;
        let tip_index = if at_start { 0 } else { last };
        let (tip_x, tip_y) = points[tip_index];
        let candidates: Vec<usize> = if at_start { (1..=last).collect() } else { (0..last).rev().collect() };
        // The first point far enough from the tip not to be covered by the head, measured in a
        // space with square pixels.
        let Some((reached, dx, dy, length)) = candidates.into_iter().find_map(|index| {
            let dx = tip_x - points[index].0;
            let dy = mult_div(tip_y - points[index].1, ysize, xsize);
            let length = f64::from(dx).hypot(f64::from(dy)) as i32;
            (length >= arrow_length).then_some((index, dx, dy, length))
        }) else {
            return;
        };
        let height_x = mult_div(arrow_length, mult_div(dx, 1000, length), 1000);
        let height_y = mult_div(mult_div(arrow_length, mult_div(dy, 1000, length), 1000), xsize, ysize);
        let base_x = mult_div(arrow_width, mult_div(dy, -1000, length), 1000);
        let base_y = mult_div(mult_div(arrow_width, mult_div(dx, 1000, length), 1000), xsize, ysize);
        let head = [
            tip_x + base_x - height_x,
            tip_y + base_y - height_y,
            tip_x - base_x - height_x,
            tip_y - base_y - height_y,
            tip_x,
            tip_y,
        ];
        self.fill_solid(buf, &head, color);
        let base = (tip_x - height_x, tip_y - height_y);
        let covered = if at_start { 0..reached } else { reached + 1..last + 1 };
        for point in &mut points[covered] {
            *point = base;
        }
    }

    pub(super) fn draw_vline(&mut self, buf: &mut dyn EditableScreen, x: i32, mut y0: i32, mut y1: i32, color: u8, mask: u16) {
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

    pub(super) fn draw_hline(&mut self, buf: &mut dyn EditableScreen, y: i32, x0: i32, x1: i32, color: u8, mask: u16) {
        let mut line_mask = mask;
        line_mask = line_mask.rotate_left((x0 & 0x0f) as u32);
        for x in x0..=x1 {
            line_mask = line_mask.rotate_left(1);
            if 1 & line_mask != 0 {
                self.set_pixel(buf, x, y, color);
            }
        }
    }

    pub fn draw_line_pub(&mut self, buf: &mut dyn crate::EditableScreen, x1: i32, y1: i32, x2: i32, y2: i32) {
        let color = self.line_color;
        self.draw_styled_polyline(buf, &[x1, y1, x2, y2], color);
    }

    pub(super) fn draw_line(&mut self, buf: &mut dyn EditableScreen, mut x0: i32, mut y0: i32, mut x1: i32, mut y1: i32, color: u8, mask: u16) {
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
}
