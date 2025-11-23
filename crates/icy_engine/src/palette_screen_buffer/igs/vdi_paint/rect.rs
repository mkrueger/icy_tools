use std::mem::swap;

use super::VdiPaint;
use crate::EditableScreen;

impl VdiPaint {
    fn draw_rect_border(&mut self, buf: &mut dyn EditableScreen, mut left: i32, mut top: i32, mut right: i32, mut bottom: i32, color: u8) {
        if top > bottom {
            swap(&mut top, &mut bottom);
        }
        if left > right {
            swap(&mut left, &mut right);
        }
        let mask = 0xFFFF;

        // Top horizontal line
        self.draw_hline(buf, top, left, right, color, mask);

        // Bottom horizontal line
        if bottom != top {
            self.draw_hline(buf, bottom, left, right, color, mask);
        }

        // Left vertical line (excluding corners to avoid overdraw)
        if bottom - top > 1 {
            self.draw_vline(buf, left, top + 1, bottom - 1, color, mask);
        }

        // Right vertical line (excluding corners to avoid overdraw)
        if right != left && bottom - top > 1 {
            self.draw_vline(buf, right, top + 1, bottom - 1, color, mask);
        }
    }

    pub fn fill_rect(&mut self, buf: &mut dyn EditableScreen, mut left: i32, mut top: i32, mut right: i32, mut bottom: i32) {
        if top > bottom {
            swap(&mut top, &mut bottom);
        }
        if left > right {
            swap(&mut left, &mut right);
        }

        for y in top..=bottom {
            for x in left..=right {
                self.fill_pixel(buf, x, y);
            }
        }
    }

    fn draw_round_rect_border(&mut self, buf: &mut dyn EditableScreen, mut left: i32, mut top: i32, mut right: i32, mut bottom: i32, filled: bool) {
        let mut points = Vec::new();
        if left > right {
            swap(&mut left, &mut right);
        }
        if top < bottom {
            swap(&mut top, &mut bottom);
        }

        let x_radius = ((buf.get_resolution().width >> 6).min((right - left) / 2) - 1).max(0);
        let y_radius = self.calc_circle_y_radius(x_radius).min((top - bottom) / 2);

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
        let xc = right - x_radius;
        let yc = bottom + y_radius;

        // upper right
        for i in 0..x_off.len() {
            points.push(xc + x_off[i]);
            points.push(yc - y_off[i]);
        }

        // lower right
        let yc = top - y_radius;
        for i in 0..x_off.len() {
            points.push(xc + x_off[4 - i]);
            points.push(yc + y_off[4 - i]);
        }

        // lower left
        let xc = left + x_radius;
        for i in 0..x_off.len() {
            points.push(xc - x_off[i]);
            points.push(yc + y_off[i]);
        }

        // upper left
        let yc = bottom + y_radius;
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

    pub fn draw_rect_pub(&mut self, buf: &mut dyn crate::EditableScreen, left: i32, top: i32, right: i32, bottom: i32) {
        self.fill_rect(buf, left, top, right, bottom);

        if self.fill_draw_border {
            self.draw_rect_border(buf, left, top, right, bottom, self.fill_color);
        }
    }

    pub fn draw_rounded_rect(&mut self, buf: &mut dyn crate::EditableScreen, left: i32, top: i32, right: i32, bottom: i32) {
        self.draw_round_rect_border(buf, left, top, right, bottom, true);
        if self.fill_draw_border {
            self.draw_round_rect_border(buf, left, top, right, bottom, false);
        }
    }
}
