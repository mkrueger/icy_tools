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

/// RIPscrip command enumeration.
///
/// Each variant corresponds to a command defined in the original RIPscrip 1.54
/// specification (see `RIPSCRIP.TXT`). Parameters are already decoded from
/// their on‑wire Base‑36 representation into signed `i32` values; string / text
/// parameters are unescaped (the parser converts `\!`, `\|`, `\\` back to
/// literal characters). Display (`fmt::Display`) re‑encodes to canonical
/// Base‑36 form and re‑escapes text where required.
///
/// Conventions:
/// - Level 0 commands use a single `|<char>` prefix (after the mandatory `!`).
/// - Level 1 commands add `1` after the pipe (e.g. `|1M`). Level 9 similar.
/// - Angles are degrees, 0 == 3 o'clock, increasing counter‑clockwise.
/// - Coordinates are pixel (graphics) unless noted as text cell (TextWindow).
/// - Boolean flags decoded as non‑zero => true.
/// - Pattern / palette / style values retain their raw numeric meaning; caller
///   can interpret bitfields (e.g. `ButtonStyle.flags`).
///
/// Rendering notes (when converting back to string):
/// - Fixed size numeric fields are zero‑padded to their defined digit count.
/// - Variable length collections (`SetPalette`, polygons, etc.) are emitted in
///   the order received.
/// - Text is escaped per spec (`!`, `|`, `\`).
///
/// This enum intentionally provides semantic names for parameters instead of
/// the terse protocol fields to aid consumers. For authoritative semantics see
/// the spec sections referenced in each variant’s doc comment.
#[derive(Debug, Clone, PartialEq)]
pub enum RipCommand {
    // Level 0 commands
    /// RIP_TEXT_WINDOW (`|w`)
    /// Defines the TTY text window (character cell coordinates). `(x0,y0)` is
    /// upper‑left, `(x1,y1)` lower‑right inclusive. `wrap` controls horizontal
    /// (and vertical per spec) wrapping: when false, text past the right edge
    /// is truncated. `size` selects font height impacting valid coordinate
    /// ranges. Setting all coords to 0 hides the window.
    TextWindow {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        wrap: bool,
        size: i32,
    },
    /// RIP_VIEWPORT (`|v`)
    /// Defines graphics clipping rectangle in pixel coordinates. All graphics
    /// primitives are clipped to this viewport. Zero rectangle disables
    /// graphics.
    ViewPort { x0: i32, y0: i32, x1: i32, y1: i32 },
    /// RIP_RESET_WINDOWS (`|*`)
    /// Restores default text (80x43) & full graphics (640x350) windows, clears
    /// them with current background, resets palette, deletes mouse regions,
    /// buttons & clipboard.
    ResetWindows,
    /// RIP_ERASE_WINDOW (`|e`)
    /// Clears the text window to current graphics background color; cursor to
    /// upper‑left. Ignored if window inactive.
    EraseWindow,
    /// RIP_ERASE_VIEW (`|E`)
    /// Clears graphics viewport to current background color (clipped). Ignored
    /// if viewport disabled.
    EraseView,
    /// RIP_GOTOXY (`|g`)
    /// Sets text cursor position inside active text window (0‑based). Similar
    /// to ANSI ESC[x;yH but zero‑based & clipped.
    GotoXY { x: i32, y: i32 },
    /// RIP_HOME (`|H`) – text cursor to (0,0) of text window.
    Home,
    /// RIP_ERASE_EOL (`|>`)
    /// Clears from current text cursor to end of line using graphics bg color
    /// (differs from ANSI ESC[K which uses ANSI bg).
    EraseEOL,
    /// RIP_COLOR (`|c`)
    /// Sets current drawing color (0–15 index into RIP palette) used for line
    /// borders & text (graphics text), not fills.
    Color { c: i32 },
    /// RIP_SET_PALETTE (`|Q`)
    /// Reassigns all 16 palette entries; each value 0–63 (master palette).
    /// Instant recolor of already drawn items referencing entries.
    SetPalette { colors: Vec<i32> },
    /// RIP_ONE_PALETTE (`|a`)
    /// Changes a single palette entry `color` (0–15) to master value `value`
    /// (0–63). Enables simple cycling.
    OnePalette { color: i32, value: i32 },
    /// RIP_WRITE_MODE (`|W`)
    /// Selects drawing mode: 0 Normal (replace), 1 XOR (invert allowing rubber
    /// banding / temporary drawings).
    WriteMode { mode: i32 },
    /// RIP_MOVE (`|m`) – move graphics pen (drawing cursor) without drawing.
    Move { x: i32, y: i32 },
    /// RIP_TEXT (`|T`)
    /// Draws graphics text at current pen position using current font style,
    /// color, write mode, etc. Pen moves to end of rendered text.
    Text { text: String },
    /// RIP_TEXT_XY (`|@`)
    /// Combined Move + Text; draws at explicit pixel position then advances
    /// pen.
    TextXY { x: i32, y: i32, text: String },
    /// RIP_FONT_STYLE (`|Y`)
    /// Sets font id, direction (00 horizontal / 01 vertical), magnification
    /// size (01..0A), and reserved field.
    FontStyle { font: i32, direction: i32, size: i32, res: i32 },
    /// RIP_PIXEL (`|X`) – draw single pixel; rarely efficient, provided for completeness.
    Pixel { x: i32, y: i32 },
    /// RIP_LINE (`|L`) – draws line with current line style pattern & thickness.
    Line { x0: i32, y0: i32, x1: i32, y1: i32 },
    /// RIP_RECTANGLE (`|R`)
    /// Draws rectangle outline (no fill) honoring line style/thickness.
    Rectangle { x0: i32, y0: i32, x1: i32, y1: i32 },
    /// RIP_BAR (`|B`) – filled rectangle (no border) using current fill pattern/color.
    Bar { x0: i32, y0: i32, x1: i32, y1: i32 },
    /// RIP_CIRCLE (`|C`) – aspect‑aware circle (not ellipse); uses line thickness.
    Circle { x_center: i32, y_center: i32, radius: i32 },
    /// RIP_OVAL (`|O`) – elliptical arc from `st_ang` to `end_ang` (counter‑clockwise).
    Oval {
        x: i32,
        y: i32,
        st_ang: i32,
        end_ang: i32,
        x_rad: i32,
        y_rad: i32,
    },
    /// RIP_FILLED_OVAL (`|o`) – filled ellipse (full 360°) using fill pattern/color; outline uses drawing color & line thickness.
    FilledOval { x: i32, y: i32, x_rad: i32, y_rad: i32 },
    /// RIP_ARC (`|A`) – circular arc from `st_ang` to `end_ang` (counter‑clockwise); full circle if 0..360.
    Arc { x: i32, y: i32, st_ang: i32, end_ang: i32, radius: i32 },
    /// RIP_OVAL_ARC (`|V`) – elliptical arc segment (not aspect corrected circle).
    OvalArc {
        x: i32,
        y: i32,
        st_ang: i32,
        end_ang: i32,
        x_rad: i32,
        y_rad: i32,
    },
    /// RIP_PIE_SLICE (`|I`) – circular sector (arc + two radial lines) filled with current fill style; outline uses line thickness.
    PieSlice { x: i32, y: i32, st_ang: i32, end_ang: i32, radius: i32 },
    /// RIP_OVAL_PIE_SLICE (`|i`) – elliptical sector (arc + radial lines to center) filled.
    OvalPieSlice {
        x: i32,
        y: i32,
        st_ang: i32,
        end_ang: i32,
        x_rad: i32,
        y_rad: i32,
    },
    /// RIP_BEZIER (`|Z`)
    /// Cubic Bezier curve defined by endpoints (x1,y1)/(x4,y4) and control
    /// points (x2,y2)/(x3,y3). `cnt` is segment count (straight line subdivisions).
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
    /// RIP_POLYGON (`|P`)
    /// Closed polygon; number of vertices inferred by point vector length /2.
    /// Outline only; use `FilledPolygon` for fill.
    Polygon { points: Vec<i32> },
    /// RIP_FILL_POLYGON (`|p`) – polygon with filled interior & outlined border.
    FilledPolygon { points: Vec<i32> },
    /// RIP_POLYLINE (`|l`) – open multi‑segment path; last point not auto‑connected.
    PolyLine { points: Vec<i32> },
    /// RIP_FILL (`|F`)
    /// Flood fill from (x,y) until encountering `border` color (which is not overwritten). No action if start pixel is border color.
    Fill { x: i32, y: i32, border: i32 },
    /// RIP_LINE_STYLE (`|=`)
    /// Sets global line pattern & thickness. `style` selects predefined pattern
    /// or custom (value 4) using 16‑bit `user_pat` bitmask MSB = start side.
    LineStyle { style: i32, user_pat: i32, thick: i32 },
    /// RIP_FILL_STYLE (`|S`)
    /// Selects predefined 8x8 fill pattern `pattern` (0–11) and fill `color`.
    FillStyle { pattern: i32, color: i32 },
    /// RIP_FILL_PATTERN (`|s`)
    /// Custom 8x8 fill pattern rows `c1..c8` (bitfields) and fill color `col`.
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
    /// RIP_MOUSE (`|1M`)
    /// Declares clickable rectangular region; sends `text` (host command) when
    /// clicked. `clk` => invert visual feedback, `clr` => clear/zoom text window
    /// before host command. `num` obsolete (spec sets to 00). `res` reserved.
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
    /// RIP_KILL_MOUSE_FIELDS (`|1K`) – clears all defined mouse regions.
    MouseFields,
    /// RIP_BEGIN_TEXT (`|1T`)
    /// Starts formatted text region; subsequent `RegionText` lines flow within
    /// rectangle until `EndText`. `res` reserved.
    BeginText { x0: i32, y0: i32, x1: i32, y1: i32, res: i32 },
    /// RIP_REGION_TEXT (`|1t`)
    /// One wrapped line inside a begin/end block. `justify` true => full width
    /// justification (adds spacing between words). No scrolling beyond bottom.
    RegionText { justify: bool, text: String },
    /// RIP_END_TEXT (`|1E`) – terminates formatted text block.
    EndText,
    /// RIP_GET_IMAGE (`|1C`) – copies rectangle to internal clipboard. `res` reserved.
    GetImage { x0: i32, y0: i32, x1: i32, y1: i32, res: i32 },
    /// RIP_PUT_IMAGE (`|1P`) – pastes clipboard at (x,y) using write `mode`; `res` reserved.
    PutImage { x: i32, y: i32, mode: i32, res: i32 },
    /// RIP_WRITE_ICON (`|1W`) – writes clipboard to disk (icon); `res` is raw byte; `data` filename (no path). Overwrites existing.
    WriteIcon { res: u8, data: String },
    /// RIP_LOAD_ICON (`|1I`) – loads icon file to screen at (x,y); optional copy to clipboard if `clipboard`==1. `res` reserved.
    LoadIcon {
        x: i32,
        y: i32,
        mode: i32,
        clipboard: i32,
        res: i32,
        file_name: String,
    },
    /// RIP_BUTTON_STYLE (`|1B`)
    /// Defines styling for subsequent `Button` instances: static or dynamic
    /// sizing, orientation (label placement), effect colors (bright/dark/
    /// surface), group id (`grp_no` 0–35), flag bitfields (`flags` primary,
    /// `flags2` secondary), underline & corner colors, bevel thickness. `res`
    /// reserved (extended feature packing).
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
    /// RIP_BUTTON (`|1U`)
    /// Instance of a button in current style. Rect defines bounds (or dynamic
    /// sizing if style configured). `hotkey` two‑digit code, per spec features
    /// influenced by flags. `res` reserved. `text` label may include escaped
    /// chars.
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
    /// RIP_DEFINE (`|1D`) – defines named data / macro region; flags plus reserved field and text payload.
    Define { flags: i32, res: i32, text: String },
    /// RIP_QUERY (`|1ESC`) – query/command with mode & reserved triple‑digit quantity plus text payload.
    Query { mode: i32, res: i32, text: String },
    /// RIP_COPY_REGION (`|1G`) – copies rectangular region to destination scan line offset `dest_line` (implementation detail); `res` reserved.
    CopyRegion {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        res: i32,
        dest_line: i32,
    },
    /// RIP_READ_SCENE (`|1R`) – loads scene file (filename only, no path).
    ReadScene { file_name: String },
    /// RIP_FILE_QUERY (`|1F`) – queries file (existence / metadata) by name.
    FileQuery { file_name: String },

    // Level 9 commands
    /// RIP_ENTER_BLOCK_MODE (`|9ESC`)
    /// Initiates block/file transfer mode: protocol `proto`, file type
    /// `file_type`, reserved, plus `file_name` for upcoming transfer session.
    EnterBlockMode {
        mode: i32,
        proto: i32,
        file_type: i32,
        res: i32,
        file_name: String,
    },

    // Special commands
    /// RIP_TEXT_VARIABLE (`|$`) – defines a variable expansion text token.
    TextVariable { text: String },
    /// RIP_NO_MORE (`|#`) – terminator: signals end of RIP command stream & return to plain text/ANSI.
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
