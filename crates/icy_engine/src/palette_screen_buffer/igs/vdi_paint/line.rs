use std::mem::swap;

use super::VdiPaint;
use crate::EditableScreen;

impl VdiPaint {
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
        let mask = self.line_kind.get_mask(self.line_user_mask);
        self.draw_line(buf, x1, y1, x2, y2, color, mask);
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
