use eframe::egui::{self, Color32, Stroke};
use icy_draw::{fl, rip_document::RipDocument};
use icy_engine::Screen;
use icy_parser_core::{FillStyle, LineStyle, RipCommand};
use std::path::Path;

use super::playback::{Action, RowMark, Timeline, Transport};
use super::widgets::{self, Icons};

#[path = "rip_button.rs"]
mod button;
use button::{ButtonDialog, ButtonKind, ButtonOptions, ButtonTarget, DialogResult};
#[path = "rip_palette.rs"]
mod palette;
use palette::{PaletteDialog, PaletteResult};
#[path = "rip_select.rs"]
mod select;
use select::{Geometry, Handle};

const WIDTH: u16 = 640;
const HEIGHT: u16 = 350;
const TOOLBAR_HEIGHT: f32 = 44.0;
/// Screen distance in points within which a Bézier handle is picked up.
const HANDLE_RADIUS: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Select,
    Pixel,
    Line,
    Rectangle,
    Bar,
    Circle,
    Oval,
    FilledOval,
    Polygon,
    FilledPolygon,
    PolyLine,
    Arc,
    OvalArc,
    PieSlice,
    OvalPieSlice,
    Text,
    Bezier,
    Button,
    Mouse,
}

impl Tool {
    /// The tool column; buttons are placed through "Create Button…" instead.
    const ALL: [Self; 18] = [
        Self::Select,
        Self::Pixel,
        Self::Line,
        Self::Rectangle,
        Self::Bar,
        Self::Circle,
        Self::Oval,
        Self::FilledOval,
        Self::Polygon,
        Self::FilledPolygon,
        Self::PolyLine,
        Self::Arc,
        Self::OvalArc,
        Self::PieSlice,
        Self::OvalPieSlice,
        Self::Text,
        Self::Bezier,
        Self::Mouse,
    ];

    fn label(self) -> String {
        match self {
            Self::Select => fl!("rip-editor-select"),
            Self::Pixel => fl!("rip-editor-pixel"),
            Self::Line => fl!("rip-editor-line"),
            Self::Rectangle => fl!("rip-editor-rectangle"),
            Self::Bar => fl!("rip-editor-bar"),
            Self::Circle => fl!("rip-editor-circle"),
            Self::Oval => fl!("rip-editor-outline-oval"),
            Self::FilledOval => fl!("rip-editor-oval"),
            Self::Polygon => fl!("rip-editor-polygon"),
            Self::FilledPolygon => fl!("rip-editor-filled-polygon"),
            Self::PolyLine => fl!("rip-editor-polyline"),
            Self::Arc => fl!("rip-editor-arc"),
            Self::OvalArc => fl!("rip-editor-oval-arc"),
            Self::PieSlice => fl!("rip-editor-pie-slice"),
            Self::OvalPieSlice => fl!("rip-editor-oval-pie-slice"),
            Self::Text => fl!("rip-editor-text"),
            Self::Bezier => fl!("rip-editor-bezier"),
            Self::Button => fl!("rip-editor-button"),
            Self::Mouse => fl!("rip-editor-mouse"),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Select => "cursor",
            Self::Pixel => "pencil",
            Self::Line => "line",
            Self::Rectangle => "rectangle_outline",
            Self::Bar => "rectangle_filled",
            Self::Circle | Self::Oval => "ellipse_outline",
            Self::FilledOval => "ellipse_filled",
            Self::Polygon => "rip_polygon",
            Self::FilledPolygon => "rip_polygon_filled",
            Self::PolyLine => "rip_polyline",
            Self::Arc => "rip_arc",
            Self::OvalArc => "rip_oval_arc",
            Self::PieSlice => "rip_pie",
            Self::OvalPieSlice => "rip_oval_pie",
            Self::Text => "text",
            Self::Bezier => "bezier",
            Self::Button => "rip_button",
            Self::Mouse => "rip_mouse",
        }
    }

    fn uses_line_style(self) -> bool {
        matches!(
            self,
            Self::Line
                | Self::Rectangle
                | Self::Circle
                | Self::Oval
                | Self::Bezier
                | Self::Polygon
                | Self::FilledPolygon
                | Self::PolyLine
                | Self::Arc
                | Self::OvalArc
                | Self::PieSlice
                | Self::OvalPieSlice
        )
    }

    fn uses_fill(self) -> bool {
        matches!(self, Self::Bar | Self::FilledOval | Self::FilledPolygon | Self::PieSlice | Self::OvalPieSlice)
    }

    fn is_poly(self) -> bool {
        matches!(self, Self::Polygon | Self::FilledPolygon | Self::PolyLine)
    }

    fn has_angles(self) -> bool {
        matches!(self, Self::Arc | Self::OvalArc | Self::PieSlice | Self::OvalPieSlice)
    }

    /// Whether the drawn shape needs a drag rather than a click.
    fn is_dragged(self) -> bool {
        !matches!(
            self,
            Self::Select | Self::Pixel | Self::Text | Self::Polygon | Self::FilledPolygon | Self::PolyLine | Self::Mouse
        )
    }

    /// The shape command itself, without the color and style commands before it; the select
    /// tool draws nothing.
    fn command(self, from: (u16, u16), to: (u16, u16), text: &str) -> Option<RipCommand> {
        let ((x0, y0), (x1, y1)) = (from, to);
        Some(match self {
            Self::Select | Self::Mouse => return None,
            Self::Pixel => RipCommand::Pixel { x: x1, y: y1 },
            Self::Line => RipCommand::Line { x0, y0, x1, y1 },
            Self::Rectangle => RipCommand::Rectangle { x0, y0, x1, y1 },
            Self::Bar => RipCommand::Bar { x0, y0, x1, y1 },
            Self::Circle => RipCommand::Circle {
                x_center: x0,
                y_center: y0,
                radius: (x1 as f32 - x0 as f32).hypot(y1 as f32 - y0 as f32).round() as u16,
            },
            Self::Oval => RipCommand::Oval {
                x: x0,
                y: y0,
                st_ang: 0,
                end_ang: 360,
                x_rad: x0.abs_diff(x1),
                y_rad: y0.abs_diff(y1),
            },
            Self::FilledOval => RipCommand::FilledOval {
                x: x0,
                y: y0,
                x_rad: x0.abs_diff(x1),
                y_rad: y0.abs_diff(y1),
            },
            Self::Arc => RipCommand::Arc {
                x: x0,
                y: y0,
                st_ang: 0,
                end_ang: 90,
                radius: (x1 as f32 - x0 as f32).hypot(y1 as f32 - y0 as f32).round() as u16,
            },
            Self::PieSlice => RipCommand::PieSlice {
                x: x0,
                y: y0,
                st_ang: 0,
                end_ang: 90,
                radius: (x1 as f32 - x0 as f32).hypot(y1 as f32 - y0 as f32).round() as u16,
            },
            Self::OvalArc => RipCommand::OvalArc {
                x: x0,
                y: y0,
                st_ang: 0,
                end_ang: 90,
                x_rad: x0.abs_diff(x1),
                y_rad: y0.abs_diff(y1),
            },
            Self::OvalPieSlice => RipCommand::OvalPieSlice {
                x: x0,
                y: y0,
                st_ang: 0,
                end_ang: 90,
                x_rad: x0.abs_diff(x1),
                y_rad: y0.abs_diff(y1),
            },
            Self::Polygon | Self::FilledPolygon | Self::PolyLine => return None,
            Self::Text => RipCommand::TextXY {
                x: x0,
                y: y0,
                text: text.to_owned(),
            },
            Self::Bezier => bezier_command(straight_bezier(from, to), 32),
            Self::Button => RipCommand::Button {
                x0: x0.min(x1),
                y0: y0.min(y1),
                x1: x0.max(x1),
                y1: y0.max(y1),
                hotkey: 0,
                flags: 0,
                res: 0,
                text: format!("<>{text}<>"),
            },
        })
    }
}

/// Control points on the straight line between the end points, a third of the way apart.
fn straight_bezier(from: (u16, u16), to: (u16, u16)) -> [(u16, u16); 4] {
    let at = |fraction: f32| {
        (
            (from.0 as f32 + (to.0 as f32 - from.0 as f32) * fraction).round() as u16,
            (from.1 as f32 + (to.1 as f32 - from.1 as f32) * fraction).round() as u16,
        )
    };
    [from, at(1.0 / 3.0), at(2.0 / 3.0), to]
}

fn bezier_command(points: [(u16, u16); 4], segments: u16) -> RipCommand {
    let [(x1, y1), (x2, y2), (x3, y3), (x4, y4)] = points;
    RipCommand::Bezier {
        x1,
        y1,
        x2,
        y2,
        x3,
        y3,
        x4,
        y4,
        cnt: segments,
    }
}

fn poly_command(tool: Tool, points: &[(u16, u16)]) -> RipCommand {
    let points = points.iter().flat_map(|&(x, y)| [x, y]).collect();
    match tool {
        Tool::Polygon => RipCommand::Polygon { points },
        Tool::FilledPolygon => RipCommand::FilledPolygon { points },
        Tool::PolyLine => RipCommand::PolyLine { points },
        _ => unreachable!("poly tool required"),
    }
}

fn line_style_name(style: LineStyle) -> String {
    match style {
        LineStyle::Solid => fl!("rip-line-solid"),
        LineStyle::Dotted => fl!("rip-line-dotted"),
        LineStyle::Center => fl!("rip-line-center"),
        LineStyle::Dashed => fl!("rip-line-dashed"),
        LineStyle::User => fl!("rip-line-user"),
    }
}

fn fill_style_name(style: FillStyle) -> String {
    match style {
        FillStyle::Empty => fl!("rip-fill-empty"),
        FillStyle::Solid => fl!("rip-fill-solid"),
        FillStyle::Line => fl!("rip-fill-line"),
        FillStyle::LtSlash => fl!("rip-fill-light-slash"),
        FillStyle::Slash => fl!("rip-fill-slash"),
        FillStyle::BkSlash => fl!("rip-fill-backslash"),
        FillStyle::LtBkSlash => fl!("rip-fill-light-backslash"),
        FillStyle::Hatch => fl!("rip-fill-hatch"),
        FillStyle::XHatch => fl!("rip-fill-cross-hatch"),
        FillStyle::Interleave => fl!("rip-fill-interleave"),
        FillStyle::WideDot => fl!("rip-fill-wide-dots"),
        FillStyle::CloseDot => fl!("rip-fill-close-dots"),
        FillStyle::User => fl!("rip-fill-user"),
    }
}

/// The drawing state at the end of the scene, so new shapes only repeat the state commands
/// that differ.
#[derive(Clone, Debug, Default, PartialEq)]
struct DrawingState {
    color: Option<u16>,
    line: Option<(LineStyle, u16, u16)>,
    fill: Option<(FillStyle, u16)>,
    font: Option<(u16, u16, u16)>,
    button_style: Option<RipCommand>,
}

impl DrawingState {
    fn at_end(commands: &[RipCommand]) -> Self {
        let mut state = Self::default();
        for command in commands {
            match command {
                RipCommand::Color { c } => state.color = Some(*c),
                RipCommand::LineStyle { style, user_pat, thick } => state.line = Some((*style, *user_pat, *thick)),
                RipCommand::FillStyle { pattern, color } => state.fill = Some((*pattern, *color)),
                // Custom patterns replace the fill style.
                RipCommand::FillPattern { .. } => state.fill = None,
                RipCommand::FontStyle { font, direction, size, .. } => state.font = Some((*font, *direction, *size)),
                RipCommand::ButtonStyle { .. } => state.button_style = Some(command.clone()),
                RipCommand::ResetWindows => state = Self::default(),
                _ => {}
            }
        }
        state
    }
}

/// A shape being moved or resized with the select tool.
#[derive(Clone, Debug, PartialEq)]
struct ShapeDrag {
    index: usize,
    handle: Handle,
    start: select::Point,
    original: Geometry,
    command: RipCommand,
    current: RipCommand,
}

/// A Bézier curve whose four points can still be moved.
#[derive(Clone, Copy, Debug, PartialEq)]
struct BezierEdit {
    points: [(u16, u16); 4],
    moving: Option<usize>,
}

/// Text being typed on the canvas.
#[derive(Clone, Debug, PartialEq)]
struct TextEdit {
    at: (u16, u16),
    text: String,
    /// The `TextXY` command being changed; new text otherwise.
    index: Option<usize>,
}

impl TextEdit {
    fn command(&self) -> RipCommand {
        RipCommand::TextXY {
            x: self.at.0,
            y: self.at.1,
            text: self.text.clone(),
        }
    }
}

fn is_mouse_region(command: &RipCommand) -> bool {
    matches!(command, RipCommand::Mouse { .. })
}

/// Characters RIP text stores unchanged; others would not survive saving.
fn is_rip_text_char(c: char) -> bool {
    matches!(c, ' '..='~')
}

/// The size in pixels of `text` in the RIP font state `font` (`None`: the engine default).
fn text_extent(font: Option<(u16, u16, u16)>, text: &str) -> (i32, i32) {
    use icy_engine::bgi::{Bgi, Direction, FontType};
    let mut bgi = Bgi::new(std::path::PathBuf::new(), icy_engine::Size::new(i32::from(WIDTH), i32::from(HEIGHT)));
    if let Some((font, direction, size)) = font {
        bgi.set_text_style(FontType::from(font as u8), Direction::from(direction as u8), i32::from(size));
    }
    let size = bgi.text_size(text);
    (size.width, size.height)
}

/// Font, direction and size of RIP text.
type FontState = (u16, u16, u16);
/// The font a scene starts with, and the one `ResetWindows` restores.
const START_FONT: FontState = (0, 0, 4);
const RESET_FONT: FontState = (2, 0, 4);
const DEFAULT_COLOR: u16 = 7;

fn font_of(command: &RipCommand) -> Option<FontState> {
    match command {
        RipCommand::FontStyle { font, direction, size, .. } => Some((*font, *direction, *size)),
        _ => None,
    }
}

fn color_of(command: &RipCommand) -> Option<u16> {
    match command {
        RipCommand::Color { c } => Some(*c),
        _ => None,
    }
}

fn font_command((font, direction, size): FontState) -> RipCommand {
    RipCommand::FontStyle { font, direction, size, res: 0 }
}

/// Commands that only change the drawing state and draw nothing.
fn is_state_command(command: &RipCommand) -> bool {
    matches!(
        command,
        RipCommand::Color { .. }
            | RipCommand::SetPalette { .. }
            | RipCommand::OnePalette { .. }
            | RipCommand::WriteMode { .. }
            | RipCommand::FontStyle { .. }
            | RipCommand::LineStyle { .. }
            | RipCommand::FillStyle { .. }
            | RipCommand::FillPattern { .. }
            | RipCommand::ButtonStyle { .. }
    )
}

fn draws_text(command: &RipCommand) -> bool {
    matches!(
        command,
        RipCommand::TextXY { .. } | RipCommand::Text { .. } | RipCommand::RegionText { .. } | RipCommand::Button { .. }
    )
}

/// The value of one drawing state in effect for the command at `index`.
fn state_before<V: Copy>(commands: &[RipCommand], index: usize, read: impl Fn(&RipCommand) -> Option<V>, start: V, reset: V) -> V {
    for command in commands[..index].iter().rev() {
        if matches!(command, RipCommand::ResetWindows) {
            return reset;
        }
        if let Some(value) = read(command) {
            return value;
        }
    }
    start
}

fn font_before(commands: &[RipCommand], index: usize) -> FontState {
    state_before(commands, index, font_of, START_FONT, RESET_FONT)
}

fn color_before(commands: &[RipCommand], index: usize) -> u16 {
    state_before(commands, index, color_of, DEFAULT_COLOR, DEFAULT_COLOR)
}

/// Makes the command at `index` draw with `value` of one drawing state, while the commands after
/// it keep drawing as before. A state command directly in front of it is changed; otherwise one is
/// inserted, and the previous value is restored after it when later commands depend on it.
#[allow(clippy::too_many_arguments)]
fn set_state_at<V: Copy + PartialEq>(
    commands: &mut Vec<RipCommand>,
    first_editable: usize,
    index: &mut usize,
    value: V,
    read: impl Fn(&RipCommand) -> Option<V>,
    make: impl Fn(V) -> RipCommand,
    (start, reset): (V, V),
    uses: impl Fn(&RipCommand) -> bool,
) {
    let before = state_before(commands, *index, &read, start, reset);
    if before == value {
        return;
    }
    let next = commands[*index + 1..]
        .iter()
        .position(|command| read(command).is_some() || matches!(command, RipCommand::ResetWindows))
        .map_or(commands.len(), |offset| *index + 1 + offset);
    if commands[*index + 1..next].iter().any(&uses) {
        commands.insert(*index + 1, make(before));
    }
    let own = (first_editable..*index)
        .rev()
        .take_while(|position| is_state_command(&commands[*position]))
        .find(|position| read(&commands[*position]).is_some());
    match own {
        Some(position) => commands[position] = make(value),
        None => {
            commands.insert(*index, make(value));
            *index += 1;
        }
    }
}

/// `commands` with the text at `index` replaced by `text` drawn in `font` and `color`, and the
/// text's new index.
fn restyle_text(commands: &[RipCommand], first_editable: usize, index: usize, text: RipCommand, font: FontState, color: u16) -> (Vec<RipCommand>, usize) {
    let mut commands = commands.to_vec();
    commands[index] = text;
    let mut index = index;
    set_state_at(
        &mut commands,
        first_editable,
        &mut index,
        font,
        font_of,
        font_command,
        (START_FONT, RESET_FONT),
        draws_text,
    );
    set_state_at(
        &mut commands,
        first_editable,
        &mut index,
        color,
        color_of,
        |c| RipCommand::Color { c },
        (DEFAULT_COLOR, DEFAULT_COLOR),
        |command| !is_state_command(command),
    );
    (commands, index)
}

fn describe(command: &RipCommand) -> Option<String> {
    Some(match command {
        RipCommand::Pixel { x, y } | RipCommand::TextXY { x, y, .. } => format!("{x}, {y}"),
        RipCommand::Line { x0, y0, x1, y1 } => format!("{x0}, {y0} → {x1}, {y1}"),
        RipCommand::Rectangle { x0, y0, x1, y1 } | RipCommand::Bar { x0, y0, x1, y1 } | RipCommand::Button { x0, y0, x1, y1, .. } => {
            if *x1 == 0 && *y1 == 0 {
                format!("{x0}, {y0}")
            } else {
                format!("{}, {} · {} × {}", x0.min(x1), y0.min(y1), x0.abs_diff(*x1) + 1, y0.abs_diff(*y1) + 1)
            }
        }
        RipCommand::Mouse { x0, y0, x1, y1, .. } => format!("{}, {} · {} × {}", x0.min(x1), y0.min(y1), x0.abs_diff(*x1) + 1, y0.abs_diff(*y1) + 1),
        RipCommand::Circle { x_center, y_center, radius } => format!("{x_center}, {y_center} · r {radius}"),
        RipCommand::Oval { x, y, x_rad, y_rad, .. }
        | RipCommand::OvalArc { x, y, x_rad, y_rad, .. }
        | RipCommand::OvalPieSlice { x, y, x_rad, y_rad, .. }
        | RipCommand::FilledOval { x, y, x_rad, y_rad } => {
            format!("{x}, {y} · {x_rad} × {y_rad}")
        }
        RipCommand::Arc { x, y, radius, .. } | RipCommand::PieSlice { x, y, radius, .. } => format!("{x}, {y} · r {radius}"),
        RipCommand::Polygon { points } | RipCommand::FilledPolygon { points } | RipCommand::PolyLine { points } => {
            let count = points.len() / 2;
            fl!("rip-editor-vertices", count = count)
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
            ..
        } => format!("{x1},{y1} ({x2},{y2}) ({x3},{y3}) {x4},{y4}"),
        _ => return None,
    })
}

/// The name of a drawing command, as its tool is called.
fn shape_name(command: &RipCommand) -> Option<String> {
    shape_tool(command).map(Tool::label)
}

/// The tool that draws `command`.
fn shape_tool(command: &RipCommand) -> Option<Tool> {
    Some(match command {
        RipCommand::Pixel { .. } => Tool::Pixel,
        RipCommand::Line { .. } => Tool::Line,
        RipCommand::Rectangle { .. } => Tool::Rectangle,
        RipCommand::Bar { .. } => Tool::Bar,
        RipCommand::Circle { .. } => Tool::Circle,
        RipCommand::Oval { .. } => Tool::Oval,
        RipCommand::FilledOval { .. } => Tool::FilledOval,
        RipCommand::Polygon { .. } => Tool::Polygon,
        RipCommand::FilledPolygon { .. } => Tool::FilledPolygon,
        RipCommand::PolyLine { .. } => Tool::PolyLine,
        RipCommand::Arc { .. } => Tool::Arc,
        RipCommand::OvalArc { .. } => Tool::OvalArc,
        RipCommand::PieSlice { .. } => Tool::PieSlice,
        RipCommand::OvalPieSlice { .. } => Tool::OvalPieSlice,
        RipCommand::TextXY { .. } => Tool::Text,
        RipCommand::Bezier { .. } => Tool::Bezier,
        RipCommand::Button { .. } => Tool::Button,
        RipCommand::Mouse { .. } => Tool::Mouse,
        _ => return None,
    })
}

/// The short name of any command in the command list.
fn command_name(command: &RipCommand) -> String {
    if let Some(name) = shape_name(command) {
        return name;
    }
    match command {
        RipCommand::Color { .. } => fl!("rip-command-color"),
        RipCommand::LineStyle { .. } => fl!("rip-command-line-style"),
        RipCommand::FillStyle { .. } | RipCommand::FillPattern { .. } => fl!("rip-command-fill-style"),
        RipCommand::FontStyle { .. } => fl!("rip-command-font-style"),
        RipCommand::ButtonStyle { .. } => fl!("rip-button-style-entry"),
        RipCommand::SetPalette { .. } => fl!("rip-command-palette"),
        RipCommand::OnePalette { .. } => fl!("rip-command-palette-slot"),
        RipCommand::MouseFields => fl!("rip-command-mouse-fields"),
        // Other commands are named after their variant, without the parameters.
        other => {
            let debug = format!("{other:?}");
            debug.split([' ', '{', '(']).next().unwrap_or_default().to_owned()
        }
    }
}

/// A compact summary next to the name; the property panel shows every parameter.
fn command_summary(command: &RipCommand) -> String {
    match command {
        RipCommand::Button { text, .. } => format!("\"{}\"", text.split("<>").nth(1).unwrap_or(text)),
        RipCommand::TextXY { text, .. } => format!("\"{text}\""),
        RipCommand::Mouse { text, .. } => format!("\"{text}\" · {}", describe(command).unwrap_or_default()),
        RipCommand::Color { c } => c.to_string(),
        RipCommand::LineStyle { style, thick, .. } => format!("{} · {thick} px", line_style_name(*style)),
        RipCommand::FillStyle { pattern, .. } => fill_style_name(*pattern),
        RipCommand::FontStyle { font, size, .. } => format!("{} · {size}", button::font_names()[(*font).min(10) as usize]),
        RipCommand::ButtonStyle { wid, hgt, .. } => format!("{wid} × {hgt}"),
        RipCommand::OnePalette { color, value } => format!("{color} → EGA {value}"),
        _ => describe(command).unwrap_or_default(),
    }
}

fn command_icon(command: &RipCommand) -> Option<&'static str> {
    if let Some(tool) = shape_tool(command) {
        return Some(tool.icon());
    }
    Some(match command {
        RipCommand::Color { .. } => "paint_brush",
        RipCommand::LineStyle { .. } => "line",
        RipCommand::FillStyle { .. } | RipCommand::FillPattern { .. } => "fill",
        RipCommand::FontStyle { .. } => "font",
        RipCommand::ButtonStyle { .. } => "rip_button",
        RipCommand::SetPalette { .. } | RipCommand::OnePalette { .. } => "dropper",
        RipCommand::MouseFields => "rip_mouse",
        _ => return None,
    })
}

/// The palette color a state command sets, shown as a swatch in the list.
fn command_swatch(command: &RipCommand) -> Option<u16> {
    match command {
        RipCommand::Color { c } => Some(*c),
        RipCommand::FillStyle { color, .. } => Some(*color),
        _ => None,
    }
}

const COMMAND_ROW_HEIGHT: f32 = 24.0;
const PROPERTY_LABEL_WIDTH: f32 = 84.0;

/// One line of the command list: number, icon, name and a short summary, never wrapped.
/// The RIP commands as a terminal receives them.
struct RipTimeline<'a>(&'a RipDocument);

impl Timeline for RipTimeline<'_> {
    fn len(&self) -> usize {
        self.0.commands().len()
    }

    fn transmitted_bytes(&self, index: usize) -> usize {
        self.0.commands().get(index).map_or(0, |command| command.to_string().len())
    }
}

struct CommandRow<'a> {
    index: usize,
    command: &'a RipCommand,
    selected: bool,
    /// How the row relates to the animation frame.
    mark: RowMark,
    /// Part of the selected button (its style or font).
    related: bool,
    preserved: bool,
}

impl CommandRow<'_> {
    fn show(self, ui: &mut egui::Ui, icons: &mut Icons, palette: &icy_engine::Palette) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), COMMAND_ROW_HEIGHT), egui::Sense::click());
        let name = command_name(self.command);
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, ui.is_enabled(), self.selected, &name));
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let visuals = ui.visuals().clone();
        let painter = ui.painter_at(rect);
        let background = if self.selected {
            Some(visuals.selection.bg_fill)
        } else if self.related {
            Some(visuals.selection.bg_fill.gamma_multiply(0.4))
        } else if response.hovered() {
            Some(visuals.widgets.hovered.weak_bg_fill)
        } else {
            None
        };
        if let Some(fill) = background {
            painter.rect_filled(rect.shrink2(egui::vec2(2.0, 1.0)), 4, fill);
        }
        self.mark.paint_background(ui, rect, background.is_some());
        let text = if self.selected {
            visuals.selection.stroke.color
        } else if self.preserved {
            visuals.weak_text_color()
        } else {
            visuals.text_color()
        };
        let text = self.mark.text(text, self.selected);
        let weak = if self.selected {
            text.gamma_multiply(0.7)
        } else {
            self.mark.text(visuals.weak_text_color(), false)
        };
        let center = rect.center().y;
        painter.text(
            egui::pos2(rect.left() + 34.0, center),
            egui::Align2::RIGHT_CENTER,
            (self.index + 1).to_string(),
            egui::FontId::monospace(11.0),
            weak,
        );
        let mut x = rect.left() + 42.0;
        if let Some(icon) = command_icon(self.command) {
            icons
                .image(ui, icon, 14.0)
                .tint(text)
                .paint_at(ui, egui::Rect::from_center_size(egui::pos2(x + 7.0, center), egui::Vec2::splat(14.0)));
        }
        x += 22.0;
        if let Some(color) = command_swatch(self.command) {
            let swatch = egui::Rect::from_min_size(egui::pos2(x, center - 6.0), egui::Vec2::splat(12.0));
            painter.rect_filled(swatch, 2, button::color32(palette, color));
            painter.rect_stroke(swatch, 2, Stroke::new(1.0, weak), egui::StrokeKind::Inside);
            x += 18.0;
        }
        let font = egui::TextStyle::Body.resolve(ui.style());
        let mut job = egui::text::LayoutJob::default();
        job.append(
            &name,
            0.0,
            egui::TextFormat {
                font_id: font.clone(),
                color: text,
                ..Default::default()
            },
        );
        let summary = command_summary(self.command);
        if !summary.is_empty() {
            job.append(
                &summary,
                8.0,
                egui::TextFormat {
                    font_id: font,
                    color: weak,
                    ..Default::default()
                },
            );
        }
        job.wrap = egui::text::TextWrapping::truncate_at_width((rect.right() - 6.0 - x).max(0.0));
        let galley = painter.layout_job(job);
        painter.galley(egui::pos2(x, center - galley.size().y / 2.0), galley, text);
        response
    }
}

fn command_field(ui: &mut egui::Ui, name: &str, value: &mut u16, maximum: u16) {
    ui.horizontal(|ui| {
        ui.add_sized([PROPERTY_LABEL_WIDTH, 20.0], egui::Label::new(egui::RichText::new(name).weak()).truncate());
        ui.add(egui::DragValue::new(value).range(0..=maximum)).on_hover_text(name);
    });
}

/// Font, size, direction and color of text in the property panel.
fn text_style_properties(ui: &mut egui::Ui, palette: &icy_engine::Palette, (font, direction, size): &mut FontState, color: &mut u16) {
    let label = |ui: &mut egui::Ui, name: String| {
        ui.add_sized([PROPERTY_LABEL_WIDTH, 20.0], egui::Label::new(egui::RichText::new(name).weak()).truncate());
    };
    ui.horizontal(|ui| {
        label(ui, fl!("rip-font"));
        let fonts = button::font_names();
        egui::ComboBox::from_id_salt("rip-text-property-font")
            .selected_text(fonts[(*font).min(10) as usize].clone())
            .show_ui(ui, |ui| {
                for (index, name) in fonts.iter().enumerate() {
                    ui.selectable_value(font, index as u16, name);
                }
            });
    });
    ui.horizontal(|ui| {
        label(ui, fl!("rip-font-size"));
        ui.add(egui::DragValue::new(size).range(1..=10));
    });
    ui.horizontal(|ui| {
        label(ui, fl!("rip-text-direction"));
        let directions = [
            (0, fl!("rip-text-horizontal"), fl!("rip-text-horizontal")),
            (1, fl!("rip-text-vertical"), fl!("rip-text-vertical")),
        ];
        widgets::segmented(ui, direction, &directions);
    });
    ui.horizontal(|ui| {
        label(ui, fl!("rip-editor-color"));
        button::color_picker_sized(ui, "text-property", palette, color, egui::vec2(30.0, 22.0));
    });
}

fn command_properties(ui: &mut egui::Ui, command: &mut RipCommand) -> bool {
    let polyline = matches!(command, RipCommand::PolyLine { .. });
    match command {
        RipCommand::Color { c } => command_field(ui, "c", c, 15),
        RipCommand::OnePalette { color, value } => {
            command_field(ui, "color", color, 15);
            ui.horizontal(|ui| {
                command_field(ui, "EGA", value, 63);
                let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 18.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 3, palette::ega_color(*value));
            });
        }
        RipCommand::FillStyle { pattern, color } => {
            egui::ComboBox::from_id_salt("rip-fill-pattern")
                .selected_text(format!("{pattern:?}"))
                .show_ui(ui, |ui| {
                    for index in 0..=12 {
                        let style = FillStyle::try_from(index).expect("valid RIP fill style");
                        ui.selectable_value(pattern, style, format!("{style:?}"));
                    }
                });
            command_field(ui, "color", color, 15);
        }
        RipCommand::Pixel { x, y } => {
            command_field(ui, "x", x, 1295);
            command_field(ui, "y", y, 1295);
        }
        RipCommand::Line { x0, y0, x1, y1 } | RipCommand::Rectangle { x0, y0, x1, y1 } | RipCommand::Bar { x0, y0, x1, y1 } => {
            command_field(ui, "x0", x0, 1295);
            command_field(ui, "y0", y0, 1295);
            command_field(ui, "x1", x1, 1295);
            command_field(ui, "y1", y1, 1295);
        }
        RipCommand::Circle { x_center, y_center, radius } => {
            command_field(ui, "x", x_center, 1295);
            command_field(ui, "y", y_center, 1295);
            command_field(ui, "radius", radius, 1295);
        }
        RipCommand::Oval {
            x,
            y,
            st_ang,
            end_ang,
            x_rad,
            y_rad,
        }
        | RipCommand::OvalArc {
            x,
            y,
            st_ang,
            end_ang,
            x_rad,
            y_rad,
        }
        | RipCommand::OvalPieSlice {
            x,
            y,
            st_ang,
            end_ang,
            x_rad,
            y_rad,
        } => {
            command_field(ui, "x", x, 1295);
            command_field(ui, "y", y, 1295);
            command_field(ui, "start angle", st_ang, 1295);
            command_field(ui, "end angle", end_ang, 1295);
            command_field(ui, "x_rad", x_rad, 1295);
            command_field(ui, "y_rad", y_rad, 1295);
        }
        RipCommand::FilledOval { x, y, x_rad, y_rad } => {
            command_field(ui, "x", x, 1295);
            command_field(ui, "y", y, 1295);
            command_field(ui, "x_rad", x_rad, 1295);
            command_field(ui, "y_rad", y_rad, 1295);
        }
        RipCommand::Arc { x, y, st_ang, end_ang, radius } | RipCommand::PieSlice { x, y, st_ang, end_ang, radius } => {
            command_field(ui, "x", x, 1295);
            command_field(ui, "y", y, 1295);
            command_field(ui, "start angle", st_ang, 360);
            command_field(ui, "end angle", end_ang, 360);
            command_field(ui, "radius", radius, 1295);
        }
        RipCommand::Polygon { points } | RipCommand::FilledPolygon { points } | RipCommand::PolyLine { points } => {
            for (index, pair) in points.chunks_exact_mut(2).enumerate() {
                ui.label(format!("{} {}", fl!("rip-editor-vertex"), index + 1));
                command_field(ui, "x", &mut pair[0], 1295);
                command_field(ui, "y", &mut pair[1], 1295);
            }
            if points.len() >= 2 && points.len() < 2590 && ui.button(fl!("rip-editor-add-vertex")).clicked() {
                let last = points[points.len() - 2..].to_vec();
                points.extend(last);
            }
            let minimum = if polyline { 4 } else { 6 };
            if points.len() > minimum && ui.button(fl!("rip-editor-remove-vertex")).clicked() {
                points.truncate(points.len() - 2);
            }
        }
        RipCommand::Mouse {
            x0,
            y0,
            x1,
            y1,
            clk,
            clr,
            text,
            ..
        } => {
            command_field(ui, "x0", x0, 1295);
            command_field(ui, "y0", y0, 1295);
            command_field(ui, "x1", x1, 1295);
            command_field(ui, "y1", y1, 1295);
            let mut invert = *clk != 0;
            if ui.checkbox(&mut invert, fl!("rip-mouse-invert")).changed() {
                *clk = u16::from(invert);
            }
            let mut clear = *clr != 0;
            if ui.checkbox(&mut clear, fl!("rip-mouse-clear")).changed() {
                *clr = u16::from(clear);
            }
            ui.add(egui::TextEdit::singleline(text).hint_text(fl!("rip-button-host-command")));
        }
        RipCommand::TextXY { x, y, text } => {
            command_field(ui, "x", x, 1295);
            command_field(ui, "y", y, 1295);
            ui.text_edit_singleline(text);
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
            for (name, value) in [
                ("x1", x1),
                ("y1", y1),
                ("x2", x2),
                ("y2", y2),
                ("x3", x3),
                ("y3", y3),
                ("x4", x4),
                ("y4", y4),
                ("segments", cnt),
            ] {
                command_field(ui, name, value, 1295);
            }
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
            for (name, value) in [
                ("x0", x0),
                ("y0", y0),
                ("x1", x1),
                ("y1", y1),
                ("hotkey", hotkey),
                ("flags", flags),
                ("res", res),
            ] {
                command_field(ui, name, value, if name == "flags" || name == "res" { 35 } else { 1295 });
            }
            ui.text_edit_singleline(text);
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
            for (name, value) in [
                ("width", wid),
                ("height", hgt),
                ("orientation", orient),
                ("flags", flags),
                ("bevel", bevsize),
                ("draw", dfore),
                ("shadow", dback),
                ("bright", bright),
                ("border", dark),
                ("fill", surface),
                ("group", grp_no),
                ("flags2", flags2),
                ("underline", uline_col),
                ("corner", corner_col),
                ("reserved", res),
            ] {
                command_field(
                    ui,
                    name,
                    value,
                    if name == "flags" {
                        65535
                    } else if name == "reserved" {
                        65535
                    } else {
                        1295
                    },
                );
            }
        }
        _ => return false,
    }
    true
}

pub struct RipEditor {
    document: RipDocument,
    selected: Option<usize>,
    preview_to_selection: bool,
    editing: Option<(usize, RipCommand)>,
    tool: Tool,
    draw_color: u16,
    border_color: u16,
    fill_color: u16,
    icons: Icons,
    line_style: LineStyle,
    line_thickness: u16,
    fill_pattern: FillStyle,
    text_font: u16,
    text_size: u16,
    text_direction: u16,
    bezier_segments: u16,
    start_angle: u16,
    end_angle: u16,
    button: ButtonOptions,
    button_dialog: Option<ButtonDialog>,
    palette_dialog: Option<PaletteDialog>,
    bezier: Option<BezierEdit>,
    poly: Vec<(u16, u16)>,
    text_edit: Option<TextEdit>,
    /// Host command, invert and clear flags of new mouse regions.
    mouse_host: String,
    mouse_invert: bool,
    mouse_clear: bool,
    /// The host command of the selected mouse region while it is typed in the toolbar.
    mouse_draft: Option<(usize, String)>,
    /// Font and color of the text edited in the property panel, by command index.
    editing_style: Option<(usize, FontState, u16)>,
    /// The restyled scene the preview currently shows for text being edited.
    shown_editable: Option<Vec<RipCommand>>,
    transport: Transport,
    /// The animation frame the preview shows.
    shown_frame: Option<usize>,
    shape_drag: Option<ShapeDrag>,
    drag: Option<((u16, u16), (u16, u16))>,
    hover: Option<(u16, u16)>,
    palette: icy_engine::Palette,
    texture: Option<egui::TextureHandle>,
    preview_dirty: bool,
    /// The unfinished shape the preview currently shows.
    shown_pending: Vec<RipCommand>,
    /// The edited shape the preview currently shows instead of the document's.
    shown_replacement: Option<(usize, RipCommand)>,
    /// The selection the command list last showed and the rows it had in view, so a selection
    /// made on the canvas scrolls into view.
    listed_selection: Option<usize>,
    visible_rows: std::ops::Range<usize>,
    error: Option<String>,
    #[cfg(test)]
    canvas_rect: Option<egui::Rect>,
}

impl RipEditor {
    pub fn new() -> Self {
        Self::from_document(RipDocument::new())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        Ok(Self::from_document(RipDocument::open(path).map_err(|error| error.to_string())?))
    }

    fn from_document(document: RipDocument) -> Self {
        Self {
            document,
            selected: None,
            preview_to_selection: false,
            editing: None,
            tool: Tool::Line,
            draw_color: 15,
            border_color: 15,
            fill_color: 15,
            icons: Icons::default(),
            line_style: LineStyle::Solid,
            line_thickness: 1,
            fill_pattern: FillStyle::Solid,
            text_font: 0,
            text_size: 1,
            text_direction: 0,
            bezier_segments: 32,
            start_angle: 0,
            end_angle: 90,
            button: ButtonOptions::default(),
            button_dialog: None,
            palette_dialog: None,
            bezier: None,
            poly: Vec::new(),
            text_edit: None,
            mouse_host: String::new(),
            mouse_invert: true,
            mouse_clear: false,
            mouse_draft: None,
            editing_style: None,
            shown_editable: None,
            transport: Transport::default(),
            shown_frame: None,
            shape_drag: None,
            drag: None,
            hover: None,
            palette: icy_engine::Palette::dos_default(),
            texture: None,
            preview_dirty: true,
            shown_pending: Vec::new(),
            shown_replacement: None,
            listed_selection: None,
            visible_rows: 0..0,
            error: None,
            #[cfg(test)]
            canvas_rect: None,
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.document.path()
    }

    pub fn save(&mut self, path: &Path, overwrite: bool) -> Result<(), String> {
        self.finish_pending();
        self.document.save_as(path, overwrite).map_err(|error| error.to_string())
    }

    pub fn modified(&self) -> bool {
        self.document.is_dirty() || self.bezier.is_some() || !self.poly.is_empty() || self.text_edit.is_some()
    }

    pub fn recovery_snapshot(&self) -> Result<icy_draw::recovery::Snapshot, String> {
        self.document.recovery_snapshot().map_err(|error| error.to_string())
    }

    pub fn from_recovery(snapshot: &icy_draw::recovery::Snapshot) -> Result<Self, String> {
        Ok(Self::from_document(RipDocument::from_recovery(snapshot).map_err(|error| error.to_string())?))
    }

    pub fn can_undo(&self) -> bool {
        self.document.can_undo() || self.bezier.is_some() || !self.poly.is_empty() || self.text_edit.is_some()
    }

    pub fn can_redo(&self) -> bool {
        self.document.can_redo()
    }

    pub fn undo(&mut self, redo: bool) {
        if self.transport.animating() {
            self.transport_action(Action::Stop, 0.0);
        }
        // Undo first drops an unfinished curve, path or text.
        if !redo && (self.bezier.take().is_some() || !self.poly.is_empty() || self.text_edit.take().is_some()) {
            self.poly.clear();
            self.preview_dirty = true;
            return;
        }
        if redo {
            self.document.redo();
        } else {
            self.document.undo();
        }
        self.selected = None;
        self.editing = None;
        self.preview_dirty = true;
    }

    fn select_tool(&mut self, tool: Tool) {
        self.finish_pending();
        self.tool = tool;
        self.drag = None;
        self.shape_drag = None;
        self.preview_dirty = true;
    }

    /// Size of buttons placed without a rectangle, from the style in effect before `index`.
    fn button_size_at(&self, index: usize) -> (u16, u16) {
        self.document.commands()[..index]
            .iter()
            .rev()
            .find_map(|command| match command {
                RipCommand::ButtonStyle { wid, hgt, .. } => Some((*wid, *hgt)),
                _ => None,
            })
            .unwrap_or((0, 0))
    }

    /// The command and geometry of an editable shape, including an edit in progress.
    fn shape(&self, index: usize) -> Option<(RipCommand, Geometry)> {
        if index < self.document.preserved_commands() {
            return None;
        }
        let command = match &self.shape_drag {
            Some(drag) if drag.index == index => drag.current.clone(),
            _ => self.document.commands().get(index)?.clone(),
        };
        // Mouse regions are only shown and picked with the mouse region tool, and only them.
        if is_mouse_region(&command) != (self.tool == Tool::Mouse) {
            return None;
        }
        let geometry = select::geometry(&command, self.button_size_at(index))?;
        Some((command, geometry))
    }

    fn mouse_commands(&self, from: (u16, u16), to: (u16, u16)) -> Vec<RipCommand> {
        vec![RipCommand::Mouse {
            num: 0,
            x0: from.0.min(to.0),
            y0: from.1.min(to.1),
            x1: from.0.max(to.0),
            y1: from.1.max(to.1),
            clk: u16::from(self.mouse_invert),
            clr: u16::from(self.mouse_clear),
            res: 0,
            text: self.mouse_host.clone(),
        }]
    }

    /// Adds a mouse region from `from` to `to` and selects it.
    fn add_mouse_region(&mut self, from: (u16, u16), to: (u16, u16)) {
        if from.0.abs_diff(to.0) < 2 || from.1.abs_diff(to.1) < 2 {
            return;
        }
        let commands = self.mouse_commands(from, to);
        self.add_commands(commands);
    }

    /// The selected shape; selecting a button's style or font selects the button.
    fn selected_shape(&self) -> Option<usize> {
        let index = self.selected?;
        if self.shape(index).is_some() {
            return Some(index);
        }
        ButtonTarget::owning(self.document.commands(), index).map(|target| target.button)
    }

    /// The topmost editable shape at `point`.
    fn shape_at(&self, point: (f32, f32), tolerance: f32) -> Option<usize> {
        (self.document.preserved_commands()..self.document.commands().len()).rev().find(|index| {
            self.shape(*index)
                .is_some_and(|(command, geometry)| select::hit(&command, &geometry, point, tolerance))
        })
    }

    fn select_shape(&mut self, index: Option<usize>) {
        if self.selected != index {
            self.selected = index;
            self.editing = None;
            if self.preview_to_selection {
                self.preview_dirty = true;
            }
        }
    }

    fn replace_shape(&mut self, index: usize, command: RipCommand) {
        match self.document.replace(index, command) {
            Ok(()) => {
                self.editing = None;
                self.preview_dirty = true;
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn nudge_selected(&mut self, dx: i32, dy: i32) {
        let Some(index) = self.selected_shape() else {
            return;
        };
        if let Some((command, geometry)) = self.shape(index) {
            let moved = select::translate(&geometry, dx, dy);
            let command = select::apply(&command, &moved, self.button_size_at(index));
            self.replace_shape(index, command);
        }
    }

    fn delete_selected_shape(&mut self) {
        let Some(index) = self.selected_shape() else {
            return;
        };
        match self.document.delete(index) {
            Ok(_) => {
                self.selected = None;
                self.editing = None;
                self.preview_dirty = true;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    /// Color and style commands that make the scene's state match the tool settings.
    fn state_commands(&self, tool: Tool) -> Vec<RipCommand> {
        let state = DrawingState::at_end(self.document.commands());
        let mut commands = Vec::new();
        let color = match tool {
            Tool::Bar | Tool::Button => None,
            Tool::Rectangle
            | Tool::Circle
            | Tool::Oval
            | Tool::FilledOval
            | Tool::Polygon
            | Tool::FilledPolygon
            | Tool::Arc
            | Tool::OvalArc
            | Tool::PieSlice
            | Tool::OvalPieSlice => Some(self.border_color),
            _ => Some(self.draw_color),
        };
        if let Some(color) = color.filter(|color| state.color != Some(*color)) {
            commands.push(RipCommand::Color { c: color });
        }
        if tool.uses_line_style() && state.line != Some((self.line_style, 0, self.line_thickness)) {
            commands.push(RipCommand::LineStyle {
                style: self.line_style,
                user_pat: 0,
                thick: self.line_thickness,
            });
        }
        if tool.uses_fill() && state.fill != Some((self.fill_pattern, self.fill_color)) {
            commands.push(RipCommand::FillStyle {
                pattern: self.fill_pattern,
                color: self.fill_color,
            });
        }
        let font = match tool {
            Tool::Text => Some((self.text_font, self.text_direction, self.text_size)),
            Tool::Button => Some((self.button.font, 0, self.button.font_size)),
            _ => None,
        };
        if let Some((font, direction, size)) = font.filter(|font| state.font != Some(*font)) {
            commands.push(RipCommand::FontStyle { font, direction, size, res: 0 });
        }
        if tool == Tool::Button {
            let style = self.button.style();
            if state.button_style.as_ref() != Some(&style) {
                commands.push(style);
            }
        }
        commands
    }

    /// The commands a shape from `from` to `to` adds, or why it cannot be added.
    fn shape_commands(&self, from: (u16, u16), to: (u16, u16)) -> Result<Vec<RipCommand>, String> {
        let mut shape = match self.tool {
            // Text is typed on the canvas, see `begin_text`.
            Tool::Text => return Err(String::new()),
            Tool::Button => {
                if let Some(problem) = self.button.problem() {
                    return Err(problem);
                }
                let dragged = from.0.abs_diff(to.0) >= 2 && from.1.abs_diff(to.1) >= 2;
                if !dragged && self.button.kind == ButtonKind::Plain && (self.button.width == 0 || self.button.height == 0) {
                    return Err(fl!("rip-editor-button-size-required"));
                }
                self.button.button(from, dragged.then_some(to))
            }
            Tool::Bezier => bezier_command(straight_bezier(from, to), self.bezier_segments),
            tool => tool.command(from, to, "").ok_or_else(String::new)?,
        };
        match &mut shape {
            RipCommand::Arc { st_ang, end_ang, .. }
            | RipCommand::OvalArc { st_ang, end_ang, .. }
            | RipCommand::PieSlice { st_ang, end_ang, .. }
            | RipCommand::OvalPieSlice { st_ang, end_ang, .. } => {
                *st_ang = self.start_angle;
                *end_ang = self.end_angle;
            }
            _ => {}
        }
        let mut commands = self.state_commands(self.tool);
        commands.push(shape);
        Ok(commands)
    }

    fn bezier_commands(&self, edit: &BezierEdit) -> Vec<RipCommand> {
        let mut commands = self.state_commands(Tool::Bezier);
        commands.push(bezier_command(edit.points, self.bezier_segments));
        commands
    }

    fn poly_commands(&self, points: &[(u16, u16)]) -> Vec<RipCommand> {
        let mut commands = self.state_commands(self.tool);
        commands.push(poly_command(self.tool, points));
        commands
    }

    /// Adds whatever is still being drawn: a Bézier curve, a path or typed text.
    fn finish_pending(&mut self) {
        self.finish_bezier();
        self.finish_poly();
        self.finish_text();
    }

    fn text_commands(&self, edit: &TextEdit) -> Vec<RipCommand> {
        let mut commands = self.state_commands(Tool::Text);
        commands.push(edit.command());
        commands
    }

    /// The font state `text_edit` is drawn with.
    fn text_font_state(&self, _edit: &TextEdit) -> Option<(u16, u16, u16)> {
        Some((self.text_font, self.text_direction, self.text_size))
    }

    /// The editable scene with the edited text in the tool's font and draw color, and its index.
    fn restyled_text(&self, index: usize, text: RipCommand) -> (Vec<RipCommand>, usize) {
        let preserved = self.document.preserved_commands();
        let (commands, index) = restyle_text(
            self.document.commands(),
            preserved,
            index,
            text,
            (self.text_font, self.text_direction, self.text_size),
            self.draw_color,
        );
        (commands[preserved..].to_vec(), index)
    }

    fn apply_editable(&mut self, editable: Vec<RipCommand>, selected: usize) {
        match self.document.replace_editable(editable) {
            Ok(()) => {
                self.selected = Some(selected);
                self.editing = None;
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        self.preview_dirty = true;
    }

    /// The topmost editable text at `point`, measured with the font it is drawn in.
    fn text_at(&self, point: (u16, u16)) -> Option<usize> {
        let commands = self.document.commands();
        let (px, py) = (i32::from(point.0), i32::from(point.1));
        (self.document.preserved_commands()..commands.len()).rev().find(|index| {
            let RipCommand::TextXY { x, y, text } = &commands[*index] else {
                return false;
            };
            let (width, height) = text_extent(Some(font_before(commands, *index)), text);
            let (x, y) = (i32::from(*x), i32::from(*y));
            px >= x - 2 && px <= x + width.max(8) + 2 && py >= y - 2 && py <= y + height.max(8) + 2
        })
    }

    /// Starts typing at `point`, or edits the text there.
    fn begin_text(&mut self, point: (u16, u16)) {
        self.finish_text();
        let edit = match self.text_at(point) {
            Some(index) => match &self.document.commands()[index] {
                RipCommand::TextXY { x, y, text } => TextEdit {
                    at: (*x, *y),
                    text: text.clone(),
                    index: Some(index),
                },
                _ => unreachable!("text_at returns text commands"),
            },
            None => TextEdit {
                at: point,
                text: String::new(),
                index: None,
            },
        };
        // Edited text shows its font and color in the tool settings, where they can be changed.
        if let Some(index) = edit.index {
            let commands = self.document.commands();
            (self.text_font, self.text_direction, self.text_size) = font_before(commands, index);
            self.draw_color = color_before(commands, index);
        }
        self.selected = edit.index;
        self.editing = None;
        self.text_edit = Some(edit);
        self.preview_dirty = true;
    }

    /// Adds the typed text, or applies the change to the edited text; empty edited text is removed.
    fn finish_text(&mut self) {
        let Some(edit) = self.text_edit.take() else {
            return;
        };
        // The edited command may have moved if the scene changed meanwhile.
        if edit
            .index
            .is_some_and(|index| !matches!(self.document.commands().get(index), Some(RipCommand::TextXY { .. })))
        {
            self.preview_dirty = true;
            return;
        }
        match edit.index {
            Some(index) if edit.text.is_empty() => match self.document.delete(index) {
                Ok(_) => {
                    self.selected = None;
                    self.editing = None;
                }
                Err(error) => self.error = Some(error.to_string()),
            },
            Some(index) => {
                let (editable, index) = self.restyled_text(index, edit.command());
                self.apply_editable(editable, index);
            }
            None if !edit.text.is_empty() => {
                let commands = self.text_commands(&edit);
                self.add_commands(commands);
            }
            None => {}
        }
        self.preview_dirty = true;
    }

    /// Applies typing to the text being edited.
    fn type_text(&mut self, context: &egui::Context) {
        if self.text_edit.is_none() || context.memory(|memory| memory.focused().is_some()) {
            return;
        }
        let (typed, backspace, enter) = context.input_mut(|input| {
            let typed: String = input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Text(text) | egui::Event::Paste(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            let backspace = input.count_and_consume_key(egui::Modifiers::NONE, egui::Key::Backspace);
            let enter = input.consume_key(egui::Modifiers::NONE, egui::Key::Enter);
            (typed, backspace, enter)
        });
        let Some(edit) = &mut self.text_edit else {
            return;
        };
        let before = edit.text.clone();
        for _ in 0..backspace {
            edit.text.pop();
        }
        edit.text.extend(typed.chars().filter(|c| is_rip_text_char(*c)));
        if edit.text != before {
            self.preview_dirty = true;
        }
        if enter {
            self.finish_text();
        }
    }

    fn finish_poly(&mut self) {
        let minimum = if self.tool == Tool::PolyLine { 2 } else { 3 };
        if self.poly.len() >= minimum {
            let commands = self.poly_commands(&self.poly);
            self.add_commands(commands);
        }
        self.poly.clear();
        self.preview_dirty = true;
    }

    /// The unfinished shape: a drag in progress or a Bézier curve being adjusted.
    fn pending_commands(&self) -> Vec<RipCommand> {
        // The palette being edited recolors the whole scene, as RIP does.
        if let Some(dialog) = self.palette_dialog.as_ref().filter(|dialog| dialog.target.is_none()) {
            return dialog.command().into_iter().collect();
        }
        if let Some(edit) = &self.bezier {
            return self.bezier_commands(edit);
        }
        if let Some(edit) = self.text_edit.as_ref().filter(|edit| edit.index.is_none() && !edit.text.is_empty()) {
            return self.text_commands(edit);
        }
        if self.tool.is_poly() && !self.poly.is_empty() {
            let mut points = self.poly.clone();
            if let Some(hover) = self.hover.filter(|hover| Some(*hover) != points.last().copied()) {
                points.push(hover);
            }
            let minimum = if self.tool == Tool::PolyLine { 2 } else { 3 };
            if points.len() >= minimum {
                return self.poly_commands(&points);
            }
        }
        match self.drag {
            Some((from, to)) if self.tool.is_dragged() => self.shape_commands(from, to).unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn refresh_preview(&mut self, context: &egui::Context) {
        let animation = self.transport.frame(None, self.document.commands().len());
        if animation != self.shown_frame {
            self.shown_frame = animation;
            self.preview_dirty = true;
        }
        let pending = if self.preview_to_selection || animation.is_some() {
            Vec::new()
        } else {
            self.pending_commands()
        };
        if pending != self.shown_pending {
            self.shown_pending = pending;
            self.preview_dirty = true;
        }
        let palette = self.palette_dialog.as_ref().and_then(|dialog| dialog.target.zip(dialog.command()));
        let replacement = self
            .shape_drag
            .as_ref()
            .filter(|drag| !self.preview_to_selection && drag.current != drag.command)
            .map(|drag| (drag.index, drag.current.clone()))
            .or(palette)
            .or_else(|| {
                // Property edits show while a number is dragged or a text is typed.
                self.editing
                    .as_ref()
                    .filter(|(index, draft)| {
                        !self.preview_to_selection
                            && *index >= self.document.preserved_commands()
                            && self.document.commands().get(*index).is_some_and(|command| command != draft)
                    })
                    .cloned()
            });
        if replacement != self.shown_replacement {
            self.shown_replacement = replacement;
            self.preview_dirty = true;
        }
        let editable = self
            .text_edit
            .as_ref()
            .filter(|_| !self.preview_to_selection)
            .and_then(|edit| Some((edit.index?, edit.command())))
            .filter(|(index, _)| matches!(self.document.commands().get(*index), Some(RipCommand::TextXY { .. })))
            .map(|(index, text)| self.restyled_text(index, text).0);
        if editable != self.shown_editable {
            self.shown_editable = editable;
            self.preview_dirty = true;
        }
        if !self.preview_dirty {
            return;
        }
        let preview = if animation.is_some() {
            self.document.preview_through(animation)
        } else if self.preview_to_selection {
            self.document.preview_through(self.selected)
        } else if let Some(editable) = &self.shown_editable {
            self.document.preview_editable(editable)
        } else if let Some((index, command)) = &self.shown_replacement {
            self.document.preview_replacing(*index, command)
        } else {
            self.document.preview_with(&self.shown_pending)
        };
        match preview {
            Ok(screen) => {
                self.palette = screen.screen().palette().clone();
                let image = egui::ColorImage::from_rgba_unmultiplied([screen.width(), screen.height()], &screen.rgba());
                if let Some(texture) = &mut self.texture {
                    texture.set(image, egui::TextureOptions::NEAREST);
                } else {
                    self.texture = Some(context.load_texture("rip-editor-canvas", image, egui::TextureOptions::NEAREST));
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        self.preview_dirty = false;
    }

    fn add_commands(&mut self, commands: Vec<RipCommand>) {
        match self.document.try_append_many(commands) {
            Ok(()) => {
                self.selected = self.document.commands().len().checked_sub(1);
                self.editing = None;
                self.preview_dirty = true;
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn add_shape(&mut self, from: (u16, u16), to: (u16, u16)) {
        match self.shape_commands(from, to) {
            Ok(commands) => {
                let placed = self.document.commands().len();
                self.add_commands(commands);
                // A placed button is selected for moving and resizing.
                if self.tool == Tool::Button && self.document.commands().len() > placed {
                    self.tool = Tool::Select;
                }
            }
            Err(error) if error.is_empty() => {}
            Err(error) => self.error = Some(error),
        }
    }

    /// Adds the curve being adjusted to the scene.
    fn finish_bezier(&mut self) {
        if let Some(edit) = self.bezier.take() {
            let commands = self.bezier_commands(&edit);
            self.add_commands(commands);
        }
    }

    fn cancel(&mut self) {
        if self.transport.animating() {
            self.transport_action(Action::Stop, 0.0);
            return;
        }
        if self.bezier.take().is_some()
            || !self.poly.is_empty()
            || self.text_edit.take().is_some()
            || self.drag.take().is_some()
            || self.shape_drag.take().is_some()
        {
            self.poly.clear();
            self.preview_dirty = true;
        } else if self.tool == Tool::Button {
            self.select_tool(Tool::Select);
        } else {
            self.select_shape(None);
        }
    }

    /// Edits the palette in effect at the end of the scene, or the `|Q` command at `target`.
    fn open_palette_dialog(&mut self, target: Option<usize>) {
        self.finish_bezier();
        let values = match target.and_then(|index| self.document.commands().get(index)) {
            Some(RipCommand::SetPalette { colors }) => {
                let mut values = palette::palette_at_end(&self.document.commands()[..target.unwrap_or_default()]);
                for (slot, color) in values.iter_mut().zip(colors) {
                    *slot = (*color).min(63);
                }
                values
            }
            _ => palette::palette_at_end(self.document.commands()),
        };
        self.palette_dialog = Some(PaletteDialog::new(values, target));
    }

    fn apply_palette(&mut self, dialog: PaletteDialog) {
        let Some(command) = dialog.command() else {
            return;
        };
        match dialog.target {
            Some(index) => self.replace_shape(index, command),
            None => self.add_commands(vec![command]),
        }
    }

    fn open_button_dialog(&mut self, target: Option<ButtonTarget>) {
        let options = match target {
            Some(target) => {
                let commands = self.document.commands();
                // A button without its own style command uses the style in effect before it.
                let style = target.style.map(|index| &commands[index]).or_else(|| {
                    commands[..target.button]
                        .iter()
                        .rev()
                        .find(|command| matches!(command, RipCommand::ButtonStyle { .. }))
                });
                let font = target.font.map(|index| &commands[index]);
                ButtonOptions::from_commands(style, font, &commands[target.button])
            }
            None => self.button.clone(),
        };
        self.button_dialog = Some(ButtonDialog::new(options, target));
    }

    fn apply_button(&mut self, target: Option<ButtonTarget>, options: ButtonOptions) {
        let Some(target) = target else {
            self.button = options;
            return;
        };
        let (x0, y0, x1, y1) = match &self.document.commands()[target.button] {
            RipCommand::Button { x0, y0, x1, y1, .. } => (*x0, *y0, *x1, *y1),
            _ => return,
        };
        let to = (x1 != 0 || y1 != 0).then_some((x1, y1));
        let mut replacements = vec![(target.button, options.button((x0, y0), to))];
        if let Some(style) = target.style {
            replacements.push((style, options.style()));
        }
        if let Some(font) = target.font {
            replacements.push((font, options.font_style()));
        }
        match self.document.replace_many(replacements) {
            Ok(()) => {
                self.editing = None;
                self.preview_dirty = true;
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    /// Host command and flags of the selected mouse region, or of new ones.
    fn mouse_toolbar(&mut self, ui: &mut egui::Ui) {
        let selected = self.selected_shape().and_then(|index| match &self.document.commands()[index] {
            RipCommand::Mouse { .. } => Some(index),
            _ => None,
        });
        let Some(index) = selected else {
            self.mouse_draft = None;
            ui.add(
                icy_engine_gui::egui::appearance::text_edit(&mut self.mouse_host)
                    .hint_text(fl!("rip-button-host-command"))
                    .desired_width(160.0),
            )
            .on_hover_text(fl!("rip-mouse-host-tooltip"));
            ui.checkbox(&mut self.mouse_invert, fl!("rip-mouse-invert"));
            ui.checkbox(&mut self.mouse_clear, fl!("rip-mouse-clear"));
            ui.weak(fl!("rip-mouse-hint"));
            return;
        };
        let RipCommand::Mouse { text, clk, clr, .. } = self.document.commands()[index].clone() else {
            return;
        };
        if self.mouse_draft.as_ref().map(|draft| draft.0) != Some(index) {
            self.mouse_draft = Some((index, text.clone()));
        }
        let mut changed = None;
        if let Some((_, draft)) = &mut self.mouse_draft {
            let response = ui
                .add(
                    icy_engine_gui::egui::appearance::text_edit(draft)
                        .hint_text(fl!("rip-button-host-command"))
                        .desired_width(160.0),
                )
                .on_hover_text(fl!("rip-mouse-host-tooltip"));
            // The command changes once, when typing ends, so it is one undo step.
            if response.lost_focus() && *draft != text {
                changed = Some((draft.clone(), clk, clr));
            }
        }
        let (mut invert, mut clear) = (clk != 0, clr != 0);
        let toggled = ui.checkbox(&mut invert, fl!("rip-mouse-invert")).changed();
        if ui.checkbox(&mut clear, fl!("rip-mouse-clear")).changed() || toggled {
            let text = self.mouse_draft.as_ref().map_or(text.clone(), |draft| draft.1.clone());
            changed = Some((text, u16::from(invert), u16::from(clear)));
        }
        if let Some((text, clk, clr)) = changed {
            if let RipCommand::Mouse { x0, y0, x1, y1, num, res, .. } = self.document.commands()[index].clone() {
                self.replace_shape(
                    index,
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
                    },
                );
            }
        }
        if self.icons.button(ui, "delete", &fl!("rip-editor-delete"), false).clicked() {
            self.delete_selected_shape();
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_space(8.0);
            ui.add(self.icons.image(ui, self.tool.icon(), 18.0).tint(ui.visuals().text_color()));
            ui.label(icy_engine_gui::egui::appearance::bold(ui, self.tool.label()));
            widgets::divider(ui);
            if self.tool.uses_line_style() {
                let styles = [LineStyle::Solid, LineStyle::Dotted, LineStyle::Center, LineStyle::Dashed].map(|style| {
                    let name = line_style_name(style);
                    (style, name.clone(), name)
                });
                widgets::segmented(ui, &mut self.line_style, &styles);
                let thickness = [
                    (1, fl!("rip-line-thin"), fl!("rip-line-thin-tooltip")),
                    (3, fl!("rip-line-thick"), fl!("rip-line-thick-tooltip")),
                ];
                widgets::segmented(ui, &mut self.line_thickness, &thickness);
            }
            if self.tool.uses_fill() {
                ui.weak(fl!("rip-fill-pattern"));
                egui::ComboBox::from_id_salt("rip-fill-pattern-tool")
                    .selected_text(fill_style_name(self.fill_pattern))
                    .show_ui(ui, |ui| {
                        for index in 0..=11 {
                            let style = FillStyle::try_from(index).expect("valid RIP fill style");
                            ui.selectable_value(&mut self.fill_pattern, style, fill_style_name(style));
                        }
                    });
            }
            if self.tool.has_angles() {
                ui.label(fl!("rip-editor-start-angle"));
                ui.add(egui::DragValue::new(&mut self.start_angle).range(0..=360).suffix("°"));
                ui.label(fl!("rip-editor-end-angle"));
                ui.add(egui::DragValue::new(&mut self.end_angle).range(0..=360).suffix("°"));
            }
            match self.tool {
                Tool::Polygon | Tool::FilledPolygon | Tool::PolyLine => {
                    ui.weak(fl!("rip-poly-hint"));
                }
                Tool::Mouse => self.mouse_toolbar(ui),
                Tool::Text => {
                    let fonts = button::font_names();
                    egui::ComboBox::from_id_salt("rip-text-font")
                        .selected_text(fonts[self.text_font.min(10) as usize].clone())
                        .show_ui(ui, |ui| {
                            for (index, name) in fonts.iter().enumerate() {
                                ui.selectable_value(&mut self.text_font, index as u16, name);
                            }
                        });
                    ui.add(egui::DragValue::new(&mut self.text_size).range(1..=10).prefix(fl!("rip-font-size-prefix")));
                    let directions = [
                        (0, fl!("rip-text-horizontal"), fl!("rip-text-horizontal")),
                        (1, fl!("rip-text-vertical"), fl!("rip-text-vertical")),
                    ];
                    widgets::segmented(ui, &mut self.text_direction, &directions);
                    ui.weak(if self.text_edit.is_some() {
                        fl!("rip-text-typing-hint")
                    } else {
                        fl!("rip-text-hint")
                    });
                }
                Tool::Bezier => {
                    ui.weak(fl!("rip-bezier-segments"));
                    ui.add(egui::DragValue::new(&mut self.bezier_segments).range(2..=200));
                    if self.bezier.is_some() {
                        ui.weak(fl!("rip-bezier-adjust-hint"));
                    }
                }
                Tool::Select => match self.selected_shape() {
                    Some(index) => {
                        let command = &self.document.commands()[index];
                        let is_button = matches!(command, RipCommand::Button { .. });
                        let name = match command {
                            RipCommand::Button { text, .. } => format!("{} \"{}\"", Tool::Button.label(), text.split("<>").nth(1).unwrap_or(text)),
                            _ => shape_name(command).unwrap_or_default(),
                        };
                        ui.label(icy_engine_gui::egui::appearance::bold(ui, name));
                        if is_button && ui.button(fl!("rip-button-edit")).clicked() {
                            if let Some(target) = ButtonTarget::find(self.document.commands(), index) {
                                self.open_button_dialog(Some(target));
                            }
                        }
                        if self.icons.button(ui, "delete", &fl!("rip-editor-delete"), false).clicked() {
                            self.delete_selected_shape();
                        }
                    }
                    None => {
                        ui.weak(fl!("rip-select-hint"));
                    }
                },
                Tool::Button => {
                    ui.weak(fl!("rip-button-place-hint", width = self.button.width, height = self.button.height));
                    let kinds = ButtonKind::ALL.map(|kind| (kind, kind.label(), kind.tooltip()));
                    widgets::segmented(ui, &mut self.button.kind, &kinds);
                    ui.add(
                        icy_engine_gui::egui::appearance::text_edit(&mut self.button.label)
                            .hint_text(fl!("rip-editor-label"))
                            .desired_width(120.0),
                    );
                    ui.add(
                        icy_engine_gui::egui::appearance::text_edit(&mut self.button.host_command)
                            .hint_text(fl!("rip-button-host-command"))
                            .desired_width(120.0),
                    );
                    if self.button.kind == ButtonKind::Icon {
                        ui.add(
                            icy_engine_gui::egui::appearance::text_edit(&mut self.button.icon_file)
                                .hint_text(fl!("rip-button-icon-file"))
                                .desired_width(100.0),
                        );
                    }
                    if ui.button(fl!("rip-button-style-open")).on_hover_text(fl!("rip-button-style-tooltip")).clicked() {
                        self.open_button_dialog(None);
                    }
                }
                _ => {}
            }
            let edited = match self.tool {
                Tool::Select => self.selected_shape().and_then(|index| self.shape(index)).map(|(command, _)| command),
                Tool::Mouse => match self.drag {
                    Some((from, to)) => self.mouse_commands(from, to).pop(),
                    None => self.selected_shape().and_then(|index| self.shape(index)).map(|(command, _)| command),
                },
                _ => self.shown_pending.last().cloned(),
            };
            let parameters = edited.as_ref().and_then(describe).or_else(|| self.hover.map(|(x, y)| format!("{x}, {y}")));
            if let Some(parameters) = parameters {
                widgets::divider(ui);
                ui.label(egui::RichText::new(parameters).monospace().color(ui.visuals().weak_text_color()));
            }
        });
    }

    fn move_selected_command(&mut self, delta: isize) {
        self.finish_text();
        let Some(index) = self.selected else {
            return;
        };
        let target = index.saturating_add_signed(delta);
        match self.document.move_command(index, target) {
            Ok(()) => {
                self.selected = Some(target);
                self.listed_selection = self.selected;
                self.editing = None;
                self.preview_dirty = true;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn delete_selected_command(&mut self) {
        self.finish_text();
        let Some(index) = self.selected else {
            return;
        };
        match self.document.delete(index) {
            Ok(_) => {
                self.selected = None;
                self.editing = None;
                self.preview_dirty = true;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    /// The command the canvas shows the drawing through: the animation frame, or the selection
    /// while previewing through it.
    fn frame(&self) -> Option<usize> {
        self.transport
            .frame(self.selected.filter(|_| self.preview_to_selection), self.document.commands().len())
    }

    /// Selects the animation frame whenever it moves, so the command list follows playback.
    fn sync_selection(&mut self) {
        let frame = self.frame();
        if let Some(frame) = self.transport.follow(frame).filter(|frame| self.selected != Some(*frame)) {
            self.selected = Some(frame);
            self.editing = None;
        }
    }

    /// Turns a paused or playing animation into a preview through its frame, where the drawing
    /// can be edited, or turns the preview off.
    fn toggle_preview(&mut self) {
        if self.transport.animating() {
            let frame = self.transport.end_for_preview(self.document.commands().len());
            self.select_shape(frame);
            self.preview_to_selection = frame.is_some();
        } else {
            self.preview_to_selection = !self.preview_to_selection;
        }
        self.preview_dirty = true;
    }

    fn transport_action(&mut self, action: Action, now: f64) {
        if action != Action::Stop {
            self.finish_pending();
            self.commit_properties();
        }
        let preview = self.selected.filter(|_| self.preview_to_selection);
        let outcome = self.transport.apply(action, &RipTimeline(&self.document), preview, now);
        if outcome.stopped {
            self.preview_to_selection = false;
        }
        if let Some(index) = outcome.select {
            self.select_shape(Some(index));
        }
        self.preview_dirty = true;
    }

    fn advance_playback(&mut self, context: &egui::Context) {
        let now = context.input(|input| input.time);
        if let Some(wait) = self.transport.advance(&RipTimeline(&self.document), now, &mut |_| {}) {
            context.request_repaint_after(std::time::Duration::from_secs_f64(wait));
        }
    }

    fn transport_bar(&mut self, ui: &mut egui::Ui) {
        let frame = self.frame();
        let now = ui.input(|input| input.time);
        if let Some(action) = self
            .transport
            .ui(ui, &mut self.icons, frame, self.document.commands().len(), &RipTimeline(&self.document))
        {
            self.transport_action(action, now);
        }
    }

    fn command_list(&mut self, context: &egui::Context, blocked: bool, editing_blocked: bool) {
        egui::SidePanel::right("rip-commands")
            .default_width(280.0)
            .min_width(220.0)
            .show(context, |ui| {
                if blocked {
                    ui.disable();
                }
                let count = self.document.commands().len();
                let preserved = self.document.preserved_commands();
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(icy_engine_gui::egui::appearance::bold(ui, fl!("rip-editor-commands")));
                    ui.weak(count.to_string());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let editable = !editing_blocked && self.selected.is_some_and(|index| index >= preserved && index < count);
                        let mut action = None;
                        ui.add_enabled_ui(editable, |ui| {
                            if self.icons.button_sized(ui, "delete", &fl!("rip-editor-delete"), false, 26.0).clicked() {
                                action = Some(0);
                            }
                        });
                        ui.add_enabled_ui(editable && self.selected.is_some_and(|index| index + 1 < count), |ui| {
                            if self.icons.button_sized(ui, "move_down", &fl!("rip-editor-down"), false, 26.0).clicked() {
                                action = Some(1);
                            }
                        });
                        ui.add_enabled_ui(editable && self.selected.is_some_and(|index| index > preserved), |ui| {
                            if self.icons.button_sized(ui, "move_up", &fl!("rip-editor-up"), false, 26.0).clicked() {
                                action = Some(-1);
                            }
                        });
                        widgets::divider(ui);
                        let through = self.preview_to_selection && !self.transport.animating();
                        if self
                            .icons
                            .button_sized(ui, "visibility", &fl!("rip-editor-preview-through"), through, 26.0)
                            .clicked()
                        {
                            self.toggle_preview();
                        }
                        match action {
                            Some(0) => self.delete_selected_command(),
                            Some(delta) => self.move_selected_command(delta),
                            None => {}
                        }
                    });
                });
                if preserved > 0 {
                    ui.add(egui::Label::new(egui::RichText::new(fl!("rip-editor-preserved")).small().weak()).wrap());
                }
                ui.separator();

                let previous = self.selected;
                let selected_button = self.selected.and_then(|index| ButtonTarget::owning(self.document.commands(), index));
                let count = self.document.commands().len();
                let list_height = (ui.available_height() - 250.0).max(120.0);
                let mut scroll = egui::ScrollArea::vertical()
                    .id_salt("rip-command-list")
                    .auto_shrink([false, false])
                    .max_height(list_height);
                // A shape picked on the canvas is scrolled into view in the list.
                if self.selected != self.listed_selection {
                    if let Some(index) = self.selected.filter(|index| !self.visible_rows.contains(index)) {
                        scroll = scroll.vertical_scroll_offset((index as f32 * COMMAND_ROW_HEIGHT - list_height / 2.0).max(0.0));
                    }
                }
                let frame = self.frame();
                let mut double_clicked = None;
                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    scroll.show_rows(ui, COMMAND_ROW_HEIGHT, count, |ui, rows| {
                        self.visible_rows = rows.clone();
                        for index in rows {
                            let Some(command) = self.document.commands().get(index) else {
                                break;
                            };
                            // A button's style and font commands are highlighted with it.
                            let related =
                                selected_button.is_some_and(|target| target.button == index || target.style == Some(index) || target.font == Some(index));
                            let row = CommandRow {
                                index,
                                command,
                                selected: self.selected == Some(index),
                                mark: RowMark::new(index, frame),
                                related,
                                preserved: index < preserved,
                            };
                            let mouse = is_mouse_region(command);
                            let shape = select::geometry(command, (0, 0)).is_some();
                            let response = row.show(ui, &mut self.icons, &self.palette);
                            if response.double_clicked() {
                                double_clicked = Some(index);
                            } else if response.clicked() && editing_blocked {
                                self.selected = Some(index);
                            } else if response.clicked() {
                                self.finish_text();
                                // Mouse regions are edited with their own tool, shapes with the select tool.
                                if mouse && self.tool != Tool::Mouse {
                                    self.select_tool(Tool::Mouse);
                                } else if !mouse && self.tool == Tool::Mouse && shape {
                                    self.select_tool(Tool::Select);
                                }
                                self.selected = Some(index);
                            }
                            if index < preserved {
                                response.on_hover_text(fl!("rip-editor-preserved"));
                            }
                        }
                    });
                });
                if let Some(index) = double_clicked {
                    // Double-clicking runs the animation up to the command.
                    self.transport_action(Action::Seek(index), ui.input(|input| input.time));
                    self.sync_selection();
                    self.selected = Some(index);
                }
                self.listed_selection = self.selected;
                // The command list follows and seeks the animation; only its editing waits.
                if editing_blocked {
                    ui.disable();
                }
                if self.selected != previous {
                    // An edit still in a focused field is kept when another command is chosen;
                    // restyling a text can add or remove commands before the new selection.
                    let (chosen, edited, before) = (self.selected, self.editing.as_ref().map(|(index, _)| *index), self.document.commands().len());
                    self.commit_properties();
                    let shift = self.document.commands().len() as isize - before as isize;
                    self.selected = chosen.map(|index| {
                        if edited.is_some_and(|edited| index > edited) {
                            (index as isize + shift).max(0) as usize
                        } else {
                            index
                        }
                    });
                    self.editing = None;
                    if self.preview_to_selection {
                        self.preview_dirty = true;
                    }
                }
                if self.editing.is_none() {
                    self.editing = self
                        .selected
                        .and_then(|index| self.document.commands().get(index).cloned().map(|cmd| (index, cmd)));
                    self.editing_style = None;
                }
                // Text also edits the font and color it is drawn with.
                let text_index = self
                    .editing
                    .as_ref()
                    .filter(|(index, draft)| *index >= preserved && matches!(draft, RipCommand::TextXY { .. }))
                    .map(|(index, _)| *index);
                match text_index {
                    Some(index) if self.editing_style.map(|style| style.0) != Some(index) => {
                        let commands = self.document.commands();
                        self.editing_style = Some((index, font_before(commands, index), color_before(commands, index)));
                    }
                    None => self.editing_style = None,
                    _ => {}
                }
                let button_target = self.selected.and_then(|index| ButtonTarget::owning(self.document.commands(), index));
                let mut open_button = None;
                let mut open_palette = None;
                let mut apply = None;
                if let Some((index, draft)) = self.editing.as_mut() {
                    ui.separator();
                    ui.horizontal(|ui| {
                        if let Some(icon) = command_icon(draft) {
                            ui.add(self.icons.image(ui, icon, 16.0));
                        }
                        ui.label(icy_engine_gui::egui::appearance::bold(ui, command_name(draft)));
                        ui.weak(format!("#{}", *index + 1));
                    });
                    ui.add_space(4.0);
                    let original = self.document.commands().get(*index);
                    if *index < preserved {
                        ui.weak(fl!("rip-editor-preserved"));
                    } else if let Some(target) = button_target {
                        ui.weak(fl!("rip-button-edit-hint"));
                        if ui
                            .add(egui::Button::new(fl!("rip-button-edit")).min_size(egui::vec2(ui.available_width(), 28.0)))
                            .clicked()
                        {
                            open_button = Some(target);
                        }
                    } else if matches!(draft, RipCommand::SetPalette { .. }) {
                        if ui
                            .add(egui::Button::new(fl!("rip-palette-edit")).min_size(egui::vec2(ui.available_width(), 28.0)))
                            .clicked()
                        {
                            open_palette = Some(*index);
                        }
                    } else {
                        let mut supported = true;
                        let area = egui::ScrollArea::vertical()
                            .id_salt("rip-properties")
                            .max_height((ui.available_height() - 8.0).max(60.0))
                            .show(ui, |ui| {
                                supported = command_properties(ui, draft);
                                if !supported {
                                    ui.weak(fl!("rip-editor-unsupported-properties"));
                                }
                                if let Some((_, font, color)) = self.editing_style.as_mut() {
                                    ui.add_space(4.0);
                                    text_style_properties(ui, &self.palette, font, color);
                                }
                            });
                        if supported {
                            let commands = self.document.commands();
                            let restyled = self
                                .editing_style
                                .is_some_and(|(index, font, color)| (font, color) != (font_before(commands, index), color_before(commands, index)));
                            let changed = original != Some(&*draft) || restyled;
                            // Changes apply without a confirmation, once a drag ends or a field
                            // of the panel is left (Enter, Tab or a click elsewhere).
                            let context = ui.ctx().clone();
                            let dragging = context.input(|input| input.pointer.any_down());
                            let typing = context
                                .memory(|memory| memory.focused())
                                .and_then(|id| context.read_response(id))
                                .is_some_and(|response| area.inner_rect.intersects(response.rect));
                            if changed && !dragging && !typing {
                                apply = Some((*index, draft.clone()));
                            }
                        }
                    }
                }
                if let Some((index, command)) = apply {
                    if let Some((_, font, color)) = self.editing_style.filter(|style| style.0 == index) {
                        let (commands, index) = restyle_text(self.document.commands(), preserved, index, command, font, color);
                        self.apply_editable(commands[preserved..].to_vec(), index);
                    } else {
                        match self.document.replace(index, command) {
                            Ok(()) => self.preview_dirty = true,
                            Err(error) => self.error = Some(error.to_string()),
                        }
                    }
                }
                if let Some(target) = open_button {
                    self.open_button_dialog(Some(target));
                }
                if let Some(index) = open_palette {
                    self.open_palette_dialog(Some(index));
                }
            });
    }

    /// Applies a changed property draft of the command list, e.g. before another one is chosen.
    fn commit_properties(&mut self) {
        let preserved = self.document.preserved_commands();
        let Some((index, draft)) = self.editing.clone().filter(|(index, _)| *index >= preserved) else {
            return;
        };
        let commands = self.document.commands();
        let style = self.editing_style.filter(|style| style.0 == index);
        let restyled = style.is_some_and(|(_, font, color)| (font, color) != (font_before(commands, index), color_before(commands, index)));
        if commands.get(index) == Some(&draft) && !restyled {
            return;
        }
        if let Some((_, font, color)) = style {
            let (commands, index) = restyle_text(self.document.commands(), preserved, index, draft, font, color);
            self.apply_editable(commands[preserved..].to_vec(), index);
        } else {
            match self.document.replace(index, draft) {
                Ok(()) => self.preview_dirty = true,
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }

    fn sidebar(&mut self, context: &egui::Context, blocked: bool) {
        let panel_fill = context.style().visuals.panel_fill;
        egui::SidePanel::left("rip-tools")
            .exact_width(136.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(panel_fill).inner_margin(egui::Margin::symmetric(6, 6)))
            .show(context, |ui| {
                ui.add_enabled_ui(!blocked, |ui| {
                    for (id, label, color) in [
                        ("draw", fl!("rip-editor-color"), &mut self.draw_color),
                        ("border", fl!("rip-editor-border-color"), &mut self.border_color),
                        ("fill", fl!("rip-editor-fill-color"), &mut self.fill_color),
                    ] {
                        ui.horizontal(|ui| {
                            ui.label(label);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                button::color_picker_sized(ui, id, &self.palette, color, egui::vec2(30.0, 26.0));
                            });
                        });
                    }
                    if ui
                        .add(egui::Button::new(fl!("rip-palette-edit")).min_size(egui::vec2(ui.available_width(), 26.0)))
                        .on_hover_text(fl!("rip-palette-edit-tooltip"))
                        .clicked()
                    {
                        self.open_palette_dialog(None);
                    }
                    ui.separator();
                    egui::Grid::new("rip-tools-grid").num_columns(3).show(ui, |ui| {
                        for (index, tool) in Tool::ALL.into_iter().enumerate() {
                            if self.icons.button_sized(ui, tool.icon(), &tool.label(), self.tool == tool, 36.0).clicked() {
                                self.select_tool(tool);
                            }
                            if index % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });
                    ui.add_space(6.0);
                    let create = egui::Button::new(fl!("rip-button-create"))
                        .selected(self.tool == Tool::Button)
                        .min_size(egui::vec2(ui.available_width(), 30.0));
                    if ui.add(create).on_hover_text(fl!("rip-button-create-tooltip")).clicked() {
                        self.finish_pending();
                        self.open_button_dialog(None);
                        if let Some(dialog) = &mut self.button_dialog {
                            dialog.create = true;
                        }
                    }
                });
            });
    }

    pub fn show(&mut self, context: &egui::Context, blocked: bool) {
        self.advance_playback(context);
        self.sync_selection();
        let blocked = blocked || self.button_dialog.is_some() || self.palette_dialog.is_some();
        let editing_blocked = blocked || self.transport.animating();
        let panel_fill = context.style().visuals.panel_fill;
        egui::TopBottomPanel::top("rip-toolbar")
            .exact_height(TOOLBAR_HEIGHT)
            .frame(egui::Frame::new().fill(panel_fill))
            .show(context, |ui| {
                if editing_blocked {
                    ui.disable();
                }
                self.toolbar(ui);
            });
        egui::TopBottomPanel::top("rip-transport")
            .exact_height(42.0)
            .frame(egui::Frame::new().fill(panel_fill))
            .show(context, |ui| {
                if blocked {
                    ui.disable();
                }
                self.transport_bar(ui);
            });
        self.sync_selection();
        self.sidebar(context, editing_blocked);
        self.command_list(context, blocked, editing_blocked);
        if !blocked && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.cancel();
        }
        if !editing_blocked {
            self.type_text(context);
            // Text fields in the toolbar and command list keep their keys.
            if matches!(self.tool, Tool::Select | Tool::Mouse) && context.memory(|memory| memory.focused().is_none()) {
                if context.input_mut(|input| {
                    input.consume_key(egui::Modifiers::NONE, egui::Key::Delete) || input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
                }) {
                    self.delete_selected_shape();
                }
                for (key, dx, dy) in [
                    (egui::Key::ArrowLeft, -1, 0),
                    (egui::Key::ArrowRight, 1, 0),
                    (egui::Key::ArrowUp, 0, -1),
                    (egui::Key::ArrowDown, 0, 1),
                ] {
                    // Shift first: the plain pattern also matches Shift+arrow.
                    for (modifiers, step) in [(egui::Modifiers::SHIFT, 10), (egui::Modifiers::NONE, 1)] {
                        if self.selected_shape().is_some() && context.input_mut(|input| input.consume_key(modifiers, key)) {
                            self.nudge_selected(dx * step, dy * step);
                        }
                    }
                }
            }
            if (self.bezier.is_some() || !self.poly.is_empty())
                && context.memory(|memory| memory.focused().is_none())
                && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
            {
                self.finish_pending();
            }
        }
        egui::CentralPanel::default().show(context, |ui| {
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            self.canvas(ui, editing_blocked);
        });
        if let Some(dialog) = &mut self.button_dialog {
            match dialog.show(context, &self.palette) {
                DialogResult::Open => {}
                DialogResult::Cancel => self.button_dialog = None,
                DialogResult::Apply(options) => {
                    let (target, create) = (dialog.target, dialog.create);
                    self.button_dialog = None;
                    self.apply_button(target, options);
                    if create {
                        self.select_tool(Tool::Button);
                    }
                }
            }
        }
        if let Some(dialog) = &mut self.palette_dialog {
            match dialog.show(context) {
                PaletteResult::Open => {}
                PaletteResult::Cancel => {
                    self.palette_dialog = None;
                    self.preview_dirty = true;
                }
                PaletteResult::Apply => {
                    if let Some(dialog) = self.palette_dialog.take() {
                        self.apply_palette(dialog);
                    }
                    self.preview_dirty = true;
                }
            }
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui, blocked: bool) {
        self.refresh_preview(ui.ctx());
        let Some(texture) = self.texture.clone() else {
            return;
        };
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            let scale = (ui.available_width() / WIDTH as f32).min(ui.available_height() / HEIGHT as f32).max(0.5);
            let size = egui::vec2(WIDTH as f32, HEIGHT as f32) * scale;
            let response = ui.add(egui::Image::new(&texture).fit_to_exact_size(size).sense(egui::Sense::click_and_drag()));
            #[cfg(test)]
            {
                self.canvas_rect = Some(response.rect);
            }
            let origin = response.rect.min;
            let at = |pos: egui::Pos2| {
                let x = ((pos.x - origin.x) / scale).floor().clamp(0.0, (WIDTH - 1) as f32) as u16;
                let y = ((pos.y - origin.y) / scale).floor().clamp(0.0, (HEIGHT - 1) as f32) as u16;
                (x, y)
            };
            let on_screen = |point: (u16, u16)| origin + egui::vec2((point.0 as f32 + 0.5) * scale, (point.1 as f32 + 0.5) * scale);
            self.hover = response.hover_pos().map(at);
            if !blocked {
                self.canvas_input(ui, &response, &at, &on_screen);
                // The texture is uploaded at the end of the frame, so the changed shape shows now.
                self.refresh_preview(ui.ctx());
            }
            if self.tool == Tool::Mouse {
                self.paint_mouse_regions(ui, &on_screen, scale);
            }
            if matches!(self.tool, Tool::Select | Tool::Mouse) {
                if let Some((command, geometry)) = self.selected_shape().and_then(|index| self.shape(index)) {
                    let to_screen = |point: select::Point| on_screen((point.0.clamp(0, 1295) as u16, point.1.clamp(0, 1295) as u16));
                    let accent = ui.visuals().selection.stroke.color;
                    let points_only = matches!(
                        command,
                        RipCommand::Line { .. }
                            | RipCommand::Bezier { .. }
                            | RipCommand::Polygon { .. }
                            | RipCommand::FilledPolygon { .. }
                            | RipCommand::PolyLine { .. }
                    );
                    // Lines and curves are shown by their points; other shapes get a frame.
                    if !points_only {
                        let (x0, y0, x1, y1) = select::bounds(&geometry);
                        let frame = egui::Rect::from_two_pos(to_screen((x0, y0)), to_screen((x1, y1))).expand(scale * 0.5 + 2.0);
                        ui.painter().rect_stroke(frame, 0.0, Stroke::new(1.0, accent), egui::StrokeKind::Middle);
                    }
                    if let (RipCommand::Bezier { .. }, Geometry::Points(points)) = (&command, &geometry) {
                        let guide = Stroke::new(1.0, Color32::from_white_alpha(90));
                        ui.painter().line_segment([to_screen(points[0]), to_screen(points[1])], guide);
                        ui.painter().line_segment([to_screen(points[3]), to_screen(points[2])], guide);
                    }
                    let handles = select::handles(&command, &geometry);
                    let count = handles.len();
                    for (index, (_, point)) in handles.into_iter().enumerate() {
                        let handle = egui::Rect::from_center_size(to_screen(point), egui::Vec2::splat(8.0));
                        let color = if !points_only {
                            accent
                        } else if index == 0 || index + 1 == count {
                            Color32::from_rgb(0xD0, 0x30, 0x30)
                        } else {
                            Color32::from_rgb(0x30, 0xC0, 0x30)
                        };
                        ui.painter().rect_filled(handle, 1.0, if points_only { Color32::BLACK } else { Color32::WHITE });
                        ui.painter()
                            .rect_stroke(handle, 1.0, Stroke::new(if points_only { 2.0 } else { 1.5 }, color), egui::StrokeKind::Middle);
                    }
                }
            }
            if let Some(edit) = &self.text_edit {
                let font = self.text_font_state(edit);
                let (width, height) = text_extent(font, &edit.text);
                let (char_width, char_height) = text_extent(font, "W");
                let vertical = font.is_some_and(|(_, direction, _)| direction == 1);
                let (x, y) = (f32::from(edit.at.0), f32::from(edit.at.1));
                let point = |px: f32, py: f32| origin + egui::vec2(px * scale, py * scale);
                let (box_width, box_height) = if vertical {
                    (char_width.max(width) as f32, height.max(char_height) as f32)
                } else {
                    (width.max(char_width) as f32, char_height.max(height) as f32)
                };
                let accent = ui.visuals().selection.stroke.color;
                let frame = egui::Rect::from_min_max(point(x, y), point(x + box_width, y + box_height)).expand(2.0);
                ui.painter()
                    .rect_stroke(frame, 1.0, Stroke::new(1.0, accent.gamma_multiply(0.6)), egui::StrokeKind::Outside);
                // A blinking caret after the last character (above it for vertical text).
                let time = ui.input(|input| input.time);
                if (time * 2.0) as i64 % 2 == 0 {
                    let caret = if vertical {
                        egui::Rect::from_min_max(point(x, y), point(x + box_width, y)).expand2(egui::vec2(0.0, 1.0))
                    } else {
                        egui::Rect::from_min_max(point(x + width as f32, y), point(x + width as f32, y + box_height)).expand2(egui::vec2(1.0, 0.0))
                    };
                    ui.painter().rect_filled(caret, 0.0, accent);
                }
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
            }
            if let Some(edit) = &self.bezier {
                let points = edit.points.map(on_screen);
                let guide = Stroke::new(1.0, Color32::from_white_alpha(90));
                ui.painter().line_segment([points[0], points[1]], guide);
                ui.painter().line_segment([points[3], points[2]], guide);
                for (index, point) in points.iter().enumerate() {
                    let color = if index == 0 || index == 3 {
                        Color32::from_rgb(0xD0, 0x30, 0x30)
                    } else {
                        Color32::from_rgb(0x30, 0xC0, 0x30)
                    };
                    let handle = egui::Rect::from_center_size(*point, egui::Vec2::splat(9.0));
                    ui.painter().rect_stroke(handle, 1.0, Stroke::new(2.0, color), egui::StrokeKind::Middle);
                }
            }
        });
    }

    /// Outlines every mouse region with its host command, and the one being dragged out.
    fn paint_mouse_regions(&self, ui: &egui::Ui, on_screen: &dyn Fn((u16, u16)) -> egui::Pos2, scale: f32) {
        let color = Color32::from_rgb(0x40, 0xC8, 0xFF);
        let painter = ui.painter();
        let area = |(x0, y0, x1, y1): (u16, u16, u16, u16)| {
            egui::Rect::from_two_pos(on_screen((x0.min(x1), y0.min(y1))), on_screen((x0.max(x1), y0.max(y1)))).expand(scale * 0.5)
        };
        let selected = self.selected_shape();
        for index in self.document.preserved_commands()..self.document.commands().len() {
            let Some((RipCommand::Mouse { x0, y0, x1, y1, text, .. }, _)) = self.shape(index) else {
                continue;
            };
            let rect = area((x0, y0, x1, y1));
            let chosen = selected == Some(index);
            painter.rect_filled(rect, 0.0, color.gamma_multiply(if chosen { 0.22 } else { 0.12 }));
            painter.rect_stroke(rect, 0.0, Stroke::new(if chosen { 2.0 } else { 1.0 }, color), egui::StrokeKind::Inside);
            let label = if text.is_empty() { fl!("rip-mouse-no-command") } else { text };
            let galley = painter.layout(label, egui::FontId::proportional(11.0), Color32::WHITE, (rect.width() - 6.0).max(0.0));
            let tag = egui::Rect::from_min_size(rect.min, galley.size() + egui::vec2(6.0, 2.0)).intersect(rect);
            painter.rect_filled(tag, 0.0, color.gamma_multiply(0.85));
            painter.with_clip_rect(tag).galley(tag.min + egui::vec2(3.0, 1.0), galley, Color32::WHITE);
        }
        if let Some((from, to)) = self.drag {
            let rect = area((from.0, from.1, to.0, to.1));
            painter.rect_filled(rect, 0.0, color.gamma_multiply(0.18));
            painter.rect_stroke(rect, 0.0, Stroke::new(1.5, color), egui::StrokeKind::Inside);
        }
    }

    fn select_input(&mut self, ui: &egui::Ui, response: &egui::Response, at: &dyn Fn(egui::Pos2) -> (u16, u16), on_screen: &dyn Fn((u16, u16)) -> egui::Pos2) {
        let pointer = response.interact_pointer_pos();
        let origin = ui.input(|input| input.pointer.press_origin());
        let scale = (on_screen((1, 0)).x - on_screen((0, 0)).x).max(0.1);
        let tolerance = HANDLE_RADIUS / scale;
        let scene = |pos: egui::Pos2| {
            let (x, y) = at(pos);
            (x as f32 + 0.5, y as f32 + 0.5)
        };
        let handle_at = |editor: &Self, pos: egui::Pos2| {
            let index = editor.selected_shape()?;
            let (command, geometry) = editor.shape(index)?;
            select::handles(&command, &geometry)
                .into_iter()
                .map(|(handle, point)| (handle, on_screen((point.0.max(0) as u16, point.1.max(0) as u16)).distance(pos)))
                .filter(|(_, distance)| *distance <= HANDLE_RADIUS)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(handle, _)| (index, handle))
        };
        if let Some(hover) = response.hover_pos() {
            let cursor = match handle_at(self, hover) {
                Some((_, handle)) => Some(handle.cursor()),
                None if self.shape_drag.is_some() => Some(egui::CursorIcon::Grabbing),
                None => self.shape_at(scene(hover), tolerance).map(|_| egui::CursorIcon::Grab),
            };
            if let Some(cursor) = cursor {
                ui.ctx().set_cursor_icon(cursor);
            }
        }
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(start) = origin.or(pointer) {
                let target = handle_at(self, start).or_else(|| self.shape_at(scene(start), tolerance).map(|index| (index, Handle::Move)));
                self.shape_drag = None;
                self.select_shape(target.map(|(index, _)| index));
                if target.is_none() && self.tool == Tool::Mouse {
                    let start = at(start);
                    self.drag = Some((start, start));
                }
                if let Some((index, handle)) = target {
                    if let Some((command, geometry)) = self.shape(index) {
                        let (x, y) = at(start);
                        self.shape_drag = Some(ShapeDrag {
                            index,
                            handle,
                            start: (i32::from(x), i32::from(y)),
                            original: geometry,
                            current: command.clone(),
                            command,
                        });
                    }
                }
            }
        }
        if let (Some(pointer), Some(index)) = (pointer, self.shape_drag.as_ref().map(|drag| drag.index)) {
            let size = self.button_size_at(index);
            if let Some(drag) = &mut self.shape_drag {
                let (x, y) = at(pointer);
                let geometry = select::drag(&drag.original, drag.handle, drag.start, (i32::from(x), i32::from(y)));
                drag.current = select::apply(&drag.command, &geometry, size);
            }
            if response.drag_stopped() {
                if let Some(drag) = self.shape_drag.take() {
                    if drag.current != drag.command {
                        self.replace_shape(drag.index, drag.current);
                    }
                }
            }
        }
        if let (Some((from, _)), Some(pointer)) = (self.drag, pointer) {
            let to = at(pointer);
            self.drag = Some((from, to));
            if response.drag_stopped() {
                self.drag = None;
                self.add_mouse_region(from, to);
            }
        }
        if response.clicked() {
            let index = pointer.and_then(|pointer| self.shape_at(scene(pointer), tolerance));
            self.select_shape(index);
        }
        if response.double_clicked() {
            let text = self.selected_shape().and_then(|index| match &self.document.commands()[index] {
                RipCommand::TextXY { x, y, .. } => Some((*x, *y)),
                _ => None,
            });
            if let Some(anchor) = text {
                // Double-clicking text types into it with the text tool.
                self.select_tool(Tool::Text);
                self.begin_text(anchor);
            } else if let Some(target) = self.selected_shape().and_then(|index| ButtonTarget::find(self.document.commands(), index)) {
                self.open_button_dialog(Some(target));
            }
        }
    }

    fn canvas_input(&mut self, ui: &egui::Ui, response: &egui::Response, at: &dyn Fn(egui::Pos2) -> (u16, u16), on_screen: &dyn Fn((u16, u16)) -> egui::Pos2) {
        let pointer = response.interact_pointer_pos();
        let origin = ui.input(|input| input.pointer.press_origin());
        if matches!(self.tool, Tool::Select | Tool::Mouse) {
            self.select_input(ui, response, at, on_screen);
            return;
        }
        if self.tool == Tool::Text {
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
            }
            if response.clicked() {
                if let Some(point) = pointer.map(at) {
                    self.begin_text(point);
                }
            } else if response.secondary_clicked() {
                self.finish_text();
            }
            return;
        }
        if self.tool.is_poly() {
            if response.secondary_clicked() {
                self.finish_poly();
            } else if response.clicked() || response.double_clicked() {
                if let Some(point) = pointer.map(at) {
                    if self.poly.last().copied() != Some(point) {
                        self.poly.push(point);
                        if self.poly.len() == 1295 {
                            self.finish_poly();
                        }
                    }
                }
            }
            return;
        }
        if self.tool == Tool::Bezier {
            if response.secondary_clicked() {
                self.finish_bezier();
                return;
            }
            if response.drag_started_by(egui::PointerButton::Primary) {
                let start = origin.or(pointer);
                let handle = self.bezier.as_ref().zip(start).and_then(|(edit, start)| {
                    edit.points
                        .iter()
                        .enumerate()
                        .map(|(index, point)| (index, on_screen(*point).distance(start)))
                        .filter(|(_, distance)| *distance <= HANDLE_RADIUS)
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(index, _)| index)
                });
                match handle {
                    Some(index) => {
                        if let Some(edit) = &mut self.bezier {
                            edit.moving = Some(index);
                        }
                    }
                    None => {
                        // Starting elsewhere finishes the curve and begins the next one.
                        self.finish_bezier();
                        if let Some(start) = start.map(at) {
                            self.drag = Some((start, start));
                        }
                    }
                }
            }
            if let (Some(edit), Some(pointer)) = (&mut self.bezier, pointer) {
                if let Some(index) = edit.moving {
                    edit.points[index] = at(pointer);
                    if response.drag_stopped() {
                        edit.moving = None;
                    }
                }
            }
            if let (Some((from, _)), Some(pointer)) = (self.drag, pointer) {
                let to = at(pointer);
                self.drag = Some((from, to));
                if response.drag_stopped() {
                    self.drag = None;
                    if from != to {
                        self.bezier = Some(BezierEdit {
                            points: straight_bezier(from, to),
                            moving: None,
                        });
                    }
                }
            }
            return;
        }
        if !self.tool.is_dragged() {
            if response.clicked() {
                if let Some(point) = pointer.map(at) {
                    self.add_shape(point, point);
                }
            }
            return;
        }
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(start) = origin.or(pointer).map(at) {
                self.drag = Some((start, start));
            }
        }
        if let (Some((from, _)), Some(pointer)) = (self.drag, pointer) {
            let to = at(pointer);
            self.drag = Some((from, to));
            if response.drag_stopped() {
                self.drag = None;
                self.add_shape(from, to);
            }
        } else if self.drag.is_some() && ui.input(|input| input.pointer.any_released()) {
            self.drag = None;
        }
        // Clicking places buttons at the style's size, or the icon's or clipboard's.
        if self.tool == Tool::Button && response.clicked() {
            if let Some(point) = pointer.map(at) {
                self.add_shape(point, point);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_palette_opens_only_for_the_chosen_color() {
        let context = egui::Context::default();
        let mut editor = RipEditor::new();
        let draw = egui::Id::new(("rip-color", "draw"));
        let border = egui::Id::new(("rip-color", "border"));
        let fill = egui::Id::new(("rip-color", "fill"));
        let frame = |editor: &mut RipEditor| {
            context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0))),
                    ..Default::default()
                },
                |context| editor.sidebar(context, false),
            )
        };
        frame(&mut editor);
        assert!(!egui::Popup::is_id_open(&context, draw));
        assert!(!egui::Popup::is_id_open(&context, border));
        assert!(!egui::Popup::is_id_open(&context, fill));

        egui::Popup::open_id(&context, border);
        frame(&mut editor);
        assert!(egui::Popup::is_id_open(&context, border));
        assert!(context.memory(|memory| memory.area_rect(border)).is_some());
        assert!(!egui::Popup::is_id_open(&context, draw));
        assert!(!egui::Popup::is_id_open(&context, fill));
    }

    #[test]
    fn drawing_tools_create_pixel_coordinate_commands() {
        assert_eq!(Tool::Pixel.command((12, 34), (12, 34), "").unwrap(), RipCommand::Pixel { x: 12, y: 34 });
        assert_eq!(
            Tool::Line.command((12, 34), (56, 78), "").unwrap(),
            RipCommand::Line {
                x0: 12,
                y0: 34,
                x1: 56,
                y1: 78
            }
        );
        assert_eq!(
            Tool::Rectangle.command((12, 34), (56, 78), "").unwrap(),
            RipCommand::Rectangle {
                x0: 12,
                y0: 34,
                x1: 56,
                y1: 78
            }
        );
        assert_eq!(
            Tool::Bar.command((12, 34), (56, 78), "").unwrap(),
            RipCommand::Bar {
                x0: 12,
                y0: 34,
                x1: 56,
                y1: 78
            }
        );
        assert_eq!(
            Tool::Circle.command((12, 34), (15, 38), "").unwrap(),
            RipCommand::Circle {
                x_center: 12,
                y_center: 34,
                radius: 5
            }
        );
        assert_eq!(
            Tool::FilledOval.command((12, 34), (56, 78), "").unwrap(),
            RipCommand::FilledOval {
                x: 12,
                y: 34,
                x_rad: 44,
                y_rad: 44
            }
        );
        assert_eq!(
            Tool::Oval.command((12, 34), (56, 78), "").unwrap(),
            RipCommand::Oval {
                x: 12,
                y: 34,
                st_ang: 0,
                end_ang: 360,
                x_rad: 44,
                y_rad: 44,
            }
        );
    }

    #[test]
    fn command_list_entries_are_short_and_localized() {
        let line = RipCommand::LineStyle {
            style: LineStyle::Solid,
            user_pat: 0,
            thick: 1,
        };
        assert_eq!(command_name(&line), fl!("rip-command-line-style"));
        assert_eq!(command_summary(&line), format!("{} · 1 px", fl!("rip-line-solid")));
        assert_eq!(command_swatch(&RipCommand::Color { c: 4 }), Some(4));
        let bezier = bezier_command([(1, 2), (3, 4), (5, 6), (7, 8)], 32);
        assert_eq!(command_name(&bezier), fl!("rip-editor-bezier"));
        assert!(!command_summary(&bezier).contains('{'));
        assert_eq!(command_name(&RipCommand::EraseWindow), "EraseWindow");
        for command in [line, bezier, RipCommand::Color { c: 4 }, RipCommand::MouseFields] {
            assert!(!command_name(&command).contains('{'));
            assert!(command_icon(&command).is_some());
        }
    }

    #[test]
    fn a_shape_selected_on_the_canvas_scrolls_into_the_command_list() {
        let context = egui::Context::default();
        let mut editor = RipEditor::new();
        editor.select_tool(Tool::Line);
        for step in 0..120 {
            editor.add_shape((step, 10), (step + 20, 40));
        }
        editor.select_shape(None);
        run(&context, &mut editor, vec![]);
        assert!(!editor.visible_rows.contains(&(editor.document.commands().len() - 1)) || editor.visible_rows.start == 0);
        let last = editor.document.commands().len() - 1;
        editor.select_shape(Some(last));
        run(&context, &mut editor, vec![]);
        run(&context, &mut editor, vec![]);
        assert!(editor.visible_rows.contains(&last), "{:?}", editor.visible_rows);
    }

    #[test]
    fn text_is_typed_on_the_canvas_and_edited_again() {
        let context = egui::Context::default();
        let mut editor = RipEditor::new();
        editor.select_tool(Tool::Text);
        editor.text_size = 2;
        run(&context, &mut editor, vec![]);
        let rect = editor.canvas_rect.unwrap();
        let scale = rect.width() / WIDTH as f32;
        let at = |(x, y): (u16, u16)| rect.min + egui::vec2((x as f32 + 0.5) * scale, (y as f32 + 0.5) * scale);
        let click = |editor: &mut RipEditor, point| {
            let pos = at(point);
            run(
                &context,
                editor,
                vec![egui::Event::PointerMoved(pos), button_event(pos, egui::PointerButton::Primary, true)],
            );
            run(&context, editor, vec![button_event(pos, egui::PointerButton::Primary, false)]);
        };
        let text = |value: &str| egui::Event::Text(value.into());

        click(&mut editor, (100, 60));
        assert_eq!(editor.text_edit.as_ref().map(|edit| edit.at), Some((100, 60)));
        run(&context, &mut editor, vec![text("Hallö!"), text("x")]);
        run(&context, &mut editor, vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
        assert_eq!(editor.text_edit.as_ref().unwrap().text, "Hall!", "unrepresentable characters are skipped");
        assert!(editor.document.commands().is_empty(), "the text is only previewed while typing");
        assert!(matches!(editor.shown_pending.last(), Some(RipCommand::TextXY { text, .. }) if text == "Hall!"));
        assert!(editor.modified() && editor.can_undo());
        run(&context, &mut editor, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
        assert!(editor.text_edit.is_none());
        assert!(matches!(
            editor.document.commands(),
            [RipCommand::Color { c: 15 }, RipCommand::FontStyle { size: 2, .. }, RipCommand::TextXY { x: 100, y: 60, text }] if text == "Hall!"
        ));
        assert!(editor.document.preview().unwrap().pixel_index(100, 60).is_some());

        // Clicking the text edits it in place; the preview replaces the original.
        click(&mut editor, (110, 65));
        assert_eq!(editor.text_edit.as_ref().and_then(|edit| edit.index), Some(2));
        run(&context, &mut editor, vec![text("o")]);
        assert!(editor
            .shown_editable
            .as_ref()
            .is_some_and(|editable| matches!(&editable[2], RipCommand::TextXY { text, .. } if text == "Hall!o")));
        // A click elsewhere finishes it and starts new text there.
        click(&mut editor, (300, 200));
        assert!(matches!(&editor.document.commands()[2], RipCommand::TextXY { text, .. } if text == "Hall!o"));
        assert_eq!(editor.document.commands().len(), 3);
        run(&context, &mut editor, vec![text("gone"), key(egui::Key::Escape, egui::Modifiers::NONE)]);
        assert!(editor.text_edit.is_none());
        assert_eq!(editor.document.commands().len(), 3, "Escape discards new text");

        // Clearing edited text removes it; undo brings it back.
        click(&mut editor, (104, 62));
        let backspaces = (0..6).map(|_| key(egui::Key::Backspace, egui::Modifiers::NONE)).collect();
        run(&context, &mut editor, backspaces);
        run(&context, &mut editor, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
        assert!(!editor.document.commands().iter().any(|command| matches!(command, RipCommand::TextXY { .. })));
        editor.undo(false);
        assert!(matches!(&editor.document.commands()[2], RipCommand::TextXY { text, .. } if text == "Hall!o"));

        // Double-clicking text with the select tool types into it.
        editor.select_tool(Tool::Select);
        // Earlier clicks must not make this a triple click (each frame is 1/60 s).
        for _ in 0..40 {
            run(&context, &mut editor, vec![]);
        }
        let pos = at((104, 62));
        run(
            &context,
            &mut editor,
            vec![egui::Event::PointerMoved(pos), button_event(pos, egui::PointerButton::Primary, true)],
        );
        run(&context, &mut editor, vec![button_event(pos, egui::PointerButton::Primary, false)]);
        run(&context, &mut editor, vec![button_event(pos, egui::PointerButton::Primary, true)]);
        run(&context, &mut editor, vec![button_event(pos, egui::PointerButton::Primary, false)]);
        assert_eq!(editor.tool, Tool::Text);
        assert_eq!(editor.text_edit.as_ref().and_then(|edit| edit.index), Some(2));
    }

    fn text(x: u16, value: &str) -> RipCommand {
        RipCommand::TextXY { x, y: 20, text: value.into() }
    }

    #[test]
    fn restyling_text_changes_its_own_font_and_keeps_later_text_unchanged() {
        let font = |size| font_command((1, 0, size));
        // The font command in front of the text is its own; the text after it keeps size 2.
        let commands = vec![
            font(2),
            RipCommand::Color { c: 4 },
            text(10, "A"),
            RipCommand::Line { x0: 0, y0: 0, x1: 9, y1: 9 },
            text(200, "B"),
        ];
        let (restyled, index) = restyle_text(&commands, 0, 2, text(10, "A"), (1, 0, 5), 4);
        assert_eq!(index, 2);
        assert_eq!(
            restyled,
            vec![
                font(5),
                RipCommand::Color { c: 4 },
                text(10, "A"),
                font(2),
                RipCommand::Line { x0: 0, y0: 0, x1: 9, y1: 9 },
                text(200, "B")
            ]
        );

        // Without its own commands, the new font and color are inserted and the old ones restored.
        let commands = vec![
            font(2),
            RipCommand::Color { c: 4 },
            text(10, "A"),
            RipCommand::Line { x0: 0, y0: 0, x1: 9, y1: 9 },
            text(200, "B"),
        ];
        let (restyled, index) = restyle_text(&commands, 0, 4, text(200, "B"), (3, 0, 1), 12);
        assert_eq!(index, 6);
        assert_eq!(
            restyled,
            vec![
                font(2),
                RipCommand::Color { c: 4 },
                text(10, "A"),
                RipCommand::Line { x0: 0, y0: 0, x1: 9, y1: 9 },
                font_command((3, 0, 1)),
                RipCommand::Color { c: 12 },
                text(200, "B"),
            ],
            "nothing follows, so nothing is restored"
        );

        // Text without any font command uses the scene's start font, which is restored after it.
        let commands = vec![text(10, "A"), text(200, "B")];
        let (restyled, index) = restyle_text(&commands, 0, 0, text(10, "A"), (0, 0, 1), DEFAULT_COLOR);
        assert_eq!(index, 1);
        assert_eq!(restyled, vec![font_command((0, 0, 1)), text(10, "A"), font_command(START_FONT), text(200, "B")]);
        let later = |commands: &[RipCommand]| {
            let mut document = RipDocument::new();
            document.try_append_many(commands.to_vec()).unwrap();
            let preview = document.preview().unwrap();
            (150..400)
                .flat_map(|x| (0..80).map(move |y| (x, y)))
                .map(|(x, y)| preview.pixel_index(x, y))
                .collect::<Vec<_>>()
        };
        assert_eq!(later(&commands), later(&restyled), "the later text is drawn as before");
    }

    #[test]
    fn clicking_text_picks_up_its_style_and_the_tool_settings_change_it() {
        let mut editor = RipEditor::new();
        editor.select_tool(Tool::Text);
        editor.text_font = 1;
        editor.text_size = 2;
        editor.draw_color = 4;
        for (x, value) in [(10, "First"), (10, "Second")] {
            editor.text_edit = Some(TextEdit {
                at: (x, if value == "First" { 20 } else { 120 }),
                text: value.into(),
                index: None,
            });
            editor.finish_text();
        }
        let first = editor
            .document
            .commands()
            .iter()
            .position(|command| matches!(command, RipCommand::TextXY { text, .. } if text == "First"))
            .unwrap();
        (editor.text_font, editor.text_size, editor.draw_color) = (0, 1, 15);
        editor.begin_text((12, 24));
        assert_eq!(editor.text_edit.as_ref().and_then(|edit| edit.index), Some(first));
        assert_eq!(
            (editor.text_font, editor.text_direction, editor.text_size, editor.draw_color),
            (1, 0, 2, 4),
            "the text's style is loaded"
        );

        editor.text_size = 3;
        editor.draw_color = 14;
        let context = egui::Context::default();
        editor.refresh_preview(&context);
        assert!(editor.shown_editable.is_some(), "the preview shows the new style before it is applied");
        editor.finish_text();
        let commands = editor.document.commands();
        let first = commands
            .iter()
            .position(|command| matches!(command, RipCommand::TextXY { text, .. } if text == "First"))
            .unwrap();
        assert_eq!(font_before(commands, first), (1, 0, 3));
        assert_eq!(color_before(commands, first), 14);
        let second = commands
            .iter()
            .position(|command| matches!(command, RipCommand::TextXY { text, .. } if text == "Second"))
            .unwrap();
        assert_eq!(
            (font_before(commands, second), color_before(commands, second)),
            ((1, 0, 2), 4),
            "later text keeps its style"
        );
        assert_eq!(editor.selected, Some(first));
        editor.undo(false);
        let commands = editor.document.commands();
        let first = commands
            .iter()
            .position(|command| matches!(command, RipCommand::TextXY { text, .. } if text == "First"))
            .unwrap();
        assert_eq!(font_before(commands, first), (1, 0, 2), "restyling is one undo step");

        // The property panel shows the font and color of selected text.
        editor.select_tool(Tool::Select);
        editor.select_shape(Some(first));
        run(&context, &mut editor, vec![]);
        assert_eq!(editor.editing_style, Some((first, (1, 0, 2), 4)));
    }

    #[test]
    fn mouse_regions_are_drawn_and_picked_only_with_their_tool() {
        let context = egui::Context::default();
        let mut editor = RipEditor::new();
        editor.select_tool(Tool::Bar);
        editor.add_shape((20, 20), (200, 120));
        editor.select_tool(Tool::Mouse);
        editor.mouse_host = "M^m".into();
        run(&context, &mut editor, vec![]);
        drag(&context, &mut editor, (40, 40), (140, 90), |editor| {
            assert_eq!(editor.drag, Some(((40, 40), (140, 90))), "the region is dragged out");
        });
        let region = editor.document.commands().len() - 1;
        assert_eq!(
            editor.document.commands()[region],
            RipCommand::Mouse {
                num: 0,
                x0: 40,
                y0: 40,
                x1: 140,
                y1: 90,
                clk: 1,
                clr: 0,
                res: 0,
                text: "M^m".into()
            }
        );
        assert_eq!(editor.selected_shape(), Some(region), "a new region is selected");
        let loaded = RipDocument::from_bytes(&editor.document.to_bytes().unwrap()).unwrap();
        assert_eq!(loaded.commands(), editor.document.commands());

        // Only regions are picked with the mouse region tool; the bar below is not.
        let bar = region - 1;
        assert_eq!(editor.shape_at((60.5, 60.5), 1.0), Some(region));
        assert_eq!(editor.shape_at((180.5, 110.5), 1.0), None);
        // Resizing works like any rectangle.
        drag(&context, &mut editor, (140, 90), (160, 100), |_| {});
        assert!(matches!(editor.document.commands()[region], RipCommand::Mouse { x1: 160, y1: 100, .. }));
        // Dragging inside a region moves it instead of adding one.
        let count = editor.document.commands().len();
        drag(&context, &mut editor, (60, 60), (70, 65), |_| {});
        assert_eq!(editor.document.commands().len(), count);
        assert!(matches!(editor.document.commands()[region], RipCommand::Mouse { x0: 50, y0: 45, .. }));
        // A plain click does not add a region.
        let pos = editor.canvas_rect.unwrap().min + egui::vec2(400.0, 300.0);
        run(
            &context,
            &mut editor,
            vec![egui::Event::PointerMoved(pos), button_event(pos, egui::PointerButton::Primary, true)],
        );
        run(&context, &mut editor, vec![button_event(pos, egui::PointerButton::Primary, false)]);
        assert_eq!(editor.document.commands().len(), count);

        // With the select tool, regions are hidden and the bar is picked instead.
        editor.select_tool(Tool::Select);
        assert_eq!(editor.shape_at((60.5, 60.5), 1.0), Some(bar));
        assert!(editor.shape(region).is_none());
        assert_eq!(command_name(&editor.document.commands()[region]), fl!("rip-editor-mouse"));
        assert_eq!(command_icon(&editor.document.commands()[region]), Some("rip_mouse"));
    }

    #[test]
    fn the_text_frame_covers_the_rendered_text() {
        for font in [(0, 0, 1), (0, 0, 3), (1, 0, 2), (3, 0, 4), (0, 1, 2), (2, 1, 3)] {
            let mut editor = RipEditor::new();
            (editor.text_font, editor.text_direction, editor.text_size) = font;
            editor.draw_color = 14;
            editor.text_edit = Some(TextEdit {
                at: (100, 100),
                text: "Hello".into(),
                index: None,
            });
            editor.finish_text();
            let preview = editor.document.preview().unwrap();
            let pixels: Vec<(i32, i32)> = (0..HEIGHT as usize)
                .flat_map(|y| (0..WIDTH as usize).map(move |x| (x, y)))
                .filter(|(x, y)| preview.pixel_index(*x, *y) == Some(14))
                .map(|(x, y)| (x as i32, y as i32))
                .collect();
            let (width, height) = text_extent(Some(font), "Hello");
            let bounds = (
                pixels.iter().map(|p| p.0).min().unwrap(),
                pixels.iter().map(|p| p.1).min().unwrap(),
                pixels.iter().map(|p| p.0).max().unwrap(),
                pixels.iter().map(|p| p.1).max().unwrap(),
            );
            let slack = 3 * i32::from(font.2);
            assert!(
                bounds.0 >= 100 - slack && bounds.1 >= 100 - slack && bounds.2 <= 100 + width + slack && bounds.3 <= 100 + height + slack,
                "{font:?}: drawn {bounds:?}, frame {width} × {height}"
            );
        }
    }

    #[test]
    fn new_shapes_round_trip_and_render_with_tool_colors() {
        for tool in [
            Tool::Arc,
            Tool::OvalArc,
            Tool::PieSlice,
            Tool::OvalPieSlice,
            Tool::Polygon,
            Tool::FilledPolygon,
            Tool::PolyLine,
        ] {
            let mut editor = RipEditor::new();
            editor.select_tool(tool);
            editor.draw_color = 3;
            editor.border_color = 4;
            editor.fill_color = 5;
            editor.start_angle = 10;
            editor.end_angle = 120;
            if tool.is_poly() {
                editor.poly = vec![(100, 100), (180, 100), (160, 180)];
                editor.finish_poly();
            } else {
                editor.add_shape((100, 100), (160, 140));
                assert!(matches!(
                    editor.document.commands().last(),
                    Some(
                        RipCommand::Arc { st_ang: 10, end_ang: 120, .. }
                            | RipCommand::OvalArc { st_ang: 10, end_ang: 120, .. }
                            | RipCommand::PieSlice { st_ang: 10, end_ang: 120, .. }
                            | RipCommand::OvalPieSlice { st_ang: 10, end_ang: 120, .. }
                    )
                ));
            }
            assert!(editor.error.is_none(), "{tool:?}: {:?}", editor.error);
            let commands = editor.document.commands();
            assert!(matches!(
                commands.last(),
                Some(
                    RipCommand::Arc { .. }
                        | RipCommand::OvalArc { .. }
                        | RipCommand::PieSlice { .. }
                        | RipCommand::OvalPieSlice { .. }
                        | RipCommand::Polygon { .. }
                        | RipCommand::FilledPolygon { .. }
                        | RipCommand::PolyLine { .. }
                )
            ));
            assert!(commands
                .iter()
                .any(|command| matches!(command, RipCommand::Color { c } if *c == if tool == Tool::PolyLine { 3 } else { 4 })));
            assert_eq!(
                commands.iter().any(|command| matches!(command, RipCommand::FillStyle { color: 5, .. })),
                tool.uses_fill()
            );
            let preview = editor.document.preview().unwrap();
            if tool == Tool::FilledPolygon {
                assert_eq!(preview.pixel_index(145, 130), Some(5));
            }
            if matches!(tool, Tool::PieSlice | Tool::OvalPieSlice) {
                assert_eq!(preview.pixel_index(120, 90), Some(5), "{tool:?}");
            }
            let context = egui::Context::default();
            let _ = context.run(egui::RawInput::default(), |context| {
                egui::CentralPanel::default().show(context, |ui| {
                    let mut draft = commands.last().unwrap().clone();
                    assert!(command_properties(ui, &mut draft), "{tool:?}");
                });
            });
            let loaded = RipDocument::from_bytes(&editor.document.to_bytes().unwrap()).unwrap();
            assert_eq!(loaded.commands(), commands, "{tool:?}");
            editor.undo(false);
            assert!(editor.document.commands().is_empty(), "{tool:?}");
        }
    }

    #[test]
    fn polygon_gesture_previews_finishes_and_cancels() {
        let context = egui::Context::default();
        let mut editor = RipEditor::new();
        editor.select_tool(Tool::FilledPolygon);
        run(&context, &mut editor, vec![]);
        let rect = editor.canvas_rect.unwrap();
        let scale = rect.width() / WIDTH as f32;
        let at = |(x, y): (u16, u16)| rect.min + egui::vec2((x as f32 + 0.5) * scale, (y as f32 + 0.5) * scale);
        let click = |editor: &mut RipEditor, point| {
            let pos = at(point);
            run(
                &context,
                editor,
                vec![egui::Event::PointerMoved(pos), button_event(pos, egui::PointerButton::Primary, true)],
            );
            run(&context, editor, vec![button_event(pos, egui::PointerButton::Primary, false)]);
        };
        click(&mut editor, (30, 30));
        click(&mut editor, (120, 30));
        click(&mut editor, (100, 100));
        run(&context, &mut editor, vec![egui::Event::PointerMoved(at((40, 110)))]);
        assert!(editor.modified());
        assert!(
            matches!(editor.shown_pending.last(), Some(RipCommand::FilledPolygon { points }) if points.len() == 8),
            "poly: {:?}, pending: {:?}",
            editor.poly,
            editor.shown_pending
        );
        run(&context, &mut editor, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
        assert!(matches!(editor.document.commands().last(), Some(RipCommand::FilledPolygon { points }) if points.len() == 6));
        click(&mut editor, (40, 40));
        run(&context, &mut editor, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
        assert!(editor.poly.is_empty());
        assert_eq!(
            editor
                .document
                .commands()
                .iter()
                .filter(|command| matches!(command, RipCommand::FilledPolygon { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn polygon_vertex_limit_fits_the_two_digit_rip_count() {
        let mut editor = RipEditor::new();
        editor.select_tool(Tool::PolyLine);
        editor.poly = (0..1295).map(|x| (x, 20)).collect();
        editor.finish_poly();
        assert!(editor.error.is_none());
        assert!(matches!(editor.document.commands().last(), Some(RipCommand::PolyLine { points }) if points.len() == 2590));
        assert_eq!(
            RipDocument::from_bytes(&editor.document.to_bytes().unwrap()).unwrap().commands(),
            editor.document.commands()
        );
    }

    #[test]
    fn adding_a_shape_sets_color_and_is_one_undo_step() {
        let mut editor = RipEditor::new();
        editor.tool = Tool::Bar;
        editor.draw_color = 4;
        editor.fill_color = 4;
        editor.add_shape((10, 20), (30, 40));
        assert_eq!(
            editor.document.commands(),
            &[
                RipCommand::FillStyle {
                    pattern: FillStyle::Solid,
                    color: 4
                },
                RipCommand::Bar {
                    x0: 10,
                    y0: 20,
                    x1: 30,
                    y1: 40
                }
            ]
        );
        assert_eq!(editor.document.preview().unwrap().pixel_index(20, 30), Some(4));
        editor.undo(false);
        assert!(editor.document.commands().is_empty());
        editor.undo(true);
        assert_eq!(editor.document.commands().len(), 2);
    }

    #[test]
    fn property_panel_supports_drawn_shapes_and_state_commands() {
        let context = egui::Context::default();
        let mut commands = vec![
            RipCommand::Pixel { x: 1, y: 2 },
            RipCommand::Line { x0: 1, y0: 2, x1: 3, y1: 4 },
            RipCommand::Rectangle { x0: 1, y0: 2, x1: 3, y1: 4 },
            RipCommand::Bar { x0: 1, y0: 2, x1: 3, y1: 4 },
            RipCommand::Circle {
                x_center: 1,
                y_center: 2,
                radius: 3,
            },
            RipCommand::FilledOval {
                x: 1,
                y: 2,
                x_rad: 3,
                y_rad: 4,
            },
            RipCommand::Oval {
                x: 1,
                y: 2,
                st_ang: 0,
                end_ang: 360,
                x_rad: 3,
                y_rad: 4,
            },
            RipCommand::TextXY { x: 1, y: 2, text: "Hi".into() },
            RipCommand::Bezier {
                x1: 1,
                y1: 2,
                x2: 3,
                y2: 4,
                x3: 5,
                y3: 6,
                x4: 7,
                y4: 8,
                cnt: 32,
            },
            RipCommand::Button {
                x0: 1,
                y0: 2,
                x1: 3,
                y1: 4,
                hotkey: 0,
                flags: 0,
                res: 0,
                text: "<>Hi".into(),
            },
            RipCommand::Color { c: 15 },
            RipCommand::FillStyle {
                pattern: FillStyle::Solid,
                color: 15,
            },
            RipCommand::Comment { text: "Read-only".into() },
        ];
        let supported = std::cell::RefCell::new(Vec::new());
        let _ = context.run(egui::RawInput::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| {
                *supported.borrow_mut() = commands.iter_mut().map(|command| command_properties(ui, command)).collect();
            });
        });
        assert_eq!(*supported.borrow(), vec![true; 12].into_iter().chain([false]).collect::<Vec<_>>());
    }

    #[test]
    fn tool_colors_and_new_commands_round_trip() {
        let mut editor = RipEditor::new();
        editor.draw_color = 14;
        editor.border_color = 12;
        editor.fill_color = 3;
        editor.tool = Tool::FilledOval;
        editor.add_shape((40, 40), (50, 50));
        assert!(matches!(
            editor.document.commands(),
            [
                RipCommand::Color { c: 12 },
                RipCommand::FillStyle { color: 3, .. },
                RipCommand::FilledOval { .. }
            ]
        ));
        assert_eq!(editor.document.preview().unwrap().pixel_index(40, 40), Some(3));

        editor.tool = Tool::Text;
        editor.text_edit = Some(TextEdit {
            at: (80, 80),
            text: "Hello".into(),
            index: None,
        });
        editor.finish_text();
        assert!(matches!(editor.document.commands().last(), Some(RipCommand::TextXY { text, .. }) if text == "Hello"));

        editor.select_tool(Tool::Bezier);
        editor.bezier = Some(BezierEdit {
            points: [(10, 10), (20, 20), (30, 5), (40, 10)],
            moving: None,
        });
        editor.finish_bezier();
        assert!(matches!(
            editor.document.commands().last(),
            Some(RipCommand::Bezier { cnt: 32, x2: 20, y2: 20, .. })
        ));

        editor.select_tool(Tool::Button);
        editor.button.label = "Hello".into();
        editor.add_shape((120, 100), (180, 130));
        assert!(matches!(editor.document.commands().last(), Some(RipCommand::Button { text, .. }) if text == "<>Hello<>"));
        assert!(editor.error.is_none(), "{:?}", editor.error);
        let preview = editor.document.preview().unwrap();
        assert!(mostly(&preview, (120, 100, 180, 130), editor.button.surface as u8), "the button face");
        editor.undo(false);
        assert!(matches!(editor.document.commands().last(), Some(RipCommand::Bezier { .. })));
    }

    #[test]
    fn invalid_button_label_does_not_change_document() {
        let mut editor = RipEditor::new();
        editor.tool = Tool::Button;
        editor.button.label = "first<>second".into();
        editor.add_shape((10, 10), (80, 30));
        assert!(editor.error.is_some());
        assert!(editor.document.commands().is_empty());
        assert!(!editor.document.is_dirty());
    }

    #[test]
    fn unrepresentable_command_is_rejected_before_undo() {
        let mut editor = RipEditor::new();
        editor.add_commands(vec![RipCommand::Pixel { x: 1296, y: 0 }]);
        assert!(editor.error.is_some());
        assert!(editor.document.commands().is_empty());
        assert!(!editor.can_undo());
    }

    #[test]
    fn dragging_on_the_preview_adds_a_replayable_line() {
        let context = egui::Context::default();
        let mut editor = RipEditor::new();
        let run = |editor: &mut RipEditor, events: Vec<egui::Event>| {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 820.0))),
                    events,
                    ..Default::default()
                },
                |context| editor.show(context, false),
            );
        };
        run(&mut editor, vec![]);
        let rect = editor.canvas_rect.unwrap();
        let from = rect.min + egui::vec2(20.0, 20.0);
        let to = rect.min + egui::vec2(120.0, 75.0);
        run(
            &mut editor,
            vec![
                egui::Event::PointerMoved(from),
                egui::Event::PointerButton {
                    pos: from,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        run(&mut editor, vec![egui::Event::PointerMoved(to)]);
        run(
            &mut editor,
            vec![egui::Event::PointerButton {
                pos: to,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(editor.document.is_dirty());
        assert!(matches!(
            editor.document.commands(),
            [RipCommand::Color { .. }, RipCommand::LineStyle { .. }, RipCommand::Line { .. }]
        ));
        let loaded = RipDocument::from_bytes(&editor.document.to_bytes().unwrap()).unwrap();
        assert_eq!(loaded.commands(), editor.document.commands());
    }

    /// Whether most pixels of the rectangle have `color`.
    fn mostly(preview: &icy_draw::rip_document::RipPreview, (x0, y0, x1, y1): (usize, usize, usize, usize), color: u8) -> bool {
        let pixels: Vec<_> = (y0..=y1).flat_map(|y| (x0..=x1).map(move |x| (x, y))).collect();
        pixels.iter().filter(|(x, y)| preview.pixel_index(*x, *y) == Some(color)).count() * 2 > pixels.len()
    }

    fn run(context: &egui::Context, editor: &mut RipEditor, events: Vec<egui::Event>) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 820.0))),
                events,
                ..Default::default()
            },
            |context| editor.show(context, false),
        );
    }

    fn button_event(pos: egui::Pos2, button: egui::PointerButton, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Drags from `from` to `to` in scene pixels, checking the preview between the steps.
    fn drag(context: &egui::Context, editor: &mut RipEditor, from: (u16, u16), to: (u16, u16), during: impl FnOnce(&mut RipEditor)) {
        let rect = editor.canvas_rect.unwrap();
        let scale = rect.width() / WIDTH as f32;
        let at = |point: (u16, u16)| rect.min + egui::vec2((point.0 as f32 + 0.5) * scale, (point.1 as f32 + 0.5) * scale);
        run(
            context,
            editor,
            vec![egui::Event::PointerMoved(at(from)), button_event(at(from), egui::PointerButton::Primary, true)],
        );
        let middle = ((from.0 + to.0) / 2, (from.1 + to.1) / 2);
        run(context, editor, vec![egui::Event::PointerMoved(at(middle))]);
        run(context, editor, vec![egui::Event::PointerMoved(at(to))]);
        during(editor);
        run(context, editor, vec![button_event(at(to), egui::PointerButton::Primary, false)]);
        run(context, editor, vec![]);
    }

    fn run_at(context: &egui::Context, editor: &mut RipEditor, time: f64, events: Vec<egui::Event>) -> egui::FullOutput {
        context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 820.0))),
                time: Some(time),
                events,
                ..Default::default()
            },
            |context| editor.show(context, false),
        )
    }

    fn lines(count: u16) -> RipEditor {
        let source: String = (0..count).map(|index| format!("!|L{}00{}1E\r\n", to_mega(index), to_mega(index))).collect();
        RipEditor::from_document(RipDocument::from_bytes(source.as_bytes()).unwrap())
    }

    fn to_mega(value: u16) -> String {
        const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        [value / 36, value % 36].iter().map(|digit| DIGITS[*digit as usize] as char).collect()
    }

    #[test]
    fn rip_animation_plays_by_transmission_time_and_follows_the_list() {
        let context = egui::Context::default();
        let mut editor = lines(150);
        run_at(&context, &mut editor, 0.0, vec![]);
        editor.transport_action(Action::PlayPause, 0.0);
        run_at(&context, &mut editor, 0.0, vec![]);
        assert_eq!((editor.transport.frame(None, 150), editor.selected), (Some(0), Some(0)));
        assert_eq!(editor.shown_frame, Some(0), "the canvas shows the drawing through the frame");
        // Each "|Lxxxxxxxx" takes 10 bytes at 1200 BPS.
        run_at(&context, &mut editor, 0.0085, vec![]);
        assert_eq!(editor.transport.frame(None, 150), Some(1));
        assert_eq!(editor.shown_frame, Some(1));
        assert_eq!(editor.selected, Some(1));
        editor.transport_action(Action::Seek(120), 0.01);
        run_at(&context, &mut editor, 0.01, vec![]);
        run_at(&context, &mut editor, 0.01, vec![]);
        assert_eq!(editor.selected, Some(120));
        assert!(editor.visible_rows.contains(&120), "{:?}", editor.visible_rows);
        editor.undo(false);
        assert!(!editor.transport.animating(), "undo stops the animation");
        run_at(&context, &mut editor, 0.02, vec![]);
        assert_eq!(editor.shown_frame, None, "the whole drawing is shown again");
    }

    #[test]
    fn rip_rows_are_double_clicked_to_run_the_animation_there() {
        let context = egui::Context::default();
        let mut editor = lines(40);
        let output = run_at(&context, &mut editor, 0.0, vec![]);
        let row = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "7" => Some(text.pos + text.galley.size() / 2.0 + egui::vec2(60.0, 0.0)),
                _ => None,
            })
            .expect("row 7 is listed");
        for (time, pressed) in [(1.0, true), (1.05, false), (1.1, true), (1.15, false)] {
            run_at(
                &context,
                &mut editor,
                time,
                vec![egui::Event::PointerMoved(row), button_event(row, egui::PointerButton::Primary, pressed)],
            );
        }
        run_at(&context, &mut editor, 1.2, vec![]);
        assert_eq!(editor.transport.playhead, Some(6));
        assert_eq!(editor.selected, Some(6));
        assert_eq!(editor.shown_frame, Some(6));
        // The eye turns the paused animation into an editable preview through its frame.
        editor.toggle_preview();
        assert!(!editor.transport.animating() && editor.preview_to_selection);
        assert_eq!(editor.selected, Some(6));
        editor.transport_action(Action::Next, 1.3);
        assert_eq!((editor.selected, editor.transport.animating()), (Some(7), false));
        editor.transport_action(Action::PlayPause, 1.3);
        assert_eq!(editor.transport.run.as_ref().unwrap().index, 8, "play continues after the preview");
        editor.transport_action(Action::Stop, 1.3);
        assert!(!editor.preview_to_selection && !editor.transport.animating());
    }

    #[test]
    fn bezier_curves_are_dragged_then_adjusted_with_handles() {
        let context = egui::Context::default();
        let mut editor = RipEditor::new();
        editor.select_tool(Tool::Bezier);
        run(&context, &mut editor, vec![]);
        drag(&context, &mut editor, (30, 60), (330, 150), |_| {});
        assert!(editor.document.commands().is_empty(), "the curve stays adjustable");
        assert_eq!(editor.bezier.unwrap().points, [(30, 60), (130, 90), (230, 120), (330, 150)]);
        // Dragging a control point bends the curve; the preview shows it before it is added.
        drag(&context, &mut editor, (130, 90), (120, 20), |editor| {
            assert!(matches!(editor.shown_pending.last(), Some(RipCommand::Bezier { x2: 120, y2: 20, .. })));
        });
        assert_eq!(editor.bezier.unwrap().points[1], (120, 20));
        assert!(editor.modified(), "an unfinished curve counts as a change");
        let rect = editor.canvas_rect.unwrap();
        let point = rect.center();
        run(
            &context,
            &mut editor,
            vec![egui::Event::PointerMoved(point), button_event(point, egui::PointerButton::Secondary, true)],
        );
        run(&context, &mut editor, vec![button_event(point, egui::PointerButton::Secondary, false)]);
        assert!(editor.bezier.is_none());
        assert!(matches!(
            editor.document.commands().last(),
            Some(RipCommand::Bezier {
                x1: 30,
                y1: 60,
                x2: 120,
                y2: 20,
                x3: 230,
                y3: 120,
                x4: 330,
                y4: 150,
                ..
            })
        ));

        // Escape drops an unfinished curve.
        let count = editor.document.commands().len();
        drag(&context, &mut editor, (10, 10), (50, 50), |_| {});
        assert!(editor.bezier.is_some());
        run(
            &context,
            &mut editor,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(editor.bezier.is_none());
        assert_eq!(editor.document.commands().len(), count);
    }

    #[test]
    fn the_bezier_tool_never_panics_on_a_plain_command() {
        assert!(matches!(
            Tool::Bezier.command((0, 0), (30, 30), "").unwrap(),
            RipCommand::Bezier { x2: 10, y2: 10, .. }
        ));
    }

    #[test]
    fn shapes_preview_live_and_only_repeat_changed_styles() {
        let context = egui::Context::default();
        let mut editor = RipEditor::new();
        editor.select_tool(Tool::Rectangle);
        editor.line_style = LineStyle::Dashed;
        editor.line_thickness = 3;
        editor.border_color = 12;
        run(&context, &mut editor, vec![]);
        drag(&context, &mut editor, (40, 40), (140, 90), |editor| {
            assert_eq!(describe(editor.shown_pending.last().unwrap()).as_deref(), Some("40, 40 · 101 × 51"));
            let preview = editor.document.preview_with(&editor.shown_pending).unwrap();
            let edge = (40..=140).filter(|x| preview.pixel_index(*x, 40) == Some(12)).count();
            assert!((30..100).contains(&edge), "the drag shows the real dashed RIP line ({edge} pixels)");
            assert!(editor.document.commands().is_empty());
        });
        assert_eq!(
            editor.document.commands(),
            &[
                RipCommand::Color { c: 12 },
                RipCommand::LineStyle {
                    style: LineStyle::Dashed,
                    user_pat: 0,
                    thick: 3
                },
                RipCommand::Rectangle {
                    x0: 40,
                    y0: 40,
                    x1: 140,
                    y1: 90
                }
            ]
        );
        drag(&context, &mut editor, (200, 40), (300, 90), |_| {});
        assert!(
            matches!(editor.document.commands()[3..], [RipCommand::Rectangle { .. }]),
            "unchanged styles are not repeated: {:?}",
            editor.document.commands()
        );
        editor.line_style = LineStyle::Solid;
        drag(&context, &mut editor, (200, 140), (300, 190), |_| {});
        assert!(matches!(
            editor.document.commands()[4..],
            [RipCommand::LineStyle { style: LineStyle::Solid, .. }, RipCommand::Rectangle { .. }]
        ));
    }

    #[test]
    fn buttons_are_placed_with_their_style_and_edited_as_one_object() {
        let mut editor = RipEditor::new();
        editor.select_tool(Tool::Button);
        editor.button.label = "OK".into();
        editor.button.host_command = "^mOK^m".into();
        editor.add_shape((100, 100), (100, 100));
        let commands = editor.document.commands().to_vec();
        assert!(
            matches!(
                commands.as_slice(),
                [
                    RipCommand::FontStyle { .. },
                    RipCommand::ButtonStyle { wid: 80, hgt: 30, .. },
                    RipCommand::Button {
                        x0: 100,
                        y0: 100,
                        x1: 0,
                        y1: 0,
                        ..
                    }
                ]
            ),
            "{commands:?}"
        );
        let preview = editor.document.preview().unwrap();
        assert!(
            mostly(&preview, (100, 100, 179, 129), editor.button.surface as u8),
            "a click uses the style size"
        );
        assert!(!mostly(&preview, (185, 100, 200, 129), editor.button.surface as u8));

        // Selecting the style selects the button, whose dialog edits both commands at once.
        let target = ButtonTarget::owning(&commands, 1).unwrap();
        editor.open_button_dialog(Some(target));
        let mut options = editor.button_dialog.take().unwrap().options;
        assert_eq!(options.label, "OK");
        assert_eq!(options.host_command, "^mOK^m");
        options.label = "Cancel".into();
        options.surface = 1;
        options.kind = ButtonKind::Clipboard;
        editor.apply_button(Some(target), options);
        let edited = editor.document.commands();
        assert!(matches!(&edited[2], RipCommand::Button { text, x0: 100, .. } if text == "<>Cancel<>^mOK^m"));
        assert!(matches!(edited[1], RipCommand::ButtonStyle { surface: 1, flags, .. } if flags & button::CLIPBOARD != 0 && flags & button::PLAIN == 0));
        editor.undo(false);
        assert_eq!(editor.document.commands(), commands.as_slice(), "the edit is one undo step");

        // A second button with the same style reuses it.
        editor.select_tool(Tool::Button);
        editor.button.label = "Next".into();
        editor.add_shape((200, 100), (260, 130));
        assert_eq!(editor.tool, Tool::Select, "a placed button is selected for editing");
        assert_eq!(editor.selected_shape(), Some(3));
        assert!(matches!(editor.document.commands()[3..], [RipCommand::Button { .. }]));
    }

    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn selected_shapes_are_resized_with_handles_and_moved() {
        let context = egui::Context::default();
        let mut editor = RipEditor::new();
        editor.select_tool(Tool::Rectangle);
        editor.add_shape((100, 100), (200, 150));
        editor.select_tool(Tool::Line);
        editor.add_shape((10, 10), (60, 60));
        editor.select_tool(Tool::Select);
        editor.select_shape(None);
        run(&context, &mut editor, vec![]);

        // Dragging a corner handle of the selected rectangle resizes it; the preview shows it live.
        let rect = editor
            .document
            .commands()
            .iter()
            .position(|command| matches!(command, RipCommand::Rectangle { .. }))
            .unwrap();
        editor.select_shape(Some(rect));
        let undo_steps = |editor: &RipEditor| editor.document.can_undo();
        drag(&context, &mut editor, (200, 150), (260, 190), |editor| {
            assert_eq!(
                editor.shown_replacement,
                Some((
                    rect,
                    RipCommand::Rectangle {
                        x0: 100,
                        y0: 100,
                        x1: 260,
                        y1: 190
                    }
                ))
            );
            assert!(
                matches!(editor.document.commands()[rect], RipCommand::Rectangle { x1: 200, .. }),
                "the document changes on release"
            );
        });
        assert_eq!(
            editor.document.commands()[rect],
            RipCommand::Rectangle {
                x0: 100,
                y0: 100,
                x1: 260,
                y1: 190
            }
        );
        assert!(undo_steps(&editor));

        // Dragging the outline moves it; dragging the line selects and moves the line instead.
        drag(&context, &mut editor, (100, 130), (110, 140), |_| {});
        assert_eq!(
            editor.document.commands()[rect],
            RipCommand::Rectangle {
                x0: 110,
                y0: 110,
                x1: 270,
                y1: 200
            }
        );
        drag(&context, &mut editor, (35, 35), (45, 30), |_| {});
        assert!(matches!(
            editor.document.commands().last(),
            Some(RipCommand::Line { x0: 20, y0: 5, x1: 70, y1: 55 })
        ));
        assert_eq!(editor.selected_shape(), Some(editor.document.commands().len() - 1));

        // Keys nudge and delete the selection, Escape deselects.
        run(&context, &mut editor, vec![key(egui::Key::ArrowRight, egui::Modifiers::SHIFT)]);
        assert!(matches!(editor.document.commands().last(), Some(RipCommand::Line { x0: 30, x1: 80, .. })));
        editor.undo(false);
        assert!(
            matches!(editor.document.commands().last(), Some(RipCommand::Line { x0: 20, .. })),
            "each change is one undo step"
        );
        editor.select_shape(Some(editor.document.commands().len() - 1));
        let count = editor.document.commands().len();
        run(&context, &mut editor, vec![key(egui::Key::Delete, egui::Modifiers::NONE)]);
        assert_eq!(editor.document.commands().len(), count - 1);
        assert!(editor.selected.is_none());

        // Clicking empty space deselects, clicking a shape selects the topmost one.
        let rect_area = editor.canvas_rect.unwrap();
        let scale = rect_area.width() / WIDTH as f32;
        let at = |point: (u16, u16)| rect_area.min + egui::vec2((point.0 as f32 + 0.5) * scale, (point.1 as f32 + 0.5) * scale);
        for (point, expected) in [((110, 150), Some(rect)), ((400, 300), None)] {
            run(
                &context,
                &mut editor,
                vec![
                    egui::Event::PointerMoved(at(point)),
                    button_event(at(point), egui::PointerButton::Primary, true),
                ],
            );
            run(&context, &mut editor, vec![button_event(at(point), egui::PointerButton::Primary, false)]);
            assert_eq!(editor.selected_shape(), expected);
        }
    }

    #[test]
    fn create_button_opens_the_dialog_then_places_and_selects_the_button() {
        let context = egui::Context::default();
        let mut editor = RipEditor::new();
        editor.select_tool(Tool::Select);
        run(&context, &mut editor, vec![]);
        editor.open_button_dialog(None);
        editor.button_dialog.as_mut().unwrap().create = true;
        editor.button_dialog.as_mut().unwrap().options.label = "Go".into();
        // Confirming the dialog with Enter would place it; here the dialog result is applied directly.
        let options = editor.button_dialog.take().unwrap().options;
        editor.apply_button(None, options);
        editor.select_tool(Tool::Button);
        run(&context, &mut editor, vec![]);
        let rect = editor.canvas_rect.unwrap();
        let scale = rect.width() / WIDTH as f32;
        let point = rect.min + egui::vec2(50.5 * scale, 60.5 * scale);
        run(
            &context,
            &mut editor,
            vec![egui::Event::PointerMoved(point), button_event(point, egui::PointerButton::Primary, true)],
        );
        run(&context, &mut editor, vec![button_event(point, egui::PointerButton::Primary, false)]);
        assert!(matches!(editor.document.commands().last(), Some(RipCommand::Button { x0: 50, y0: 60, x1: 0, y1: 0, text, .. }) if text == "<>Go<>"));
        assert_eq!(editor.tool, Tool::Select);
        let button = editor.document.commands().len() - 1;
        assert_eq!(editor.selected_shape(), Some(button));
        // Its frame handles resize it to an explicit rectangle.
        drag(&context, &mut editor, (129, 89), (150, 100), |_| {});
        assert!(matches!(
            editor.document.commands()[button],
            RipCommand::Button {
                x0: 50,
                y0: 60,
                x1: 150,
                y1: 100,
                ..
            }
        ));

        // Escape while placing returns to selection without adding anything.
        editor.select_tool(Tool::Button);
        run(&context, &mut editor, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
        assert_eq!(editor.tool, Tool::Select);
    }

    #[test]
    fn the_palette_dialog_previews_live_and_writes_one_command() {
        let mut editor = RipEditor::new();
        editor.select_tool(Tool::Bar);
        editor.fill_color = 4;
        editor.add_shape((10, 10), (50, 50));
        let rgb = |editor: &RipEditor, preview: &icy_draw::rip_document::RipPreview| {
            let _ = editor;
            preview.pixel_rgba(20, 20).unwrap()
        };
        let before = rgb(&editor, &editor.document.preview().unwrap());
        assert_eq!(before, [0xAA, 0, 0, 255]);

        // Remapping slot 4 recolors the bar in the preview before anything is written.
        editor.open_palette_dialog(None);
        editor.palette_dialog.as_mut().unwrap().values[4] = 9;
        let pending = editor.pending_commands();
        assert_eq!(pending, vec![RipCommand::OnePalette { color: 4, value: 9 }]);
        let preview = editor.document.preview_with(&pending).unwrap();
        assert_eq!(rgb(&editor, &preview), [0, 0, 0xFF, 255], "EGA 9 is bright blue");
        let count = editor.document.commands().len();
        let dialog = editor.palette_dialog.take().unwrap();
        editor.apply_palette(dialog);
        assert_eq!(editor.document.commands().len(), count + 1);
        assert_eq!(rgb(&editor, &editor.document.preview().unwrap()), [0, 0, 0xFF, 255]);

        // Several slots become one |Q; the dialog starts from the palette in effect.
        editor.open_palette_dialog(None);
        let dialog = editor.palette_dialog.as_mut().unwrap();
        assert_eq!(dialog.values[4], 9);
        dialog.values[1] = 36;
        dialog.values[2] = 18;
        let dialog = editor.palette_dialog.take().unwrap();
        editor.apply_palette(dialog);
        let set = editor.document.commands().len() - 1;
        assert!(matches!(&editor.document.commands()[set], RipCommand::SetPalette { colors } if colors[1] == 36 && colors[4] == 9));

        // A |Q command is edited in place, as one undo step.
        editor.open_palette_dialog(Some(set));
        editor.palette_dialog.as_mut().unwrap().values[1] = 1;
        let dialog = editor.palette_dialog.take().unwrap();
        editor.apply_palette(dialog);
        assert_eq!(editor.document.commands().len(), set + 1);
        assert!(matches!(&editor.document.commands()[set], RipCommand::SetPalette { colors } if colors[1] == 1));
        editor.undo(false);
        assert!(matches!(&editor.document.commands()[set], RipCommand::SetPalette { colors } if colors[1] == 36));

        // An unchanged palette writes nothing.
        editor.open_palette_dialog(None);
        let dialog = editor.palette_dialog.take().unwrap();
        let count = editor.document.commands().len();
        editor.apply_palette(dialog);
        assert_eq!(editor.document.commands().len(), count);
    }

    #[test]
    fn property_edits_apply_without_a_confirm_button() {
        let context = egui::Context::default();
        let mut editor = RipEditor::new();
        editor.select_tool(Tool::Rectangle);
        editor.add_shape((10, 10), (60, 40));
        let rect = editor.document.commands().len() - 1;
        editor.select_tool(Tool::Select);
        editor.select_shape(Some(rect));
        run(&context, &mut editor, vec![]);
        assert!(matches!(editor.editing, Some((index, _)) if index == rect));

        // While a value is dragged the preview shows it, the document changes on release.
        if let Some((_, RipCommand::Rectangle { x1, .. })) = &mut editor.editing {
            *x1 = 120;
        }
        let panel = egui::pos2(1200.0, 500.0);
        run(
            &context,
            &mut editor,
            vec![egui::Event::PointerMoved(panel), button_event(panel, egui::PointerButton::Primary, true)],
        );
        assert!(matches!(editor.document.commands()[rect], RipCommand::Rectangle { x1: 60, .. }));
        assert!(matches!(&editor.shown_replacement, Some((index, RipCommand::Rectangle { x1: 120, .. })) if *index == rect));
        run(&context, &mut editor, vec![button_event(panel, egui::PointerButton::Primary, false)]);
        assert!(matches!(editor.document.commands()[rect], RipCommand::Rectangle { x1: 120, .. }));
        editor.undo(false);
        assert!(
            matches!(editor.document.commands()[rect], RipCommand::Rectangle { x1: 60, .. }),
            "one change is one undo step"
        );

        // A pending edit is kept when another command is chosen.
        editor.select_shape(Some(rect));
        run(&context, &mut editor, vec![]);
        if let Some((_, RipCommand::Rectangle { y1, .. })) = &mut editor.editing {
            *y1 = 90;
        }
        editor.commit_properties();
        assert!(matches!(editor.document.commands()[rect], RipCommand::Rectangle { y1: 90, .. }));
    }
}
