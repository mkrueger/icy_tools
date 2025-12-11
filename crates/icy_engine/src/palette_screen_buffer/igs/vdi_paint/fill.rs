use std::collections::HashSet;

use super::VdiPaint;
use crate::{EditableScreen, Position};

impl VdiPaint {
    pub fn fill_poly(&mut self, buf: &mut dyn EditableScreen, points: &[i32]) {
        const MAX_VERTICES: usize = 512;

        let mut y_max = points[1];
        let mut y_min = points[1];

        let mut i = 3;
        while i < points.len() {
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

                // Calculate deltas of each endpoint with current scan line.
                let dy1 = (y - y1) as i32;
                let dy2 = (y - y2) as i32;

                // Determine whether the current vector intersects with the scan line.
                if (dy1 ^ dy2) < 0 {
                    let x1 = points[i * 2];
                    let x2 = points[next_point * 2];

                    // Calculate X delta of current vector
                    let dx = (x2 - x1) << 1; // Left shift so we can round by adding 1 below

                    // Stop if we have reached the max number of vertices allowed (512)
                    if edge_buffer.len() >= MAX_VERTICES {
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

            if edge_buffer.len() < 2 {
                continue;
            }

            // Sort the X-coordinates, so they are arranged left to right.
            edge_buffer.sort_by(|a, b| a.partial_cmp(b).unwrap());

            // Loop through all edges in pairs, filling the pixels in between.
            let mut j = 0;
            while j < edge_buffer.len() {
                let x1 = edge_buffer[j] as i32;
                j += 1;
                let x2 = edge_buffer[j] as i32;
                j += 1;

                for k in x1..=x2 {
                    self.fill_pixel(buf, k, y);
                }
            }
        }
        if self.fill_draw_border {
            self.draw_poly(buf, points, self.fill_color, true);
        }
    }

    pub fn flood_fill(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32) {
        let res = buf.resolution();

        if x < 0 || y < 0 || x >= res.width || y >= res.height {
            return;
        }
        let old_px = self.pixel(buf, x, y);

        let mut vec = vec![Position::new(x, y)];
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

            let cp = self.pixel(buf, pos.x, pos.y);
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
}
