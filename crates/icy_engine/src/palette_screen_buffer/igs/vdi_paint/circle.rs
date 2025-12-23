use icy_parser_core::TerminalResolution;

use super::super::util::{gdp_curve, TWOPI};
use super::VdiPaint;
use crate::EditableScreen;

impl VdiPaint {
    pub(super) fn calc_circle_y_radius(&self, radius: i32) -> i32 {
        let (xsize, ysize) = match self.terminal_resolution {
            TerminalResolution::Low => (338, 372),
            TerminalResolution::Medium => (169, 372),
            TerminalResolution::High => (372, 372),
        };

        (radius * xsize) / ysize
    }

    fn fill_circle(&mut self, buf: &mut dyn EditableScreen, center_x: i32, center_y: i32, radius: i32) {
        let y_rad = self.calc_circle_y_radius(radius);
        let points: Vec<i32> = gdp_curve(center_x, center_y, radius, y_rad, 0, TWOPI as i32);
        self.fill_poly(buf, &points);
    }

    fn draw_circle_border(&mut self, buf: &mut dyn EditableScreen, center_x: i32, center_y: i32, radius: i32, color: u8) {
        let y_rad = self.calc_circle_y_radius(radius);
        let points: Vec<i32> = gdp_curve(center_x, center_y, radius, y_rad, 0, TWOPI as i32);
        self.draw_poly(buf, &points, color, false);
    }

    fn draw_ellipse_border(&mut self, buf: &mut dyn EditableScreen, center_x: i32, center_y: i32, radius_x: i32, radius_y: i32, color: u8) {
        let points: Vec<i32> = gdp_curve(center_x, center_y, radius_x, radius_y, 0, TWOPI as i32);
        self.draw_poly(buf, &points, color, false);
    }

    fn draw_elliptical_pieslice_border(
        &mut self,
        buf: &mut dyn EditableScreen,
        center_x: i32,
        center_y: i32,
        radius_x: i32,
        radius_y: i32,
        start_angle: i32,
        end_angle: i32,
    ) {
        let radius_y = self.calc_circle_y_radius(radius_y);
        let mut points = gdp_curve(center_x, center_y, radius_x, radius_y, start_angle * 10, end_angle * 10);
        points.extend_from_slice(&[center_x, center_y]);
        self.draw_poly(buf, &points, self.fill_color, true);
    }

    fn fill_elliptical_pieslice(
        &mut self,
        buf: &mut dyn EditableScreen,
        center_x: i32,
        center_y: i32,
        radius_x: i32,
        radius_y: i32,
        start_angle: i32,
        end_angle: i32,
    ) {
        let radius_y = self.calc_circle_y_radius(radius_y);
        let mut points = gdp_curve(center_x, center_y, radius_x, radius_y, start_angle * 10, end_angle * 10);
        points.extend_from_slice(&[center_x, center_y]);
        self.fill_poly(buf, &points);
    }

    fn draw_pieslice_border(&mut self, buf: &mut dyn EditableScreen, center_x: i32, center_y: i32, radius: i32, start_angle: i32, end_angle: i32) {
        let yr = self.calc_circle_y_radius(radius);
        let mut points = gdp_curve(center_x, center_y, radius, yr, start_angle * 10, end_angle * 10);
        points.extend_from_slice(&[center_x, center_y]);
        self.draw_poly(buf, &points, self.fill_color, true);
    }

    fn fill_pieslice(&mut self, buf: &mut dyn EditableScreen, center_x: i32, center_y: i32, radius: i32, start_angle: i32, end_angle: i32) {
        let yr = self.calc_circle_y_radius(radius);
        let mut points = gdp_curve(center_x, center_y, radius, yr, start_angle * 10, end_angle * 10);
        points.extend_from_slice(&[center_x, center_y]);
        self.fill_poly(buf, &points);
    }

    fn fill_ellipse(&mut self, buf: &mut dyn EditableScreen, center_x: i32, center_y: i32, radius_x: i32, radius_y: i32) {
        let points: Vec<i32> = gdp_curve(center_x, center_y, radius_x, radius_y, 0, TWOPI as i32);
        self.fill_poly(buf, &points);
    }

    pub fn draw_arc(&mut self, buf: &mut dyn EditableScreen, center_x: i32, center_y: i32, radius_x: i32, radius_y: i32, start_angle: i32, end_angle: i32) {
        let points = gdp_curve(center_x, center_y, radius_x, radius_y, start_angle * 10, end_angle * 10);
        self.draw_poly(buf, &points, self.line_color, false);
    }

    pub fn draw_circle_pub(&mut self, buf: &mut dyn crate::EditableScreen, center_x: i32, center_y: i32, radius: i32) {
        self.fill_circle(buf, center_x, center_y, radius);
        if self.fill_draw_border {
            self.draw_circle_border(buf, center_x, center_y, radius, self.fill_color);
        }
    }

    pub fn draw_ellipse_pub(&mut self, buf: &mut dyn crate::EditableScreen, center_x: i32, center_y: i32, radius_x: i32, radius_y: i32) {
        self.fill_ellipse(buf, center_x, center_y, radius_x, radius_y);
        if self.fill_draw_border {
            self.draw_ellipse_border(buf, center_x, center_y, radius_x, radius_y, self.fill_color);
        }
    }

    pub fn draw_arc_pub(
        &mut self,
        buf: &mut dyn crate::EditableScreen,
        center_x: i32,
        center_y: i32,
        radius_x: i32,
        radius_y: i32,
        start_angle: i32,
        end_angle: i32,
    ) {
        self.draw_arc(buf, center_x, center_y, radius_x, radius_y, start_angle, end_angle);
    }

    pub fn draw_pieslice_pub(&mut self, buf: &mut dyn crate::EditableScreen, center_x: i32, center_y: i32, radius: i32, start_angle: i32, end_angle: i32) {
        self.fill_pieslice(buf, center_x, center_y, radius, start_angle, end_angle);

        if self.fill_draw_border {
            self.draw_pieslice_border(buf, center_x, center_y, radius, start_angle, end_angle);
        }
    }

    pub fn draw_elliptical_pieslice_pub(
        &mut self,
        buf: &mut dyn crate::EditableScreen,
        center_x: i32,
        center_y: i32,
        radius_x: i32,
        radius_y: i32,
        start_angle: i32,
        end_angle: i32,
    ) {
        self.fill_elliptical_pieslice(buf, center_x, center_y, radius_x, radius_y, start_angle, end_angle);
        if self.fill_draw_border {
            self.draw_elliptical_pieslice_border(buf, center_x, center_y, radius_x, radius_y, start_angle, end_angle);
        }
    }
}
