use std::vec::Vec;

use icy_parser_core::LineKind;

use super::VdiPaint;
use crate::EditableScreen;

impl VdiPaint {
    pub(super) fn draw_poly(&mut self, buf: &mut dyn EditableScreen, parameters: &[i32], color: u8, close: bool) {
        let mut x = parameters[0];
        let mut y = parameters[1];
        let mask = self.line_kind.get_mask(self.line_user_mask);
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
        let mask = self.line_kind.get_mask(self.line_user_mask);
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

    pub fn draw_poly_marker(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32) {
        let points = self.polymarker_type.get_points();
        let num_lines = points[0];
        let mut i = 1;
        let old_type = self.line_kind;
        let scale = self.polymarker_size;
        self.line_kind = LineKind::Solid;
        for _ in 0..num_lines {
            let num_points = points[i] as usize;
            i += 1;
            let mut p = Vec::new();
            for _x in 0..num_points {
                p.push(scale * points[i] + x);
                i += 1;
                p.push(scale * points[i] + y);
                i += 1;
            }
            self.draw_polyline(buf, self.polymarker_color, &p);
        }
        self.line_kind = old_type;
    }
}
