//! Block elements and box drawing drawn to fill the cell exactly, as terminals do.
//!
//! Font glyphs for these characters follow each font's own metrics, so blocks leave
//! gaps and lines do not meet the neighbouring cells. Shades use the classic VGA
//! dither patterns on the 8×16 logical grid of the cell.

use tiny_skia::{Paint, PathBuilder, Pixmap, Stroke, Transform};

const NONE: u8 = 0;
const LIGHT: u8 = 1;
const HEAVY: u8 = 2;
const DOUBLE: u8 = 3;

/// Arms of a box-drawing character in the order up, right, down, left.
type Arms = [u8; 4];

const UP: usize = 0;
const RIGHT: usize = 1;
const DOWN: usize = 2;
const LEFT: usize = 3;

/// U+250C..=U+254B: corners, tees and crosses in light and heavy combinations.
const LIGHT_HEAVY_JOINTS: [Arms; 64] = [
    [0, 1, 1, 0], // ┌
    [0, 2, 1, 0], // ┍
    [0, 1, 2, 0], // ┎
    [0, 2, 2, 0], // ┏
    [0, 0, 1, 1], // ┐
    [0, 0, 1, 2], // ┑
    [0, 0, 2, 1], // ┒
    [0, 0, 2, 2], // ┓
    [1, 1, 0, 0], // └
    [1, 2, 0, 0], // ┕
    [2, 1, 0, 0], // ┖
    [2, 2, 0, 0], // ┗
    [1, 0, 0, 1], // ┘
    [1, 0, 0, 2], // ┙
    [2, 0, 0, 1], // ┚
    [2, 0, 0, 2], // ┛
    [1, 1, 1, 0], // ├
    [1, 2, 1, 0], // ┝
    [2, 1, 1, 0], // ┞
    [1, 1, 2, 0], // ┟
    [2, 1, 2, 0], // ┠
    [2, 2, 1, 0], // ┡
    [1, 2, 2, 0], // ┢
    [2, 2, 2, 0], // ┣
    [1, 0, 1, 1], // ┤
    [1, 0, 1, 2], // ┥
    [2, 0, 1, 1], // ┦
    [1, 0, 2, 1], // ┧
    [2, 0, 2, 1], // ┨
    [2, 0, 1, 2], // ┩
    [1, 0, 2, 2], // ┪
    [2, 0, 2, 2], // ┫
    [0, 1, 1, 1], // ┬
    [0, 1, 1, 2], // ┭
    [0, 2, 1, 1], // ┮
    [0, 2, 1, 2], // ┯
    [0, 1, 2, 1], // ┰
    [0, 1, 2, 2], // ┱
    [0, 2, 2, 1], // ┲
    [0, 2, 2, 2], // ┳
    [1, 1, 0, 1], // ┴
    [1, 1, 0, 2], // ┵
    [1, 2, 0, 1], // ┶
    [1, 2, 0, 2], // ┷
    [2, 1, 0, 1], // ┸
    [2, 1, 0, 2], // ┹
    [2, 2, 0, 1], // ┺
    [2, 2, 0, 2], // ┻
    [1, 1, 1, 1], // ┼
    [1, 1, 1, 2], // ┽
    [1, 2, 1, 1], // ┾
    [1, 2, 1, 2], // ┿
    [2, 1, 1, 1], // ╀
    [1, 1, 2, 1], // ╁
    [2, 1, 2, 1], // ╂
    [2, 1, 1, 2], // ╃
    [2, 2, 1, 1], // ╄
    [1, 1, 2, 2], // ╅
    [1, 2, 2, 1], // ╆
    [2, 2, 1, 2], // ╇
    [1, 2, 2, 2], // ╈
    [2, 1, 2, 2], // ╉
    [2, 2, 2, 1], // ╊
    [2, 2, 2, 2], // ╋
];

/// U+2550..=U+256C: double lines and their joints with single lines.
const DOUBLE_JOINTS: [Arms; 29] = [
    [0, 3, 0, 3], // ═
    [3, 0, 3, 0], // ║
    [0, 3, 1, 0], // ╒
    [0, 1, 3, 0], // ╓
    [0, 3, 3, 0], // ╔
    [0, 0, 1, 3], // ╕
    [0, 0, 3, 1], // ╖
    [0, 0, 3, 3], // ╗
    [1, 3, 0, 0], // ╘
    [3, 1, 0, 0], // ╙
    [3, 3, 0, 0], // ╚
    [1, 0, 0, 3], // ╛
    [3, 0, 0, 1], // ╜
    [3, 0, 0, 3], // ╝
    [1, 3, 1, 0], // ╞
    [3, 1, 3, 0], // ╟
    [3, 3, 3, 0], // ╠
    [1, 0, 1, 3], // ╡
    [3, 0, 3, 1], // ╢
    [3, 0, 3, 3], // ╣
    [0, 3, 1, 3], // ╤
    [0, 1, 3, 1], // ╥
    [0, 3, 3, 3], // ╦
    [1, 3, 0, 3], // ╧
    [3, 1, 0, 1], // ╨
    [3, 3, 0, 3], // ╩
    [1, 3, 1, 3], // ╪
    [3, 1, 3, 1], // ╫
    [3, 3, 3, 3], // ╬
];

fn box_arms(ch: char) -> Option<Arms> {
    Some(match ch {
        '─' => [0, 1, 0, 1],
        '━' => [0, 2, 0, 2],
        '│' => [1, 0, 1, 0],
        '┃' => [2, 0, 2, 0],
        '\u{250C}'..='\u{254B}' => LIGHT_HEAVY_JOINTS[ch as usize - 0x250C],
        '\u{2550}'..='\u{256C}' => DOUBLE_JOINTS[ch as usize - 0x2550],
        '╴' => [0, 0, 0, 1],
        '╵' => [1, 0, 0, 0],
        '╶' => [0, 1, 0, 0],
        '╷' => [0, 0, 1, 0],
        '╸' => [0, 0, 0, 2],
        '╹' => [2, 0, 0, 0],
        '╺' => [0, 2, 0, 0],
        '╻' => [0, 0, 2, 0],
        '╼' => [0, 2, 0, 1],
        '╽' => [1, 0, 2, 0],
        '╾' => [0, 1, 0, 2],
        '╿' => [2, 0, 1, 0],
        _ => return None,
    })
}

/// Dashed lines: (vertical, heavy, segments).
fn dashes(ch: char) -> Option<(bool, bool, i32)> {
    Some(match ch {
        '┄' => (false, false, 3),
        '┅' => (false, true, 3),
        '┆' => (true, false, 3),
        '┇' => (true, true, 3),
        '┈' => (false, false, 4),
        '┉' => (false, true, 4),
        '┊' => (true, false, 4),
        '┋' => (true, true, 4),
        '╌' => (false, false, 2),
        '╍' => (false, true, 2),
        '╎' => (true, false, 2),
        '╏' => (true, true, 2),
        _ => return None,
    })
}

struct Mask {
    width: i32,
    height: i32,
    data: Vec<u8>,
}

impl Mask {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width: width as i32,
            height: height as i32,
            data: vec![0; width as usize * height as usize],
        }
    }

    fn fill(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let (x0, x1) = (x0.clamp(0, self.width), x1.clamp(0, self.width));
        let (y0, y1) = (y0.clamp(0, self.height), y1.clamp(0, self.height));
        if x0 >= x1 {
            return;
        }
        for y in y0..y1 {
            self.data[(y * self.width + x0) as usize..(y * self.width + x1) as usize].fill(255);
        }
    }
}

/// `value * numerator / denominator`, rounded.
fn part(value: i32, numerator: i32, denominator: i32) -> i32 {
    (value * numerator + denominator / 2) / denominator
}

/// The pixel band of a line of `size` pixels centered on `center`.
fn band(center: i32, size: i32) -> (i32, i32) {
    (center - size / 2, center - size / 2 + size)
}

/// A cell-filling mask for block elements and box drawing, or `None` to use a font.
pub(super) fn glyph(text: &str, width: u32, height: u32) -> Option<Vec<u8>> {
    let mut chars = text.chars();
    let ch = chars.next()?;
    if chars.next().is_some() || width == 0 || height == 0 {
        return None;
    }
    let mut mask = Mask::new(width, height);
    if block(&mut mask, ch) || box_drawing(&mut mask, ch) || stroked(&mut mask, ch) {
        Some(mask.data)
    } else {
        None
    }
}

fn block(mask: &mut Mask, ch: char) -> bool {
    let (w, h) = (mask.width, mask.height);
    let (hw, hh) = (part(w, 1, 2), part(h, 1, 2));
    match ch {
        '▀' => mask.fill(0, 0, w, hh),
        '\u{2581}'..='\u{2587}' => mask.fill(0, h - part(h, ch as i32 - 0x2580, 8), w, h),
        '█' => mask.fill(0, 0, w, h),
        '\u{2589}'..='\u{258F}' => mask.fill(0, 0, part(w, 0x2590 - ch as i32, 8), h),
        '▐' => mask.fill(hw, 0, w, h),
        '░' | '▒' | '▓' => {
            for y in 0..h {
                let row = y * 16 / h;
                for x in 0..w {
                    let column = x * 8 / w;
                    let sparse = if row % 2 == 0 { column % 4 == 2 } else { column % 4 == 0 };
                    let on = match ch {
                        '░' => sparse,
                        '▒' => (row + column) % 2 == 0,
                        _ => !sparse,
                    };
                    if on {
                        mask.data[(y * w + x) as usize] = 255;
                    }
                }
            }
        }
        '▔' => mask.fill(0, 0, w, part(h, 1, 8)),
        '▕' => mask.fill(w - part(w, 1, 8), 0, w, h),
        '\u{2596}'..='\u{259F}' => {
            // Quadrants as bits: upper left, upper right, lower left, lower right.
            let quadrants: u8 = match ch {
                '▖' => 0b0010,
                '▗' => 0b0001,
                '▘' => 0b1000,
                '▙' => 0b1011,
                '▚' => 0b1001,
                '▛' => 0b1110,
                '▜' => 0b1101,
                '▝' => 0b0100,
                '▞' => 0b0110,
                _ => 0b0111,
            };
            for (bit, (x0, y0, x1, y1)) in [
                (0b1000, (0, 0, hw, hh)),
                (0b0100, (hw, 0, w, hh)),
                (0b0010, (0, hh, hw, h)),
                (0b0001, (hw, hh, w, h)),
            ] {
                if quadrants & bit != 0 {
                    mask.fill(x0, y0, x1, y1);
                }
            }
        }
        _ => return false,
    }
    true
}

/// Line geometry shared by all box-drawing characters of one cell size.
struct Lines {
    light: i32,
    double_offset: i32,
    cx: i32,
    cy: i32,
}

impl Lines {
    fn of(mask: &Mask) -> Self {
        let light = part(mask.width, 1, 8).max(1);
        Self {
            light,
            double_offset: light,
            cx: mask.width / 2,
            cy: mask.height / 2,
        }
    }

    fn thickness(&self, style: u8) -> i32 {
        if style == HEAVY {
            self.light * 2
        } else {
            self.light
        }
    }
}

fn box_drawing(mask: &mut Mask, ch: char) -> bool {
    if let Some((vertical, heavy, segments)) = dashes(ch) {
        let lines = Lines::of(mask);
        let thickness = lines.thickness(if heavy { HEAVY } else { LIGHT });
        let length = if vertical { mask.height } else { mask.width };
        let gap = part(length, 1, segments * 4).max(1);
        for segment in 0..segments {
            let start = part(length, segment, segments) + gap / 2;
            let end = part(length, segment + 1, segments) - (gap - gap / 2);
            if vertical {
                let (x0, x1) = band(lines.cx, thickness);
                mask.fill(x0, start, x1, end);
            } else {
                let (y0, y1) = band(lines.cy, thickness);
                mask.fill(start, y0, end, y1);
            }
        }
        return true;
    }
    let Some(arms) = box_arms(ch) else {
        return false;
    };
    let lines = Lines::of(mask);
    for direction in [UP, RIGHT, DOWN, LEFT] {
        if arms[direction] != NONE {
            draw_arm(mask, &lines, &arms, direction);
        }
    }
    true
}

/// Draws one arm from the cell edge towards the center, joining the other arms like
/// the VGA box-drawing glyphs: double lines meet in corners, single lines touch them.
fn draw_arm(mask: &mut Mask, lines: &Lines, arms: &Arms, direction: usize) {
    let vertical = direction == UP || direction == DOWN;
    let from_start = direction == UP || direction == LEFT;
    let (center, across, extent) = if vertical {
        (lines.cy, lines.cx, mask.height)
    } else {
        (lines.cx, lines.cy, mask.width)
    };
    // The sides of an arm, on the lower and the higher coordinate across it.
    let (low_side, high_side) = if vertical { (LEFT, RIGHT) } else { (UP, DOWN) };
    let opposite = (direction + 2) % 4;
    let has = |arm: usize| arms[arm] != NONE;
    let d = lines.double_offset;
    let t = lines.light;
    let toward_far = if from_start { d } else { -d };
    let near = band(center - toward_far, t);
    let far = band(center + toward_far, t);
    let center_band = |style: u8| band(center, lines.thickness(style));

    let mut draw = |target: (i32, i32), across_band: (i32, i32)| {
        let (a0, a1) = if from_start { (0, target.1) } else { (target.0, extent) };
        if vertical {
            mask.fill(across_band.0, a0, across_band.1, a1);
        } else {
            mask.fill(a0, across_band.0, a1, across_band.1);
        }
    };

    let style = arms[direction];
    if style != DOUBLE {
        let sides = [low_side, high_side];
        let target = if sides.iter().any(|&side| arms[side] == DOUBLE) {
            if has(opposite) || !(has(low_side) && has(high_side)) {
                far
            } else {
                near
            }
        } else {
            let widest = sides.iter().filter(|&&side| has(side)).map(|&side| arms[side]).max().unwrap_or(style);
            center_band(widest)
        };
        draw(target, band(across, lines.thickness(style)));
        return;
    }
    for (side, other, offset) in [(low_side, high_side, -d), (high_side, low_side, d)] {
        let target = if has(side) {
            if arms[side] == DOUBLE {
                near
            } else {
                center_band(arms[side])
            }
        } else if has(opposite) {
            far
        } else if has(other) {
            if arms[other] == DOUBLE {
                far
            } else {
                center_band(arms[other])
            }
        } else {
            center_band(LIGHT)
        };
        draw(target, band(across + offset, t));
    }
}

/// Rounded corners and diagonals, stroked with anti-aliasing.
fn stroked(mask: &mut Mask, ch: char) -> bool {
    if !matches!(ch, '╭' | '╮' | '╯' | '╰' | '╱' | '╲' | '╳') {
        return false;
    }
    let lines = Lines::of(mask);
    let (w, h) = (mask.width as f32, mask.height as f32);
    let t = lines.light as f32;
    // Centers of the straight lines, so arcs continue them seamlessly.
    let lx = band(lines.cx, lines.light).0 as f32 + t / 2.0;
    let ly = band(lines.cy, lines.light).0 as f32 + t / 2.0;
    let mut path = PathBuilder::new();
    match ch {
        '╱' | '╲' | '╳' => {
            if ch != '╲' {
                path.move_to(w, 0.0);
                path.line_to(0.0, h);
            }
            if ch != '╱' {
                path.move_to(0.0, 0.0);
                path.line_to(w, h);
            }
        }
        _ => {
            let radius = (w / 2.0).min(h / 2.0);
            let (horizontal_end, sx) = if matches!(ch, '╭' | '╰') { (w, 1.0) } else { (0.0, -1.0) };
            let (vertical_end, sy) = if matches!(ch, '╭' | '╮') { (h, 1.0) } else { (0.0, -1.0) };
            path.move_to(lx, vertical_end);
            path.line_to(lx, ly + sy * radius);
            path.quad_to(lx, ly, lx + sx * radius, ly);
            path.line_to(horizontal_end, ly);
        }
    }
    let (Some(path), Some(mut pixmap)) = (path.finish(), Pixmap::new(mask.width as u32, mask.height as u32)) else {
        return false;
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    let stroke = Stroke {
        width: t,
        ..Default::default()
    };
    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    for (out, pixel) in mask.data.iter_mut().zip(pixmap.data().chunks_exact(4)) {
        *out = pixel[3];
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(mask: &[u8], width: u32, x: u32, y: u32) -> u8 {
        mask[(y * width + x) as usize]
    }

    fn coverage(mask: &[u8]) -> f32 {
        mask.iter().map(|&alpha| alpha as f32 / 255.0).sum::<f32>() / mask.len() as f32
    }

    #[test]
    fn blocks_fill_their_exact_part_of_the_cell() {
        for (w, h) in [(8, 16), (16, 32), (9, 16), (32, 64)] {
            let full = glyph("█", w, h).unwrap();
            assert!(full.iter().all(|&alpha| alpha == 255));
            let upper = glyph("▀", w, h).unwrap();
            for x in 0..w {
                assert_eq!(at(&upper, w, x, 0), 255);
                assert_eq!(at(&upper, w, x, h / 2 - 1), 255);
                assert_eq!(at(&upper, w, x, h / 2), 0);
                assert_eq!(at(&upper, w, x, h - 1), 0);
            }
            let lower = glyph("▄", w, h).unwrap();
            assert!(upper.iter().zip(&lower).all(|(a, b)| a ^ b == 255), "▀ and ▄ tile the cell");
            let left = glyph("▌", w, h).unwrap();
            let right = glyph("▐", w, h).unwrap();
            assert!(left.iter().zip(&right).all(|(a, b)| a ^ b == 255), "▌ and ▐ tile the cell");
            let quadrants = glyph("▚", w, h).unwrap();
            assert_eq!(at(&quadrants, w, 0, 0), 255);
            assert_eq!(at(&quadrants, w, w - 1, 0), 0);
            assert_eq!(at(&quadrants, w, w - 1, h - 1), 255);
            assert!((coverage(&glyph("▂", w, h).unwrap()) - 0.25).abs() < 0.07);
            assert!((coverage(&glyph("▊", w, h).unwrap()) - 0.75).abs() < 0.07);
        }
    }

    #[test]
    fn shades_use_dither_patterns_with_their_density() {
        for (w, h) in [(8, 16), (24, 48)] {
            for (ch, density) in [("░", 0.25), ("▒", 0.5), ("▓", 0.75)] {
                let mask = glyph(ch, w, h).unwrap();
                assert!(mask.iter().all(|&alpha| alpha == 0 || alpha == 255), "{ch} is a crisp pattern");
                assert_eq!(coverage(&mask), density, "{ch} at {w}x{h}");
            }
        }
    }

    #[test]
    fn lines_reach_the_cell_edges_so_neighbours_connect() {
        for (w, h) in [(8, 16), (16, 32)] {
            let horizontal = glyph("─", w, h).unwrap();
            assert!(horizontal.iter().any(|&a| a == 255));
            for y in 0..h {
                assert_eq!(at(&horizontal, w, 0, y), at(&horizontal, w, w - 1, y), "row {y}");
            }
            let vertical = glyph("│", w, h).unwrap();
            for x in 0..w {
                assert_eq!(at(&vertical, w, x, 0), at(&vertical, w, x, h - 1), "column {x}");
            }
            // A cross is the union of both lines.
            let cross = glyph("┼", w, h).unwrap();
            assert!(cross.iter().zip(horizontal.iter().zip(&vertical)).all(|(c, (a, b))| *c == (*a).max(*b)));
            assert!(coverage(&glyph("━", w, h).unwrap()) > coverage(&horizontal));
        }
    }

    #[test]
    fn double_lines_meet_in_corners_and_keep_their_gap() {
        let (w, h) = (16, 32);
        let horizontal = glyph("═", w, h).unwrap();
        let rows: Vec<u32> = (0..h).filter(|&y| at(&horizontal, w, 0, y) == 255).collect();
        assert!(
            rows.len() >= 2 && rows.windows(2).any(|pair| pair[1] - pair[0] > 1),
            "two separate lines: {rows:?}"
        );
        let (top, bottom) = (rows[0], *rows.last().unwrap());
        // ╔: the outer line turns the corner, the inner one leaves the gap open.
        let corner = glyph("╔", w, h).unwrap();
        let vertical = glyph("║", w, h).unwrap();
        let columns: Vec<u32> = (0..w).filter(|&x| at(&vertical, w, x, 0) == 255).collect();
        let outer = columns[0];
        // The first column of the inner line, after the gap.
        let inner = *columns.windows(2).find(|pair| pair[1] - pair[0] > 1).unwrap().last().unwrap();
        assert_eq!(at(&corner, w, outer, top), 255, "outer corner is closed");
        assert_eq!(at(&corner, w, w - 1, top), 255);
        assert_eq!(at(&corner, w, outer, h - 1), 255);
        assert_eq!(at(&corner, w, inner, bottom), 255, "inner corner is closed");
        assert_eq!(at(&corner, w, inner - 1, bottom), 0, "inner line does not cross the gap");
        assert_eq!(at(&corner, w, outer, top - 1), 0, "nothing above the outer corner");
        // ╬: four corners with an open center.
        let cross = glyph("╬", w, h).unwrap();
        assert_eq!(at(&cross, w, w / 2, h / 2), 0);
        assert_eq!(at(&cross, w, outer, 0), 255);
        assert_eq!(at(&cross, w, 0, top), 255);
        // ╞: the single vertical line runs through, the double arm touches it.
        let tee = glyph("╞", w, h).unwrap();
        let single_line = glyph("│", w, h).unwrap();
        let single: Vec<u32> = (0..w).filter(|&x| at(&single_line, w, x, 0) == 255).collect();
        assert!((0..h).all(|y| at(&tee, w, single[0], y) == 255));
        assert_eq!(at(&tee, w, *single.last().unwrap() + 1, top), 255);
    }

    #[test]
    fn dashes_arcs_and_diagonals_are_drawn() {
        let (w, h) = (16, 32);
        let dashed = glyph("┄", w, h).unwrap();
        let row = h / 2;
        let lit = (0..w).filter(|&x| at(&dashed, w, x, row) == 255).count();
        assert!(lit > 0 && lit < w as usize, "dashes have gaps");
        let arc = glyph("╭", w, h).unwrap();
        assert!(at(&arc, w, w - 1, row) > 0, "arc reaches the right edge");
        assert!(at(&arc, w, w / 2, h - 1) > 0, "arc reaches the bottom edge");
        assert_eq!(at(&arc, w, 0, 0), 0);
        let diagonal = glyph("╲", w, h).unwrap();
        assert!(at(&diagonal, w, 0, 0) > 0 && at(&diagonal, w, w - 1, h - 1) > 0);
        assert_eq!(at(&diagonal, w, w - 1, 0), 0);
    }

    #[test]
    fn other_characters_use_fonts() {
        for text in ["A", "■", "e\u{301}", "─a"] {
            assert!(glyph(text, 8, 16).is_none(), "{text}");
        }
    }
}
