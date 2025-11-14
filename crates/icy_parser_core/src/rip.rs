//! RIPscrip (Remote Imaging Protocol Script) parser
//!
//! RIPscrip is a graphics-based BBS protocol that extends ANSI art with vector graphics,
//! buttons, and mouse support. Commands start with !| and use base-36 encoded parameters.

use crate::{AnsiParser, CommandParser, CommandSink};

/// Helper function to parse a base-36 character into a digit
#[inline]
fn parse_base36_digit(ch: u8) -> Option<i32> {
    match ch {
        b'0'..=b'9' => Some((ch - b'0') as i32),
        b'A'..=b'Z' => Some((ch - b'A' + 10) as i32),
        b'a'..=b'z' => Some((ch - b'a' + 10) as i32),
        _ => None,
    }
}

/// All RIPscrip commands
#[derive(Debug, Clone, PartialEq)]
pub enum RipCommand {
    // Level 0 commands
    /// |w - Text Window: x0, y0, x1, y1, wrap, size
    TextWindow {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        wrap: bool,
        size: i32,
    },
    /// |v - Viewport: x0, y0, x1, y1
    ViewPort { x0: i32, y0: i32, x1: i32, y1: i32 },
    /// |* - Reset Windows
    ResetWindows,
    /// |e - Erase Window
    EraseWindow,
    /// |E - Erase View (graphics viewport)
    EraseView,
    /// |g - Goto XY: x, y
    GotoXY { x: i32, y: i32 },
    /// |H - Home (goto 0,0)
    Home,
    /// |> - Erase to End of Line
    EraseEOL,
    /// |c - Color: c (0-15)
    Color { c: i32 },
    /// |Q - Set Palette: 16 colors (0-63 each)
    SetPalette { colors: Vec<i32> },
    /// |a - One Palette: color index, value
    OnePalette { color: i32, value: i32 },
    /// |W - Write Mode: mode (0=normal, 1=xor)
    WriteMode { mode: i32 },
    /// |m - Move: x, y
    Move { x: i32, y: i32 },
    /// |T - Text: text string
    Text { text: String },
    /// |@ - Text XY: x, y, text string
    TextXY { x: i32, y: i32, text: String },
    /// |Y - Font Style: font, direction, size, res
    FontStyle { font: i32, direction: i32, size: i32, res: i32 },
    /// |X - Pixel: x, y
    Pixel { x: i32, y: i32 },
    /// |L - Line: x0, y0, x1, y1
    Line { x0: i32, y0: i32, x1: i32, y1: i32 },
    /// |R - Rectangle: x0, y0, x1, y1
    Rectangle { x0: i32, y0: i32, x1: i32, y1: i32 },
    /// |B - Bar (filled rectangle): x0, y0, x1, y1
    Bar { x0: i32, y0: i32, x1: i32, y1: i32 },
    /// |C - Circle: x_center, y_center, radius
    Circle { x_center: i32, y_center: i32, radius: i32 },
    /// |O - Oval: x, y, start_angle, end_angle, x_radius, y_radius
    Oval {
        x: i32,
        y: i32,
        st_ang: i32,
        end_ang: i32,
        x_rad: i32,
        y_rad: i32,
    },
    /// |o - Filled Oval: x, y, x_radius, y_radius
    FilledOval { x: i32, y: i32, x_rad: i32, y_rad: i32 },
    /// |A - Arc: x, y, start_angle, end_angle, radius
    Arc { x: i32, y: i32, st_ang: i32, end_ang: i32, radius: i32 },
    /// |V - Oval Arc: x, y, start_angle, end_angle, x_radius, y_radius
    OvalArc {
        x: i32,
        y: i32,
        st_ang: i32,
        end_ang: i32,
        x_rad: i32,
        y_rad: i32,
    },
    /// |I - Pie Slice: x, y, start_angle, end_angle, radius
    PieSlice { x: i32, y: i32, st_ang: i32, end_ang: i32, radius: i32 },
    /// |i - Oval Pie Slice: x, y, start_angle, end_angle, x_radius, y_radius
    OvalPieSlice {
        x: i32,
        y: i32,
        st_ang: i32,
        end_ang: i32,
        x_rad: i32,
        y_rad: i32,
    },
    /// |Z - Bezier: x1, y1, x2, y2, x3, y3, x4, y4, count
    Bezier {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        x3: i32,
        y3: i32,
        x4: i32,
        y4: i32,
        cnt: i32,
    },
    /// |P - Polygon: points (npoints followed by x,y pairs)
    Polygon { points: Vec<i32> },
    /// |p - Filled Polygon: points (npoints followed by x,y pairs)
    FilledPolygon { points: Vec<i32> },
    /// |l - Polyline: points (npoints followed by x,y pairs)
    PolyLine { points: Vec<i32> },
    /// |F - Fill: x, y, border_color
    Fill { x: i32, y: i32, border: i32 },
    /// |= - Line Style: style, user_pattern, thickness
    LineStyle { style: i32, user_pat: i32, thick: i32 },
    /// |S - Fill Style: pattern, color
    FillStyle { pattern: i32, color: i32 },
    /// |s - Fill Pattern: 8 bytes + color
    FillPattern {
        c1: i32,
        c2: i32,
        c3: i32,
        c4: i32,
        c5: i32,
        c6: i32,
        c7: i32,
        c8: i32,
        col: i32,
    },

    // Level 1 commands
    /// |1M - Mouse: num, x0, y0, x1, y1, click, clear, reserved, text
    Mouse {
        num: i32,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        clk: i32,
        clr: i32,
        res: i32,
        text: String,
    },
    /// |1K - Mouse Fields (clear all mouse regions)
    MouseFields,
    /// |1T - Begin Text: x0, y0, x1, y1, reserved
    BeginText { x0: i32, y0: i32, x1: i32, y1: i32, res: i32 },
    /// |1t - Region Text: x, y, w, h, reserved
    RegionText { x: i32, y: i32, w: i32, h: i32, res: i32 },
    /// |1E - End Text
    EndText,
    /// |1C - Get Image (copy): x0, y0, x1, y1, reserved
    GetImage { x0: i32, y0: i32, x1: i32, y1: i32, res: i32 },
    /// |1P - Put Image (paste): x, y, mode, reserved
    PutImage { x: i32, y: i32, mode: i32, res: i32 },
    /// |1W - Write Icon: reserved, data string
    WriteIcon { res: u8, data: String },
    /// |1I - Load Icon: x, y, mode, clipboard, reserved, filename
    LoadIcon {
        x: i32,
        y: i32,
        mode: i32,
        clipboard: i32,
        res: i32,
        file_name: String,
    },
    /// |1B - Button Style: width, height, orientation, flags, bevel_size, label_color, shadow_color, bright, dark, surface, group, flags2, underline_color, corner_color, reserved
    ButtonStyle {
        wid: i32,
        hgt: i32,
        orient: i32,
        flags: i32,
        bevsize: i32,
        dfore: i32,
        dback: i32,
        bright: i32,
        dark: i32,
        surface: i32,
        grp_no: i32,
        flags2: i32,
        uline_col: i32,
        corner_col: i32,
        res: i32,
    },
    /// |1U - Button: x0, y0, x1, y1, hotkey, flags, reserved, text
    Button {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        hotkey: i32,
        flags: i32,
        res: i32,
        text: String,
    },
    /// |1D - Define: reserved, text
    Define { res: u8, text: String },
    /// |1ESC - Query
    Query { query: Vec<i32> },
    /// |1G - Copy Region: x0, y0, x1, y1, dest_x, dest_y, mode, reserved
    CopyRegion {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        dest_x: i32,
        dest_y: i32,
        mode: i32,
        res: i32,
    },
    /// |1R - Read Scene: filename
    ReadScene { file_name: String },
    /// |1F - File Query: filename
    FileQuery { file_name: String },

    // Level 9 commands
    /// |9ESC - Enter Block Mode
    EnterBlockMode,

    // Special commands
    /// |$ - Text Variable: text
    TextVariable { text: String },
    /// |# - No More RIP (end of RIP commands)
    NoMore,
}

#[derive(Default, Clone, Debug, PartialEq)]
enum State {
    #[default]
    Default,
    GotExclaim,
    GotPipe,
    ReadCommand,
    ReadLevel1,
    ReadLevel9,
    ReadParams,
    SkipToEOL(Box<State>), // Store the state to return to after EOL
}

#[derive(Default, Clone, Debug, PartialEq)]
enum ParserMode {
    #[default]
    NonRip, // Use ANSI parser for text
    Rip,    // RIP command mode
}

#[derive(Default)]
struct CommandBuilder {
    cmd_char: u8,
    level: u8,
    param_state: usize,
    npoints: i32,

    // Reusable buffers for command parameters
    i32_params: Vec<i32>,
    string_param: String,
    char_param: u8,
}

impl CommandBuilder {
    fn reset(&mut self) {
        self.cmd_char = 0;
        self.level = 0;
        self.param_state = 0;
        self.npoints = 0;
        self.i32_params.clear();
        self.string_param.clear();
        self.char_param = 0;
    }

    fn parse_base36_2digit(&mut self, ch: u8, target_idx: usize) -> Result<bool, ()> {
        let digit = parse_base36_digit(ch).ok_or(())?;
        if self.param_state % 2 == 0 {
            if self.i32_params.len() <= target_idx {
                self.i32_params.resize(target_idx + 1, 0);
            }
            self.i32_params[target_idx] = digit;
        } else {
            self.i32_params[target_idx] = self.i32_params[target_idx] * 36 + digit;
        }
        self.param_state += 1;
        Ok(false) // Not done yet
    }

    fn parse_base36_complete(&mut self, ch: u8, target_idx: usize, final_state: usize) -> Result<bool, ()> {
        let digit = parse_base36_digit(ch).ok_or(())?;
        if self.param_state % 2 == 0 {
            if self.i32_params.len() <= target_idx {
                self.i32_params.resize(target_idx + 1, 0);
            }
            self.i32_params[target_idx] = digit;
        } else {
            self.i32_params[target_idx] = self.i32_params[target_idx] * 36 + digit;
        }
        self.param_state += 1;
        Ok(self.param_state > final_state)
    }
}

pub struct RipParser {
    mode: ParserMode,
    state: State,
    builder: CommandBuilder,
    ansi_parser: AnsiParser,
}

impl RipParser {
    pub fn new() -> Self {
        Self {
            mode: ParserMode::default(),
            state: State::Default,
            builder: CommandBuilder::default(),
            ansi_parser: AnsiParser::new(),
        }
    }

    fn emit_command(&mut self, sink: &mut dyn CommandSink) {
        let cmd = match (self.builder.level, self.builder.cmd_char) {
            // Level 0 commands
            (0, b'w') if self.builder.i32_params.len() >= 5 => RipCommand::TextWindow {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
                wrap: self.builder.i32_params[4] != 0,
                size: *self.builder.i32_params.get(5).unwrap_or(&0),
            },
            (0, b'v') if self.builder.i32_params.len() >= 4 => RipCommand::ViewPort {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
            },
            (0, b'*') => RipCommand::ResetWindows,
            (0, b'e') => RipCommand::EraseWindow,
            (0, b'E') => RipCommand::EraseView,
            (0, b'g') if self.builder.i32_params.len() >= 2 => RipCommand::GotoXY {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
            },
            (0, b'H') => RipCommand::Home,
            (0, b'>') => RipCommand::EraseEOL,
            (0, b'c') if !self.builder.i32_params.is_empty() => RipCommand::Color { c: self.builder.i32_params[0] },
            (0, b'Q') => RipCommand::SetPalette {
                colors: self.builder.i32_params.clone(),
            },
            (0, b'a') if self.builder.i32_params.len() >= 2 => RipCommand::OnePalette {
                color: self.builder.i32_params[0],
                value: self.builder.i32_params[1],
            },
            (0, b'W') if !self.builder.i32_params.is_empty() => RipCommand::WriteMode {
                mode: self.builder.i32_params[0],
            },
            (0, b'm') if self.builder.i32_params.len() >= 2 => RipCommand::Move {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
            },
            (0, b'T') => RipCommand::Text {
                text: self.builder.string_param.clone(),
            },
            (0, b'@') if self.builder.i32_params.len() >= 2 => RipCommand::TextXY {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                text: self.builder.string_param.clone(),
            },
            (0, b'Y') if self.builder.i32_params.len() >= 4 => RipCommand::FontStyle {
                font: self.builder.i32_params[0],
                direction: self.builder.i32_params[1],
                size: self.builder.i32_params[2],
                res: self.builder.i32_params[3],
            },
            (0, b'X') if self.builder.i32_params.len() >= 2 => RipCommand::Pixel {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
            },
            (0, b'L') if self.builder.i32_params.len() >= 4 => RipCommand::Line {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
            },
            (0, b'R') if self.builder.i32_params.len() >= 4 => RipCommand::Rectangle {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
            },
            (0, b'B') if self.builder.i32_params.len() >= 4 => RipCommand::Bar {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
            },
            (0, b'C') if self.builder.i32_params.len() >= 3 => RipCommand::Circle {
                x_center: self.builder.i32_params[0],
                y_center: self.builder.i32_params[1],
                radius: self.builder.i32_params[2],
            },
            (0, b'O') if self.builder.i32_params.len() >= 6 => RipCommand::Oval {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                st_ang: self.builder.i32_params[2],
                end_ang: self.builder.i32_params[3],
                x_rad: self.builder.i32_params[4],
                y_rad: self.builder.i32_params[5],
            },
            (0, b'o') if self.builder.i32_params.len() >= 4 => RipCommand::FilledOval {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                x_rad: self.builder.i32_params[2],
                y_rad: self.builder.i32_params[3],
            },
            (0, b'A') if self.builder.i32_params.len() >= 5 => RipCommand::Arc {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                st_ang: self.builder.i32_params[2],
                end_ang: self.builder.i32_params[3],
                radius: self.builder.i32_params[4],
            },
            (0, b'V') if self.builder.i32_params.len() >= 6 => RipCommand::OvalArc {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                st_ang: self.builder.i32_params[2],
                end_ang: self.builder.i32_params[3],
                x_rad: self.builder.i32_params[4],
                y_rad: self.builder.i32_params[5],
            },
            (0, b'I') if self.builder.i32_params.len() >= 5 => RipCommand::PieSlice {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                st_ang: self.builder.i32_params[2],
                end_ang: self.builder.i32_params[3],
                radius: self.builder.i32_params[4],
            },
            (0, b'i') if self.builder.i32_params.len() >= 6 => RipCommand::OvalPieSlice {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                st_ang: self.builder.i32_params[2],
                end_ang: self.builder.i32_params[3],
                x_rad: self.builder.i32_params[4],
                y_rad: self.builder.i32_params[5],
            },
            (0, b'Z') if self.builder.i32_params.len() >= 9 => RipCommand::Bezier {
                x1: self.builder.i32_params[0],
                y1: self.builder.i32_params[1],
                x2: self.builder.i32_params[2],
                y2: self.builder.i32_params[3],
                x3: self.builder.i32_params[4],
                y3: self.builder.i32_params[5],
                x4: self.builder.i32_params[6],
                y4: self.builder.i32_params[7],
                cnt: self.builder.i32_params[8],
            },
            (0, b'P') => RipCommand::Polygon {
                points: self.builder.i32_params.clone(),
            },
            (0, b'p') => RipCommand::FilledPolygon {
                points: self.builder.i32_params.clone(),
            },
            (0, b'l') => RipCommand::PolyLine {
                points: self.builder.i32_params.clone(),
            },
            (0, b'F') if self.builder.i32_params.len() >= 3 => RipCommand::Fill {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                border: self.builder.i32_params[2],
            },
            (0, b'=') if self.builder.i32_params.len() >= 3 => RipCommand::LineStyle {
                style: self.builder.i32_params[0],
                user_pat: self.builder.i32_params[1],
                thick: self.builder.i32_params[2],
            },
            (0, b'S') if self.builder.i32_params.len() >= 2 => RipCommand::FillStyle {
                pattern: self.builder.i32_params[0],
                color: self.builder.i32_params[1],
            },
            (0, b's') if self.builder.i32_params.len() >= 9 => RipCommand::FillPattern {
                c1: self.builder.i32_params[0],
                c2: self.builder.i32_params[1],
                c3: self.builder.i32_params[2],
                c4: self.builder.i32_params[3],
                c5: self.builder.i32_params[4],
                c6: self.builder.i32_params[5],
                c7: self.builder.i32_params[6],
                c8: self.builder.i32_params[7],
                col: self.builder.i32_params[8],
            },
            (0, b'$') => RipCommand::TextVariable {
                text: self.builder.string_param.clone(),
            },
            (0, b'#') => RipCommand::NoMore,

            // Level 1 commands
            (1, b'M') if self.builder.i32_params.len() >= 8 => RipCommand::Mouse {
                num: self.builder.i32_params[0],
                x0: self.builder.i32_params[1],
                y0: self.builder.i32_params[2],
                x1: self.builder.i32_params[3],
                y1: self.builder.i32_params[4],
                clk: self.builder.i32_params[5],
                clr: self.builder.i32_params[6],
                res: self.builder.i32_params[7],
                text: self.builder.string_param.clone(),
            },
            (1, b'K') => RipCommand::MouseFields,
            (1, b'T') if self.builder.i32_params.len() >= 5 => RipCommand::BeginText {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
                res: self.builder.i32_params[4],
            },
            (1, b't') if self.builder.i32_params.len() >= 5 => RipCommand::RegionText {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                w: self.builder.i32_params[2],
                h: self.builder.i32_params[3],
                res: self.builder.i32_params[4],
            },
            (1, b'E') => RipCommand::EndText,
            (1, b'C') if self.builder.i32_params.len() >= 5 => RipCommand::GetImage {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
                res: self.builder.i32_params[4],
            },
            (1, b'P') if self.builder.i32_params.len() >= 4 => RipCommand::PutImage {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                mode: self.builder.i32_params[2],
                res: self.builder.i32_params[3],
            },
            (1, b'W') => RipCommand::WriteIcon {
                res: self.builder.char_param,
                data: self.builder.string_param.clone(),
            },
            (1, b'I') if self.builder.i32_params.len() >= 5 => RipCommand::LoadIcon {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                mode: self.builder.i32_params[2],
                clipboard: self.builder.i32_params[3],
                res: self.builder.i32_params[4],
                file_name: self.builder.string_param.clone(),
            },
            (1, b'B') if self.builder.i32_params.len() >= 15 => RipCommand::ButtonStyle {
                wid: self.builder.i32_params[0],
                hgt: self.builder.i32_params[1],
                orient: self.builder.i32_params[2],
                flags: self.builder.i32_params[3],
                bevsize: self.builder.i32_params[4],
                dfore: self.builder.i32_params[5],
                dback: self.builder.i32_params[6],
                bright: self.builder.i32_params[7],
                dark: self.builder.i32_params[8],
                surface: self.builder.i32_params[9],
                grp_no: self.builder.i32_params[10],
                flags2: self.builder.i32_params[11],
                uline_col: self.builder.i32_params[12],
                corner_col: self.builder.i32_params[13],
                res: self.builder.i32_params[14],
            },
            (1, b'U') if self.builder.i32_params.len() >= 7 => RipCommand::Button {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
                hotkey: self.builder.i32_params[4],
                flags: self.builder.i32_params[5],
                res: self.builder.i32_params[6],
                text: self.builder.string_param.clone(),
            },
            (1, b'D') => RipCommand::Define {
                res: self.builder.char_param,
                text: self.builder.string_param.clone(),
            },
            (1, 0x1B) => RipCommand::Query {
                query: self.builder.i32_params.clone(),
            },
            (1, b'G') if self.builder.i32_params.len() >= 8 => RipCommand::CopyRegion {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
                dest_x: self.builder.i32_params[4],
                dest_y: self.builder.i32_params[5],
                mode: self.builder.i32_params[6],
                res: self.builder.i32_params[7],
            },
            (1, b'R') => RipCommand::ReadScene {
                file_name: self.builder.string_param.clone(),
            },
            (1, b'F') => RipCommand::FileQuery {
                file_name: self.builder.string_param.clone(),
            },

            // Level 9 commands
            (9, 0x1B) => RipCommand::EnterBlockMode,

            _ => {
                // Unknown command - don't emit anything
                return;
            }
        };

        sink.emit_rip(cmd);
    }

    fn parse_params(&mut self, ch: u8, sink: &mut dyn CommandSink) -> bool {
        // Handle line continuation - backslash skips to end of line
        if ch == b'\\' {
            self.state = State::SkipToEOL(Box::new(State::ReadParams));
            return true;
        }

        // Handle command termination
        if ch == b'\r' {
            return true;
        }
        if ch == b'\n' {
            self.emit_command(sink);
            self.builder.reset();
            self.state = State::Default;
            // Stay in RIP mode after command completes
            return true;
        }
        if ch == b'|' {
            self.emit_command(sink);
            self.builder.reset();
            self.state = State::GotPipe;
            return true;
        }

        // Parse parameters based on command
        let result = match (self.builder.level, self.builder.cmd_char) {
            // Commands with no parameters
            (0, b'*') | (0, b'e') | (0, b'E') | (0, b'H') | (0, b'>') | (0, b'#') | (1, b'K') | (1, b'E') | (9, 0x1B) => {
                // Immediate commands - complete immediately
                self.emit_command(sink);
                self.builder.reset();
                self.state = State::GotExclaim;
                return true;
            }

            // Text commands (consume rest as string)
            (0, b'T') | (0, b'$') | (1, b'R') | (1, b'F') => {
                self.builder.string_param.push(ch as char);
                Ok(false)
            }

            // TextXY, Button - initial params then string
            (0, b'@') if self.builder.param_state < 4 => {
                let result = self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 3);
                // Don't signal completion even if params are done - we still need the string
                match result {
                    Ok(_) => Ok(false),
                    Err(e) => Err(e),
                }
            }
            (0, b'@') => {
                self.builder.string_param.push(ch as char);
                Ok(false)
            }

            // Button - 7 params (14 digits) then string
            (1, b'U') if self.builder.param_state < 14 => {
                let result = self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 13);
                // Don't signal completion even if params are done - we still need the string
                match result {
                    Ok(_) => Ok(false),
                    Err(e) => Err(e),
                }
            }
            (1, b'U') => {
                self.builder.string_param.push(ch as char);
                Ok(false)
            }

            // Mouse - 8 params (16 digits) then string
            (1, b'M') if self.builder.param_state < 16 => {
                let result = self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 15);
                // Don't signal completion even if params are done - we still need the string
                match result {
                    Ok(_) => Ok(false),
                    Err(e) => Err(e),
                }
            }
            (1, b'M') => {
                self.builder.string_param.push(ch as char);
                Ok(false)
            }

            // WriteIcon, Define - char then string
            (1, b'W') if self.builder.param_state == 0 => {
                self.builder.char_param = ch;
                self.builder.param_state += 1;
                Ok(false)
            }
            (1, b'W') => {
                self.builder.string_param.push(ch as char);
                Ok(false)
            }
            (1, b'D') if self.builder.param_state == 0 => {
                self.builder.char_param = ch;
                self.builder.param_state += 1;
                Ok(false)
            }
            (1, b'D') => {
                self.builder.string_param.push(ch as char);
                Ok(false)
            }

            // LoadIcon - 5 params (10 digits) then string
            (1, b'I') if self.builder.param_state < 10 => {
                let result = self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 9);
                // Don't signal completion even if params are done - we still need the string
                match result {
                    Ok(_) => Ok(false),
                    Err(e) => Err(e),
                }
            }
            (1, b'I') => {
                self.builder.string_param.push(ch as char);
                Ok(false)
            }

            // Simple 2-digit parameter commands
            (0, b'c') => self.builder.parse_base36_complete(ch, 0, 1),
            (0, b'W') => self.builder.parse_base36_complete(ch, 0, 1),

            // 4-digit parameter commands
            (0, b'g') | (0, b'm') | (0, b'X') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 3),

            // 6-digit parameter commands
            (0, b'a') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 3),
            (0, b'C') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 5),
            (0, b'F') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 5),

            // 8-digit parameter commands
            (0, b'v') | (0, b'L') | (0, b'R') | (0, b'B') | (0, b'o') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 7),

            // TextWindow: 4 two-digit params, then wrap (1 digit), then size (1 digit)
            (0, b'w') if self.builder.param_state < 8 => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 8),
            (0, b'w') if self.builder.param_state == 8 => {
                if let Some(digit) = parse_base36_digit(ch) {
                    self.builder.i32_params.push(digit);
                    self.builder.param_state += 1;
                    Ok(false)
                } else {
                    Err(())
                }
            }
            (0, b'w') => {
                // param_state == 9: final single digit parameter (size)
                if let Some(digit) = parse_base36_digit(ch) {
                    self.builder.i32_params.push(digit);
                    self.builder.param_state += 1;
                    Ok(true)
                } else {
                    Err(())
                }
            }

            // A - Arc (10 digits: 5 params)
            (0, b'A') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 9),
            (0, b'I') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 9),

            // O, V, i - Oval commands (12 digits: 6 params)
            (0, b'O') | (0, b'V') | (0, b'i') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 11),

            // Y - Font Style (8 digits)
            (0, b'Y') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 7),

            // Z - Bezier (18 digits: 9 params)
            (0, b'Z') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 17),

            // = - Line Style (2 + 4 + 2 = 8 digits)
            (0, b'=') if self.builder.param_state < 2 => self.builder.parse_base36_complete(ch, 0, 1),
            (0, b'=') if self.builder.param_state < 6 => {
                if let Some(digit) = parse_base36_digit(ch) {
                    if self.builder.param_state == 2 {
                        self.builder.i32_params.push(digit);
                    } else {
                        let idx = self.builder.i32_params.len() - 1;
                        self.builder.i32_params[idx] = self.builder.i32_params[idx] * 36 + digit;
                    }
                    self.builder.param_state += 1;
                    Ok(false)
                } else {
                    Err(())
                }
            }
            (0, b'=') => self.builder.parse_base36_complete(ch, 2, 7),

            // S - Fill Style (4 digits)
            (0, b'S') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 3),

            // s - Fill Pattern (18 digits)
            (0, b's') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 17),

            // Q - Set Palette (32 digits for 16 colors)
            (0, b'Q') => {
                if let Some(digit) = parse_base36_digit(ch) {
                    if self.builder.param_state % 2 == 0 {
                        self.builder.i32_params.push(digit);
                    } else {
                        let idx = self.builder.i32_params.len() - 1;
                        self.builder.i32_params[idx] = self.builder.i32_params[idx] * 36 + digit;
                    }
                    self.builder.param_state += 1;
                    Ok(self.builder.param_state >= 32)
                } else {
                    Err(())
                }
            }

            // P, p, l - Polygon/PolyLine (variable length based on npoints)
            (0, b'P') | (0, b'p') | (0, b'l') if self.builder.param_state < 2 => {
                if let Some(digit) = parse_base36_digit(ch) {
                    if self.builder.param_state == 0 {
                        self.builder.npoints = digit;
                    } else {
                        self.builder.npoints = self.builder.npoints * 36 + digit;
                    }
                    self.builder.param_state += 1;
                    Ok(false)
                } else {
                    Err(())
                }
            }
            (0, b'P') | (0, b'p') | (0, b'l') => {
                if let Some(digit) = parse_base36_digit(ch) {
                    if self.builder.param_state % 2 == 0 {
                        self.builder.i32_params.push(digit);
                    } else {
                        let idx = self.builder.i32_params.len() - 1;
                        self.builder.i32_params[idx] = self.builder.i32_params[idx] * 36 + digit;
                    }
                    self.builder.param_state += 1;
                    let expected = 2 + self.builder.npoints * 4;
                    Ok(self.builder.param_state >= expected as usize)
                } else {
                    Err(())
                }
            }

            // Level 1 commands
            (1, b'T') | (1, b't') | (1, b'C') | (1, b'P') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 9),

            (1, b'B') if self.builder.param_state < 10 => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 9),
            (1, b'B') if self.builder.param_state < 14 => {
                // Flags: 4 digits
                if let Some(digit) = parse_base36_digit(ch) {
                    if self.builder.param_state == 10 {
                        self.builder.i32_params.push(digit);
                    } else {
                        let idx = self.builder.i32_params.len() - 1;
                        self.builder.i32_params[idx] = self.builder.i32_params[idx] * 36 + digit;
                    }
                    self.builder.param_state += 1;
                    Ok(false)
                } else {
                    Err(())
                }
            }
            (1, b'B') if self.builder.param_state < 36 => {
                // Remaining 2-digit fields
                self.builder.parse_base36_complete(ch, 5 + (self.builder.param_state - 14) / 2, 35)
            }
            (1, b'B') => {
                // Reserved: 7 digits
                if let Some(digit) = parse_base36_digit(ch) {
                    if self.builder.param_state == 36 {
                        self.builder.i32_params.push(digit);
                    } else {
                        let idx = self.builder.i32_params.len() - 1;
                        self.builder.i32_params[idx] = self.builder.i32_params[idx] * 36 + digit;
                    }
                    self.builder.param_state += 1;
                    Ok(self.builder.param_state >= 43)
                } else {
                    Err(())
                }
            }

            (1, b'G') => self.builder.parse_base36_complete(ch, self.builder.param_state / 2, 15),

            (1, 0x1B) => {
                // Query - collect all digits
                if let Some(digit) = parse_base36_digit(ch) {
                    self.builder.i32_params.push(digit);
                    self.builder.param_state += 1;
                    Ok(false)
                } else {
                    Err(())
                }
            }

            _ => Err(()),
        };

        match result {
            Ok(true) => {
                // Command complete
                self.emit_command(sink);
                self.builder.reset();
                self.state = State::GotExclaim;
                true
            }
            Ok(false) => {
                // Continue parsing
                true
            }
            Err(()) => {
                // Parse error - abort command and return to NonRip mode
                self.builder.reset();
                self.mode = ParserMode::NonRip;
                self.state = State::Default;
                false
            }
        }
    }
}

impl Default for RipParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandParser for RipParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        for &ch in input {
            // Check for backslash (line continuation) in any RIP state
            if self.mode == ParserMode::Rip && ch == b'\\' && !matches!(self.state, State::SkipToEOL(_) | State::Default) {
                self.state = State::SkipToEOL(Box::new(self.state.clone()));
                continue;
            }

            match &self.state.clone() {
                State::Default => {
                    match self.mode {
                        ParserMode::NonRip => {
                            if ch == b'!' {
                                self.mode = ParserMode::Rip;
                                self.state = State::GotExclaim;
                            } else {
                                // Pass through to ANSI parser
                                self.ansi_parser.parse(&[ch], sink);
                            }
                        }
                        ParserMode::Rip => {
                            if ch == b'!' {
                                self.state = State::GotExclaim;
                            } else {
                                // In RIP mode without !, treat as error and go back to NonRip
                                self.mode = ParserMode::NonRip;
                                self.ansi_parser.parse(&[ch], sink);
                            }
                        }
                    }
                }
                State::GotExclaim => {
                    if ch == b'!' {
                        // Double ! - stay in GotExclaim
                        continue;
                    } else if ch == b'|' {
                        self.state = State::GotPipe;
                    } else if ch == b'\n' || ch == b'\r' {
                        // End of line after ! - reset to NonRip mode
                        self.mode = ParserMode::NonRip;
                        self.state = State::Default;
                        self.ansi_parser.parse(&[ch], sink);
                    } else {
                        // Not a RIP command - emit ! and continue in NonRip mode
                        self.mode = ParserMode::NonRip;
                        self.state = State::Default;
                        self.ansi_parser.parse(b"!", sink);
                        self.ansi_parser.parse(&[ch], sink);
                    }
                }
                State::GotPipe => {
                    // Read command character
                    if ch == b'1' {
                        self.builder.level = 1;
                        self.state = State::ReadLevel1;
                    } else if ch == b'9' {
                        self.builder.level = 9;
                        self.state = State::ReadLevel9;
                    } else if ch == b'#' {
                        // No more RIP
                        self.builder.cmd_char = b'#';
                        self.builder.level = 0;
                        self.emit_command(sink);
                        self.builder.reset();
                        self.mode = ParserMode::NonRip;
                        self.state = State::Default;
                    } else {
                        // Level 0 command
                        self.builder.level = 0;
                        self.builder.cmd_char = ch;
                        self.state = State::ReadParams;
                    }
                }
                State::ReadLevel1 => {
                    self.builder.cmd_char = ch;
                    self.state = State::ReadParams;
                }
                State::ReadLevel9 => {
                    self.builder.cmd_char = ch;
                    self.state = State::ReadParams;
                }
                State::ReadParams => {
                    if !self.parse_params(ch, sink) {
                        // Parse error - already reset by parse_params
                    }
                }
                State::SkipToEOL(return_state) => {
                    if ch == b'\n' {
                        // Return to the saved state
                        self.state = (**return_state).clone();
                    }
                    // Ignore everything else until newline
                }
                State::ReadCommand => {
                    // Shouldn't reach here
                    self.mode = ParserMode::NonRip;
                    self.state = State::Default;
                }
            }
        }
    }
}
