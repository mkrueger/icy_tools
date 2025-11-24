use icy_parser_core::BlitMode;

use super::VdiPaint;
use crate::{EditableScreen, Position};

#[derive(Debug, Clone)]
pub struct BlitRegion {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl BlitRegion {
    pub fn from_corners(p1: Position, p2: Position) -> Self {
        let x1 = p1.x.min(p2.x) as usize;
        let y1 = p1.y.min(p2.y) as usize;
        let x2 = p1.x.max(p2.x) as usize;
        let y2 = p1.y.max(p2.y) as usize;

        Self {
            x: x1,
            y: y1,
            width: (x2 - x1) + 1,
            height: (y2 - y1) + 1,
        }
    }

    pub fn clip_to_bounds(&mut self, screen_width: usize, screen_height: usize) -> bool {
        if self.x >= screen_width || self.y >= screen_height {
            return false;
        }

        if self.x + self.width > screen_width {
            self.width = screen_width - self.x;
        }
        if self.y + self.height > screen_height {
            self.height = screen_height - self.y;
        }

        self.width > 0 && self.height > 0
    }

    pub fn adjust_for_negative_dest(&mut self, dest: &mut Position) -> bool {
        if dest.x < 0 {
            let offset = (-dest.x) as usize;
            if offset >= self.width {
                return false;
            }
            self.x += offset;
            self.width -= offset;
            dest.x = 0;
        }

        if dest.y < 0 {
            let offset = (-dest.y) as usize;
            if offset >= self.height {
                return false;
            }
            self.y += offset;
            self.height -= offset;
            dest.y = 0;
        }

        true
    }
}

#[derive(Debug, Clone)]
pub struct BlitSurface {
    data: Vec<u8>,
    width: usize,
    height: usize,
}

impl BlitSurface {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            data: Vec::with_capacity(width * height),
            width,
            height,
        }
    }

    pub fn from_screen(screen: &[u8], region: &BlitRegion, pitch: usize) -> Self {
        let mut buffer = Self::new(region.width, region.height);

        for y in 0..region.height {
            let src_offset = (region.y + y) * pitch + region.x;
            let src_end = src_offset + region.width;
            buffer.data.extend_from_slice(&screen[src_offset..src_end]);
        }

        buffer
    }

    #[allow(dead_code)]
    fn get_pixel(&self, x: usize, y: usize) -> u8 {
        self.data[y * self.width + x]
    }
}

fn blit_px(blit_mode: BlitMode, s: u8, d: u8) -> u8 {
    let dest = match blit_mode {
        BlitMode::Clear => 0,
        BlitMode::And => s & d,
        BlitMode::AndNot => s & !d,
        BlitMode::Replace => s,
        BlitMode::Erase => !s & d,
        BlitMode::Unchanged => d,
        BlitMode::Xor => s ^ d,
        BlitMode::Transparent => s | d,
        BlitMode::NotOr => !(s | d),
        BlitMode::NotXor => !(s ^ d),
        BlitMode::NotD => !d,
        BlitMode::OrNot => s | !d,
        BlitMode::NotS => !s,
        BlitMode::ReverseTransparent => !s | d,
        BlitMode::NotAnd => !(s & d),
        BlitMode::Fill => 1,
    };

    dest & 0xF
}

impl VdiPaint {
    fn copy_buffer_region(src_buffer: &[u8], src_width: usize, src_height: usize, region: &BlitRegion) -> Option<BlitSurface> {
        let mut region = region.clone();

        if !region.clip_to_bounds(src_width, src_height) {
            return None;
        }

        Some(BlitSurface::from_screen(src_buffer, &region, src_width))
    }

    fn blit_to_buffer(
        src_buffer: &BlitSurface,
        mut src_region: BlitRegion,
        dest_buffer: &mut [u8],
        dest_width: usize,
        dest_height: usize,
        mut dest_pos: Position,
        blit_mode: BlitMode,
    ) {
        if !src_region.adjust_for_negative_dest(&mut dest_pos) {
            return;
        }

        // Clip source region to source buffer bounds
        if !src_region.clip_to_bounds(src_buffer.width, src_buffer.height) {
            return;
        }

        let dest_x = dest_pos.x as usize;
        let dest_y = dest_pos.y as usize;

        if dest_x >= dest_width || dest_y >= dest_height {
            return;
        }

        let copy_width = src_region.width.min(dest_width - dest_x);
        let copy_height = src_region.height.min(dest_height - dest_y);

        for y in 0..copy_height {
            for x in 0..copy_width {
                let src_offset = (src_region.y + y) * src_buffer.width + (src_region.x + x);
                let dest_offset = (dest_y + y) * dest_width + (dest_x + x);

                let src_color = src_buffer.data[src_offset];
                let dest_color = dest_buffer[dest_offset];

                dest_buffer[dest_offset] = blit_px(blit_mode, src_color, dest_color);
            }
        }
    }

    pub fn blit_screen_to_screen(&mut self, buf: &mut dyn EditableScreen, blit_mode: BlitMode, from: Position, to: Position, dest: Position) {
        let src_region = BlitRegion::from_corners(from, to);

        if let Some(buffer) = self.copy_region_to_buffer(buf, &src_region) {
            self.blit_buffer_to_screen(buf, &buffer, dest, blit_mode);
        }
    }

    pub fn blit_screen_to_memory(&mut self, buf: &mut dyn EditableScreen, _blit_mode: BlitMode, from: Position, to: Position) {
        let region = BlitRegion::from_corners(from, to);

        if let Some(buffer) = self.copy_region_to_buffer(buf, &region) {
            self.blit_buffer = buffer;
        }
    }

    pub fn blit_memory_to_screen(&mut self, buf: &mut dyn EditableScreen, blit_mode: BlitMode, dest: Position) {
        let buffer = self.blit_buffer.clone();
        self.blit_buffer_to_screen(buf, &buffer, dest, blit_mode);
    }

    fn copy_region_to_buffer(&self, buf: &dyn EditableScreen, region: &BlitRegion) -> Option<BlitSurface> {
        let res = buf.get_resolution();
        let mut region = region.clone();

        if !region.clip_to_bounds(res.width as usize, res.height as usize) {
            return None;
        }

        Some(BlitSurface::from_screen(buf.screen(), &region, res.width as usize))
    }

    fn blit_buffer_to_screen(&self, buf: &mut dyn EditableScreen, buffer: &BlitSurface, dest: Position, blit_mode: BlitMode) {
        let res = buf.get_resolution();
        let screen_width = res.width as usize;
        let screen_height = res.height as usize;

        let src_region = BlitRegion {
            x: 0,
            y: 0,
            width: buffer.width,
            height: buffer.height,
        };

        Self::blit_to_buffer(buffer, src_region, buf.screen_mut(), screen_width, screen_height, dest, blit_mode);
    }

    pub fn blit_piece_of_memory_to_screen(&self, buf: &mut dyn EditableScreen, blit_mode: BlitMode, from: Position, to: Position, dest: Position) {
        let buffer = &self.blit_buffer;
        let region = BlitRegion::from_corners(from, to);

        let res = buf.get_resolution();
        let screen_width = res.width as usize;
        let screen_height = res.height as usize;

        Self::blit_to_buffer(buffer, region, buf.screen_mut(), screen_width, screen_height, dest, blit_mode);
    }

    pub fn blit_memory_to_memory(&mut self, blit_mode: BlitMode, from: Position, to: Position, dest: Position) {
        let region = BlitRegion::from_corners(from, to);

        let temp_buffer = if let Some(buf) = Self::copy_buffer_region(&self.blit_buffer.data, self.blit_buffer.width, self.blit_buffer.height, &region) {
            buf
        } else {
            return;
        };

        let src_region = BlitRegion {
            x: 0,
            y: 0,
            width: temp_buffer.width,
            height: temp_buffer.height,
        };

        Self::blit_to_buffer(
            &temp_buffer,
            src_region,
            &mut self.blit_buffer.data,
            self.blit_buffer.width,
            self.blit_buffer.height,
            dest,
            blit_mode,
        );
    }

    pub fn blit_between_regions(&mut self, buf: &mut dyn EditableScreen, from: Position, to: Position, dest: Position, blit_mode: BlitMode) {
        let region = BlitRegion::from_corners(from, to);

        if let Some(temp_buffer) = Self::copy_buffer_region(&self.blit_buffer.data, self.blit_buffer.width, self.blit_buffer.height, &region) {
            self.blit_buffer_to_screen(buf, &temp_buffer, dest, blit_mode);
        }
    }
}
