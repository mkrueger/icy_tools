use std::{f64::consts, path::PathBuf};

use crate::{BitFont, EGA_PALETTE, EditableScreen, Palette, Position, Rectangle, Size};

mod character;
pub use character::*;

mod font;
pub use font::*;
use icy_parser_core::FillStyle;

#[derive(Clone, Copy)]
pub enum Color {
    Black,
    Blue,
    Green,
    Cyan,
    Red,
    Magenta,
    Brown,
    LightGray,
    DarkGray,
    LightBlue,
    LightGreen,
    LightCyan,
    LightRed,
    LightMagenta,
    Yellow,
    White,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WriteMode {
    Copy,
    Xor,
    Or,
    And,
    Not,
}

impl WriteMode {
    pub fn from(write_mode: u8) -> WriteMode {
        match write_mode {
            // 0 => WriteMode::Copy,
            1 => WriteMode::Xor,
            2 => WriteMode::Or,
            3 => WriteMode::And,
            4 => WriteMode::Not,
            _ => WriteMode::Copy,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LineStyle {
    Solid,
    Dotted,
    Center,
    Dashed,
    User,
}

impl LineStyle {
    const LINE_PATTERNS: [u32; 5] = [
        // Solid
        0xFFFF, // Dotted
        0xCCCC, // Center
        0xF878, // Dashed
        0xF8F8, // User
        0xFFFF,
    ];

    pub fn from(line_style: u8) -> LineStyle {
        match line_style {
            1 => LineStyle::Dotted,
            2 => LineStyle::Center,
            3 => LineStyle::Dashed,
            4 => LineStyle::User,
            _ => LineStyle::Solid,
        }
    }

    pub fn line_pattern(&self) -> Vec<bool> {
        let offset = match self {
            LineStyle::Solid => 0,
            LineStyle::Dotted => 1,
            LineStyle::Center => 2,
            LineStyle::Dashed => 3,
            LineStyle::User => 4,
        };

        let mut res = Vec::new();
        for i in 0..16 {
            res.push((LineStyle::LINE_PATTERNS[offset] & (1 << i)) != 0);
        }
        res
    }
}

#[derive(Clone, Copy)]
pub enum Direction {
    Horizontal,
    Vertical,
}

impl Direction {
    pub fn from(direction: u8) -> Direction {
        match direction {
            // 0 => Direction::Horizontal,
            1 => Direction::Vertical,
            _ => Direction::Horizontal,
        }
    }
}

#[derive(Clone, Copy)]
pub enum FontType {
    Default,
    Triplex,
    Small,
    SansSerif,
    Gothic,
    Script,
    Simplex,
    TriplexScript,
    Complex,
    European,
    BoldOutline,
    User,
}

impl FontType {
    pub fn font(&self) -> &Font {
        match self {
            FontType::User | FontType::Default => &FONTS[0],
            FontType::Triplex => &FONTS[1],
            FontType::Small => &FONTS[2],
            FontType::SansSerif => &FONTS[3],
            FontType::Gothic => &FONTS[4],
            FontType::Script => &FONTS[5],
            FontType::Simplex => &FONTS[6],
            FontType::TriplexScript => &FONTS[7],
            FontType::Complex => &FONTS[8],
            FontType::European => &FONTS[9],
            FontType::BoldOutline => &FONTS[10],
        }
    }
}

lazy_static::lazy_static! {
    pub static ref DEFAULT_BITFONT : BitFont = BitFont::from_sauce_name("IBM VGA50").unwrap();

    static ref FONTS: Vec<Font> = vec![
        Font::load(include_bytes!("fonts/SANS.CHR")).unwrap(),
        Font::load(include_bytes!("fonts/TRIP.CHR")).unwrap(),
        Font::load(include_bytes!("fonts/LITT.CHR")).unwrap(),
        Font::load(include_bytes!("fonts/SANS.CHR")).unwrap(),
        Font::load(include_bytes!("fonts/GOTH.CHR")).unwrap(),
        Font::load(include_bytes!("fonts/SCRI.CHR")).unwrap(),
        Font::load(include_bytes!("fonts/SIMP.CHR")).unwrap(),
        Font::load(include_bytes!("fonts/TSCR.CHR")).unwrap(),
        Font::load(include_bytes!("fonts/LCOM.CHR")).unwrap(),
        Font::load(include_bytes!("fonts/EURO.CHR")).unwrap(),
        Font::load(include_bytes!("fonts/BOLD.CHR")).unwrap(),
    ];
}

impl FontType {
    pub fn from(font_type: u8) -> FontType {
        match font_type {
            // 0 => FontType::Default,
            1 => FontType::Triplex,
            2 => FontType::Small,
            3 => FontType::SansSerif,
            4 => FontType::Gothic,
            5 => FontType::Script,
            6 => FontType::Simplex,
            7 => FontType::TriplexScript,
            8 => FontType::Complex,
            9 => FontType::European,
            10 => FontType::BoldOutline,
            11 => FontType::User,
            _ => FontType::Default,
        }
    }
}

const DEFAULT_USER_PATTERN: [u8; 8] = [0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55];
const RAD2DEG: f64 = 180.0 / consts::PI;
const DEG2RAD: f64 = consts::PI / 180.0;
const ASPECT: f64 = 350.0 / 480.0 * 1.06; //0.772; //7.0/9.0; //350.0 / 480.0 * 1.066666;

pub struct Image {
    pub width: i32,
    pub height: i32,
    pub data: Vec<u8>,
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub enum LabelOrientation {
    Above,
    Left,
    #[default]
    Center,
    Right,
    Below,
}

impl LabelOrientation {
    pub fn from(orientation: u8) -> LabelOrientation {
        match orientation {
            0 => LabelOrientation::Above,
            1 => LabelOrientation::Left,
            // 2 => LabelOrientation::Center,
            3 => LabelOrientation::Right,
            4 => LabelOrientation::Below,
            _ => LabelOrientation::Center,
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct ButtonStyle2 {
    pub size: Size,
    pub orientation: LabelOrientation,

    pub bevel_size: i32,

    pub label_color: i32,
    pub drop_shadow_color: i32,
    pub bright: i32,
    pub dark: i32,

    pub flags: i32,
    pub flags2: i32,

    pub surface_color: i32,
    pub group: i32,
    pub underline_color: i32,
    pub corner_color: i32,
}
// Button flags: 1000010100110010                0
// Button flags:      11100110110              110

impl ButtonStyle2 {
    pub fn is_clipboard_button(&self) -> bool {
        self.flags & 1 != 0
    }

    pub fn is_invertable_button(&self) -> bool {
        self.flags & 2 != 0
    }

    pub fn reset_screen_after_click(&self) -> bool {
        self.flags & 4 != 0
    }

    pub fn display_chisel(&self) -> bool {
        self.flags & 8 != 0
    }

    pub fn display_recessed(&self) -> bool {
        self.flags & 16 != 0
    }

    pub fn display_dropshadow(&self) -> bool {
        self.flags & 32 != 0
    }

    pub fn stamp_image_on_clipboard(&self) -> bool {
        self.flags & 64 != 0
    }

    pub fn is_icon_button(&self) -> bool {
        self.flags & 128 != 0
    }

    pub fn is_plain_button(&self) -> bool {
        self.flags & 256 != 0
    }

    pub fn display_bevel_special_effect(&self) -> bool {
        self.flags & 512 != 0
    }

    pub fn is_mouse_button(&self) -> bool {
        self.flags & 1024 != 0
    }

    pub fn underline_hotkey(&self) -> bool {
        self.flags & 2048 != 0
    }

    pub fn use_hotkey_for_icon_button(&self) -> bool {
        self.flags & 4096 != 0
    }

    pub fn adj_vertical_center(&self) -> bool {
        self.flags & 8192 != 0
    }

    pub fn belongs_to_a_radio_group(&self) -> bool {
        self.flags & 16384 != 0
    }

    pub fn display_sunken_effect(&self) -> bool {
        self.flags & 32768 != 0
    }

    pub fn is_checkbox_button(&self) -> bool {
        self.flags2 & 1 != 0
    }

    pub fn highlight_hotkey(&self) -> bool {
        self.flags2 & 2 != 0
    }

    pub fn explode(&self) -> bool {
        self.flags2 & 4 != 0
    }

    pub fn left_justify_label(&self) -> bool {
        self.flags2 & 8 != 0
    }

    pub fn right_justify_label(&self) -> bool {
        self.flags2 & 16 != 0
    }
}

pub struct Bgi {
    color: u8,
    bkcolor: u8,

    button_style: ButtonStyle2,
    write_mode: WriteMode,
    line_style: LineStyle,
    fill_style: FillStyle,
    fill_user_pattern: Vec<u8>,
    fill_color: u8,
    direction: Direction,
    font: FontType,
    viewport: Rectangle,
    line_thickness: i32,
    line_pattern: Vec<bool>,
    current_pos: Position,
    char_size: i32,
    pub suspend_text: bool,
    pub rip_image: Option<Image>,

    pub file_path: PathBuf,
}

mod bezier_handler {
    use core::f64;
    use std::f64::consts;

    const ST_ARR: [f64; 4] = [1.0, 3.0, 3.0, 1.0];

    pub fn first(n: i32, v: f64) -> f64 {
        match n {
            1 => v,
            2 => v * v,
            3 => v * v * v,
            _ => 1.0,
        }
    }

    pub fn second(n: i32, v: f64) -> f64 {
        match n {
            2 => (1.0 - v).log(consts::E).exp(),
            1 => (2.0 * (1.0 - v).log(consts::E)).exp(),
            0 => (3.0 * (1.0 - v).log(consts::E)).exp(),
            _ => 1.0,
        }
    }

    pub fn bezier(v: f64, n: i32) -> f64 {
        ST_ARR[n as usize] * first(n, v) * second(n, v)
    }
}

#[derive(Default, Clone)]
struct LineInfo {
    x1: i32,
    x2: i32,
    y: i32,
}

#[derive(Default, Clone)]
struct FillLineInfo {
    dir: i32,
    x1: i32,
    x2: i32,
    y: i32,
}

impl FillLineInfo {
    pub fn new(li: &LineInfo, dir: i32) -> Self {
        Self {
            dir,
            x1: li.x1,
            x2: li.x2,
            y: li.y,
        }
    }

    pub fn from(y: i32, x1: i32, x2: i32, dir: i32) -> Self {
        Self { dir, x1, x2, y }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct MouseField {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub host_command: Option<String>,
    pub style: ButtonStyle2,
}

impl MouseField {
    pub fn new(x1: i32, y1: i32, x2: i32, y2: i32, host_command: Option<String>, style: ButtonStyle2) -> Self {
        Self {
            x1,
            y1,
            x2,
            y2,
            host_command,
            style,
        }
    }

    pub fn contains_field(&self, field: &MouseField) -> bool {
        self.x1 <= field.x1 && self.y1 <= field.y1
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x1 && x <= self.x2 && y >= self.y1 && y <= self.y2
    }
}

impl Bgi {
    pub fn new(file_path: PathBuf, resolution: Size) -> Bgi {
        Bgi {
            color: 7,
            bkcolor: 0,
            write_mode: WriteMode::Copy,
            line_style: LineStyle::Solid,
            line_pattern: LineStyle::Solid.line_pattern(),
            fill_style: FillStyle::Solid,
            fill_user_pattern: DEFAULT_USER_PATTERN.to_vec(),
            fill_color: 0,
            direction: Direction::Horizontal,
            font: FontType::Default,
            viewport: resolution.into(),
            line_thickness: 1,
            current_pos: Position::new(0, 0),
            char_size: 4,
            rip_image: None,
            button_style: ButtonStyle2::default(),
            suspend_text: false,
            file_path,
        }
    }

    pub fn color(&self) -> u8 {
        self.color
    }

    pub fn set_color(&mut self, c: u8) -> u8 {
        let old = self.color;
        self.color = c % 16;
        old
    }

    pub fn get_bk_color(&self) -> u8 {
        self.bkcolor
    }

    pub fn set_bk_color(&mut self, c: u8) -> u8 {
        let old = self.bkcolor;
        self.bkcolor = c % 16;
        old
    }

    pub fn get_fill_style(&self) -> FillStyle {
        self.fill_style
    }

    pub fn set_fill_style(&mut self, style: FillStyle) -> FillStyle {
        let old = self.fill_style;
        self.fill_style = style;
        old
    }

    pub fn get_fill_color(&self) -> u8 {
        self.fill_color
    }

    pub fn set_fill_color(&mut self, color: u8) -> u8 {
        let old = self.fill_color;
        self.fill_color = color % 16;
        old
    }

    pub fn line_style(&self) -> LineStyle {
        self.line_style
    }

    pub fn set_line_style(&mut self, style: LineStyle) -> LineStyle {
        let old = self.line_style;
        self.line_style = style;
        self.line_pattern = style.line_pattern();
        old
    }

    pub fn line_thickness(&self) -> i32 {
        self.line_thickness
    }

    pub fn set_line_thickness(&mut self, thickness: i32) {
        self.line_thickness = thickness;
    }

    pub fn set_line_pattern(&mut self, pattern: i32) {
        let mut res = Vec::new();
        for i in 0..16 {
            res.push(pattern & (1 << i) != 0);
        }
        self.line_pattern = res;
    }

    // Palette delegation helpers (Option A: use external buffer palette only)
    pub fn set_palette(&mut self, buf: &mut dyn EditableScreen, colors: &[i32]) {
        let pal = buf.palette_mut();
        pal.clear();
        for c in colors {
            pal.push(EGA_PALETTE[*c as usize].clone());
        }
    }

    pub fn set_palette_color(&mut self, buf: &mut dyn EditableScreen, index: i32, color: u8) {
        buf.palette_mut().set_color(index as u32, EGA_PALETTE[color as usize].clone());
    }

    pub fn get_font_type(&self) -> FontType {
        self.font
    }

    pub fn get_text_direction(&self) -> Direction {
        self.direction
    }

    pub fn get_font_size(&self) -> i32 {
        self.char_size
    }

    pub fn set_text_style(&mut self, font: FontType, direction: Direction, char_size: i32) {
        self.font = font;
        self.direction = direction;
        self.char_size = char_size.clamp(1, 10);
    }

    #[inline(always)]
    pub fn pixel(&self, buf: &mut dyn EditableScreen, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= buf.resolution().width || y >= buf.resolution().height {
            return 0;
        }
        let off = (y * buf.resolution().width + x) as usize;
        unsafe {
            // Use unsafe for bounds-checked access we've already validated
            *buf.screen().get_unchecked(off)
        }
    }

    pub fn fill_pattern(&self) -> &Vec<u8> {
        &self.fill_user_pattern
    }

    pub fn set_button_style(&mut self, style: ButtonStyle2) {
        self.button_style = style;
    }

    #[inline(always)]
    pub fn put_pixel(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, color: u8) {
        if !self.viewport.contains(x, y) {
            return;
        }
        if x < 0 || y < 0 || x >= buf.resolution().width || y >= buf.resolution().height {
            return;
        }
        let mut new_index = color % 16;

        if !matches!(self.write_mode, WriteMode::Copy) {
            let cur = self.pixel(buf, x, y);
            new_index = match self.write_mode {
                WriteMode::Copy => color % 16,
                WriteMode::Xor => cur ^ color,
                WriteMode::Or => cur | color,
                WriteMode::And => cur & color,
                WriteMode::Not => (!color) & 0x0F,
            } % 16;
        }
        let off = (y * buf.resolution().width + x) as usize;
        unsafe {
            *buf.screen_mut().get_unchecked_mut(off) = new_index;
        }
    }

    pub fn write_mode(&self) -> WriteMode {
        self.write_mode
    }

    pub fn set_write_mode(&mut self, mode: WriteMode) -> WriteMode {
        let old = self.write_mode;
        self.write_mode = mode;
        old
    }

    fn fill_x(&mut self, buf: &mut dyn EditableScreen, y: i32, startx: i32, count: i32, offset: &mut i32) {
        let mut start_y = y - self.line_thickness / 2;
        let mut end_y = start_y + self.line_thickness - 1;
        let mut end_x = startx + count;
        if count > 0 {
            end_x -= 1;
        } else {
            end_x += 1;
            *offset -= count;
        }

        if start_y < 0 {
            start_y = 0;
        }

        end_y = end_y.min(self.viewport.bottom() - 1);

        let inc = if count >= 0 { 1 } else { -1 };
        let mut startx = startx;
        if startx > end_x {
            std::mem::swap(&mut startx, &mut end_x);
        }

        if startx >= self.viewport.right() {
            return;
        }

        if startx < 0 {
            startx = 0;
        }

        end_x = end_x.min(self.viewport.right() - 1);

        for x in startx..=end_x {
            if self.line_pattern[*offset as usize % self.line_pattern.len()] {
                for cy in start_y..=end_y {
                    self.put_pixel(buf, x, cy, self.color);
                }
            }
            *offset += inc;
        }
        if count < 0 {
            *offset -= count;
        }
    }

    pub fn fill_y(&mut self, buf: &mut dyn EditableScreen, x: i32, start_y: i32, count: i32, offset: &mut i32) {
        let mut start_x = x - self.line_thickness / 2;
        let mut end_x = start_x + self.line_thickness - 1;
        let mut end_y = start_y + count;
        if count > 0 {
            end_y -= 1;
        } else {
            end_y += 1;
            *offset -= count;
        }

        if start_x < 0 {
            start_x = 0;
        }

        end_x = end_x.min(self.viewport.right() - 1);
        let mut start_y = start_y;
        if start_y > end_y {
            std::mem::swap(&mut start_y, &mut end_y);
        }

        if start_y >= self.viewport.bottom() {
            return;
        }

        if start_y < 0 {
            start_y = 0;
        }

        end_y = end_y.min(self.viewport.bottom() - 1);

        for y in start_y..=end_y {
            if self.line_pattern[*offset as usize % self.line_pattern.len()] {
                for cx in start_x..=end_x {
                    self.put_pixel(buf, cx, y, self.color);
                }
            }
            *offset += 1;
        }
        if count < 0 {
            *offset += count;
        }
    }

    pub fn line(&mut self, buf: &mut dyn EditableScreen, x1: i32, y1: i32, x2: i32, y2: i32) {
        let ly_delta = (y2 - y1).abs();
        let lx_delta2 = (x2 - x1).abs();
        let mut offset = 0;
        if lx_delta2 == 0 {
            self.fill_y(buf, x1, y1.min(y2), ly_delta + 1, &mut offset);
        } else if ly_delta == 0 {
            self.fill_x(buf, y1, x1.min(x2), lx_delta2 + 1, &mut offset);
        } else if lx_delta2 >= ly_delta {
            let l_advance = 1;
            let (mut pos, l_step) = if y1 < y2 {
                (Position::new(x1, y1), if x1 > x2 { -1 } else { 1 })
            } else {
                (Position::new(x2, y2), if x2 > x1 { -1 } else { 1 })
            };

            let l_whole_step = (lx_delta2 / ly_delta) * l_step;
            let mut l_adj_up = lx_delta2 % ly_delta;
            let l_adj_down = ly_delta * 2;
            let mut l_error = l_adj_up - l_adj_down;
            l_adj_up *= 2;

            let mut l_start_length = (l_whole_step / 2) + l_step;
            let l_end_length = l_start_length;
            if (l_adj_up == 0) && ((l_whole_step & 0x01) == 0) {
                l_start_length -= l_step;
            }

            if (l_whole_step & 0x01) != 0 {
                l_error += ly_delta;
            }

            self.fill_x(buf, pos.y, pos.x, l_start_length, &mut offset);
            pos.x += l_start_length;
            pos.y += l_advance;
            for _ in 0..(ly_delta - 1) {
                let mut l_run_length = l_whole_step;
                l_error += l_adj_up;
                if l_error > 0 {
                    l_run_length += l_step;
                    l_error -= l_adj_down;
                }
                self.fill_x(buf, pos.y, pos.x, l_run_length, &mut offset);
                pos.x += l_run_length;
                pos.y += l_advance;
            }
            self.fill_x(buf, pos.y, pos.x, l_end_length, &mut offset);
        } else if lx_delta2 < ly_delta {
            let (mut pos, l_advance) = if y1 < y2 {
                (Position::new(x1, y1), if x1 > x2 { -1 } else { 1 })
            } else {
                (Position::new(x2, y2), if x2 > x1 { -1 } else { 1 })
            };

            let l_whole_step = ly_delta / lx_delta2;
            let mut l_adj_up = ly_delta % lx_delta2;
            let l_adj_down = lx_delta2 * 2;
            let mut l_error = l_adj_up - l_adj_down;
            l_adj_up *= 2;
            let mut l_start_length = (l_whole_step / 2) + 1;
            let l_end_length = l_start_length;
            if (l_adj_up == 0) && ((l_whole_step & 0x01) == 0) {
                l_start_length -= 1;
            }
            if (l_whole_step & 0x01) != 0 {
                l_error += lx_delta2;
            }

            self.fill_y(buf, pos.x, pos.y, l_start_length, &mut offset);
            pos.y += l_start_length;
            pos.x += l_advance;

            for _ in 0..(lx_delta2 - 1) {
                let mut l_run_length = l_whole_step;
                l_error += l_adj_up;
                if l_error > 0 {
                    l_run_length += 1;
                    l_error -= l_adj_down;
                }
                self.fill_y(buf, pos.x, pos.y, l_run_length, &mut offset);
                pos.y += l_run_length;
                pos.x += l_advance;
            }
            self.fill_y(buf, pos.x, pos.y, l_end_length, &mut offset);
        }
    }

    fn draw_line(&mut self, buf: &mut dyn EditableScreen, x0: i32, y0: i32, x1: i32, y1: i32, color: u8) {
        let dx = (x0 - x1).abs();
        let dy = (y0 - y1).abs();

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

    pub fn move_to(&mut self, x: i32, y: i32) {
        self.current_pos = Position::new(x, y);
    }

    pub fn line_to(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32) {
        self.line(buf, self.current_pos.x, self.current_pos.y, x, y);
        self.move_to(x, y);
    }

    pub fn line_rel(&mut self, buf: &mut dyn EditableScreen, dx: i32, dy: i32) {
        let x = self.current_pos.x + dx;
        let y = self.current_pos.y + dy;
        self.line(buf, self.current_pos.x, self.current_pos.y, x, y);
        self.move_to(x, y);
    }

    fn find_line(&self, x: i32, y: i32, border: u8, buf: &mut dyn EditableScreen) -> Option<LineInfo> {
        // find end pixel
        let mut endx = self.viewport.width();
        for ex in x..self.viewport.width() {
            let col = self.pixel(buf, ex, y);
            if col == border {
                endx = ex;
                break;
            }
        }
        endx -= 1;

        // find beginning pixel
        let mut startx = -1;
        for sx in (0..x).rev() {
            let col = self.pixel(buf, sx, y);
            if col == border {
                startx = sx;
                break;
            }
        }
        startx += 1;

        // a weird condition for solid fills and the sides of the screen
        if (startx == 0 || endx == buf.resolution().width - 1) && (endx == startx) {
            return None;
        }

        Some(LineInfo { x1: startx, x2: endx, y })
    }

    pub fn rectangle(&mut self, buf: &mut dyn EditableScreen, left: i32, top: i32, right: i32, bottom: i32) {
        self.line(buf, left, top, right, top);
        self.line(buf, left, bottom, right, bottom);
        self.line(buf, right, top, right, bottom);
        self.line(buf, left, top, left, bottom);
    }

    pub fn flood_fill(&mut self, buf: &mut dyn EditableScreen, start_x: i32, start_y: i32, edge: u8) {
        // Early bounds / trivial exit
        if !self.viewport.contains(start_x, start_y) {
            return;
        }

        // If starting pixel is already edge, nothing to do.
        if self.pixel(buf, start_x, start_y) == edge {
            return;
        }

        // Retrieve pattern (clone user pattern first to avoid borrow issues during pixel writes)
        let user_pattern = self.fill_user_pattern.clone();
        let pattern = self.fill_style.fill_pattern(&user_pattern);

        let vp_left = self.viewport.left();
        let vp_top = self.viewport.top();
        let vp_right = self.viewport.right();
        let vp_bottom = self.viewport.bottom();

        let width = self.viewport.width();
        let height = self.viewport.height();

        // Visited bitmap so we don’t revisit horizontal spans
        let mut visited = vec![false; (width * height) as usize];

        // SavedPoint replicates original C struct (x,y,oy)
        #[derive(Clone, Copy)]
        struct SavedPoint {
            x: i32,
            y: i32,
            oy: i32,
        }

        // Stack of SavedPoint (LIFO)
        let mut stack: Vec<SavedPoint> = Vec::new();
        stack.push(SavedPoint {
            x: start_x,
            y: start_y,
            oy: start_y,
        });

        // Helper to map absolute coords into visited index
        let idx = |x: i32, y: i32| -> Option<usize> {
            if x < vp_left || x >= vp_right || y < vp_top || y >= vp_bottom {
                return None;
            }
            Some(((y - vp_top) * width + (x - vp_left)) as usize)
        };

        while let Some(SavedPoint { x, y, oy }) = stack.pop() {
            // Bounds & skip checks
            if !self.viewport.contains(x, y) {
                continue;
            }

            // Skip if pixel is edge or already visited
            if self.pixel(buf, x, y) == edge {
                continue;
            }
            if let Some(i) = idx(x, y) {
                if visited[i] {
                    continue;
                }
            }

            // Move left until edge (or viewport boundary)
            let mut scan_x = x;
            while scan_x > vp_left {
                let nx = scan_x - 1;
                let px = self.pixel(buf, nx, y);
                if px == edge {
                    break;
                }
                if let Some(i) = idx(nx, y) {
                    if visited[i] {
                        break;
                    }
                }
                scan_x -= 1;
            }

            // Initialize pattern bit positions
            let mut vx = (vp_left + scan_x) & 0x07;
            let vy = (vp_top + y) & 0x07;

            // Pre-calc offsets for previous & next line visitation
            let prev_y = y - 1;
            let next_y = y + 1;

            // iszero condition (copy of C logic: skip vertical adjacency if current pattern row is all zero)
            let pattern_row = pattern[vy as usize];
            let iszero = pattern_row == 0;

            let mut prevline_active = false;
            let mut nextline_active = false;

            let mut cur_x = scan_x;
            while cur_x < vp_right {
                // Stop if edge encountered
                let col = self.pixel(buf, cur_x, y);
                if col == edge {
                    break;
                }

                // Stop if already filled
                let already = idx(cur_x, y).map(|i| visited[i]).unwrap_or(true);
                if already {
                    break;
                }

                // Determine fill foreground/background based on pattern bit
                let cur_pattern_row = pattern[vy as usize];
                let bit_mask = 0x80u8 >> vx;
                let use_fg = (cur_pattern_row & bit_mask) != 0;
                let write_color = if use_fg { self.fill_color } else { self.bkcolor };
                self.put_pixel(buf, cur_x, y, write_color);

                // Mark visited
                if let Some(i) = idx(cur_x, y) {
                    visited[i] = true;
                }

                // Only consider spawning vertical segments when:
                //  - pattern row is zero OR pattern bit set (matches C: (row==0) || (row & mask))
                if cur_pattern_row == 0 || use_fg {
                    // Previous line logic
                    if prev_y >= vp_top && !iszero && !(iszero && oy == prev_y) {
                        let prev_pixel = self.pixel(buf, cur_x, prev_y);
                        let prev_visited = idx(cur_x, prev_y).map(|i| visited[i]).unwrap_or(true);
                        if prevline_active {
                            if prev_pixel == edge {
                                prevline_active = false;
                            }
                        } else {
                            if cur_x > vp_left && cur_x < vp_right - 1 && prev_pixel != edge && !prev_visited {
                                prevline_active = true;
                                stack.push(SavedPoint { x: cur_x, y: prev_y, oy: y });
                            }
                        }
                    }

                    // Next line logic
                    if next_y < vp_bottom && !iszero && !(iszero && oy == next_y) {
                        let next_pixel = self.pixel(buf, cur_x, next_y);
                        let next_visited = idx(cur_x, next_y).map(|i| visited[i]).unwrap_or(true);
                        if nextline_active {
                            if next_pixel == edge {
                                nextline_active = false;
                            }
                        } else {
                            if cur_x > vp_left && cur_x < vp_right - 1 && next_pixel != edge && !next_visited {
                                nextline_active = true;
                                stack.push(SavedPoint { x: cur_x, y: next_y, oy: y });
                            }
                        }
                    }
                }

                // Advance pattern x position
                vx = (vx + 1) & 0x07;
                cur_x += 1;
            }
        }
    }

    pub fn flood_fill2(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, border: u8) {
        if !self.viewport.contains(x, y) {
            return;
        }
        let mut fill_lines = vec![Vec::new(); self.viewport.height() as usize];
        let mut point_stack = Vec::new();

        if self.pixel(buf, x, y) != border {
            let li = self.find_line(x, y, border, buf);
            if let Some(li) = li {
                point_stack.push(FillLineInfo::new(&li, 1));
                point_stack.push(FillLineInfo::new(&li, -1));

                fill_lines[li.y as usize].push(li);

                while let Some(fli) = point_stack.pop() {
                    let cury = fli.y + fli.dir;
                    if cury < self.viewport.bottom() && cury >= self.viewport.top() {
                        let mut cx = fli.x1;
                        while cx <= fli.x2 {
                            let cur_px = self.pixel(buf, cx, cury);
                            if cur_px == border || cur_px == self.fill_color && matches!(self.fill_style, FillStyle::Solid) {
                                cx += 1;
                                continue; // it's a border color, so don't scan any more this direction
                            }

                            if already_drawn(&fill_lines, cx, cury) {
                                cx += 1;
                                continue; // already been here
                            }

                            let li = self.find_line(cx, cury, border, buf); // find the borders on this line
                            if let Some(li) = li {
                                cx = li.x2;
                                point_stack.push(FillLineInfo::new(&li, fli.dir));
                                if self.fill_color != 0 {
                                    // bgi doesn't go backwards when filling black!  why?  dunno.  it just does.
                                    // if we go out of current line's bounds, check the opposite dir for those
                                    if li.x2 > fli.x2 {
                                        point_stack.push(FillLineInfo::from(li.y, fli.x2 + 1, li.x2, -fli.dir));
                                    }
                                    if li.x1 < fli.x1 {
                                        point_stack.push(FillLineInfo::from(li.y, li.x1, fli.x1 - 1, -fli.dir));
                                    }
                                }

                                fill_lines[li.y as usize].push(li);
                            }
                            cx += 1;
                        }
                    }
                }
            }
        }
        for fill_line in &fill_lines {
            for li in fill_line {
                self.bar(buf, li.x1, li.y, li.x2, li.y);
            }
        }
    }

    pub fn bar(&mut self, buf: &mut dyn EditableScreen, left: i32, top: i32, right: i32, bottom: i32) {
        self.bar_rect(buf, Rectangle::from(left, top, right - left + 1, bottom - top + 1));
    }

    pub fn bar_rect(&mut self, buf: &mut dyn EditableScreen, rect: Rectangle) {
        let rect = rect.intersect(&self.viewport);
        if rect.width() == 0 || rect.height() == 0 {
            return;
        }
        let right = rect.right();
        let bottom = rect.bottom();
        if matches!(self.fill_style, FillStyle::Solid) {
            let width = buf.resolution().width;
            let screen = buf.screen_mut();
            let color = self.fill_color;
            for y in rect.top()..rect.bottom() {
                let start = (y * width as i32 + rect.left()) as usize;
                let end = start + rect.width() as usize;
                screen[start..end].fill(color);
            }
        } else {
            // Avoid borrowing self.fill_user_pattern immutably across the pixel writes by cloning first
            let user_pattern = self.fill_user_pattern.clone();
            let pattern = self.fill_style.fill_pattern(&user_pattern);
            let mut ypat = rect.top() % 8;
            for y in rect.top()..bottom {
                let mut xpatmask = (128 >> (rect.left() % 8)) as u8;
                let pattern_row = pattern[ypat as usize];
                for x in rect.left()..right {
                    let col = if (pattern_row & xpatmask) != 0 { self.fill_color } else { self.bkcolor };
                    self.put_pixel(buf, x, y, col);
                    xpatmask >>= 1;
                    if xpatmask == 0 {
                        xpatmask = 128;
                    }
                }
                ypat = (ypat + 1) % 8;
            }
        }
    }

    pub fn rip_bezier(&mut self, buf: &mut dyn EditableScreen, x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32, x4: i32, y4: i32, cnt: i32) {
        let mut targets = Vec::new();
        targets.push(x1);
        targets.push(y1);
        for step in 1..cnt {
            let tf = (step as f64) / cnt as f64;
            let tr = ((cnt - step) as f64) / cnt as f64;
            let tfs = tf.powf(2.0);
            let tfstr = tfs * tr;
            let tf_c = tf.powf(3.0);
            let tr_s = tr.powf(2.0);
            let tftrs = tf * tr_s;
            let trc = tr.powf(3.0);

            let x = trc * x1 as f64 + 3.0 * tftrs * x2 as f64 + 3.0 * tfstr * x3 as f64 + tf_c * x4 as f64;
            let y = trc * y1 as f64 + 3.0 * tftrs * y2 as f64 + 3.0 * tfstr * y3 as f64 + tf_c * y4 as f64;
            targets.push(x as i32);
            targets.push(y as i32);
        }
        targets.push(x4);
        targets.push(y4);

        let mut j = 2;
        while j < targets.len() {
            self.line(buf, targets[j - 2], targets[j - 1], targets[j], targets[j + 1]);
            j += 2;
        }
    }

    pub fn draw_bezier(&mut self, buf: &mut dyn EditableScreen, count: i32, points: &[Position], segments: i32) {
        let mut x1 = points[0].x;
        let mut y1 = points[0].y;
        let mut v = 1;
        loop {
            let mut x3 = 0.0;
            let mut y3 = 0.0;
            let br = v as f64 / segments as f64;
            for (i, point) in points.iter().enumerate() {
                let ar = bezier_handler::bezier(br, i as i32);
                x3 += point.x as f64 * ar;
                y3 += point.y as f64 * ar;
            }
            let x2 = (x3).round() as i32;
            let y2 = (y3).round() as i32;
            self.line(buf, x1, y1, x2, y2);
            x1 = x2;
            y1 = y2;
            v += 1;
            if v >= segments {
                break;
            }
        }

        self.line(buf, x1, y1, points[count as usize - 1].x, points[count as usize - 1].y);
    }

    pub fn draw_poly(&mut self, buf: &mut dyn EditableScreen, points: &[Position]) {
        let mut last_point = points[0];
        for point in points {
            self.line(buf, last_point.x, last_point.y, point.x, point.y);
            last_point = *point;
        }
        self.line(buf, last_point.x, last_point.y, points[0].x, points[0].y);
    }

    pub fn draw_poly_line(&mut self, buf: &mut dyn EditableScreen, points: &[Position]) {
        let mut last_point = points[0];
        for point in points {
            self.line(buf, last_point.x, last_point.y, point.x, point.y);
            last_point = *point;
        }
    }

    pub fn fill_poly(&mut self, buf: &mut dyn EditableScreen, points: &[Position]) {
        if points.len() <= 1 {
            return;
        }

        let mut rows = create_scan_rows();
        if !self.viewport.contains_pt(points[0]) {
            return;
        }
        for i in 1..points.len() as i32 {
            /*if !self.viewport.Contains(points[i]) {
                return;
            }*/
            scan_lines(i - 1, i, &mut rows, points, false);
        }
        scan_lines(points.len() as i32 - 1, 0, &mut rows, points, false);

        if !matches!(self.fill_style, FillStyle::Empty) {
            for i in 1..rows.len() as i32 {
                let row = &mut rows[i as usize];
                let y = i - 1;
                if !row.is_empty() {
                    row.sort_unstable();
                    let mut on = false;
                    let mut lastx = -1;
                    for curx in row {
                        if on {
                            self.bar(buf, lastx, y, *curx, y);
                        }
                        on = !on;
                        lastx = *curx;
                    }
                }
            }
        }
        if self.color != 0 {
            self.draw_poly(buf, points);
        }
    }

    pub fn arc(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, start_angle: i32, end_angle: i32, radius: i32) {
        self.ellipse(buf, x, y, start_angle, end_angle, radius, (radius as f64 * ASPECT).round() as i32);
    }

    pub fn fill_scan(&mut self, buf: &mut dyn EditableScreen, rows: &mut Vec<Vec<i32>>) {
        for y in 0..rows.len() - 2 {
            let row = &mut rows[y + 1];
            if !row.is_empty() {
                row.sort_unstable();
                self.bar(buf, row[0], y as i32, row[row.len() - 1], y as i32);
            }
        }
    }

    pub fn draw_scan(&mut self, buf: &mut dyn EditableScreen, rows: &mut Vec<Vec<i32>>) {
        for i in 0..rows.len() as i32 {
            let row = &mut rows[i as usize];
            if row.is_empty() {
                continue;
            }
            let y = i - 1;
            row.dedup();
            for x in row {
                self.put_pixel(buf, *x, y, self.color);
            }
        }
    }

    pub fn outline_scan(&mut self, buf: &mut dyn EditableScreen, rows: &mut Vec<Vec<i32>>) {
        let old_line_style = self.line_style();
        if !matches!(old_line_style, LineStyle::Solid) {
            self.set_line_style(LineStyle::Solid);
        }

        let mut lastminx = 0;
        let mut lastmaxx = 0;
        let mut first = true;
        let rows_len = rows.len();
        for i in 0..rows_len {
            rows[i].sort_unstable();
            if rows[i].len() > 2 {
                let a = (rows[i]).len() - 2;
                rows[i].drain(1..a);
            }
            let y = i - 1;
            if !rows[i].is_empty() {
                let minx = (&mut rows[i])[0];
                let a = rows[i].len() - 1;
                let maxx = (&mut rows[i])[a];
                let mut hasnext = i < rows_len - 1;
                let mut last = false;
                let mut nextminx = 0;
                let mut nextmaxx = 0;
                //let mut nextrow = if hasnext { Some(&rows[i + 1]) } else { None };

                if hasnext && !rows[i + 1].is_empty() {
                    nextminx = rows[i + 1][0];
                    nextmaxx = rows[i + 1][rows[i + 1].len() - 1];
                } else {
                    last = true;
                    hasnext = false;
                }

                if first {
                    if hasnext {
                        if nextmaxx > nextminx {
                            self.line(buf, nextminx + 1, y as i32, nextmaxx - 1, y as i32);
                        } else {
                            self.line(buf, nextminx, y as i32, nextmaxx, y as i32);
                        }
                    }
                    first = false;
                } else if last {
                    if lastmaxx > lastminx {
                        self.line(buf, lastminx + 1, y as i32, lastmaxx - 1, y as i32);
                    } else {
                        self.line(buf, lastminx, y as i32, lastmaxx, y as i32);
                    }
                } else {
                    if minx >= lastminx {
                        let mn_x = if minx > lastminx { lastminx + 1 } else { lastminx };
                        self.line(buf, mn_x, y as i32, minx, y as i32);
                    }

                    if rows[i].len() > 1 && maxx <= lastmaxx {
                        let mx_x = if maxx < lastmaxx { lastmaxx - 1 } else { lastmaxx };
                        self.line(buf, mx_x, y as i32, maxx, y as i32);
                    }
                }
                if hasnext {
                    if minx < lastminx && minx >= nextminx {
                        let mn_x = if minx > nextminx { nextminx + 1 } else { nextminx };
                        self.line(buf, mn_x, y as i32, minx, y as i32);
                    }

                    if rows[i].len() > 1 && hasnext && rows[i + 1].len() > 1 && maxx > lastmaxx && maxx <= nextmaxx {
                        let mx_x = if maxx < nextmaxx { nextmaxx - 1 } else { nextmaxx };
                        self.line(buf, mx_x, y as i32, maxx, y as i32);
                    }
                }
                lastminx = minx;
                lastmaxx = maxx;
            }
        }

        if !matches!(old_line_style, LineStyle::Solid) {
            self.set_line_style(old_line_style);
        }
    }

    pub fn symmetry_fill(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, xoffset: i32, yoffset: i32) {
        self.bar(buf, x - xoffset, y - yoffset, x + xoffset, y - yoffset);
        self.bar(buf, x - xoffset, y + yoffset, x + xoffset, y + yoffset);
    }

    pub fn circle(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, radius: i32) {
        let ry = (radius as f64 * ASPECT) as i32;
        let rx = radius;
        self.ellipse(buf, x, y, 0, 360, rx, ry);
    }

    pub fn ellipse(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, start_angle: i32, end_angle: i32, radius_x: i32, radius_y: i32) {
        // Handle degenerate cases
        let mut radius_x = radius_x;
        let mut radius_y = radius_y;

        if radius_y == 0 {
            radius_y = 1;
            radius_x -= 1;
        }
        if radius_x <= 0 {
            radius_x = 1;
        }

        // Initialize variables using McIlroy's algorithm
        let mut ex = 0;
        let mut ey = radius_y;
        let a2 = (radius_x as i64) * (radius_x as i64);
        let b2 = (radius_y as i64) * (radius_y as i64);
        let crit1 = -(a2 / 4 + (radius_x % 2) as i64 + b2);
        let crit2 = -(b2 / 4 + (radius_y % 2) as i64 + a2);
        let crit3 = -(b2 / 4 + (radius_y % 2) as i64);
        let mut t = -(a2 * ey as i64); // t = e(x+1/2,y-1/2) - (a²+b²)/4
        let mut dxt = 2 * b2 * ex as i64;
        let mut dyt = -2 * a2 * ey as i64;
        let d2xt = 2 * b2;
        let d2yt = 2 * a2;

        let inv = end_angle < start_angle;
        let mut skip = false;

        // Helper closure to check if angle is in range
        let in_range = |angle: i32| -> bool {
            if inv {
                angle <= end_angle || angle >= start_angle
            } else {
                angle >= start_angle && angle <= end_angle
            }
        };

        while ey >= 0 && ex <= radius_x {
            // Calculate angle for current position
            let angle = if ey == 0 {
                90
            } else {
                let angle_rad = (ex as f64 / ey as f64).atan();
                let angle_deg = angle_rad * RAD2DEG;
                (90.0 - angle_deg).round() as i32
            };

            if !skip {
                if ex != 0 || ey != 0 {
                    // Top-left quadrant (180 - angle)
                    let qangle = 180 - angle;
                    if in_range(qangle) {
                        self.put_pixel(buf, x - ex, y - ey, self.color);
                    }
                }

                if ex != 0 && ey != 0 {
                    // Top-right quadrant (angle)
                    if in_range(angle) {
                        self.put_pixel(buf, x + ex, y - ey, self.color);
                    }

                    // Bottom-left quadrant (180 + angle)
                    let qangle = 180 + angle;
                    if in_range(qangle) {
                        self.put_pixel(buf, x - ex, y + ey, self.color);
                    }
                }

                // Bottom-right quadrant (360 - angle)
                let qangle = 360 - angle;
                if in_range(qangle) {
                    self.put_pixel(buf, x + ex, y + ey, self.color);
                }
            }

            skip = false;

            // Determine next pixel position using decision variables
            if (t + b2 * ex as i64 <= crit1) || (t + a2 * ey as i64 <= crit3) {
                // Move in x direction
                ex += 1;
                dxt += d2xt;
                t += dxt;

                // Check if this is an angle move (skip next)
                if !((t + b2 * ex as i64 <= crit1) || (t + a2 * ey as i64 <= crit3)) && (t - a2 * ey as i64 > crit2) {
                    skip = true;
                }
            } else if t - a2 * ey as i64 > crit2 {
                // Move in y direction
                ey -= 1;
                dyt += d2yt;
                t += dyt;

                // Check if this is an angle move (skip next)
                if (t + b2 * ex as i64 <= crit1) || (t + a2 * ey as i64 <= crit3) {
                    skip = true;
                }
            } else {
                // Move diagonally
                ex += 1;
                dxt += d2xt;
                t += dxt;
                ey -= 1;
                dyt += d2yt;
                t += dyt;
            }
        }

        // Handle thick lines (line_thickness == 3)
        if self.line_thickness == 3 {
            let old_thickness = self.line_thickness;
            self.line_thickness = 1;

            // Draw inner ellipse
            if radius_x > 1 && radius_y > 1 {
                self.ellipse(buf, x, y, start_angle, end_angle, radius_x - 1, radius_y - 1);
            }

            // Draw outer ellipse
            self.ellipse(buf, x, y, start_angle, end_angle, radius_x + 1, radius_y + 1);

            self.line_thickness = old_thickness;
        }
    }

    pub fn scan_ellipse(&mut self, x: i32, y: i32, start_angle: i32, end_angle: i32, radius_x: i32, radius_y: i32, rows: &mut Vec<Vec<i32>>) {
        // Handle degenerate cases
        let mut radius_x = radius_x;
        let mut radius_y = radius_y;

        if radius_y == 0 {
            radius_y = 1;
            radius_x -= 1;
        }
        if radius_x <= 0 {
            radius_x = 1;
        }

        // Initialize variables using McIlroy's algorithm
        let mut ex = 0;
        let mut ey = radius_y;
        let a2 = (radius_x as i64) * (radius_x as i64);
        let b2 = (radius_y as i64) * (radius_y as i64);
        let crit1 = -(a2 / 4 + (radius_x % 2) as i64 + b2);
        let crit2 = -(b2 / 4 + (radius_y % 2) as i64 + a2);
        let crit3 = -(b2 / 4 + (radius_y % 2) as i64);
        let mut t = -(a2 * ey as i64);
        let mut dxt = 2 * b2 * ex as i64;
        let mut dyt = -2 * a2 * ey as i64;
        let d2xt = 2 * b2;
        let d2yt = 2 * a2;

        let inv = end_angle < start_angle;
        let mut skip = false;

        // Helper closure to check if angle is in range
        let in_range = |angle: i32| -> bool {
            if inv {
                angle <= end_angle || angle >= start_angle
            } else {
                angle >= start_angle && angle <= end_angle
            }
        };

        while ey >= 0 && ex <= radius_x {
            // Calculate angle for current position
            let angle = if ey == 0 {
                90
            } else {
                let angle_rad = (ex as f64 / ey as f64).atan();
                let angle_deg = angle_rad * RAD2DEG;
                (90.0 - angle_deg).round() as i32
            };

            if !skip {
                if ex != 0 || ey != 0 {
                    // Top-left quadrant
                    let qangle = 180 - angle;
                    if in_range(qangle) {
                        add_scan_row(rows, x - ex, y - ey);
                    }
                }

                if ex != 0 && ey != 0 {
                    // Top-right quadrant
                    if in_range(angle) {
                        add_scan_row(rows, x + ex, y - ey);
                    }

                    // Bottom-left quadrant
                    let qangle = 180 + angle;
                    if in_range(qangle) {
                        add_scan_row(rows, x - ex, y + ey);
                    }
                }

                // Bottom-right quadrant
                let qangle = 360 - angle;
                if in_range(qangle) {
                    add_scan_row(rows, x + ex, y + ey);
                }
            }

            skip = false;

            // Determine next pixel position
            if (t + b2 * ex as i64 <= crit1) || (t + a2 * ey as i64 <= crit3) {
                ex += 1;
                dxt += d2xt;
                t += dxt;

                if !((t + b2 * ex as i64 <= crit1) || (t + a2 * ey as i64 <= crit3)) && (t - a2 * ey as i64 > crit2) {
                    skip = true;
                }
            } else if t - a2 * ey as i64 > crit2 {
                ey -= 1;
                dyt += d2yt;
                t += dyt;

                if (t + b2 * ex as i64 <= crit1) || (t + a2 * ey as i64 <= crit3) {
                    skip = true;
                }
            } else {
                ex += 1;
                dxt += d2xt;
                t += dxt;
                ey -= 1;
                dyt += d2yt;
                t += dyt;
            }
        }
    }

    pub fn fill_ellipse(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, start_angle: i32, end_angle: i32, radius_x: i32, radius_y: i32) {
        let mut rows = create_scan_rows();
        self.scan_ellipse(x, y, start_angle, end_angle, radius_x, radius_y, &mut rows);
        self.fill_scan(buf, &mut rows);
        self.draw_scan(buf, &mut rows);
    }

    pub fn clear_device(&mut self, buf: &mut dyn EditableScreen) {
        let res = buf.resolution();
        self.bar(buf, 0, 0, res.width, res.height);
        self.move_to(0, 0);
    }

    pub fn sector(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, start_angle: i32, end_angle: i32, radiusx: i32, radius_y: i32) {
        let center = Position::new(x, y);
        let mut rows = create_scan_rows();
        let start_point = center + get_angle_size(start_angle, radiusx, radius_y);
        let end_point = center + get_angle_size(end_angle, radiusx, radius_y);

        let oldthickness = self.line_thickness();
        if !matches!(self.line_style, LineStyle::Solid) {
            self.set_line_thickness(1);
        }

        self.scan_ellipse(x, y, start_angle, end_angle, radiusx, radius_y, &mut rows);

        scan_line(center, start_point, &mut rows, true);
        scan_line(center, end_point, &mut rows, true);

        if !matches!(self.fill_style, FillStyle::Empty) {
            self.fill_scan(buf, &mut rows);
        }

        if matches!(self.line_style, LineStyle::Solid) {
            rows = create_scan_rows(); // ugh, twice, really?!
            self.scan_ellipse(x, y, start_angle, end_angle, radiusx, radius_y, &mut rows);
            self.draw_scan(buf, &mut rows);
        }

        if !matches!(self.line_style, LineStyle::Solid) {
            self.set_line_thickness(oldthickness);
        }

        self.line(buf, center.x, center.y, start_point.x, start_point.y);
        self.line(buf, center.x, center.y, end_point.x, end_point.y);
    }

    pub fn pie_slice(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, start_angle: i32, end_angle: i32, radius: i32) {
        self.sector(buf, x, y, start_angle, end_angle, radius, (radius as f64 * ASPECT) as i32);
    }

    pub fn graph_defaults(&mut self, buf: &mut dyn EditableScreen) {
        *buf.palette_mut() = Palette::dos_default();
        self.viewport = Rectangle::from(0, 0, buf.resolution().width, buf.resolution().height);
        self.set_color(7);
        self.set_bk_color(0);
        self.set_line_style(LineStyle::Solid);
        self.set_user_fill_pattern(&DEFAULT_USER_PATTERN);
        self.set_fill_style(FillStyle::Solid);
        self.set_fill_color(0);
        self.clear_device(buf);
        self.char_size = 4;
        self.font = FontType::Small;
        buf.clear_mouse_fields();
        self.suspend_text = false;
    }

    pub fn set_user_fill_pattern(&mut self, pattern: &[u8]) {
        self.fill_user_pattern = pattern.to_vec();
    }
    /*
    public void Bar3d(int left, int top, int right, int bottom, int depth, int topflag, IList<Rectangle> updates = null)
    {
        int temp;
        const double tan30 = 1.0 / 1.73205080756887729352;
        if (left > right)
        {
            temp = left;
            left = right;
            right = temp;
        }
        if (bottom < top)
        {
            temp = bottom;
            bottom = top;
            top = temp;
        }
        var drawUpdates = updates ?? new List<Rectangle>();
        Bar(left + lineThickness, top + lineThickness, right - lineThickness + 1, bottom - lineThickness + 1, drawUpdates);

        int dy = (int)(depth * tan30);
        var p = new Point[topflag != 0 ? 11 : 8];
        p[0].X = right;
        p[0].Y = bottom;
        p[1].X = right;
        p[1].Y = top;
        p[2].X = left;
        p[2].Y = top;
        p[3].X = left;
        p[3].Y = bottom;
        p[4].X = right;
        p[4].Y = bottom;
        p[5].X = right + depth;
        p[5].Y = bottom - dy;
        p[6].X = right + depth;
        p[6].Y = top - dy;
        p[7].X = right;
        p[7].Y = top;

        if (topflag != 0)
        {
            p[8].X = right + depth;
            p[8].Y = top - dy;
            p[9].X = left + depth;
            p[9].Y = top - dy;
            p[10].X = left;
            p[10].Y = top;
        }
        DrawPoly(p, drawUpdates);
        UpdateRegion(drawUpdates);
        if (updates == null)
            UpdateRegion(drawUpdates);
    }*/

    pub fn out_text(&mut self, buf: &mut dyn EditableScreen, str: &str) {
        self.current_pos = self.out_text_xy(buf, self.current_pos.x, self.current_pos.y, str);
    }

    pub fn out_text_xy(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, str: &str) -> Position {
        if str.is_empty() {
            return self.current_pos;
        }
        let font = self.font;

        let mut xf = x;
        let mut yf = y;

        if matches!(font, FontType::Default) {
            // Handle bitmap font with magnification
            let mag = self.char_size; // char_size is the magnification factor

            // For vertical text with bitmap font, adjust starting position
            if matches!(self.direction, Direction::Vertical) {
                yf += self.text_size(str).height;
            }

            for c in str.chars() {
                if let Some(glyph) = DEFAULT_BITFONT.glyph(c) {
                    let char_x = if matches!(self.direction, Direction::Vertical) { xf } else { xf };
                    let char_y = if matches!(self.direction, Direction::Vertical) { yf - 8 * mag } else { yf };

                    for gy in 0..8 {
                        for gx in 0..8 {
                            // Check pixel in bitmap.pixels[row][col]
                            let pixel_set = if gy < glyph.bitmap.pixels.len() as i32 && gx < glyph.bitmap.pixels[gy as usize].len() as i32 {
                                glyph.bitmap.pixels[gy as usize][gx as usize]
                            } else {
                                false
                            };
                            if pixel_set {
                                // Draw magnified pixel
                                for my in 0..mag {
                                    for mx in 0..mag {
                                        self.put_pixel(buf, char_x + gx * mag + mx, char_y + gy * mag + my, self.color);
                                    }
                                }
                            }
                        }
                    }

                    if matches!(self.direction, Direction::Horizontal) {
                        xf += 8 * mag;
                    } else {
                        yf -= 8 * mag;
                    }
                }
            }
            return Position::new(xf, yf);
        }

        // Vector font handling - this was already correct
        let old_thickness = self.line_thickness;
        self.line_thickness = 1;
        let oldline = self.line_style();
        self.set_line_style(LineStyle::Solid);

        let loaded_font = font.font();
        let text_size = loaded_font.text_size(str, self.direction, self.char_size);
        if matches!(self.direction, Direction::Vertical) {
            yf += text_size.height;
        }
        for c in str.chars() {
            let width = loaded_font.draw_character(self, buf, xf, yf, self.direction, self.char_size, c as u8);
            if matches!(self.direction, Direction::Horizontal) {
                xf += width as i32;
            } else {
                yf -= width as i32;
            }
        }
        self.line_thickness = old_thickness;
        self.set_line_style(oldline);

        Position::new(xf, yf)
    }

    pub fn text_size(&mut self, str: &str) -> Size {
        if str.is_empty() {
            return Size::new(0, 0);
        }

        let font = self.font;
        if matches!(font, FontType::Default) {
            // Apply magnification to bitmap font
            let mag = self.char_size;
            if matches!(self.direction, Direction::Horizontal) {
                return Size::new(str.len() as i32 * 8 * mag, 8 * mag);
            } else {
                return Size::new(8 * mag, str.len() as i32 * 8 * mag);
            }
        }

        let loaded_font = font.font();
        loaded_font.text_size(str, self.direction, self.char_size)
    }

    pub fn get_text_settings(&self) -> (FontType, Direction, i32) {
        (self.font, self.direction, self.char_size)
    }

    pub fn image(&self, buf: &mut dyn EditableScreen, x0: i32, y0: i32, x1: i32, y1: i32) -> Image {
        let mut image = Vec::new();
        for y in y0..y1 {
            for x in x0..x1 {
                image.push(self.pixel(buf, x, y));
            }
        }
        Image {
            width: x1 - x0,
            height: y1 - y0,
            data: image,
        }
    }

    pub fn put_rip_image(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, op: WriteMode) {
        if let Some(rip_image) = self.rip_image.take() {
            self.put_image(buf, x, y, &rip_image, op);
            self.rip_image = Some(rip_image);
        }
    }

    pub fn put_image(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, image: &Image, op: WriteMode) {
        let old_wm = self.write_mode();
        self.set_write_mode(op);

        let mut pos = 0;
        for iy in 0..image.height {
            for ix in 0..image.width {
                let col = image.data[pos];
                pos += 1;

                let x = x + ix;
                let y = y + iy;
                if !self.viewport.contains(x, y) {
                    continue;
                }
                self.put_pixel(buf, x, y, col);
            }
        }

        self.set_write_mode(old_wm);
    }
    pub fn put_image2(&mut self, buf: &mut dyn EditableScreen, src_x: i32, src_y: i32, width: i32, height: i32, x: i32, y: i32, image: &Image, op: WriteMode) {
        let old_wm = self.write_mode();
        self.set_write_mode(op);

        for iy in src_y..src_y + height {
            if iy >= image.height {
                break;
            }
            for ix in src_x..src_x + width {
                if ix >= image.width {
                    break;
                }
                let o = ix as usize + (iy * image.width) as usize;
                let col = image.data[o];

                let x = x + ix;
                let y = y + iy;
                if !self.viewport.contains(x, y) {
                    continue;
                }
                self.put_pixel(buf, x, y, col);
            }
        }

        self.set_write_mode(old_wm);
    }

    pub fn set_viewport(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.viewport = Rectangle::from(x0, y0, x1 - x0, y1 - y0);
    }
    pub fn clear_viewport(&mut self, buf: &mut dyn EditableScreen) {
        self.bar_rect(buf, self.viewport);
    }

    fn render_button_label(&mut self, buf: &mut dyn EditableScreen, text: &str, tx: i32, ty: i32, hotkey: u8, ch: u8, cs: u8, ul: u8) {
        // Draw drop shadow if enabled
        if self.button_style.display_dropshadow() {
            self.set_color(cs);
            self.out_text_xy(buf, tx + 1, ty + 1, text);
        }

        // Draw main text
        self.set_color(ch);
        self.out_text_xy(buf, tx, ty, text);

        // Handle hotkey highlighting and underlining
        if hotkey != 0 && hotkey != 255 {
            let hk_ch = (hotkey as char).to_ascii_uppercase();
            for (i, character) in text.chars().enumerate() {
                if character.to_ascii_uppercase() == hk_ch {
                    let prefix_size = self.text_size(&text[0..i]);

                    // Highlight hotkey character
                    if self.button_style.highlight_hotkey() {
                        self.set_color(ul);
                        self.out_text_xy(buf, tx + prefix_size.width, ty, &character.to_string());
                    }

                    // Underline hotkey character
                    if self.button_style.underline_hotkey() {
                        let hotkey_size = self.text_size(&character.to_string());

                        // Draw drop shadow for underline
                        if self.button_style.display_dropshadow() {
                            self.draw_line(
                                buf,
                                tx + prefix_size.width + 1,
                                ty + hotkey_size.height + 2,
                                tx + prefix_size.width + hotkey_size.width,
                                ty + hotkey_size.height + 2,
                                cs,
                            );
                        }

                        // Draw main underline
                        self.draw_line(
                            buf,
                            tx + prefix_size.width,
                            ty + hotkey_size.height + 1,
                            tx + prefix_size.width + hotkey_size.width - 1,
                            ty + hotkey_size.height + 1,
                            ul,
                        );
                    }
                    break;
                }
            }
        }
    }

    pub fn add_button(
        &mut self,
        buf: &mut dyn EditableScreen,
        x1: i32,
        y1: i32,
        mut x2: i32,
        mut y2: i32,
        hotkey: u8,
        _flags: i32,
        _icon_file: Option<&str>,
        text: &str,
        host_command: Option<String>,
        pressed: bool,
    ) {
        let bg = 0;
        let ch = self.button_style.label_color as u8;
        let cs = self.button_style.dark as u8;
        let su = self.button_style.surface_color as u8;
        let ul = self.button_style.underline_color as u8;
        let cc = self.button_style.corner_color as u8;
        let br = self.button_style.bright as u8;

        let mut width = x2 - x1 + 1;
        let mut height = y2 - y1 + 1;

        if x2 == 0 {
            width = self.button_style.size.width;
            x2 = x1 + width - 1;
        }
        if y2 == 0 {
            height = self.button_style.size.height;
            y2 = y1 + height - 1;
        }

        buf.add_mouse_field(MouseField::new(x1, y1, x2, y2, host_command, self.button_style.clone()));

        // Calculate the button surface coordinates
        let surface_x1 = x1;
        let surface_y1 = y1;
        let surface_x2 = x2;
        let surface_y2 = y2;

        // Draw recessed frame if needed
        if self.button_style.display_recessed() && !pressed {
            let recessed_x1 = x1 - 2;
            let recessed_y1 = y1 - 2;
            let recessed_x2 = x2 + 2;
            let recessed_y2 = y2 + 2;

            // Outer recessed frame
            self.draw_line(buf, recessed_x1, recessed_y1, recessed_x2, recessed_y1, cs);
            self.draw_line(buf, recessed_x1, recessed_y1, recessed_x1, recessed_y2, cs);
            self.draw_line(buf, recessed_x2, recessed_y1, recessed_x2, recessed_y2, br);
            self.draw_line(buf, recessed_x1, recessed_y2, recessed_x2, recessed_y2, br);

            self.put_pixel(buf, recessed_x1, recessed_y1, cc);
            self.put_pixel(buf, recessed_x2, recessed_y1, cc);
            self.put_pixel(buf, recessed_x1, recessed_y2, cc);
            self.put_pixel(buf, recessed_x2, recessed_y2, cc);

            // Inner black frame
            self.draw_line(buf, recessed_x1 + 1, recessed_y1 + 1, recessed_x2 - 1, recessed_y1 + 1, bg);
            self.draw_line(buf, recessed_x1 + 1, recessed_y1 + 1, recessed_x1 + 1, recessed_y2 - 1, bg);
            self.draw_line(buf, recessed_x2 - 1, recessed_y1 + 1, recessed_x2 - 1, recessed_y2 - 1, bg);
            self.draw_line(buf, recessed_x1 + 1, recessed_y2 - 1, recessed_x2 - 1, recessed_y2 - 1, bg);
        }

        // Draw bevel effect if needed
        if self.button_style.display_bevel_special_effect() {
            for i in 1..=self.button_style.bevel_size {
                self.draw_line(buf, x1 - i, y1 - i, x2 + i, y1 - i, br);
                self.draw_line(buf, x1 - i, y1 - i, x1 - i, y2 + i, br);
                self.draw_line(buf, x2 + i, y2 + i, x2 + i, y1 - i, cs);
                self.draw_line(buf, x2 + i, y2 + i, x1 - i, y2 + i, cs);
                self.put_pixel(buf, x1 - i, y1 - i, cc);
                self.put_pixel(buf, x2 + i, y1 - i, cc);
                self.put_pixel(buf, x1 - i, y2 + i, cc);
                self.put_pixel(buf, x2 + i, y2 + i, cc);
            }
        }

        // Fill button surface
        for y in surface_y1..=surface_y2 {
            for x in surface_x1..=surface_x2 {
                self.put_pixel(buf, x, y, su);
            }
        }

        // Draw sunken effect (button border)
        if self.button_style.display_sunken_effect() {
            if pressed {
                // Pressed state: inverted border colors
                self.draw_line(buf, x1, y1, x2, y1, cs);
                self.draw_line(buf, x1, y1, x1, y2, cs);
                self.draw_line(buf, x2, y2, x2, y1, br);
                self.draw_line(buf, x2, y2, x1, y2, br);
            } else {
                // Normal state
                self.draw_line(buf, x1, y1, x2, y1, br);
                self.draw_line(buf, x1, y1, x1, y2, br);
                self.draw_line(buf, x2, y2, x2, y1, cs);
                self.draw_line(buf, x2, y2, x1, y2, cs);
            }
            self.put_pixel(buf, x1, y1, cc);
            self.put_pixel(buf, x2, y1, cc);
            self.put_pixel(buf, x2, y2, cc);
            self.put_pixel(buf, x1, y2, cc);
        }

        // Draw chisel effect
        if self.button_style.display_chisel() {
            let (xinset, yinset) = chisel_inset(y2 - y1 + 1);

            if pressed {
                // Pressed state: inverted chisel colors
                self.draw_line(buf, x1 + xinset, y1 + yinset, x2 - xinset, y1 + yinset, cs);
                self.draw_line(buf, x1 + xinset, y1 + yinset, x1 + xinset, y2 - yinset, cs);

                self.draw_line(buf, x1 + xinset + 1, y2 - yinset, x2 - xinset, y2 - yinset, br);
                self.draw_line(buf, x2 - xinset, y1 + yinset + 1, x2 - xinset, y2 - yinset, br);
            } else {
                // Normal state
                self.draw_line(buf, x1 + xinset, y1 + yinset, x2 - xinset, y1 + yinset, br);
                self.draw_line(buf, x1 + xinset, y1 + yinset, x1 + xinset, y2 - yinset, br);

                self.draw_line(buf, x1 + xinset + 1, y2 - yinset, x2 - xinset, y2 - yinset, cs);
                self.draw_line(buf, x2 - xinset, y1 + yinset + 1, x2 - xinset, y2 - yinset, cs);
            }
        }

        // Draw text label
        if !text.is_empty() {
            let mut text = text.to_string();
            if let Some(strip) = text.strip_prefix("<>") {
                text = strip.to_string();
            }
            if text.ends_with("<>") {
                text.pop();
                text.pop();
            }

            let old_col = self.color();
            let text_size = self.text_size(&text);

            // Calculate base position for text (use original button area for reference)
            let (tx, ty) = match self.button_style.orientation {
                LabelOrientation::Above => {
                    let tx = if self.button_style.left_justify_label() {
                        x1
                    } else if self.button_style.right_justify_label() {
                        x1 + width - text_size.width
                    } else {
                        x1 + (width - text_size.width) / 2
                    };
                    (tx, y1 - text_size.height - 2)
                }
                LabelOrientation::Left => (x1 - text_size.width - 2, y1 + (height - text_size.height) / 2),
                LabelOrientation::Right => (x1 + width + 2, y1 + (height - text_size.height) / 2),
                LabelOrientation::Below => {
                    let tx = if self.button_style.left_justify_label() {
                        x1
                    } else if self.button_style.right_justify_label() {
                        x1 + width - text_size.width
                    } else {
                        x1 + (width - text_size.width) / 2
                    };
                    (tx, y1 + height + 2)
                }
                LabelOrientation::Center => (x1 + (width - text_size.width) / 2, y1 + (height - text_size.height) / 2),
            };

            // Render the label text with all effects
            self.render_button_label(buf, &text, tx, ty, hotkey, ch, cs, ul);
            self.set_color(old_col);
        }
    }
}

fn chisel_inset(height: i32) -> (i32, i32) {
    if height < 12 {
        return (1, 1);
    }
    if height < 25 {
        return (3, 2);
    }
    if height < 40 {
        return (4, 3);
    }
    if height < 75 {
        return (6, 5);
    }
    if height < 150 {
        return (7, 5);
    }
    if height < 200 {
        return (8, 6);
    }
    if height < 250 {
        return (10, 7);
    }
    if height < 300 {
        return (11, 8);
    }
    (13, 9)
}

fn scan_line(start: Position, end: Position, rows: &mut Vec<Vec<i32>>, full: bool) {
    let ydelta = (end.y - start.y).abs();

    if full || start.y < end.y {
        add_scan_row(rows, start.x, start.y);
    }
    if ydelta > 0 {
        let x_delta = if start.y > end.y { start.x - end.x } else { end.x - start.x };
        let min_x = if start.y > end.y { end.x } else { start.x };
        let mut pos_y = start.y.min(end.y);

        pos_y += 1;
        for count in 1..ydelta {
            let pos_x = (x_delta * count / ydelta) + min_x;

            if pos_y >= 0 && pos_y < rows.len() as i32 {
                add_scan_row(rows, pos_x, pos_y);
            }
            pos_y += 1;
        }
    }
    if full || end.y < start.y {
        add_scan_row(rows, end.x, end.y);
    }
}

fn scan_lines(start_index: i32, end_index: i32, rows: &mut Vec<Vec<i32>>, points: &[Position], full: bool) {
    scan_line(points[start_index as usize], points[end_index as usize], rows, full);
}

fn create_scan_rows() -> Vec<Vec<i32>> {
    vec![Vec::new(); 352]
}

fn add_scan_row(rows: &mut Vec<Vec<i32>>, x: i32, y: i32) {
    if !(-1..=350).contains(&y) {
        return;
    }
    let y = (y + 1) as usize;
    if rows.len() <= y {
        rows.resize(y + 1, Vec::new());
    }
    rows[y].push(x);
}

pub fn arc_coords(angle: f64, rx: f64, ry: f64) -> Position {
    if rx == 0.0 || ry == 0.0 {
        return Position::new(0, 0);
    }

    let s = (angle * DEG2RAD).sin();
    let c = (angle * DEG2RAD).cos();
    if s.abs() < c.abs() {
        let tg = s / c;
        let xr = (rx * rx * ry * ry / (ry * ry + rx * rx * tg * tg)).sqrt();
        Position::new(
            (if c >= 0.0 { xr } else { -xr }).round() as i32,
            (if s >= 0.0 { -xr * tg } else { xr * tg }).round() as i32,
        )
    } else {
        let ctg = c / s;
        let yr = (rx * rx * ry * ry / (rx * rx + ry * ry * ctg * ctg)).sqrt();
        Position::new(
            (if c >= 0.0 { yr * ctg } else { -yr * ctg }).round() as i32,
            (if s >= 0.0 { -yr } else { yr }).round() as i32,
        )
    }
}

pub fn get_angle_size(angle: i32, radius_x: i32, radius_y: i32) -> Position {
    Position::new(
        ((angle as f64 * DEG2RAD).cos() * radius_x as f64).round() as i32,
        -((angle as f64 * DEG2RAD).sin() * radius_y as f64).round() as i32,
    )
}

fn already_drawn(fill_lines: &[Vec<LineInfo>], x: i32, y: i32) -> bool {
    for li in &fill_lines[y as usize] {
        if y == li.y && x >= li.x1 && x <= li.x2 {
            return true;
        }
    }
    false
}
