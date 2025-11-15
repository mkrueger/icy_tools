use std::fmt;

/// Escape special characters in RIPscrip text strings.
/// Per spec: ! and | are command delimiters, \ is escape character.
/// Must escape: \! \| \\
fn escape_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '!' => result.push_str("\\!"),
            '|' => result.push_str("\\|"),
            '\\' => result.push_str("\\\\"),
            _ => result.push(ch),
        }
    }
    result
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
    /// |1t - Region Text: justify flag + text
    RegionText { justify: bool, text: String },
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
    /// |1D - Define: flags, reserved, text
    Define { flags: i32, res: i32, text: String },
    /// |1ESC - Query
    Query { mode: i32, res: i32, text: String },
    /// |1G - Copy Region: x0, y0, x1, y1, res, dest_line
    CopyRegion {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        res: i32,
        dest_line: i32,
    },
    /// |1R - Read Scene: filename
    ReadScene { file_name: String },
    /// |1F - File Query: filename
    FileQuery { file_name: String },

    // Level 9 commands
    /// |9ESC - Enter Block Mode
    EnterBlockMode {
        mode: i32,
        proto: i32,
        file_type: i32,
        res: i32,
        file_name: String,
    },

    // Special commands
    /// |$ - Text Variable: text
    TextVariable { text: String },
    /// |# - No More RIP (end of RIP commands)
    NoMore,
}

impl fmt::Display for RipCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Level 0 commands
            RipCommand::TextWindow { x0, y0, x1, y1, wrap, size } => {
                write!(
                    f,
                    "|w{}{}{}{}{}{}",
                    to_base_36(2, *x0),
                    to_base_36(2, *y0),
                    to_base_36(2, *x1),
                    to_base_36(2, *y1),
                    if *wrap { '1' } else { '0' },
                    to_base_36(1, *size),
                )
            }
            RipCommand::ViewPort { x0, y0, x1, y1 } => {
                write!(f, "|v{}{}{}{}", to_base_36(2, *x0), to_base_36(2, *y0), to_base_36(2, *x1), to_base_36(2, *y1))
            }
            RipCommand::ResetWindows => write!(f, "|*"),
            RipCommand::EraseWindow => write!(f, "|e"),
            RipCommand::EraseView => write!(f, "|E"),
            RipCommand::GotoXY { x, y } => {
                write!(f, "|g{}{}", to_base_36(2, *x), to_base_36(2, *y))
            }
            RipCommand::Home => write!(f, "|H"),
            RipCommand::EraseEOL => write!(f, "|>"),
            RipCommand::Color { c } => write!(f, "|c{}", to_base_36(2, *c)),
            RipCommand::SetPalette { colors } => {
                write!(f, "|Q")?;
                for color in colors {
                    write!(f, "{}", to_base_36(2, *color))?;
                }
                Ok(())
            }
            RipCommand::OnePalette { color, value } => {
                write!(f, "|a{}{}", to_base_36(2, *color), to_base_36(2, *value))
            }
            RipCommand::WriteMode { mode } => write!(f, "|W{}", to_base_36(2, *mode)),
            RipCommand::Move { x, y } => {
                write!(f, "|m{}{}", to_base_36(2, *x), to_base_36(2, *y))
            }
            RipCommand::Text { text } => write!(f, "|T{}", escape_text(text)),
            RipCommand::TextXY { x, y, text } => {
                write!(f, "|@{}{}{}", to_base_36(2, *x), to_base_36(2, *y), escape_text(text))
            }
            RipCommand::FontStyle { font, direction, size, res } => {
                write!(
                    f,
                    "|Y{}{}{}{}",
                    to_base_36(2, *font),
                    to_base_36(2, *direction),
                    to_base_36(2, *size),
                    to_base_36(2, *res)
                )
            }
            RipCommand::Pixel { x, y } => {
                write!(f, "|X{}{}", to_base_36(2, *x), to_base_36(2, *y))
            }
            RipCommand::Line { x0, y0, x1, y1 } => {
                write!(f, "|L{}{}{}{}", to_base_36(2, *x0), to_base_36(2, *y0), to_base_36(2, *x1), to_base_36(2, *y1))
            }
            RipCommand::Rectangle { x0, y0, x1, y1 } => {
                write!(f, "|R{}{}{}{}", to_base_36(2, *x0), to_base_36(2, *y0), to_base_36(2, *x1), to_base_36(2, *y1))
            }
            RipCommand::Bar { x0, y0, x1, y1 } => {
                write!(f, "|B{}{}{}{}", to_base_36(2, *x0), to_base_36(2, *y0), to_base_36(2, *x1), to_base_36(2, *y1))
            }
            RipCommand::Circle { x_center, y_center, radius } => {
                write!(f, "|C{}{}{}", to_base_36(2, *x_center), to_base_36(2, *y_center), to_base_36(2, *radius))
            }
            RipCommand::Oval {
                x,
                y,
                st_ang,
                end_ang,
                x_rad,
                y_rad,
            } => {
                write!(
                    f,
                    "|O{}{}{}{}{}{}",
                    to_base_36(2, *x),
                    to_base_36(2, *y),
                    to_base_36(2, *st_ang),
                    to_base_36(2, *end_ang),
                    to_base_36(2, *x_rad),
                    to_base_36(2, *y_rad)
                )
            }
            RipCommand::FilledOval { x, y, x_rad, y_rad } => {
                write!(
                    f,
                    "|o{}{}{}{}",
                    to_base_36(2, *x),
                    to_base_36(2, *y),
                    to_base_36(2, *x_rad),
                    to_base_36(2, *y_rad)
                )
            }
            RipCommand::Arc { x, y, st_ang, end_ang, radius } => {
                write!(
                    f,
                    "|A{}{}{}{}{}",
                    to_base_36(2, *x),
                    to_base_36(2, *y),
                    to_base_36(2, *st_ang),
                    to_base_36(2, *end_ang),
                    to_base_36(2, *radius)
                )
            }
            RipCommand::OvalArc {
                x,
                y,
                st_ang,
                end_ang,
                x_rad,
                y_rad,
            } => {
                write!(
                    f,
                    "|V{}{}{}{}{}{}",
                    to_base_36(2, *x),
                    to_base_36(2, *y),
                    to_base_36(2, *st_ang),
                    to_base_36(2, *end_ang),
                    to_base_36(2, *x_rad),
                    to_base_36(2, *y_rad)
                )
            }
            RipCommand::PieSlice { x, y, st_ang, end_ang, radius } => {
                write!(
                    f,
                    "|I{}{}{}{}{}",
                    to_base_36(2, *x),
                    to_base_36(2, *y),
                    to_base_36(2, *st_ang),
                    to_base_36(2, *end_ang),
                    to_base_36(2, *radius)
                )
            }
            RipCommand::OvalPieSlice {
                x,
                y,
                st_ang,
                end_ang,
                x_rad,
                y_rad,
            } => {
                write!(
                    f,
                    "|i{}{}{}{}{}{}",
                    to_base_36(2, *x),
                    to_base_36(2, *y),
                    to_base_36(2, *st_ang),
                    to_base_36(2, *end_ang),
                    to_base_36(2, *x_rad),
                    to_base_36(2, *y_rad)
                )
            }
            RipCommand::Bezier {
                x1,
                y1,
                x2,
                y2,
                x3,
                y3,
                x4,
                y4,
                cnt,
            } => {
                write!(
                    f,
                    "|Z{}{}{}{}{}{}{}{}{}",
                    to_base_36(2, *x1),
                    to_base_36(2, *y1),
                    to_base_36(2, *x2),
                    to_base_36(2, *y2),
                    to_base_36(2, *x3),
                    to_base_36(2, *y3),
                    to_base_36(2, *x4),
                    to_base_36(2, *y4),
                    to_base_36(2, *cnt)
                )
            }
            RipCommand::Polygon { points } => {
                write!(f, "|P{}", to_base_36(2, (points.len() / 2) as i32))?;
                for p in points {
                    write!(f, "{}", to_base_36(2, *p))?;
                }
                Ok(())
            }
            RipCommand::FilledPolygon { points } => {
                write!(f, "|p{}", to_base_36(2, (points.len() / 2) as i32))?;
                for p in points {
                    write!(f, "{}", to_base_36(2, *p))?;
                }
                Ok(())
            }
            RipCommand::PolyLine { points } => {
                write!(f, "|l{}", to_base_36(2, (points.len() / 2) as i32))?;
                for p in points {
                    write!(f, "{}", to_base_36(2, *p))?;
                }
                Ok(())
            }
            RipCommand::Fill { x, y, border } => {
                write!(f, "|F{}{}{}", to_base_36(2, *x), to_base_36(2, *y), to_base_36(2, *border))
            }
            RipCommand::LineStyle { style, user_pat, thick } => {
                write!(f, "|={}{}{}", to_base_36(2, *style), to_base_36(4, *user_pat), to_base_36(2, *thick))
            }
            RipCommand::FillStyle { pattern, color } => {
                write!(f, "|S{}{}", to_base_36(2, *pattern), to_base_36(2, *color))
            }
            RipCommand::FillPattern {
                c1,
                c2,
                c3,
                c4,
                c5,
                c6,
                c7,
                c8,
                col,
            } => {
                write!(
                    f,
                    "|s{}{}{}{}{}{}{}{}{}",
                    to_base_36(2, *c1),
                    to_base_36(2, *c2),
                    to_base_36(2, *c3),
                    to_base_36(2, *c4),
                    to_base_36(2, *c5),
                    to_base_36(2, *c6),
                    to_base_36(2, *c7),
                    to_base_36(2, *c8),
                    to_base_36(2, *col)
                )
            }

            // Level 1 commands
            RipCommand::Mouse {
                num,
                x0,
                y0,
                x1,
                y1,
                clk,
                clr,
                res,
                text,
            } => {
                write!(
                    f,
                    "|1M{}{}{}{}{}{}{}{}{}",
                    to_base_36(2, *num),
                    to_base_36(2, *x0),
                    to_base_36(2, *y0),
                    to_base_36(2, *x1),
                    to_base_36(2, *y1),
                    to_base_36(1, *clk),
                    to_base_36(1, *clr),
                    to_base_36(5, *res),
                    escape_text(text)
                )
            }
            RipCommand::MouseFields => write!(f, "|1K"),
            RipCommand::BeginText { x0, y0, x1, y1, res } => {
                write!(
                    f,
                    "|1T{}{}{}{}{}",
                    to_base_36(2, *x0),
                    to_base_36(2, *y0),
                    to_base_36(2, *x1),
                    to_base_36(2, *y1),
                    to_base_36(2, *res)
                )
            }
            RipCommand::RegionText { justify, text } => {
                write!(f, "|1t{}{}", if *justify { '1' } else { '0' }, escape_text(text))
            }
            RipCommand::EndText => write!(f, "|1E"),
            RipCommand::GetImage { x0, y0, x1, y1, res } => {
                write!(
                    f,
                    "|1C{}{}{}{}{}",
                    to_base_36(2, *x0),
                    to_base_36(2, *y0),
                    to_base_36(2, *x1),
                    to_base_36(2, *y1),
                    to_base_36(1, *res)
                )
            }
            RipCommand::PutImage { x, y, mode, res } => {
                write!(
                    f,
                    "|1P{}{}{}{}",
                    to_base_36(2, *x),
                    to_base_36(2, *y),
                    to_base_36(2, *mode),
                    to_base_36(1, *res)
                )
            }
            RipCommand::WriteIcon { res, data } => write!(f, "|1W{}{}", *res as char, escape_text(data)),
            RipCommand::LoadIcon {
                x,
                y,
                mode,
                clipboard,
                res,
                file_name,
            } => {
                write!(
                    f,
                    "|1I{}{}{}{}{}{}",
                    to_base_36(2, *x),
                    to_base_36(2, *y),
                    to_base_36(2, *mode),
                    to_base_36(1, *clipboard),
                    to_base_36(2, *res),
                    escape_text(file_name)
                )
            }
            RipCommand::ButtonStyle {
                wid,
                hgt,
                orient,
                flags,
                bevsize,
                dfore,
                dback,
                bright,
                dark,
                surface,
                grp_no,
                flags2,
                uline_col,
                corner_col,
                res,
            } => {
                write!(
                    f,
                    "|1B{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
                    to_base_36(2, *wid),
                    to_base_36(2, *hgt),
                    to_base_36(2, *orient),
                    to_base_36(4, *flags),
                    to_base_36(2, *bevsize),
                    to_base_36(2, *dfore),
                    to_base_36(2, *dback),
                    to_base_36(2, *bright),
                    to_base_36(2, *dark),
                    to_base_36(2, *surface),
                    to_base_36(2, *grp_no),
                    to_base_36(2, *flags2),
                    to_base_36(2, *uline_col),
                    to_base_36(2, *corner_col),
                    to_base_36(6, *res)
                )
            }
            RipCommand::Button {
                x0,
                y0,
                x1,
                y1,
                hotkey,
                flags,
                res,
                text,
            } => {
                write!(
                    f,
                    "|1U{}{}{}{}{}{}{}{}",
                    to_base_36(2, *x0),
                    to_base_36(2, *y0),
                    to_base_36(2, *x1),
                    to_base_36(2, *y1),
                    to_base_36(2, *hotkey),
                    to_base_36(1, *flags),
                    to_base_36(1, *res),
                    escape_text(text)
                )
            }
            RipCommand::Define { flags, res, text } => {
                write!(f, "|1D{}{}{}", to_base_36(3, *flags), to_base_36(2, *res), escape_text(text))
            }
            RipCommand::Query { mode, res, text } => {
                write!(f, "|1\x1B{}{}{}", to_base_36(1, *mode), to_base_36(3, *res), escape_text(text))
            }
            RipCommand::CopyRegion {
                x0,
                y0,
                x1,
                y1,
                res,
                dest_line,
            } => {
                write!(
                    f,
                    "|1G{}{}{}{}{}{}",
                    to_base_36(2, *x0),
                    to_base_36(2, *y0),
                    to_base_36(2, *x1),
                    to_base_36(2, *y1),
                    to_base_36(2, *res),
                    to_base_36(2, *dest_line)
                )
            }
            RipCommand::ReadScene { file_name } => write!(f, "|1R{}", escape_text(file_name)),
            RipCommand::FileQuery { file_name } => write!(f, "|1F{}", escape_text(file_name)),

            // Level 9 commands
            RipCommand::EnterBlockMode {
                mode,
                proto,
                file_type,
                res,
                file_name,
            } => {
                write!(
                    f,
                    "|9\x1B{}{}{}{}{}",
                    to_base_36(1, *mode),
                    to_base_36(1, *proto),
                    to_base_36(2, *file_type),
                    to_base_36(4, *res),
                    escape_text(file_name)
                )
            }

            // Special commands
            RipCommand::TextVariable { text } => write!(f, "|${}", escape_text(text)),
            RipCommand::NoMore => write!(f, "|#"),
        }
    }
}

/// Convert a number to base-36 representation with a fixed length
pub fn to_base_36(len: usize, number: i32) -> String {
    let mut res = String::new();
    let mut number = number;
    for _ in 0..len {
        let num2 = (number % 36) as u8;
        let ch2 = if num2 < 10 { (num2 + b'0') as char } else { (num2 - 10 + b'A') as char };

        res = ch2.to_string() + res.as_str();
        number /= 36;
    }
    res
}
