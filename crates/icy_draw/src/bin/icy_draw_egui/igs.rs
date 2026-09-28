//! The IGS (Atari ST Instant Graphics and Sound) editor: a command list, a property panel and
//! a canvas rendered with the IGS engine, with drawing tools for the VDI shapes.

use eframe::egui::{self, Color32, Stroke};
use icy_draw::{
    fl,
    igs_document::{IgsDocument, IgsDrawState},
};
use icy_engine::Screen;
use icy_parser_core::{
    DrawingMode, IgsCommand, IgsItem, IgsParameter, LineKind, LineMarkerStyle, PatternType, PenType, PolymarkerKind, TerminalResolution, TextEffects,
    TextRotation,
};
use std::path::Path;

use super::widgets::{self, Icons};

#[path = "igs_palette.rs"]
mod palette;
use palette::{PaletteDialog, PaletteResult};
#[path = "igs_properties.rs"]
mod properties;
#[path = "igs_select.rs"]
mod select;
use select::{Canvas, Geometry, Handle, Point};

const TOOLBAR_HEIGHT: f32 = 44.0;
/// Screen distance in points within which a handle is picked up.
const HANDLE_RADIUS: f32 = 8.0;
const COMMAND_ROW_HEIGHT: f32 = 24.0;
/// VDI polylines and polygons take at most 128 points.
const MAX_POINTS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Select,
    Marker,
    Line,
    PolyLine,
    Rectangle,
    RoundedRectangle,
    FilledRectangle,
    Circle,
    Ellipse,
    Arc,
    EllipticalArc,
    PieSlice,
    EllipticalPieSlice,
    Polygon,
    FloodFill,
    Text,
}

impl Tool {
    const ALL: [Self; 16] = [
        Self::Select,
        Self::Marker,
        Self::Line,
        Self::PolyLine,
        Self::Rectangle,
        Self::RoundedRectangle,
        Self::FilledRectangle,
        Self::Circle,
        Self::Ellipse,
        Self::Arc,
        Self::EllipticalArc,
        Self::PieSlice,
        Self::EllipticalPieSlice,
        Self::Polygon,
        Self::FloodFill,
        Self::Text,
    ];

    fn label(self) -> String {
        match self {
            Self::Select => fl!("igs-tool-select"),
            Self::Marker => fl!("igs-tool-marker"),
            Self::Line => fl!("igs-tool-line"),
            Self::PolyLine => fl!("igs-tool-polyline"),
            Self::Rectangle => fl!("igs-tool-rectangle"),
            Self::RoundedRectangle => fl!("igs-tool-rounded-rectangle"),
            Self::FilledRectangle => fl!("igs-tool-filled-rectangle"),
            Self::Circle => fl!("igs-tool-circle"),
            Self::Ellipse => fl!("igs-tool-ellipse"),
            Self::Arc => fl!("igs-tool-arc"),
            Self::EllipticalArc => fl!("igs-tool-elliptical-arc"),
            Self::PieSlice => fl!("igs-tool-pie-slice"),
            Self::EllipticalPieSlice => fl!("igs-tool-elliptical-pie-slice"),
            Self::Polygon => fl!("igs-tool-polygon"),
            Self::FloodFill => fl!("igs-tool-flood-fill"),
            Self::Text => fl!("igs-tool-text"),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Select => "cursor",
            Self::Marker => "add",
            Self::Line => "line",
            Self::PolyLine => "rip_polyline",
            Self::Rectangle | Self::RoundedRectangle => "rectangle_outline",
            Self::FilledRectangle => "rectangle_filled",
            Self::Circle | Self::Ellipse => "ellipse_filled",
            Self::Arc => "rip_arc",
            Self::EllipticalArc => "rip_oval_arc",
            Self::PieSlice => "rip_pie",
            Self::EllipticalPieSlice => "rip_oval_pie",
            Self::Polygon => "rip_polygon_filled",
            Self::FloodFill => "fill",
            Self::Text => "text",
        }
    }

    /// The pen the shape is drawn with.
    fn pen(self) -> Option<PenType> {
        Some(match self {
            Self::Select => return None,
            Self::Marker => PenType::Polymarker,
            Self::Line | Self::PolyLine | Self::Arc | Self::EllipticalArc => PenType::Line,
            Self::Text => PenType::Text,
            _ => PenType::Fill,
        })
    }

    fn uses_line_style(self) -> bool {
        self.pen() == Some(PenType::Line)
    }

    fn uses_fill(self) -> bool {
        self.pen() == Some(PenType::Fill)
    }

    fn has_angles(self) -> bool {
        matches!(self, Self::Arc | Self::EllipticalArc | Self::PieSlice | Self::EllipticalPieSlice)
    }

    fn is_poly(self) -> bool {
        matches!(self, Self::PolyLine | Self::Polygon)
    }

    /// Whether the shape needs a drag rather than a click.
    fn is_dragged(self) -> bool {
        !matches!(
            self,
            Self::Select | Self::Marker | Self::FloodFill | Self::Text | Self::PolyLine | Self::Polygon
        )
    }

    /// The shape command from `from` to `to`, without the attribute commands before it.
    fn command(self, canvas: &Canvas, from: Point, to: Point, (start_angle, end_angle): (i32, i32)) -> Option<IgsCommand> {
        let v = IgsParameter::Value;
        let ((x0, y0), (x1, y1)) = (from, to);
        let (left, top, right, bottom) = (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
        let (dx, dy) = (x1 - x0, y1 - y0);
        let radius = (dx as f32).hypot(dy as f32).round() as i32;
        Some(match self {
            Self::Select | Self::Text | Self::PolyLine | Self::Polygon => return None,
            Self::Marker => IgsCommand::PolymarkerPlot { x: v(x1), y: v(y1) },
            Self::FloodFill => IgsCommand::FloodFill { x: v(x1), y: v(y1) },
            Self::Line => IgsCommand::Line {
                x1: v(x0),
                y1: v(y0),
                x2: v(x1),
                y2: v(y1),
            },
            Self::Rectangle | Self::RoundedRectangle => IgsCommand::Box {
                x1: v(left),
                y1: v(top),
                x2: v(right),
                y2: v(bottom),
                rounded: self == Self::RoundedRectangle,
            },
            Self::FilledRectangle => IgsCommand::FilledRectangle {
                x1: v(left),
                y1: v(top),
                x2: v(right),
                y2: v(bottom),
            },
            Self::Circle => IgsCommand::Circle {
                x: v(x0),
                y: v(y0),
                radius: v(canvas.circle_radius(dx, dy)),
            },
            Self::Ellipse => IgsCommand::Ellipse {
                x: v(x0),
                y: v(y0),
                x_radius: v(dx.abs()),
                y_radius: v(dy.abs()),
            },
            Self::Arc => IgsCommand::Arc {
                x: v(x0),
                y: v(y0),
                radius: v(radius),
                start_angle: v(start_angle),
                end_angle: v(end_angle),
            },
            Self::PieSlice => IgsCommand::PieSlice {
                x: v(x0),
                y: v(y0),
                radius: v(canvas.circle_radius(dx, dy)),
                start_angle: v(start_angle),
                end_angle: v(end_angle),
            },
            Self::EllipticalArc => IgsCommand::EllipticalArc {
                x: v(x0),
                y: v(y0),
                x_radius: v(dx.abs()),
                y_radius: v(dy.abs()),
                start_angle: v(start_angle),
                end_angle: v(end_angle),
            },
            Self::EllipticalPieSlice => IgsCommand::EllipticalPieSlice {
                x: v(x0),
                y: v(y0),
                x_radius: v(dx.abs()),
                y_radius: v(canvas.circle_y_parameter(dy.abs())),
                start_angle: v(start_angle),
                end_angle: v(end_angle),
            },
        })
    }
}

fn poly_command(tool: Tool, points: &[Point]) -> IgsCommand {
    let points = points.iter().flat_map(|(x, y)| [IgsParameter::Value(*x), IgsParameter::Value(*y)]).collect();
    if tool == Tool::PolyLine {
        IgsCommand::PolyLine { points }
    } else {
        IgsCommand::PolyFill { points }
    }
}

/// The tool that draws `command`.
fn shape_tool(command: &IgsCommand) -> Option<Tool> {
    Some(match command {
        IgsCommand::PolymarkerPlot { .. } => Tool::Marker,
        IgsCommand::Line { .. } => Tool::Line,
        IgsCommand::PolyLine { .. } => Tool::PolyLine,
        IgsCommand::Box { rounded: false, .. } => Tool::Rectangle,
        IgsCommand::Box { rounded: true, .. } | IgsCommand::RoundedRectangles { .. } => Tool::RoundedRectangle,
        IgsCommand::FilledRectangle { .. } => Tool::FilledRectangle,
        IgsCommand::Circle { .. } => Tool::Circle,
        IgsCommand::Ellipse { .. } => Tool::Ellipse,
        IgsCommand::Arc { .. } => Tool::Arc,
        IgsCommand::EllipticalArc { .. } => Tool::EllipticalArc,
        IgsCommand::PieSlice { .. } => Tool::PieSlice,
        IgsCommand::EllipticalPieSlice { .. } => Tool::EllipticalPieSlice,
        IgsCommand::PolyFill { .. } => Tool::Polygon,
        IgsCommand::FloodFill { .. } => Tool::FloodFill,
        IgsCommand::WriteText { .. } => Tool::Text,
        _ => return None,
    })
}

/// Bytes as Latin-1 text, the way the Atari ST character set maps printable characters.
fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&byte| char::from(byte)).collect()
}

fn describe(command: &IgsCommand) -> Option<String> {
    Some(match command {
        IgsCommand::PolymarkerPlot { x, y }
        | IgsCommand::FloodFill { x, y }
        | IgsCommand::LineDrawTo { x, y }
        | IgsCommand::SetDrawtoBegin { x, y }
        | IgsCommand::PositionCursor { x, y }
        | IgsCommand::WriteText { x, y, .. } => format!("{x}, {y}"),
        IgsCommand::Line { x1, y1, x2, y2 }
        | IgsCommand::Box { x1, y1, x2, y2, .. }
        | IgsCommand::FilledRectangle { x1, y1, x2, y2 }
        | IgsCommand::RoundedRectangles { x1, y1, x2, y2, .. } => format!("{x1}, {y1} → {x2}, {y2}"),
        IgsCommand::Circle { x, y, radius } | IgsCommand::Arc { x, y, radius, .. } | IgsCommand::PieSlice { x, y, radius, .. } => {
            format!("{x}, {y} · r {radius}")
        }
        IgsCommand::Ellipse { x, y, x_radius, y_radius }
        | IgsCommand::EllipticalArc { x, y, x_radius, y_radius, .. }
        | IgsCommand::EllipticalPieSlice { x, y, x_radius, y_radius, .. } => format!("{x}, {y} · {x_radius} × {y_radius}"),
        IgsCommand::PolyLine { points } | IgsCommand::PolyFill { points } => fl!("igs-editor-vertices", count = ((points.len() / 2) as u32)),
        _ => return None,
    })
}

fn pen_type_name(pen: PenType) -> String {
    match pen {
        PenType::Polymarker => fl!("igs-pen-marker"),
        PenType::Line => fl!("igs-pen-line"),
        PenType::Fill => fl!("igs-pen-fill"),
        PenType::Text => fl!("igs-pen-text"),
    }
}

fn pattern_name(pattern: PatternType) -> String {
    match pattern {
        PatternType::Hollow => fl!("igs-fill-hollow"),
        PatternType::Solid => fl!("igs-fill-solid"),
        PatternType::Pattern(index) => fl!("igs-fill-pattern", index = index),
        PatternType::Hatch(index) => fl!("igs-fill-hatch", index = index),
        PatternType::UserDefined(index) => fl!("igs-fill-user", index = index),
        PatternType::Random => fl!("igs-fill-random"),
        PatternType::StarTrek => "Star Trek".into(),
    }
}

fn line_kind_name(kind: LineKind) -> String {
    match kind {
        LineKind::Solid => fl!("igs-line-solid"),
        LineKind::LongDash => fl!("igs-line-long-dash"),
        LineKind::Dotted => fl!("igs-line-dotted"),
        LineKind::DashDot => fl!("igs-line-dash-dot"),
        LineKind::Dashed => fl!("igs-line-dashed"),
        LineKind::DashDotDot => fl!("igs-line-dash-dot-dot"),
        LineKind::UserDefined => fl!("igs-line-user"),
    }
}

fn marker_name(kind: PolymarkerKind) -> String {
    match kind {
        PolymarkerKind::Point => fl!("igs-marker-point"),
        PolymarkerKind::Plus => fl!("igs-marker-plus"),
        PolymarkerKind::Star => fl!("igs-marker-star"),
        PolymarkerKind::Square => fl!("igs-marker-square"),
        PolymarkerKind::DiagonalCross => fl!("igs-marker-cross"),
        PolymarkerKind::Diamond => fl!("igs-marker-diamond"),
    }
}

fn drawing_mode_name(mode: DrawingMode) -> String {
    match mode {
        DrawingMode::Replace => fl!("igs-mode-replace"),
        DrawingMode::Transparent => fl!("igs-mode-transparent"),
        DrawingMode::Xor => fl!("igs-mode-xor"),
        DrawingMode::ReverseTransparent => fl!("igs-mode-reverse-transparent"),
    }
}

fn resolution_name(resolution: TerminalResolution) -> String {
    match resolution {
        TerminalResolution::Low => fl!("igs-resolution-low"),
        TerminalResolution::Medium => fl!("igs-resolution-medium"),
        TerminalResolution::High => fl!("igs-resolution-high"),
    }
}

/// The short name of an item in the command list.
fn item_name(item: &IgsItem) -> String {
    let command = match item {
        IgsItem::Text(text) if text.invalid => return fl!("igs-command-invalid"),
        IgsItem::Text(_) => return fl!("igs-command-text"),
        IgsItem::Command(command) => command.command(),
    };
    if let Some(tool) = shape_tool(command) {
        return tool.label();
    }
    match command {
        IgsCommand::ColorSet { .. } => fl!("igs-command-color"),
        IgsCommand::AttributeForFills { .. } => fl!("igs-command-fill"),
        IgsCommand::SetLineOrMarkerStyle { style } => match style {
            LineMarkerStyle::PolyMarkerSize(..) => fl!("igs-command-marker-style"),
            _ => fl!("igs-command-line-style"),
        },
        IgsCommand::SetPenColor { .. } => fl!("igs-command-pen-color"),
        IgsCommand::DrawingMode { .. } => fl!("igs-command-drawing-mode"),
        IgsCommand::HollowSet { .. } => fl!("igs-command-hollow"),
        IgsCommand::TextEffects { .. } => fl!("igs-command-text-effects"),
        IgsCommand::SetResolution { .. } => fl!("igs-command-resolution"),
        IgsCommand::ScreenClear { .. } => fl!("igs-command-clear"),
        IgsCommand::Loop(_) => fl!("igs-command-loop"),
        IgsCommand::Pause { .. } => fl!("igs-command-pause"),
        IgsCommand::LineDrawTo { .. } => fl!("igs-command-draw-to"),
        // Other commands are named after their variant, without the parameters.
        other => {
            let debug = format!("{other:?}");
            debug.split([' ', '{', '(']).next().unwrap_or_default().to_owned()
        }
    }
}

/// A compact summary next to the name; the property panel shows every parameter.
fn item_summary(item: &IgsItem) -> String {
    let command = match item {
        IgsItem::Text(text) => return properties::escape(&text.bytes).replace('\n', "↵"),
        IgsItem::Command(command) => command.command(),
    };
    match command {
        IgsCommand::WriteText { text, .. } => format!("\"{}\"", latin1(text)),
        IgsCommand::ColorSet { pen, color } => format!("{} · {color}", pen_type_name(*pen)),
        IgsCommand::AttributeForFills { pattern_type, border } => {
            if *border {
                format!("{} · {}", pattern_name(*pattern_type), fl!("igs-fill-border"))
            } else {
                pattern_name(*pattern_type)
            }
        }
        IgsCommand::SetLineOrMarkerStyle { style } => match style {
            LineMarkerStyle::PolyMarkerSize(kind, size) => format!("{} · {size}", marker_name(*kind)),
            LineMarkerStyle::LineThickness(kind, thickness) => format!("{} · {thickness} px", line_kind_name(*kind)),
            LineMarkerStyle::LineEndpoints(kind, ..) => line_kind_name(*kind),
        },
        IgsCommand::SetPenColor { pen, red, green, blue } => format!("{pen} → {red} {green} {blue}"),
        IgsCommand::DrawingMode { mode } => drawing_mode_name(*mode),
        IgsCommand::HollowSet { enabled } => {
            if *enabled {
                fl!("igs-on")
            } else {
                fl!("igs-off")
            }
        }
        IgsCommand::TextEffects { size, .. } => fl!("igs-text-size-summary", size = size),
        IgsCommand::SetResolution { resolution, .. } => resolution_name(*resolution),
        IgsCommand::Loop(data) => format!("{} → {} · {}", data.from, data.to, data.step),
        IgsCommand::Pause { pause_type } => format!("{} ms", pause_type.ms()),
        _ => describe(command).unwrap_or_default(),
    }
}

fn item_icon(item: &IgsItem) -> Option<&'static str> {
    let command = item.command()?;
    if let Some(tool) = shape_tool(command) {
        return Some(tool.icon());
    }
    Some(match command {
        IgsCommand::ColorSet { .. } => "paint_brush",
        IgsCommand::AttributeForFills { .. } | IgsCommand::HollowSet { .. } => "fill",
        IgsCommand::SetLineOrMarkerStyle { .. } => "line",
        IgsCommand::SetPenColor { .. } => "dropper",
        IgsCommand::TextEffects { .. } => "font",
        IgsCommand::Loop(_) => "repeat",
        IgsCommand::Pause { .. } => "pause",
        IgsCommand::LineDrawTo { .. } => "line",
        _ => return None,
    })
}

/// The color a state command sets, shown as a swatch in the list.
fn item_swatch(item: &IgsItem, palette: &icy_engine::Palette, resolution: TerminalResolution) -> Option<Color32> {
    match item.command()? {
        IgsCommand::ColorSet { color, .. } => Some(palette::pen_color(palette, resolution, *color)),
        IgsCommand::SetPenColor { red, green, blue, .. } => Some(Color32::from_rgb(red.min(&7) * 34, green.min(&7) * 34, blue.min(&7) * 34)),
        _ => None,
    }
}

/// Tool settings: the attributes new shapes are drawn with.
#[derive(Clone, Debug, PartialEq)]
struct Attributes {
    line_color: u8,
    fill_color: u8,
    text_color: u8,
    marker_color: u8,
    pattern: PatternType,
    border: bool,
    line_kind: LineKind,
    line_thickness: u8,
    marker: PolymarkerKind,
    marker_size: u8,
    drawing_mode: DrawingMode,
    text_effects: TextEffects,
    text_size: u8,
    text_rotation: TextRotation,
    start_angle: i32,
    end_angle: i32,
}

impl Default for Attributes {
    fn default() -> Self {
        Self {
            line_color: 1,
            fill_color: 1,
            text_color: 1,
            marker_color: 1,
            pattern: PatternType::Solid,
            border: false,
            line_kind: LineKind::Solid,
            line_thickness: 1,
            marker: PolymarkerKind::Point,
            marker_size: 1,
            drawing_mode: DrawingMode::Replace,
            text_effects: TextEffects::NORMAL,
            text_size: 9,
            text_rotation: TextRotation::Degrees0,
            start_angle: 0,
            end_angle: 90,
        }
    }
}

impl Attributes {
    /// The commands that make `state` match these attributes for `tool`.
    fn commands(&self, tool: Tool, state: &IgsDrawState) -> Vec<IgsCommand> {
        let mut commands = Vec::new();
        let Some(pen) = tool.pen() else {
            return commands;
        };
        let (current, wanted) = match pen {
            PenType::Line => (state.line_color, self.line_color),
            PenType::Fill => (state.fill_color, self.fill_color),
            PenType::Text => (state.text_color, self.text_color),
            PenType::Polymarker => (state.marker_color, self.marker_color),
        };
        if current != Some(wanted) {
            commands.push(IgsCommand::ColorSet { pen, color: wanted });
        }
        if tool.uses_line_style() {
            let style = LineMarkerStyle::LineThickness(self.line_kind, self.line_thickness);
            if state.line != Some(style) {
                commands.push(IgsCommand::SetLineOrMarkerStyle { style });
            }
        }
        if tool.uses_fill() && state.fill != Some((self.pattern, self.border)) {
            commands.push(IgsCommand::AttributeForFills {
                pattern_type: self.pattern,
                border: self.border,
            });
        }
        if tool == Tool::Marker {
            let style = LineMarkerStyle::PolyMarkerSize(self.marker, self.marker_size);
            if state.marker != Some(style) {
                commands.push(IgsCommand::SetLineOrMarkerStyle { style });
            }
        }
        if tool == Tool::Text && state.text != Some((self.text_effects, self.text_size, self.text_rotation)) {
            commands.push(IgsCommand::TextEffects {
                effects: self.text_effects,
                size: self.text_size,
                rotation: self.text_rotation,
            });
        }
        if state.drawing_mode != Some(self.drawing_mode) {
            commands.push(IgsCommand::DrawingMode { mode: self.drawing_mode });
        }
        commands
    }
}

struct ShapeDrag {
    index: usize,
    handle: Handle,
    start: Point,
    original: Geometry,
    command: IgsCommand,
    current: IgsCommand,
}

/// Text being typed on the canvas; `index` is the edited text command.
struct TextEdit {
    at: Point,
    text: Vec<u8>,
    index: Option<usize>,
}

impl TextEdit {
    fn command(&self) -> IgsCommand {
        IgsCommand::WriteText {
            x: IgsParameter::Value(self.at.0),
            y: IgsParameter::Value(self.at.1),
            text: self.text.clone(),
        }
    }
}

/// What the canvas shows besides the document.
#[derive(Clone, Debug, PartialEq)]
enum PreviewRequest {
    Through(Option<usize>),
    With(Vec<IgsCommand>),
    Replacing(usize, IgsCommand),
}

pub struct IgsEditor {
    document: IgsDocument,
    selected: Option<usize>,
    preview_to_selection: bool,
    /// The property draft of the selected command.
    editing: Option<(usize, IgsCommand)>,
    /// The escaped source of the selected item while it is edited.
    source: Option<(usize, String)>,
    tool: Tool,
    attributes: Attributes,
    icons: Icons,
    palette_dialog: Option<PaletteDialog>,
    poly: Vec<Point>,
    text_edit: Option<TextEdit>,
    shape_drag: Option<ShapeDrag>,
    drag: Option<(Point, Point)>,
    hover: Option<Point>,
    palette: icy_engine::Palette,
    canvas: Canvas,
    texture: Option<egui::TextureHandle>,
    shown: Option<(u64, PreviewRequest)>,
    /// The selection the command list last showed and the rows it had in view, so a selection
    /// made on the canvas scrolls into view.
    listed_selection: Option<usize>,
    visible_rows: std::ops::Range<usize>,
    error: Option<String>,
    #[cfg(test)]
    canvas_rect: Option<egui::Rect>,
}

impl IgsEditor {
    pub fn new(resolution: TerminalResolution) -> Self {
        Self::from_document(IgsDocument::new(resolution))
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        Ok(Self::from_document(IgsDocument::open(path).map_err(|error| error.to_string())?))
    }

    fn from_document(document: IgsDocument) -> Self {
        let resolution = document.resolution();
        Self {
            document,
            selected: None,
            preview_to_selection: false,
            editing: None,
            source: None,
            tool: Tool::Line,
            attributes: Attributes::default(),
            icons: Icons::default(),
            palette_dialog: None,
            poly: Vec::new(),
            text_edit: None,
            shape_drag: None,
            drag: None,
            hover: None,
            palette: icy_engine::TerminalResolutionExt::palette(&resolution).clone(),
            canvas: Canvas::new(resolution),
            texture: None,
            shown: None,
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
        self.commit_properties();
        self.document.save_as(path, overwrite).map_err(|error| error.to_string())
    }

    pub fn modified(&self) -> bool {
        let typed = self.text_edit.as_ref().is_some_and(|edit| match edit.index {
            Some(index) => self.document.command(index) != Some(&edit.command()),
            None => !edit.text.is_empty(),
        });
        self.document.is_dirty() || !self.poly.is_empty() || typed
    }

    pub fn recovery_snapshot(&self) -> Result<icy_draw::recovery::Snapshot, String> {
        self.document.recovery_snapshot().map_err(|error| error.to_string())
    }

    pub fn from_recovery(snapshot: &icy_draw::recovery::Snapshot) -> Result<Self, String> {
        Ok(Self::from_document(IgsDocument::from_recovery(snapshot).map_err(|error| error.to_string())?))
    }

    pub fn can_undo(&self) -> bool {
        self.document.can_undo() || !self.poly.is_empty() || self.text_edit.is_some()
    }

    pub fn can_redo(&self) -> bool {
        self.document.can_redo()
    }

    pub fn undo(&mut self, redo: bool) {
        // Both hold item indices that undo and redo may shift.
        self.shape_drag = None;
        self.drag = None;
        // Undo first drops an unfinished path or text.
        let pending = !self.poly.is_empty() || self.text_edit.take().is_some();
        self.poly.clear();
        if !redo && pending {
            return;
        }
        if redo {
            self.document.redo();
        } else {
            self.document.undo();
        }
        self.selected = self.selected.filter(|index| *index < self.document.len());
        self.editing = None;
        self.source = None;
    }

    fn select_tool(&mut self, tool: Tool) {
        self.finish_pending();
        self.tool = tool;
        self.drag = None;
        self.shape_drag = None;
    }

    fn state_at_end(&self) -> IgsDrawState {
        self.document.state_before(self.document.len())
    }

    /// The command and geometry of a shape, including a drag in progress.
    fn shape(&self, index: usize) -> Option<(IgsCommand, Geometry)> {
        let command = match &self.shape_drag {
            Some(drag) if drag.index == index => drag.current.clone(),
            _ => self.document.command(index)?.clone(),
        };
        // Only text needs the attributes in effect, which takes a scan of the commands before it.
        let text_size = match command {
            IgsCommand::WriteText { .. } => self.document.state_before(index).text.map_or(9, |(_, size, _)| size),
            _ => 9,
        };
        let geometry = select::geometry(&command, &self.canvas, text_size)?;
        Some((command, geometry))
    }

    /// The selected item if it is a shape.
    fn selected_shape(&self) -> Option<usize> {
        self.selected.filter(|index| self.shape(*index).is_some())
    }

    /// The topmost shape at `point`.
    fn shape_at(&self, point: (f32, f32), tolerance: f32) -> Option<usize> {
        (0..self.document.len()).rev().find(|index| {
            self.shape(*index)
                .is_some_and(|(command, geometry)| select::hit(&command, &geometry, point, tolerance))
        })
    }

    fn select_item(&mut self, index: Option<usize>) {
        if self.selected != index {
            self.commit_properties();
            self.selected = index;
            self.editing = None;
            self.source = None;
        }
    }

    fn set_error(&mut self, result: Result<(), icy_draw::igs_document::IgsDocumentError>) -> bool {
        match result {
            Ok(()) => {
                self.error = None;
                true
            }
            Err(error) => {
                self.error = Some(error.to_string());
                false
            }
        }
    }

    fn replace_command(&mut self, index: usize, command: IgsCommand) {
        let result = self.document.replace(index, command);
        if self.set_error(result) {
            self.editing = None;
            self.source = None;
        }
    }

    fn nudge_selected(&mut self, dx: i32, dy: i32) {
        let Some(index) = self.selected_shape() else {
            return;
        };
        if let Some((command, geometry)) = self.shape(index) {
            let moved = select::translate(&geometry, &self.canvas, dx, dy);
            self.replace_command(index, select::apply(&command, &moved, &self.canvas));
        }
    }

    fn delete_selected(&mut self) {
        self.finish_text();
        let Some(index) = self.selected else {
            return;
        };
        let result = self.document.delete(index).map(|_| ());
        if self.set_error(result) {
            self.selected = None;
            self.editing = None;
            self.source = None;
        }
    }

    fn move_selected(&mut self, delta: isize) {
        self.finish_text();
        self.commit_properties();
        let Some(index) = self.selected else {
            return;
        };
        let target = index.saturating_add_signed(delta);
        let result = self.document.move_item(index, target);
        if self.set_error(result) {
            self.selected = Some(target);
            self.listed_selection = self.selected;
            self.editing = None;
            self.source = None;
        }
    }

    /// Attribute commands for `tool` followed by `shape`.
    fn with_attributes(&self, tool: Tool, shape: IgsCommand) -> Vec<IgsCommand> {
        let mut commands = self.attributes.commands(tool, &self.state_at_end());
        commands.push(shape);
        commands
    }

    fn shape_commands(&self, from: Point, to: Point) -> Vec<IgsCommand> {
        let angles = (self.attributes.start_angle, self.attributes.end_angle);
        match self.tool.command(&self.canvas, from, to, angles) {
            Some(shape) => self.with_attributes(self.tool, shape),
            None => Vec::new(),
        }
    }

    fn text_commands(&self, edit: &TextEdit) -> Vec<IgsCommand> {
        self.with_attributes(Tool::Text, edit.command())
    }

    /// Appends commands as one undo step and selects the last one.
    fn add_commands(&mut self, commands: Vec<IgsCommand>) {
        if commands.is_empty() {
            return;
        }
        self.commit_properties();
        let result = self.document.append_many(commands);
        if self.set_error(result) {
            self.selected = self.document.len().checked_sub(1);
            self.editing = None;
            self.source = None;
        }
    }

    fn add_shape(&mut self, from: Point, to: Point) {
        // A click without a drag adds nothing for shapes that have an extent.
        if self.tool.is_dragged() && from == to {
            return;
        }
        let commands = self.shape_commands(from, to);
        self.add_commands(commands);
    }

    fn finish_pending(&mut self) {
        self.finish_poly();
        self.finish_text();
    }

    fn finish_poly(&mut self) {
        let minimum = if self.tool == Tool::PolyLine { 2 } else { 3 };
        if self.poly.len() >= minimum {
            let commands = self.with_attributes(self.tool, poly_command(self.tool, &self.poly));
            self.add_commands(commands);
        }
        self.poly.clear();
    }

    /// The topmost text at `point`.
    fn text_at(&self, point: Point) -> Option<usize> {
        let point = (point.0 as f32 + 0.5, point.1 as f32 + 0.5);
        (0..self.document.len()).rev().find(|index| {
            matches!(self.document.command(*index), Some(IgsCommand::WriteText { .. }))
                && self
                    .shape(*index)
                    .is_some_and(|(command, geometry)| select::hit(&command, &geometry, point, 2.0))
        })
    }

    /// Starts typing at `point`, or edits the text there.
    fn begin_text(&mut self, point: Point) {
        self.finish_text();
        let edit = match self.text_at(point).and_then(|index| Some((index, self.document.command(index)?.clone()))) {
            Some((index, IgsCommand::WriteText { x, y, text })) => TextEdit {
                at: (select::value(&x).unwrap_or_default(), select::value(&y).unwrap_or_default()),
                text,
                index: Some(index),
            },
            _ => {
                // The anchor is the baseline; the click marks the top of the glyphs.
                let (_, _, top) = select::text_extent(self.attributes.text_size, 1);
                TextEdit {
                    at: (point.0, point.1 + top),
                    text: Vec::new(),
                    index: None,
                }
            }
        };
        self.select_item(edit.index);
        self.text_edit = Some(edit);
    }

    /// Adds the typed text or applies the change to edited text; cleared text is removed.
    fn finish_text(&mut self) {
        let Some(edit) = self.text_edit.take() else {
            return;
        };
        match edit.index {
            Some(index) if !matches!(self.document.command(index), Some(IgsCommand::WriteText { .. })) => {}
            Some(index) if edit.text.is_empty() => {
                let result = self.document.delete(index).map(|_| ());
                if self.set_error(result) {
                    self.selected = None;
                    self.editing = None;
                }
            }
            Some(index) => self.replace_command(index, edit.command()),
            None if !edit.text.is_empty() => {
                let commands = self.text_commands(&edit);
                self.add_commands(commands);
            }
            None => {}
        }
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
        for _ in 0..backspace {
            edit.text.pop();
        }
        // `@` ends IGS text, so it cannot be typed.
        edit.text.extend(typed.bytes().filter(|byte| (0x20..0x7F).contains(byte) && *byte != b'@'));
        if enter {
            self.finish_text();
        }
    }

    fn cancel(&mut self) {
        if !self.poly.is_empty() || self.text_edit.take().is_some() || self.drag.take().is_some() || self.shape_drag.take().is_some() {
            self.poly.clear();
        } else {
            self.select_item(None);
        }
    }

    fn open_palette_dialog(&mut self) {
        self.finish_pending();
        self.palette_dialog = Some(PaletteDialog::new(&self.palette, self.canvas.resolution));
    }

    /// The unfinished shape: a drag in progress, a path or new text.
    fn pending_commands(&self) -> Vec<IgsCommand> {
        if let Some(dialog) = &self.palette_dialog {
            return dialog.commands();
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
                return self.with_attributes(self.tool, poly_command(self.tool, &points));
            }
        }
        match self.drag {
            Some((from, to)) if self.tool.is_dragged() && from != to => self.shape_commands(from, to),
            _ => Vec::new(),
        }
    }

    fn preview_request(&self) -> PreviewRequest {
        if self.preview_to_selection {
            return PreviewRequest::Through(self.selected);
        }
        if let Some(drag) = self.shape_drag.as_ref().filter(|drag| drag.current != drag.command) {
            return PreviewRequest::Replacing(drag.index, drag.current.clone());
        }
        if let Some(edit) = &self.text_edit {
            if let Some(index) = edit
                .index
                .filter(|index| matches!(self.document.command(*index), Some(IgsCommand::WriteText { .. })))
            {
                return PreviewRequest::Replacing(index, edit.command());
            }
        }
        if let Some((index, draft)) = self
            .editing
            .as_ref()
            .filter(|(index, draft)| self.document.command(*index).is_some_and(|command| command != draft))
        {
            return PreviewRequest::Replacing(*index, draft.clone());
        }
        PreviewRequest::With(self.pending_commands())
    }

    fn refresh_preview(&mut self, context: &egui::Context) {
        let request = self.preview_request();
        let key = (self.document.revision(), request);
        if self.shown.as_ref() == Some(&key) && self.texture.is_some() {
            return;
        }
        let preview = match &key.1 {
            PreviewRequest::Through(through) => self.document.preview_through(*through),
            PreviewRequest::With(extra) if extra.is_empty() => self.document.preview(),
            PreviewRequest::With(extra) => self.document.preview_with(self.document.len(), extra),
            PreviewRequest::Replacing(index, command) => self.document.preview_replacing(*index, command),
        };
        match preview {
            Ok(preview) => {
                self.palette = preview.screen().palette().clone();
                self.canvas = Canvas::new(preview.resolution());
                let image = egui::ColorImage::from_rgba_unmultiplied([preview.width(), preview.height()], &preview.rgba());
                if let Some(texture) = &mut self.texture {
                    texture.set(image, egui::TextureOptions::NEAREST);
                } else {
                    self.texture = Some(context.load_texture("igs-editor-canvas", image, egui::TextureOptions::NEAREST));
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        self.shown = Some(key);
    }

    /// Applies a changed property draft, e.g. before another item is chosen.
    fn commit_properties(&mut self) {
        let Some((index, draft)) = self.editing.clone() else {
            return;
        };
        if self.document.command(index).is_some_and(|command| *command != draft) {
            if self.text_edit.as_ref().is_some_and(|edit| edit.index == Some(index)) {
                self.text_edit = None;
            }
            let result = self.document.replace(index, draft);
            self.set_error(result);
        }
    }

    /// Replaces the selected item with edited IGS source.
    fn apply_source(&mut self, index: usize, text: &str) {
        let Some(bytes) = properties::unescape(text) else {
            self.error = Some(fl!("igs-editor-invalid-escape"));
            return;
        };
        match self.document.replace_source(index, &bytes) {
            Ok(_) => {
                self.error = None;
                self.selected = Some(index);
                self.editing = None;
                self.source = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
}

/// One line of the command list: number, icon, name and a short summary, never wrapped.
fn command_row(ui: &mut egui::Ui, icons: &mut Icons, index: usize, item: &IgsItem, selected: bool, swatch: Option<Color32>) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), COMMAND_ROW_HEIGHT), egui::Sense::click());
    let name = item_name(item);
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, ui.is_enabled(), selected, &name));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let visuals = ui.visuals().clone();
    let painter = ui.painter_at(rect);
    if selected {
        painter.rect_filled(rect.shrink2(egui::vec2(2.0, 1.0)), 4, visuals.selection.bg_fill);
    } else if response.hovered() {
        painter.rect_filled(rect.shrink2(egui::vec2(2.0, 1.0)), 4, visuals.widgets.hovered.weak_bg_fill);
    }
    let invalid = matches!(item, IgsItem::Text(text) if text.invalid);
    let text = if selected {
        visuals.selection.stroke.color
    } else if invalid {
        visuals.warn_fg_color
    } else if matches!(item, IgsItem::Text(_)) {
        visuals.weak_text_color()
    } else {
        visuals.text_color()
    };
    let weak = if selected { text.gamma_multiply(0.7) } else { visuals.weak_text_color() };
    let center = rect.center().y;
    painter.text(
        egui::pos2(rect.left() + 34.0, center),
        egui::Align2::RIGHT_CENTER,
        (index + 1).to_string(),
        egui::FontId::monospace(11.0),
        weak,
    );
    let mut x = rect.left() + 42.0;
    if let Some(icon) = item_icon(item) {
        icons
            .image(ui, icon, 14.0)
            .tint(text)
            .paint_at(ui, egui::Rect::from_center_size(egui::pos2(x + 7.0, center), egui::Vec2::splat(14.0)));
    }
    x += 22.0;
    if let Some(color) = swatch {
        let swatch = egui::Rect::from_min_size(egui::pos2(x, center - 6.0), egui::Vec2::splat(12.0));
        painter.rect_filled(swatch, 2, color);
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
    let summary = item_summary(item);
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

impl IgsEditor {
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_space(8.0);
            ui.add(self.icons.image(ui, self.tool.icon(), 18.0).tint(ui.visuals().text_color()));
            ui.label(icy_engine_gui::egui::appearance::bold(ui, self.tool.label()));
            widgets::divider(ui);
            let attributes = &mut self.attributes;
            if self.tool.uses_line_style() {
                properties::line_kind(ui, "igs-tool-line-kind", &mut attributes.line_kind);
                ui.add(egui::DragValue::new(&mut attributes.line_thickness).range(1..=41).suffix(" px"))
                    .on_hover_text(fl!("igs-thickness"));
            }
            if self.tool.uses_fill() && self.tool != Tool::FloodFill {
                properties::pattern(ui, "igs-tool-pattern", &mut attributes.pattern);
                ui.checkbox(&mut attributes.border, fl!("igs-fill-border"));
            } else if self.tool == Tool::FloodFill {
                properties::pattern(ui, "igs-tool-pattern", &mut attributes.pattern);
            }
            if self.tool == Tool::Marker {
                properties::marker(ui, "igs-tool-marker", &mut attributes.marker);
                ui.add(egui::DragValue::new(&mut attributes.marker_size).range(1..=8))
                    .on_hover_text(fl!("igs-size"));
            }
            if self.tool.has_angles() {
                ui.label(fl!("igs-start-angle"));
                ui.add(egui::DragValue::new(&mut attributes.start_angle).range(0..=360).suffix("°"));
                ui.label(fl!("igs-end-angle"));
                ui.add(egui::DragValue::new(&mut attributes.end_angle).range(0..=360).suffix("°"));
            }
            match self.tool {
                Tool::Text => {
                    ui.add(egui::DragValue::new(&mut attributes.text_size).range(1..=40).prefix(fl!("igs-size-prefix")));
                    properties::text_effects(ui, &mut attributes.text_effects);
                    properties::rotation(ui, &mut attributes.text_rotation);
                    ui.weak(if self.text_edit.is_some() {
                        fl!("igs-text-typing-hint")
                    } else {
                        fl!("igs-text-hint")
                    });
                }
                Tool::PolyLine | Tool::Polygon => {
                    ui.weak(fl!("igs-poly-hint"));
                }
                Tool::Select => match self.selected_shape() {
                    Some(index) => {
                        let name = self.document.items().get(index).map(item_name).unwrap_or_default();
                        ui.label(icy_engine_gui::egui::appearance::bold(ui, name));
                        if self.icons.button(ui, "delete", &fl!("igs-editor-delete"), false).clicked() {
                            self.delete_selected();
                        }
                    }
                    None => {
                        ui.weak(fl!("igs-select-hint"));
                    }
                },
                _ => {}
            }
            let edited = match self.tool {
                Tool::Select => self.selected_shape().and_then(|index| self.shape(index)).map(|(command, _)| command),
                _ => self.pending_commands().pop(),
            };
            let parameters = edited.as_ref().and_then(describe).or_else(|| self.hover.map(|(x, y)| format!("{x}, {y}")));
            if let Some(parameters) = parameters {
                widgets::divider(ui);
                ui.label(egui::RichText::new(parameters).monospace().color(ui.visuals().weak_text_color()));
            }
        });
    }

    fn sidebar(&mut self, context: &egui::Context, blocked: bool) {
        let panel_fill = context.style().visuals.panel_fill;
        egui::SidePanel::left("igs-tools")
            .exact_width(148.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(panel_fill).inner_margin(egui::Margin::symmetric(6, 6)))
            .show(context, |ui| {
                ui.add_enabled_ui(!blocked, |ui| {
                    ui.weak(resolution_name(self.canvas.resolution));
                    let resolution = self.canvas.resolution;
                    for (id, label, pen) in [
                        ("line", fl!("igs-pen-line"), &mut self.attributes.line_color),
                        ("fill", fl!("igs-pen-fill"), &mut self.attributes.fill_color),
                        ("text", fl!("igs-pen-text"), &mut self.attributes.text_color),
                        ("marker", fl!("igs-pen-marker"), &mut self.attributes.marker_color),
                    ] {
                        *pen = (*pen).min(palette::pen_count(resolution) - 1);
                        ui.horizontal(|ui| {
                            ui.label(label);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                palette::pen_picker(ui, id, &self.palette, resolution, pen, egui::vec2(30.0, 24.0));
                            });
                        });
                    }
                    if ui
                        .add(egui::Button::new(fl!("igs-palette-edit")).min_size(egui::vec2(ui.available_width(), 26.0)))
                        .on_hover_text(fl!("igs-palette-edit-tooltip"))
                        .clicked()
                    {
                        self.open_palette_dialog();
                    }
                    ui.horizontal(|ui| {
                        ui.label(fl!("igs-drawing-mode"));
                    });
                    properties::drawing_mode(ui, "igs-tool-mode", &mut self.attributes.drawing_mode);
                    ui.separator();
                    egui::Grid::new("igs-tools-grid").num_columns(3).show(ui, |ui| {
                        for (index, tool) in Tool::ALL.into_iter().enumerate() {
                            if self.icons.button_sized(ui, tool.icon(), &tool.label(), self.tool == tool, 38.0).clicked() {
                                self.select_tool(tool);
                            }
                            if index % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });
                });
            });
    }

    fn command_list(&mut self, context: &egui::Context, blocked: bool) {
        egui::SidePanel::right("igs-commands")
            .default_width(300.0)
            .min_width(220.0)
            .show(context, |ui| {
                if blocked {
                    ui.disable();
                }
                let count = self.document.len();
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(icy_engine_gui::egui::appearance::bold(ui, fl!("igs-editor-commands")));
                    ui.weak(count.to_string());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let selected = self.selected.filter(|index| *index < count);
                        let mut action = None;
                        ui.add_enabled_ui(selected.is_some(), |ui| {
                            if self.icons.button_sized(ui, "delete", &fl!("igs-editor-delete"), false, 26.0).clicked() {
                                action = Some(0);
                            }
                        });
                        ui.add_enabled_ui(selected.is_some_and(|index| index + 1 < count), |ui| {
                            if self.icons.button_sized(ui, "move_down", &fl!("igs-editor-down"), false, 26.0).clicked() {
                                action = Some(1);
                            }
                        });
                        ui.add_enabled_ui(selected.is_some_and(|index| index > 0), |ui| {
                            if self.icons.button_sized(ui, "move_up", &fl!("igs-editor-up"), false, 26.0).clicked() {
                                action = Some(-1);
                            }
                        });
                        widgets::divider(ui);
                        let through = self.preview_to_selection;
                        if self
                            .icons
                            .button_sized(ui, "visibility", &fl!("igs-editor-preview-through"), through, 26.0)
                            .clicked()
                        {
                            self.preview_to_selection = !through;
                        }
                        match action {
                            Some(0) => self.delete_selected(),
                            Some(delta) => self.move_selected(delta),
                            None => {}
                        }
                    });
                });
                ui.separator();

                let previous = self.selected;
                let count = self.document.len();
                let list_height = (ui.available_height() - 280.0).max(120.0);
                let mut scroll = egui::ScrollArea::vertical()
                    .id_salt("igs-command-list")
                    .auto_shrink([false, false])
                    .max_height(list_height);
                // A shape picked on the canvas is scrolled into view in the list.
                if self.selected != self.listed_selection {
                    if let Some(index) = self.selected.filter(|index| !self.visible_rows.contains(index)) {
                        scroll = scroll.vertical_scroll_offset((index as f32 * COMMAND_ROW_HEIGHT - list_height / 2.0).max(0.0));
                    }
                }
                let mut clicked = None;
                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    scroll.show_rows(ui, COMMAND_ROW_HEIGHT, count, |ui, rows| {
                        self.visible_rows = rows.clone();
                        for index in rows {
                            let Some(item) = self.document.items().get(index) else {
                                break;
                            };
                            let swatch = item_swatch(item, &self.palette, self.canvas.resolution);
                            if command_row(ui, &mut self.icons, index, item, self.selected == Some(index), swatch).clicked() {
                                clicked = Some(index);
                            }
                        }
                    });
                });
                if let Some(index) = clicked {
                    self.finish_text();
                    self.select_item(Some(index));
                }
                self.listed_selection = self.selected;
                if self.selected != previous {
                    self.editing = None;
                    self.source = None;
                }
                self.property_panel(ui);
            });
    }

    fn property_panel(&mut self, ui: &mut egui::Ui) {
        let Some(index) = self.selected.filter(|index| *index < self.document.len()) else {
            return;
        };
        if self.editing.as_ref().is_none_or(|(edited, _)| *edited != index) {
            self.editing = self.document.command(index).cloned().map(|command| (index, command));
        }
        ui.separator();
        let item = &self.document.items()[index];
        ui.horizontal(|ui| {
            if let Some(icon) = item_icon(item) {
                ui.add(self.icons.image(ui, icon, 16.0));
            }
            ui.label(icy_engine_gui::egui::appearance::bold(ui, item_name(item)));
            ui.weak(format!("#{}", index + 1));
        });
        ui.add_space(4.0);
        let is_loop = matches!(item.command(), Some(IgsCommand::Loop(_)));
        let mut apply = None;
        let mut apply_source = None;
        let area = egui::ScrollArea::vertical()
            .id_salt("igs-properties")
            .max_height((ui.available_height() - 8.0).max(60.0))
            .show(ui, |ui| {
                let mut typed = false;
                if let Some((_, draft)) = self.editing.as_mut().filter(|_| !is_loop) {
                    typed = properties::command(ui, draft, &self.palette, self.canvas.resolution);
                }
                if is_loop {
                    ui.weak(fl!("igs-editor-loop-source"));
                } else if !typed {
                    ui.weak(fl!("igs-editor-source-only"));
                }
                ui.add_space(4.0);
                let source = match &self.source {
                    Some((edited, text)) if *edited == index => text.clone(),
                    _ => self.document.item_source(index).map(|bytes| properties::escape(&bytes)).unwrap_or_default(),
                };
                egui::CollapsingHeader::new(fl!("igs-editor-source"))
                    .id_salt(("igs-source", index))
                    .default_open(is_loop || !typed)
                    .show(ui, |ui| {
                        let mut text = source.clone();
                        ui.add(
                            egui::TextEdit::multiline(&mut text)
                                .font(egui::TextStyle::Monospace)
                                .desired_rows(3)
                                .desired_width(f32::INFINITY),
                        );
                        if text != source {
                            self.source = Some((index, text.clone()));
                        }
                        let original = self.document.item_source(index).map(|bytes| properties::escape(&bytes)).unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.add_enabled_ui(text != original, |ui| {
                                if ui.button(fl!("igs-editor-apply-source")).clicked() {
                                    apply_source = Some(text.clone());
                                }
                                if ui.button(fl!("igs-editor-revert-source")).clicked() {
                                    self.source = None;
                                }
                            });
                        });
                    });
                typed
            });
        if let Some(text) = apply_source {
            self.apply_source(index, &text);
            return;
        }
        if area.inner {
            if let Some((index, draft)) = self.editing.clone() {
                // Changes apply without a confirmation, once a drag ends or a field is left.
                let context = ui.ctx().clone();
                let dragging = context.input(|input| input.pointer.any_down());
                let typing = context
                    .memory(|memory| memory.focused())
                    .and_then(|id| context.read_response(id))
                    .is_some_and(|response| area.inner_rect.intersects(response.rect));
                let changed = self.document.command(index) != Some(&draft);
                if changed && !dragging && !typing {
                    apply = Some((index, draft));
                }
            }
        }
        if let Some((index, command)) = apply {
            // The text typed on the canvas would otherwise overwrite the property change.
            if self.text_edit.as_ref().is_some_and(|edit| edit.index == Some(index)) {
                self.text_edit = None;
            }
            let result = self.document.replace(index, command);
            if !self.set_error(result) {
                self.editing = None;
            }
        }
    }
}

impl IgsEditor {
    pub fn show(&mut self, context: &egui::Context, blocked: bool) {
        let blocked = blocked || self.palette_dialog.is_some();
        let panel_fill = context.style().visuals.panel_fill;
        egui::TopBottomPanel::top("igs-toolbar")
            .exact_height(TOOLBAR_HEIGHT)
            .frame(egui::Frame::new().fill(panel_fill))
            .show(context, |ui| {
                if blocked {
                    ui.disable();
                }
                self.toolbar(ui);
            });
        self.sidebar(context, blocked);
        self.command_list(context, blocked);
        if !blocked {
            if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                self.cancel();
            }
            self.type_text(context);
            // Text fields in the toolbar and command list keep their keys.
            if self.tool == Tool::Select && context.memory(|memory| memory.focused().is_none()) {
                if self.selected.is_some()
                    && context.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Delete) || input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
                    })
                {
                    self.delete_selected();
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
            if !self.poly.is_empty()
                && context.memory(|memory| memory.focused().is_none())
                && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
            {
                self.finish_poly();
            }
        }
        egui::CentralPanel::default().show(context, |ui| {
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            self.canvas(ui, blocked);
        });
        if let Some(dialog) = &mut self.palette_dialog {
            match dialog.show(context) {
                PaletteResult::Open => {}
                PaletteResult::Cancel => self.palette_dialog = None,
                PaletteResult::Apply => {
                    if let Some(dialog) = self.palette_dialog.take() {
                        if dialog.resolution() == self.canvas.resolution {
                            self.add_commands(dialog.commands());
                        }
                    }
                }
            }
        }
    }

    /// Canvas pixels per screen point, horizontally and vertically. Medium resolution pixels
    /// are twice as tall as wide, like on an Atari ST monitor.
    fn scale(&self, available: egui::Vec2) -> egui::Vec2 {
        let aspect = if self.canvas.resolution == TerminalResolution::Medium { 2.0 } else { 1.0 };
        let (width, height) = (self.canvas.width as f32, self.canvas.height as f32 * aspect);
        let scale = (available.x / width).min(available.y / height).max(0.5);
        egui::vec2(scale, scale * aspect)
    }

    fn canvas(&mut self, ui: &mut egui::Ui, blocked: bool) {
        self.refresh_preview(ui.ctx());
        let Some(texture) = self.texture.clone() else {
            return;
        };
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            let scale = self.scale(ui.available_size());
            let size = egui::vec2(self.canvas.width as f32 * scale.x, self.canvas.height as f32 * scale.y);
            let response = ui.add(
                egui::Image::new(&texture)
                    .maintain_aspect_ratio(false)
                    .fit_to_exact_size(size)
                    .sense(egui::Sense::click_and_drag()),
            );
            #[cfg(test)]
            {
                self.canvas_rect = Some(response.rect);
            }
            let origin = response.rect.min;
            let (width, height) = (self.canvas.width, self.canvas.height);
            let at = move |pos: egui::Pos2| -> Point {
                (
                    ((pos.x - origin.x) / scale.x).floor().clamp(0.0, (width - 1) as f32) as i32,
                    ((pos.y - origin.y) / scale.y).floor().clamp(0.0, (height - 1) as f32) as i32,
                )
            };
            let on_screen = move |point: Point| origin + egui::vec2((point.0 as f32 + 0.5) * scale.x, (point.1 as f32 + 0.5) * scale.y);
            self.hover = response.hover_pos().map(at);
            if !blocked {
                self.canvas_input(ui, &response, &at, &on_screen, scale);
                // The texture is uploaded at the end of the frame, so the changed shape shows now.
                self.refresh_preview(ui.ctx());
            }
            let accent = ui.visuals().selection.stroke.color;
            if self.tool == Tool::Select {
                if let Some((command, geometry)) = self.selected_shape().and_then(|index| self.shape(index)) {
                    let points_only = select::points_only(&command);
                    if !points_only {
                        let (x0, y0, x1, y1) = select::bounds(&geometry);
                        let frame = egui::Rect::from_two_pos(on_screen((x0, y0)), on_screen((x1, y1))).expand2(scale * 0.5 + egui::Vec2::splat(2.0));
                        ui.painter().rect_stroke(frame, 0.0, Stroke::new(1.0, accent), egui::StrokeKind::Middle);
                    }
                    let handles = select::handles(&command, &geometry);
                    let count = handles.len();
                    for (index, (_, point)) in handles.into_iter().enumerate() {
                        let handle = egui::Rect::from_center_size(on_screen(point), egui::Vec2::splat(8.0));
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
                let size = match edit.index {
                    Some(index) => self.document.state_before(index).text.map_or(9, |(_, size, _)| size),
                    None => self.attributes.text_size,
                };
                let (width, height, top) = select::text_extent(size, edit.text.len());
                let (glyph, _, _) = select::text_extent(size, 1);
                let frame = egui::Rect::from_min_max(
                    on_screen((edit.at.0, edit.at.1 - top)),
                    on_screen((edit.at.0 + width.max(glyph), edit.at.1 - top + height)),
                )
                .expand(2.0);
                ui.painter()
                    .rect_stroke(frame, 1.0, Stroke::new(1.0, accent.gamma_multiply(0.6)), egui::StrokeKind::Outside);
                // A blinking caret after the last character.
                let time = ui.input(|input| input.time);
                if (time * 2.0) as i64 % 2 == 0 {
                    let x = on_screen((edit.at.0 + width, 0)).x - scale.x * 0.5;
                    let caret = egui::Rect::from_min_max(egui::pos2(x - 1.0, frame.top() + 2.0), egui::pos2(x + 1.0, frame.bottom() - 2.0));
                    ui.painter().rect_filled(caret, 0.0, accent);
                }
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
            }
        });
    }

    fn select_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        at: &dyn Fn(egui::Pos2) -> Point,
        on_screen: &dyn Fn(Point) -> egui::Pos2,
        scale: egui::Vec2,
    ) {
        let pointer = response.interact_pointer_pos();
        let origin = ui.input(|input| input.pointer.press_origin());
        let tolerance = HANDLE_RADIUS / scale.x.max(0.1);
        let scene = |pos: egui::Pos2| {
            let (x, y) = at(pos);
            (x as f32 + 0.5, y as f32 + 0.5)
        };
        let handle_at = |editor: &Self, pos: egui::Pos2| {
            let index = editor.selected_shape()?;
            let (command, geometry) = editor.shape(index)?;
            select::handles(&command, &geometry)
                .into_iter()
                .map(|(handle, point)| (handle, on_screen(point).distance(pos)))
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
                self.select_item(target.map(|(index, _)| index));
                // A typed property value is kept; the drag starts from it.
                self.commit_properties();
                if let Some((index, handle)) = target {
                    if let Some((command, geometry)) = self.shape(index) {
                        self.shape_drag = Some(ShapeDrag {
                            index,
                            handle,
                            start: at(start),
                            original: geometry,
                            current: command.clone(),
                            command,
                        });
                    }
                }
            }
        }
        if let Some(pointer) = pointer {
            let canvas = self.canvas;
            if let Some(drag) = &mut self.shape_drag {
                let geometry = select::drag(&drag.original, &canvas, drag.handle, drag.start, at(pointer));
                drag.current = select::apply(&drag.command, &geometry, &canvas);
            }
            if response.drag_stopped() {
                if let Some(drag) = self.shape_drag.take() {
                    if drag.current != drag.command {
                        self.replace_command(drag.index, drag.current);
                    }
                }
            }
        }
        if response.clicked() {
            let index = pointer.and_then(|pointer| self.shape_at(scene(pointer), tolerance));
            self.select_item(index);
        }
        if response.double_clicked() {
            let text = self.selected_shape().and_then(|index| match self.document.command(index) {
                Some(IgsCommand::WriteText { .. }) => self.shape(index).map(|(_, geometry)| select::bounds(&geometry)),
                _ => None,
            });
            if let Some((x, y, _, _)) = text {
                // Double-clicking text types into it with the text tool.
                self.select_tool(Tool::Text);
                self.begin_text((x, y));
            }
        }
    }

    fn canvas_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        at: &dyn Fn(egui::Pos2) -> Point,
        on_screen: &dyn Fn(Point) -> egui::Pos2,
        scale: egui::Vec2,
    ) {
        let pointer = response.interact_pointer_pos();
        let origin = ui.input(|input| input.pointer.press_origin());
        if self.tool == Tool::Select {
            self.select_input(ui, response, at, on_screen, scale);
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
                        if self.poly.len() == MAX_POINTS {
                            self.finish_poly();
                        }
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(context: &egui::Context, editor: &mut IgsEditor, events: Vec<egui::Event>) {
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

    fn screen(editor: &IgsEditor, point: Point) -> egui::Pos2 {
        let rect = editor.canvas_rect.unwrap();
        let scale = egui::vec2(rect.width() / editor.canvas.width as f32, rect.height() / editor.canvas.height as f32);
        rect.min + egui::vec2((point.0 as f32 + 0.5) * scale.x, (point.1 as f32 + 0.5) * scale.y)
    }

    /// Drags from `from` to `to` in canvas pixels.
    fn drag(context: &egui::Context, editor: &mut IgsEditor, from: Point, to: Point) {
        let (start, end) = (screen(editor, from), screen(editor, to));
        run(
            context,
            editor,
            vec![egui::Event::PointerMoved(start), button_event(start, egui::PointerButton::Primary, true)],
        );
        run(context, editor, vec![egui::Event::PointerMoved(start.lerp(end, 0.5))]);
        run(context, editor, vec![egui::Event::PointerMoved(end)]);
        run(context, editor, vec![button_event(end, egui::PointerButton::Primary, false)]);
        run(context, editor, vec![]);
    }

    fn click(context: &egui::Context, editor: &mut IgsEditor, point: Point) {
        let pos = screen(editor, point);
        run(
            context,
            editor,
            vec![egui::Event::PointerMoved(pos), button_event(pos, egui::PointerButton::Primary, true)],
        );
        run(context, editor, vec![button_event(pos, egui::PointerButton::Primary, false)]);
        run(context, editor, vec![]);
    }

    fn commands(editor: &IgsEditor) -> Vec<IgsCommand> {
        editor.document.items().iter().filter_map(IgsItem::command).cloned().collect()
    }

    #[test]
    fn every_tool_adds_its_shape_with_attributes_as_one_undo_step() {
        for tool in Tool::ALL.into_iter().filter(|tool| *tool != Tool::Select && *tool != Tool::Text) {
            let mut editor = IgsEditor::new(TerminalResolution::Low);
            let before = editor.document.len();
            editor.select_tool(tool);
            editor.attributes.line_color = 3;
            editor.attributes.fill_color = 4;
            editor.attributes.marker_color = 5;
            editor.attributes.start_angle = 10;
            editor.attributes.end_angle = 120;
            if tool.is_poly() {
                editor.poly = vec![(100, 100), (180, 100), (160, 180)];
                editor.finish_poly();
            } else {
                editor.add_shape((100, 100), (160, 140));
            }
            assert!(editor.error.is_none(), "{tool:?}: {:?}", editor.error);
            let commands = commands(&editor);
            let shape = commands.last().unwrap();
            assert_eq!(shape_tool(shape), Some(tool), "{tool:?}");
            let color = match tool.pen().unwrap() {
                PenType::Line => 3,
                PenType::Fill => 4,
                PenType::Polymarker => 5,
                PenType::Text => unreachable!(),
            };
            assert!(
                commands.contains(&IgsCommand::ColorSet {
                    pen: tool.pen().unwrap(),
                    color
                }),
                "{tool:?}"
            );
            assert_eq!(
                commands.iter().any(|command| matches!(command, IgsCommand::AttributeForFills { .. })),
                tool.uses_fill(),
                "{tool:?}"
            );
            if tool.has_angles() {
                assert!(describe(shape).is_some());
                assert!(matches!(
                    shape,
                    IgsCommand::Arc {
                        start_angle: IgsParameter::Value(10),
                        end_angle: IgsParameter::Value(120),
                        ..
                    } | IgsCommand::EllipticalArc {
                        start_angle: IgsParameter::Value(10),
                        end_angle: IgsParameter::Value(120),
                        ..
                    } | IgsCommand::PieSlice {
                        start_angle: IgsParameter::Value(10),
                        end_angle: IgsParameter::Value(120),
                        ..
                    } | IgsCommand::EllipticalPieSlice {
                        start_angle: IgsParameter::Value(10),
                        end_angle: IgsParameter::Value(120),
                        ..
                    }
                ));
            }
            let context = egui::Context::default();
            let _ = context.run(egui::RawInput::default(), |context| {
                egui::CentralPanel::default().show(context, |ui| {
                    let mut draft = shape.clone();
                    assert!(properties::command(ui, &mut draft, &editor.palette, TerminalResolution::Low), "{tool:?}");
                });
            });
            let reopened = IgsDocument::from_bytes(&editor.document.to_bytes().unwrap()).unwrap();
            assert_eq!(
                reopened.items().iter().filter_map(IgsItem::command).cloned().collect::<Vec<_>>(),
                commands,
                "{tool:?}"
            );
            editor.undo(false);
            assert_eq!(editor.document.len(), before, "{tool:?}");
        }
    }

    #[test]
    fn attributes_are_only_written_when_they_change() {
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        editor.select_tool(Tool::Line);
        editor.add_shape((0, 0), (10, 10));
        let first = editor.document.len();
        editor.add_shape((10, 10), (20, 0));
        assert_eq!(editor.document.len(), first + 1, "the second line reuses the attributes");
        editor.attributes.line_color = 2;
        editor.add_shape((20, 0), (30, 10));
        assert_eq!(
            editor.document.command(editor.document.len() - 2),
            Some(&IgsCommand::ColorSet { pen: PenType::Line, color: 2 })
        );
    }

    #[test]
    fn dragged_boxes_render_filled_on_the_medium_resolution_canvas() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Medium);
        editor.select_tool(Tool::Rectangle);
        editor.attributes.fill_color = 2;
        run(&context, &mut editor, vec![]);
        let rect = editor.canvas_rect.unwrap();
        // Medium resolution pixels are shown twice as tall as wide.
        assert!((rect.height() / rect.width() - 400.0 / 640.0).abs() < 0.01, "{rect:?}");
        drag(&context, &mut editor, (100, 50), (300, 150));
        let Some(IgsCommand::Box {
            x1,
            y1,
            x2,
            y2,
            rounded: false,
        }) = editor.document.command(editor.document.len() - 1).cloned()
        else {
            panic!("expected a box: {:?}", commands(&editor))
        };
        assert_eq!((x1, y1, x2, y2), (100.into(), 50.into(), 300.into(), 150.into()));
        let preview = editor.document.preview().unwrap();
        let register = palette::pens(TerminalResolution::Medium)[2];
        assert_eq!(preview.pixel_index(200, 100), Some(register));
        assert_eq!(preview.pixel_index(50, 20), Some(0));
    }

    #[test]
    fn shapes_are_selected_moved_nudged_and_deleted_on_the_canvas() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        editor.select_tool(Tool::FilledRectangle);
        editor.add_shape((50, 50), (100, 90));
        let index = editor.document.len() - 1;
        editor.select_tool(Tool::Select);
        editor.select_item(None);
        run(&context, &mut editor, vec![]);
        click(&context, &mut editor, (70, 70));
        assert_eq!(editor.selected, Some(index));
        drag(&context, &mut editor, (70, 70), (80, 60));
        assert_eq!(
            editor.document.command(index),
            Some(&IgsCommand::FilledRectangle {
                x1: 60.into(),
                y1: 40.into(),
                x2: 110.into(),
                y2: 80.into()
            })
        );
        editor.nudge_selected(-10, 5);
        assert!(matches!(
            editor.document.command(index),
            Some(IgsCommand::FilledRectangle {
                x1: IgsParameter::Value(50),
                y1: IgsParameter::Value(45),
                ..
            })
        ));
        let length = editor.document.len();
        editor.delete_selected();
        assert_eq!(editor.document.len(), length - 1);
        assert!(editor.selected.is_none());
    }

    #[test]
    fn text_is_typed_on_the_canvas_and_edited_again() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        editor.select_tool(Tool::Text);
        editor.attributes.text_color = 3;
        run(&context, &mut editor, vec![]);
        click(&context, &mut editor, (40, 40));
        assert!(editor.text_edit.is_some());
        run(&context, &mut editor, vec![egui::Event::Text("Hi@ ST".into())]);
        run(
            &context,
            &mut editor,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let (_, _, top) = select::text_extent(9, 1);
        let index = editor.document.len() - 1;
        assert_eq!(
            editor.document.command(index),
            Some(&IgsCommand::WriteText {
                x: 40.into(),
                y: (40 + top).into(),
                text: b"Hi ST".to_vec()
            })
        );
        assert!(commands(&editor).contains(&IgsCommand::ColorSet { pen: PenType::Text, color: 3 }));
        let preview = editor.document.preview().unwrap();
        assert!((40..60).any(|x| (40..48).any(|y| preview.pixel_index(x, y) != Some(0))));

        click(&context, &mut editor, (42, 42));
        assert_eq!(editor.text_edit.as_ref().and_then(|edit| edit.index), Some(index));
        editor.text_edit.as_mut().unwrap().text = b"Bye".to_vec();
        editor.finish_text();
        assert!(matches!(editor.document.command(index), Some(IgsCommand::WriteText { text, .. }) if text == b"Bye"));
    }

    #[test]
    fn loops_are_edited_as_source() {
        let mut editor = IgsEditor::from_document(IgsDocument::from_bytes(b"G#&>0,10,2,0,L,4,x,0,x,50:\r\n").unwrap());
        assert!(matches!(editor.document.command(0), Some(IgsCommand::Loop(_))));
        let source = properties::escape(&editor.document.item_source(0).unwrap());
        editor.apply_source(0, &source.replace("0,10,2", "0,20,5"));
        assert!(editor.error.is_none(), "{:?}", editor.error);
        let Some(IgsCommand::Loop(data)) = editor.document.command(0) else {
            panic!("expected a loop")
        };
        assert_eq!((data.to, data.step), (20, 5));
        assert_eq!(editor.document.to_bytes().unwrap(), b"G#&>0,20,5,0,L,4,x,0,x,50:\r\n");
        editor.apply_source(0, "G#~:");
        assert!(editor.error.is_some());
        let context = egui::Context::default();
        editor.selected = Some(0);
        run(&context, &mut editor, vec![]);
    }

    #[test]
    fn the_palette_dialog_previews_and_adds_pen_colors() {
        let mut editor = IgsEditor::new(TerminalResolution::Medium);
        let context = egui::Context::default();
        run(&context, &mut editor, vec![]);
        editor.open_palette_dialog();
        let dialog = editor.palette_dialog.as_mut().unwrap();
        assert!(dialog.commands().is_empty());
        dialog.set_levels(1, [7, 0, 0]);
        let changed = vec![IgsCommand::SetPenColor {
            pen: 1,
            red: 7,
            green: 0,
            blue: 0,
        }];
        assert_eq!(editor.preview_request(), PreviewRequest::With(changed.clone()));
        editor.refresh_preview(&context);
        assert_eq!(palette::pen_color(&editor.palette, TerminalResolution::Medium, 1), Color32::from_rgb(238, 0, 0));
        let before = editor.document.len();
        let dialog = editor.palette_dialog.take().unwrap();
        editor.add_commands(dialog.commands());
        assert_eq!(editor.document.len(), before + 1);
        assert_eq!(editor.document.command(before), changed.first());
    }

    #[test]
    fn a_shape_selected_on_the_canvas_scrolls_into_the_command_list() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        editor.select_tool(Tool::Line);
        for step in 0..120 {
            editor.add_shape((step, 10), (step + 20, 40));
        }
        editor.select_item(None);
        run(&context, &mut editor, vec![]);
        let last = editor.document.len() - 1;
        editor.select_item(Some(last));
        run(&context, &mut editor, vec![]);
        run(&context, &mut editor, vec![]);
        assert!(editor.visible_rows.contains(&last), "{:?}", editor.visible_rows);
    }

    #[test]
    fn selecting_commands_with_out_of_range_values_leaves_them_unchanged() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::from_document(IgsDocument::from_bytes(b"G#S>4,7,0,0:\r\nG#R>1,0:\r\nG#C>1,9:\r\n").unwrap());
        for index in [0, 2] {
            editor.selected = Some(index);
            run(&context, &mut editor, vec![]);
            run(&context, &mut editor, vec![]);
            assert!(!editor.modified(), "selecting item {index} changed {:?}", editor.document.command(index));
        }
    }

    #[test]
    fn a_typed_property_survives_a_following_canvas_drag() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        editor.select_tool(Tool::Circle);
        editor.add_shape((100, 100), (130, 100));
        let index = editor.document.len() - 1;
        editor.select_tool(Tool::Select);
        editor.select_item(Some(index));
        run(&context, &mut editor, vec![]);
        let Some((_, IgsCommand::Circle { radius, .. })) = editor.editing.as_mut() else {
            panic!("expected a circle draft")
        };
        *radius = IgsParameter::Value(20);
        // The draft is still in the panel when the drag starts.
        let (start, end) = (screen(&editor, (100, 100)), screen(&editor, (110, 110)));
        run(
            &context,
            &mut editor,
            vec![egui::Event::PointerMoved(start), button_event(start, egui::PointerButton::Primary, true)],
        );
        run(&context, &mut editor, vec![egui::Event::PointerMoved(start.lerp(end, 0.5))]);
        run(&context, &mut editor, vec![egui::Event::PointerMoved(end)]);
        run(&context, &mut editor, vec![button_event(end, egui::PointerButton::Primary, false)]);
        assert!(
            matches!(
                editor.document.command(index),
                Some(IgsCommand::Circle {
                    x: IgsParameter::Value(110),
                    y: IgsParameter::Value(110),
                    radius: IgsParameter::Value(20)
                })
            ),
            "{:?}",
            editor.document.command(index)
        );
    }

    #[test]
    fn property_edits_end_the_text_edit_of_the_same_text() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::from_document(IgsDocument::from_bytes(b"G#W>40,50,Old@\r\n").unwrap());
        editor.select_tool(Tool::Text);
        editor.begin_text((41, 45));
        assert_eq!(editor.text_edit.as_ref().and_then(|edit| edit.index), Some(0));
        run(&context, &mut editor, vec![]);
        let Some((_, IgsCommand::WriteText { text, .. })) = editor.editing.as_mut() else {
            panic!("expected a text draft")
        };
        *text = b"New".to_vec();
        run(&context, &mut editor, vec![]);
        assert!(editor.text_edit.is_none());
        editor.finish_pending();
        assert!(matches!(editor.document.command(0), Some(IgsCommand::WriteText { text, .. }) if text == b"New"));
    }

    #[test]
    fn undo_and_redo_drop_edits_that_hold_indices() {
        let mut editor = IgsEditor::from_document(IgsDocument::from_bytes(b"G#W>40,50,A@\r\nG#W>40,90,B@\r\n").unwrap());
        editor.selected = Some(0);
        editor.delete_selected();
        editor.undo(false);
        editor.select_tool(Tool::Text);
        editor.begin_text((41, 85));
        assert_eq!(editor.text_edit.as_ref().and_then(|edit| edit.index), Some(1));
        editor.undo(true);
        assert!(editor.text_edit.is_none() && editor.shape_drag.is_none());
        editor.finish_pending();
        assert!(matches!(editor.document.command(0), Some(IgsCommand::WriteText { text, .. }) if text == b"B"));
    }

    #[test]
    fn property_edits_apply_to_the_document() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        editor.select_tool(Tool::Circle);
        editor.add_shape((100, 100), (130, 100));
        let index = editor.document.len() - 1;
        run(&context, &mut editor, vec![]);
        let Some((_, IgsCommand::Circle { radius, .. })) = editor.editing.as_mut() else {
            panic!("expected a circle draft")
        };
        *radius = IgsParameter::Value(12);
        run(&context, &mut editor, vec![]);
        assert!(matches!(
            editor.document.command(index),
            Some(IgsCommand::Circle {
                radius: IgsParameter::Value(12),
                ..
            })
        ));
        editor.undo(false);
        assert!(matches!(editor.document.command(index), Some(IgsCommand::Circle { radius: IgsParameter::Value(radius), .. }) if *radius != 12));
    }
}
