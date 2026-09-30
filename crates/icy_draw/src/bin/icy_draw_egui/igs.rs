//! The IGS (Atari ST Instant Graphics and Sound) editor: a command list, a property panel and
//! a canvas rendered with the IGS engine, with drawing tools for the VDI shapes.

use eframe::egui::{self, Color32, Stroke};
use icy_draw::{
    fl,
    igs_document::{IgsDocument, IgsDrawState},
};
use icy_engine::Screen;
use icy_parser_core::{
    ArrowEnd, BlitMode, BlitOperation, DrawingMode, IgsCommand, IgsItem, IgsParameter, LineKind, LineMarkerStyle, PatternType, PenType, PolymarkerKind,
    TerminalResolution, TextEffects, TextRotation,
};
use std::path::Path;

use super::playback::{Action, RowMark, Timeline, Transport};
use super::widgets::{self, Icons};

#[path = "igs_line.rs"]
mod line;
#[path = "igs_marker.rs"]
mod marker;
#[path = "igs_palette.rs"]
mod palette;
use palette::{PaletteDialog, PaletteResult};
#[path = "igs_pattern.rs"]
mod pattern;
use pattern::{PatternDialog, PatternResult};
#[path = "igs_properties.rs"]
mod properties;
#[path = "igs_select.rs"]
mod select;
use select::{Canvas, Geometry, Handle, Point};

const TOOLBAR_HEIGHT: f32 = 44.0;
/// The tool sidebar fits three 38 point tool buttons; longer labels widen it up to the maximum.
const SIDEBAR_WIDTH: f32 = 148.0;
const SIDEBAR_MAX_WIDTH: f32 = 320.0;
const SIDEBAR_MARGIN: f32 = 6.0;
const PEN_SWATCH: egui::Vec2 = egui::vec2(30.0, 24.0);
/// The smallest width of a tool button in the sidebar.
const TOOL_BUTTON: f32 = 38.0;
/// Screen distance in points within which a handle is picked up.
const HANDLE_RADIUS: f32 = 8.0;
/// How opaque the marker preview under the pointer is.
const MARKER_PREVIEW_OPACITY: f32 = 0.55;
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
    Spray,
    CopyArea,
    Zone,
}

impl Tool {
    const ALL: [Self; 19] = [
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
        Self::Spray,
        Self::CopyArea,
        Self::Zone,
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
            Self::Spray => fl!("igs-tool-spray"),
            Self::CopyArea => fl!("igs-tool-copy-area"),
            Self::Zone => fl!("igs-tool-zone"),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Select => "cursor",
            Self::Marker => "polymarker",
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
            Self::Spray => "spray",
            Self::CopyArea => "select",
            Self::Zone => "rip_mouse",
        }
    }

    /// The pen the shape is drawn with.
    fn pen(self) -> Option<PenType> {
        Some(match self {
            Self::Select | Self::CopyArea | Self::Zone => return None,
            Self::Marker | Self::Spray => PenType::Polymarker,
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
            Self::Select | Self::Marker | Self::FloodFill | Self::Text | Self::PolyLine | Self::Polygon | Self::CopyArea | Self::Zone
        )
    }

    /// How holding Shift constrains the end point of the shape.
    fn constraint(self) -> Option<Constraint> {
        match self {
            Self::Line | Self::PolyLine | Self::Polygon => Some(Constraint::Angle),
            Self::Rectangle | Self::RoundedRectangle | Self::FilledRectangle | Self::Ellipse | Self::EllipticalArc | Self::EllipticalPieSlice => {
                Some(Constraint::Square)
            }
            _ => None,
        }
    }

    /// The shape command from `from` to `to`, without the attribute commands before it.
    fn command(self, canvas: &Canvas, from: Point, to: Point, attributes: &Attributes) -> Option<IgsCommand> {
        let (start_angle, end_angle) = (attributes.start_angle, attributes.end_angle);
        let v = IgsParameter::Value;
        let ((x0, y0), (x1, y1)) = (from, to);
        let (left, top, right, bottom) = (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
        let (dx, dy) = (x1 - x0, y1 - y0);
        let radius = (dx as f32).hypot(dy as f32).round() as i32;
        Some(match self {
            Self::Select | Self::Text | Self::PolyLine | Self::Polygon | Self::CopyArea | Self::Zone => return None,
            Self::Marker => IgsCommand::PolymarkerPlot { x: v(x1), y: v(y1) },
            Self::FloodFill => IgsCommand::FloodFill { x: v(x1), y: v(y1) },
            // IG sprays at most 255 pixels in each direction.
            Self::Spray => IgsCommand::SprayPaint {
                x: v(left),
                y: v(top),
                width: v((right - left).min(255)),
                height: v((bottom - top).min(255)),
                density: v(attributes.spray_density),
            },
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

/// What holding Shift does to the end of a shape, measured on screen so that medium
/// resolution's tall pixels still give 45° lines and round circles.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Constraint {
    /// Lines in steps of 45°.
    Angle,
    /// Squares and circles.
    Square,
}

/// How much taller than wide a pixel of `resolution` is shown.
fn pixel_aspect(resolution: TerminalResolution) -> f32 {
    if resolution == TerminalResolution::Medium {
        2.0
    } else {
        1.0
    }
}

/// `to` moved so the shape from `from` follows `constraint`, kept on the canvas.
fn constrain(constraint: Constraint, canvas: &Canvas, from: Point, to: Point) -> Point {
    let aspect = pixel_aspect(canvas.resolution);
    let (dx, dy) = ((to.0 - from.0) as f32, (to.1 - from.1) as f32 * aspect);
    let (x, y) = match constraint {
        Constraint::Angle => {
            let step = std::f32::consts::FRAC_PI_4;
            let angle = (dy.atan2(dx) / step).round() * step;
            let length = dx.hypot(dy);
            (angle.cos() * length, angle.sin() * length)
        }
        Constraint::Square => {
            let side = dx.abs().max(dy.abs());
            (side.copysign(dx), side.copysign(dy))
        }
    };
    (
        (from.0 + x.round() as i32).clamp(0, canvas.width - 1),
        (from.1 + (y / aspect).round() as i32).clamp(0, canvas.height - 1),
    )
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
        IgsCommand::SprayPaint { width, height, density, .. } if !is_spray_rotation(width, height, density) => Tool::Spray,
        IgsCommand::GrabScreen {
            operation: BlitOperation::ScreenToScreen { .. },
            ..
        } => Tool::CopyArea,
        IgsCommand::DefineZone { zone_id, .. } if !(9997..=9999).contains(zone_id) => Tool::Zone,
        _ => return None,
    })
}

fn is_zone(command: &IgsCommand) -> bool {
    shape_tool(command) == Some(Tool::Zone)
}

/// `X 0,pen,0,0,0,0` switches spray paint color rotation instead of spraying.
fn is_spray_rotation(width: &IgsParameter, height: &IgsParameter, density: &IgsParameter) -> bool {
    [width, height, density].iter().all(|parameter| **parameter == IgsParameter::Value(0))
}

fn blit_mode_name(mode: BlitMode) -> String {
    match mode {
        BlitMode::Replace => fl!("igs-mode-replace"),
        BlitMode::Transparent => fl!("igs-mode-transparent"),
        BlitMode::Xor => fl!("igs-mode-xor"),
        BlitMode::ReverseTransparent => fl!("igs-mode-reverse-transparent"),
        BlitMode::And => fl!("igs-blit-and"),
        BlitMode::NotS => fl!("igs-blit-invert"),
        BlitMode::Clear => fl!("igs-blit-clear"),
        BlitMode::AndNot => fl!("igs-blit-and-not"),
        BlitMode::Erase => fl!("igs-blit-erase"),
        BlitMode::Unchanged => fl!("igs-blit-unchanged"),
        BlitMode::NotOr => fl!("igs-blit-nor"),
        BlitMode::NotXor => fl!("igs-blit-xnor"),
        BlitMode::NotD => fl!("igs-blit-invert-destination"),
        BlitMode::OrNot => fl!("igs-blit-or-not"),
        BlitMode::NotAnd => fl!("igs-blit-nand"),
        BlitMode::Fill => fl!("igs-blit-fill"),
    }
}

fn sound_name(effect: usize) -> String {
    icy_engine_gui::music::sound_effects::sound_name(effect).map_or_else(|| effect.to_string(), str::to_owned)
}

/// The GIST sound table of 20 effects, changed by `b>20`, restored by `b>22`, and how often
/// the first five effects repeat (`b>23`).
#[derive(Clone, Debug, PartialEq)]
struct SoundTable {
    effects: Vec<Vec<i16>>,
    loops: u32,
}

impl SoundTable {
    fn new() -> Self {
        Self {
            effects: (0..20)
                .map(|index| icy_engine_gui::music::sound_effects::sound_data(index).map_or_else(|| vec![0; 56], |data| data.to_vec()))
                .collect(),
            loops: 1,
        }
    }

    /// The table in effect before item `index`.
    fn before(items: &[IgsItem], index: usize) -> Self {
        let mut table = Self::new();
        for command in items[..index.min(items.len())].iter().filter_map(IgsItem::command) {
            table.apply(command);
        }
        table
    }

    fn apply(&mut self, command: &IgsCommand) {
        match command {
            IgsCommand::AlterSoundEffect {
                sound_effect,
                element_num,
                negative_flag,
                thousands,
                hundreds,
                ..
            } => {
                let value = i32::from((*thousands).min(32)) * 1000 + i32::from(*hundreds);
                let value = if *negative_flag != 0 { -value } else { value };
                if let Some(word) = self
                    .effects
                    .get_mut((*sound_effect as usize).min(19))
                    .and_then(|effect| effect.get_mut(*element_num as usize))
                {
                    *word = value as i16;
                }
            }
            IgsCommand::RestoreSoundEffect { sound_effect } => {
                let index = (*sound_effect as usize).min(19);
                self.effects[index] = Self::new().effects[index].clone();
            }
            IgsCommand::SetEffectLoops { count } => self.loops = *count,
            _ => {}
        }
    }

    /// What `command` plays with this table.
    fn sounds(&self, command: &IgsCommand) -> Vec<Sound> {
        match command {
            IgsCommand::BellsAndWhistles { sound_effect } => {
                let index = (*sound_effect as usize).min(19);
                let repeats = if index <= 4 { self.loops.clamp(1, 16) } else { 1 };
                (0..repeats).map(|_| Sound::Gist(self.effects[index].clone())).collect()
            }
            IgsCommand::AlterSoundEffect { play: true, sound_effect, .. } => {
                let mut table = self.clone();
                table.apply(command);
                vec![Sound::Gist(table.effects[(*sound_effect as usize).min(19)].clone())]
            }
            IgsCommand::ChipMusic {
                sound_effect,
                voice,
                volume,
                pitch,
                ..
            } if *pitch > 0 => vec![Sound::Chip {
                data: self.effects[(*sound_effect as usize).min(19)].clone(),
                voice: *voice,
                volume: *volume,
                pitch: *pitch,
            }],
            IgsCommand::StopAllSound => vec![Sound::StopAll],
            _ => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Sound {
    Gist(Vec<i16>),
    Chip { data: Vec<i16>, voice: u8, volume: u8, pitch: u8 },
    StopAll,
}

/// The audio output, opened on the first sound.
#[derive(Default)]
struct SoundPlayer {
    #[cfg(not(test))]
    thread: Option<icy_engine_gui::music::SoundThread>,
    #[cfg(test)]
    played: Vec<Sound>,
}

impl SoundPlayer {
    fn play(&mut self, sounds: Vec<Sound>) {
        #[cfg(test)]
        self.played.extend(sounds);
        #[cfg(not(test))]
        for sound in sounds {
            let thread = self.thread.get_or_insert_with(icy_engine_gui::music::SoundThread::new);
            let _ = match sound {
                Sound::Gist(data) => thread.play_gist(data),
                Sound::Chip { data, voice, volume, pitch } => thread.play_chip_music(data, voice, volume, pitch),
                Sound::StopAll => thread.stop_snd_all(),
            };
        }
    }

    fn stop(&mut self) {
        #[cfg(test)]
        self.played.push(Sound::StopAll);
        #[cfg(not(test))]
        if let Some(thread) = &mut self.thread {
            let _ = thread.stop_snd_all();
        }
    }
}

/// How many timed steps a terminal shows `command` in: a loop with a delay pauses after
/// every iteration, everything else is drawn at once.
fn steps(command: &IgsCommand) -> usize {
    match command {
        IgsCommand::Loop(data) if data.delay > 0 && data.step != 0 && !data.params.is_empty() => ((data.to - data.from).abs() / data.step.abs() + 1) as usize,
        IgsCommand::RotateColorRegisters {
            start_reg,
            end_reg,
            count,
            delay,
        } if *count > 0 && *delay > 0 && start_reg != end_reg => (*count).min(9999) as usize,
        _ => 1,
    }
}

/// How long the terminal waits after each step of `command`.
fn delay_ms(command: &IgsCommand) -> u64 {
    match command {
        IgsCommand::Pause { pause_type } => pause_type.ms(),
        IgsCommand::ChipMusic { timing, .. } => (*timing).max(0) as u64 * 5,
        // The delay is in 1/200 seconds.
        IgsCommand::Loop(data) if data.delay > 0 => data.delay as u64 * 5,
        IgsCommand::RotateColorRegisters { delay, .. } => (*delay).clamp(0, 9999) as u64 * 5,
        _ => 0,
    }
}

/// The IGS items as a terminal receives and shows them.
struct IgsTimeline<'a>(&'a IgsDocument);

impl Timeline for IgsTimeline<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn transmitted_bytes(&self, index: usize) -> usize {
        self.0.item_source(index).map_or(0, |bytes| bytes.len())
    }

    fn steps(&self, index: usize) -> usize {
        self.0.command(index).map_or(1, steps)
    }

    fn step_delay(&self, index: usize) -> f64 {
        self.0.command(index).map_or(0, delay_ms) as f64 / 1000.0
    }
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
        IgsCommand::LoadFillPattern { .. } => fl!("igs-command-fill-pattern"),
        IgsCommand::BellsAndWhistles { .. } | IgsCommand::AlterSoundEffect { .. } => fl!("igs-command-sound"),
        IgsCommand::ChipMusic { .. } => fl!("igs-command-chip-music"),
        IgsCommand::StopAllSound => fl!("igs-command-stop-sound"),
        IgsCommand::DefineZone { .. } => fl!("igs-command-clear-zones"),
        IgsCommand::SprayPaint { .. } => fl!("igs-command-spray-rotation"),
        IgsCommand::RotateColorRegisters { .. } => fl!("igs-command-color-rotation"),
        IgsCommand::SetColorRegister { .. } => fl!("igs-command-color-register"),
        IgsCommand::LoadColorPalette { .. } => fl!("igs-command-color-registers"),
        IgsCommand::InputCommand { .. } => fl!("igs-command-input"),
        IgsCommand::Cursor { .. } => fl!("igs-cursor"),
        IgsCommand::InverseVideo { .. } => fl!("igs-inverse-video"),
        IgsCommand::SetTextColor { .. } => fl!("igs-template-text-color"),
        IgsCommand::LoadBitblitMemory { .. } => fl!("igs-command-blit-memory"),
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
        IgsCommand::LoadFillPattern { pattern, .. } => fl!("igs-pattern-slot-summary", slot = pattern),
        IgsCommand::BellsAndWhistles { sound_effect } | IgsCommand::AlterSoundEffect { sound_effect, .. } => sound_name(*sound_effect as usize),
        IgsCommand::ChipMusic {
            sound_effect, voice, pitch, ..
        } => format!("{} · {voice} · {pitch}", sound_name(*sound_effect as usize)),
        IgsCommand::DefineZone { zone_id, string, .. } if !(9997..=9999).contains(zone_id) => format!("{zone_id} · \"{}\"", latin1(string)),
        IgsCommand::GrabScreen {
            operation:
                BlitOperation::ScreenToScreen {
                    src_x1,
                    src_y1,
                    src_x2,
                    src_y2,
                    dest_x,
                    dest_y,
                },
            ..
        } => format!("{src_x1}, {src_y1} → {src_x2}, {src_y2} ⇒ {dest_x}, {dest_y}"),
        IgsCommand::SprayPaint { x, width, height, density, .. } if is_spray_rotation(width, height, density) => match x {
            IgsParameter::Value(0) => fl!("igs-off"),
            pen => fl!("igs-spray-rotation-summary", pen = pen.to_string()),
        },
        IgsCommand::SprayPaint { width, height, density, .. } => format!("{width} × {height} · {density}"),
        IgsCommand::RotateColorRegisters {
            start_reg, end_reg, count: 0, ..
        } => format!("{start_reg}–{end_reg} · {}", fl!("igs-rotate-reset")),
        IgsCommand::RotateColorRegisters {
            start_reg,
            end_reg,
            count,
            delay,
        } => format!("{start_reg} → {end_reg} · {count}× · {} ms", delay * 5),
        IgsCommand::SetColorRegister { register, value } => format!("{register} = {:03X}", value),
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
        IgsCommand::LoadFillPattern { .. } => "fill",
        IgsCommand::BellsAndWhistles { .. } | IgsCommand::AlterSoundEffect { .. } | IgsCommand::ChipMusic { .. } | IgsCommand::StopAllSound => "play",
        IgsCommand::LineDrawTo { .. } => "line",
        IgsCommand::SprayPaint { .. } => "spray",
        IgsCommand::RotateColorRegisters { .. } => "repeat",
        IgsCommand::SetColorRegister { .. } | IgsCommand::LoadColorPalette { .. } => "dropper",
        IgsCommand::InputCommand { .. } => "rip_mouse",
        _ => return None,
    })
}

/// The color a state command sets, shown as a swatch in the list.
fn item_swatch(item: &IgsItem, palette: &icy_engine::Palette, resolution: TerminalResolution) -> Option<Color32> {
    match item.command()? {
        IgsCommand::ColorSet { color, .. } => Some(palette::pen_color(palette, resolution, *color)),
        IgsCommand::SetPenColor { red, green, blue, .. } => Some(Color32::from_rgb(red.min(&7) * 34, green.min(&7) * 34, blue.min(&7) * 34)),
        IgsCommand::SetColorRegister { value, .. } => {
            let channel = |shift: i32| ((value >> shift) & 7) as u8 * 34;
            Some(Color32::from_rgb(channel(8), channel(4), channel(0)))
        }
        _ => None,
    }
}

/// A named group of the Add menu with its commands as IGS source.
type TemplateGroup = (String, Vec<(String, &'static [u8])>);

/// Commands offered by the Add menu, as IGS source, grouped like the menu of IG's drawing
/// program.
fn templates() -> Vec<TemplateGroup> {
    vec![
        (
            fl!("igs-group-attributes"),
            vec![
                (fl!("igs-template-pen-color"), b"G#C>1,1:"),
                (fl!("igs-template-pen-palette"), b"G#S>1,7,0,0:"),
                (fl!("igs-template-drawing-mode"), b"G#M>1:"),
                (fl!("igs-template-line-style"), b"G#T>2,1,1:"),
                (fl!("igs-template-marker-style"), b"G#T>1,1,1:"),
                (fl!("igs-template-fill"), b"G#A>2,2,1:"),
                (fl!("igs-template-hollow"), b"G#H>1:"),
                (fl!("igs-template-text-effects"), b"G#E>0,9,0:"),
            ],
        ),
        (
            fl!("igs-group-drawing"),
            vec![
                (fl!("igs-template-draw-to-start"), b"G#X>10,0,0:"),
                (fl!("igs-template-draw-to"), b"G#D>100,100:"),
                (fl!("igs-template-random-range"), b"G#X>2,0,100:"),
                (fl!("igs-template-spray-rotation"), b"G#X>0,1,0,0,0,0:"),
            ],
        ),
        (
            fl!("igs-group-screen"),
            vec![
                (fl!("igs-template-clear"), b"G#s>4:"),
                (fl!("igs-template-initialize"), b"G#I>0:"),
                (fl!("igs-template-resolution"), b"G#R>0,2:"),
                (fl!("igs-template-wipe-blit"), b"G#X>11,0,0,0:"),
            ],
        ),
        (
            fl!("igs-group-colors"),
            vec![
                (fl!("igs-template-color-register"), b"G#X>1,4,1911:"),
                (fl!("igs-template-color-rotation"), b"G#X>8,1,15,20,10:"),
                (fl!("igs-template-color-rotation-reset"), b"G#X>8,1,15,0,0:"),
            ],
        ),
        (
            fl!("igs-group-flow"),
            vec![
                (fl!("igs-template-loop"), b"G#&>10,100,10,0,O,3,x,100,x:"),
                (fl!("igs-template-pause-seconds"), b"G#t>1:"),
                (fl!("igs-template-pause-vsync"), b"G#q>30:"),
            ],
        ),
        (
            fl!("igs-group-text"),
            vec![
                (fl!("igs-template-text"), b"Hello, Atari!"),
                (fl!("igs-template-text-color"), b"G#c>1,3:"),
                (fl!("igs-template-inverse-text"), b"G#v>1:"),
                (fl!("igs-template-cursor-off"), b"G#k>0:"),
                (fl!("igs-template-position-cursor"), b"G#p>0,0:"),
            ],
        ),
        (
            fl!("igs-group-sound"),
            vec![
                (fl!("igs-template-sound"), b"G#b>0:"),
                (fl!("igs-template-chip-music"), b"G#n>0,0,15,40,50,0:"),
                (fl!("igs-template-effect-loops"), b"G#b>23,2:"),
                (fl!("igs-template-stop-sound"), b"G#b>21:"),
            ],
        ),
        (
            fl!("igs-group-interaction"),
            vec![(fl!("igs-template-input"), b"G#<>1,0,1:"), (fl!("igs-template-clear-zones"), b"G#X>4,9999:")],
        ),
    ]
}

/// Drawing attributes in effect where new commands are inserted, with the VDI defaults for
/// those never set. The tool controls show these and change them with explicit commands.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Current {
    line_color: u8,
    fill_color: u8,
    text_color: u8,
    marker_color: u8,
    pattern: PatternType,
    border: bool,
    line_kind: LineKind,
    line_thickness: u8,
    /// The user line pattern user defined lines are drawn with.
    line_pattern: u8,
    line_ends: (ArrowEnd, ArrowEnd),
    marker: PolymarkerKind,
    marker_size: u8,
    drawing_mode: DrawingMode,
    text_effects: TextEffects,
    text_size: u8,
    text_rotation: TextRotation,
}

impl Current {
    fn new(state: &IgsDrawState) -> Self {
        // Pen 1 draws with the default foreground register in every resolution.
        // The value of a user defined line is its line pattern, not a width.
        let (line_kind, line_thickness, line_pattern) = match state.line {
            Some(LineMarkerStyle::LineThickness(LineKind::UserDefined, pattern)) => (LineKind::UserDefined, 1, pattern.max(1)),
            Some(LineMarkerStyle::LineThickness(kind, width)) => (kind, width, 1),
            _ => (LineKind::Solid, 1, 1),
        };
        let (marker, marker_size) = match state.marker {
            Some(LineMarkerStyle::PolyMarkerSize(kind, size)) => (kind, size),
            _ => (PolymarkerKind::Point, 1),
        };
        let (pattern, border) = state.fill.unwrap_or((PatternType::Solid, false));
        let (text_effects, text_size, text_rotation) = state.text.unwrap_or((TextEffects::NORMAL, 9, TextRotation::Degrees0));
        Self {
            line_color: state.line_color.unwrap_or(1),
            fill_color: state.fill_color.unwrap_or(1),
            text_color: state.text_color.unwrap_or(1),
            marker_color: state.marker_color.unwrap_or(1),
            pattern,
            border,
            line_kind,
            line_thickness,
            line_pattern,
            line_ends: state.line_ends.unwrap_or((ArrowEnd::Square, ArrowEnd::Square)),
            marker,
            marker_size,
            drawing_mode: state.drawing_mode.unwrap_or(DrawingMode::Replace),
            text_effects,
            text_size,
            text_rotation,
        }
    }

    /// The third value of `T 2,kind,n`: IG only draws solid lines wide, and user defined lines
    /// take the number of their line pattern.
    fn line_value(&self) -> u8 {
        match self.line_kind {
            LineKind::Solid => self.line_thickness,
            LineKind::UserDefined => self.line_pattern,
            _ => 1,
        }
    }

    fn line_style(&self) -> line::LineStyle {
        line::LineStyle {
            kind: self.line_kind,
            width: self.line_thickness,
            pattern: self.line_pattern,
            ends: self.line_ends,
        }
    }

    fn set_line_style(&mut self, style: line::LineStyle) {
        self.line_kind = style.kind;
        self.line_thickness = style.width;
        self.line_pattern = style.pattern;
        self.line_ends = style.ends;
    }

    fn pen(&self, pen: PenType) -> u8 {
        match pen {
            PenType::Line => self.line_color,
            PenType::Fill => self.fill_color,
            PenType::Text => self.text_color,
            PenType::Polymarker => self.marker_color,
        }
    }

    fn pen_mut(&mut self, pen: PenType) -> &mut u8 {
        match pen {
            PenType::Line => &mut self.line_color,
            PenType::Fill => &mut self.fill_color,
            PenType::Text => &mut self.text_color,
            PenType::Polymarker => &mut self.marker_color,
        }
    }

    /// The commands that turn these attributes into `wanted`, one per attribute that differs.
    fn changes(&self, wanted: &Current) -> Vec<IgsCommand> {
        let mut commands = Vec::new();
        for pen in [PenType::Line, PenType::Fill, PenType::Text, PenType::Polymarker] {
            if self.pen(pen) != wanted.pen(pen) {
                commands.push(IgsCommand::ColorSet { pen, color: wanted.pen(pen) });
            }
        }
        if (self.pattern, self.border) != (wanted.pattern, wanted.border) {
            commands.push(IgsCommand::AttributeForFills {
                pattern_type: wanted.pattern,
                border: wanted.border,
            });
        }
        if (self.line_kind, self.line_value()) != (wanted.line_kind, wanted.line_value()) {
            commands.push(IgsCommand::SetLineOrMarkerStyle {
                style: LineMarkerStyle::LineThickness(wanted.line_kind, wanted.line_value()),
            });
        }
        if self.line_ends != wanted.line_ends {
            let (left, right) = wanted.line_ends;
            commands.push(IgsCommand::SetLineOrMarkerStyle {
                style: LineMarkerStyle::LineEndpoints(wanted.line_kind, left, right),
            });
        }
        if (self.marker, self.marker_size) != (wanted.marker, wanted.marker_size) {
            commands.push(IgsCommand::SetLineOrMarkerStyle {
                style: LineMarkerStyle::PolyMarkerSize(wanted.marker, wanted.marker_size),
            });
        }
        if self.drawing_mode != wanted.drawing_mode {
            commands.push(IgsCommand::DrawingMode { mode: wanted.drawing_mode });
        }
        if (self.text_effects, self.text_size, self.text_rotation) != (wanted.text_effects, wanted.text_size, wanted.text_rotation) {
            commands.push(IgsCommand::TextEffects {
                effects: wanted.text_effects,
                size: wanted.text_size,
                rotation: wanted.text_rotation,
            });
        }
        commands
    }
}

/// Whether `a` and `b` set the same attribute, so the later one makes the earlier redundant.
fn same_attribute(a: &IgsCommand, b: &IgsCommand) -> bool {
    match (a, b) {
        (IgsCommand::ColorSet { pen: first, .. }, IgsCommand::ColorSet { pen: second, .. }) => first == second,
        (IgsCommand::SetLineOrMarkerStyle { style: first }, IgsCommand::SetLineOrMarkerStyle { style: second }) => {
            std::mem::discriminant(first) == std::mem::discriminant(second)
        }
        (IgsCommand::AttributeForFills { .. }, IgsCommand::AttributeForFills { .. })
        | (IgsCommand::DrawingMode { .. }, IgsCommand::DrawingMode { .. })
        | (IgsCommand::TextEffects { .. }, IgsCommand::TextEffects { .. }) => true,
        _ => false,
    }
}

/// The columns and square button size of the tool grid that fill `width`: as many columns of
/// buttons about [`TOOL_BUTTON`] points wide as fit, stretched to the full width.
fn tool_grid(width: f32, spacing: f32) -> (usize, f32) {
    let columns = (((width + spacing) / (TOOL_BUTTON + spacing)).floor() as usize).max(3);
    let size = ((width - spacing * (columns - 1) as f32) / columns as f32).floor();
    (columns, size)
}

/// Parameters of new shapes that are not IGS state: arc angles, spray density, the copy mode
/// and the host string of mouse zones.
#[derive(Clone, Debug, PartialEq)]
struct Attributes {
    start_angle: i32,
    end_angle: i32,
    /// Markers sprayed by the spray tool.
    spray_density: i32,
    blit_mode: BlitMode,
    /// The host string of new mouse zones.
    zone_host: String,
}

impl Default for Attributes {
    fn default() -> Self {
        Self {
            start_angle: 0,
            end_angle: 90,
            spray_density: 200,
            blit_mode: BlitMode::Replace,
            zone_host: String::new(),
        }
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

/// What the canvas shows: the document with one command replaced and `extra` inserted at
/// `at`, cut after the item at `through`.
#[derive(Clone, Debug, PartialEq)]
struct PreviewRequest {
    through: Option<usize>,
    replace: Option<(usize, IgsCommand)>,
    at: usize,
    extra: Vec<IgsCommand>,
    /// The steps of the last shown item drawn so far, e.g. iterations of a playing loop.
    steps: Option<usize>,
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
    pattern_dialog: Option<PatternDialog>,
    /// The area the copy tool copies, until the copy is placed.
    copy_source: Option<(Point, Point)>,
    transport: Transport,
    /// The GIST sound table the running animation plays with.
    sound_table: SoundTable,
    sound: SoundPlayer,
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
    /// Attribute changes from the tool controls not added as commands yet.
    attribute_draft: Option<Current>,
    /// The user pattern slot the fill pattern picker asked to draw.
    pattern_request: Option<u8>,
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
            pattern_dialog: None,
            copy_source: None,
            transport: Transport::default(),
            sound_table: SoundTable::new(),
            sound: SoundPlayer::default(),
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
            attribute_draft: None,
            pattern_request: None,
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
        if self.animating() {
            self.stop_playback();
        }
        self.attribute_draft = None;
        // These hold item indices that undo and redo may shift.
        self.shape_drag = None;
        self.drag = None;
        self.pattern_dialog = None;
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
        self.copy_source = None;
        self.tool = tool;
        self.drag = None;
        self.shape_drag = None;
    }

    /// The command and geometry of a shape, including a drag in progress.
    fn shape(&self, index: usize) -> Option<(IgsCommand, Geometry)> {
        let command = match &self.shape_drag {
            Some(drag) if drag.index == index => drag.current.clone(),
            _ => self.document.command(index)?.clone(),
        };
        // Mouse zones are only shown and picked with their tool, and only them.
        if is_zone(&command) != (self.tool == Tool::Zone) {
            return None;
        }
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

    /// Where new commands go: after the selection while previewing through it, otherwise at
    /// the end.
    fn insertion_index(&self) -> usize {
        let len = self.document.len();
        match self.selected.filter(|_| self.preview_to_selection) {
            Some(index) => (index + 1).min(len),
            None => len,
        }
    }

    /// The drawing attributes where new commands are inserted.
    fn current(&self) -> Current {
        Current::new(&self.document.state_before(self.insertion_index()))
    }

    /// Changes drawing attributes with explicit commands where new commands are inserted. A
    /// command for the same attribute right before that point is changed instead of adding
    /// another one, so trying out colors leaves one command.
    fn set_attributes(&mut self, wanted: Current) {
        let changes = self.current().changes(&wanted);
        if changes.is_empty() {
            return;
        }
        self.finish_pending();
        self.commit_properties();
        for command in changes {
            let index = self.insertion_index();
            let previous = index
                .checked_sub(1)
                .filter(|previous| self.document.command(*previous).is_some_and(|existing| same_attribute(existing, &command)));
            match previous {
                Some(previous) => {
                    self.replace_command(previous, command);
                    self.selected = Some(previous);
                }
                None => self.add_commands(vec![command]),
            }
        }
    }

    /// Keeps attribute changes from the tool controls until the pointer is released and no field
    /// is being typed in, so dragging a value adds one command.
    fn draft_attributes(&mut self, wanted: Current) {
        self.attribute_draft = (wanted != self.current()).then_some(wanted);
    }

    fn commit_attribute_draft(&mut self, context: &egui::Context) {
        let busy = context.input(|input| input.pointer.any_down()) || context.wants_keyboard_input();
        if !busy {
            if let Some(wanted) = self.attribute_draft.take() {
                self.set_attributes(wanted);
            }
        }
    }

    /// The user patterns where new commands go, as fills there use them.
    fn user_patterns(&self) -> [Vec<u16>; 8] {
        pattern::user_patterns(self.document.items()[..self.insertion_index()].iter().filter_map(IgsItem::command))
    }

    /// Swatch colors: the fill pen on the background register.
    fn fill_colors(&self, fill: u8) -> (Color32, Color32) {
        let resolution = self.canvas.resolution;
        (
            palette::pen_color(&self.palette, resolution, fill),
            palette::pen_color(&self.palette, resolution, 0),
        )
    }

    fn apply_picker(&mut self, wanted: &mut Current, change: pattern::PickerChange) {
        if let Some(pattern) = change.pattern {
            wanted.pattern = pattern;
        }
        if let Some(border) = change.border {
            wanted.border = border;
        }
        if let Some(slot) = change.edit {
            self.pattern_request = Some(slot);
        }
    }

    fn apply_marker(wanted: &mut Current, change: marker::MarkerChange) {
        if let Some(kind) = change.kind {
            wanted.marker = kind;
        }
        if let Some(size) = change.size {
            wanted.marker_size = size;
        }
    }

    fn shape_commands(&self, from: Point, to: Point) -> Vec<IgsCommand> {
        self.tool.command(&self.canvas, from, to, &self.attributes).into_iter().collect()
    }

    fn text_commands(&self, edit: &TextEdit) -> Vec<IgsCommand> {
        vec![edit.command()]
    }

    /// Inserts commands as one undo step and selects the shape among them, or the last one.
    fn add_commands(&mut self, commands: Vec<IgsCommand>) {
        if commands.is_empty() {
            return;
        }
        self.commit_properties();
        let index = self.insertion_index();
        let offset = commands.iter().rposition(|command| shape_tool(command).is_some()).unwrap_or(commands.len() - 1);
        let result = self.document.insert_many(index, commands);
        if self.set_error(result) {
            self.selected = Some(index + offset);
            self.editing = None;
            self.source = None;
        }
    }

    /// Inserts IGS source, e.g. pasted text or a template, and selects the last inserted item.
    fn insert_source(&mut self, source: &[u8]) {
        self.finish_pending();
        self.commit_properties();
        let index = self.insertion_index();
        match self.document.insert_source(index, source) {
            Ok(count) => {
                self.error = None;
                self.selected = Some(index + count - 1);
                self.editing = None;
                self.source = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    /// The selected item as escaped IGS source, for the clipboard.
    fn copy_selected(&self) -> Option<String> {
        let index = self.selected.filter(|index| *index < self.document.len())?;
        self.document.range_source(index..index + 1).ok().map(|bytes| properties::escape(&bytes))
    }

    fn paste(&mut self, text: &str) {
        match properties::unescape(text) {
            Some(bytes) => self.insert_source(&bytes),
            None => self.error = Some(fl!("igs-editor-invalid-escape")),
        }
    }

    pub fn duplicate_selected(&mut self) {
        let Some(index) = self.selected.filter(|index| *index < self.document.len()) else {
            return;
        };
        self.finish_pending();
        self.commit_properties();
        let result = self
            .document
            .range_source(index..index + 1)
            .and_then(|bytes| self.document.insert_source(index + 1, &bytes));
        match result {
            Ok(count) => {
                self.error = None;
                self.selected = Some(index + count);
                self.editing = None;
                self.source = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn has_selection(&self) -> bool {
        self.selected.is_some_and(|index| index < self.document.len())
    }

    pub fn delete_selection(&mut self) {
        self.delete_selected();
    }

    /// Selects the resolution the drawing starts in: the first `R` command, or a new one.
    fn set_resolution(&mut self, resolution: TerminalResolution) {
        self.finish_pending();
        self.commit_properties();
        let first = self
            .document
            .items()
            .iter()
            .position(|item| matches!(item.command(), Some(IgsCommand::SetResolution { .. })));
        let inserted = first.is_none();
        let result = match first.and_then(|index| Some((index, self.document.command(index)?.clone()))) {
            Some((index, IgsCommand::SetResolution { palette, .. })) => self.document.replace(index, IgsCommand::SetResolution { resolution, palette }),
            _ => self.document.insert(
                0,
                IgsCommand::SetResolution {
                    resolution,
                    palette: icy_parser_core::PaletteMode::IgDefault,
                },
            ),
        };
        if self.set_error(result) {
            if inserted {
                self.selected = self.selected.map(|index| index + 1);
                self.listed_selection = self.listed_selection.map(|index| index + 1);
            }
            self.editing = None;
            self.source = None;
        }
    }

    /// The resolution the drawing starts in, the one the resolution box edits.
    fn start_resolution(&self) -> TerminalResolution {
        self.document
            .items()
            .iter()
            .find_map(|item| match item.command() {
                Some(IgsCommand::SetResolution { resolution, .. }) => Some(*resolution),
                _ => None,
            })
            .unwrap_or(icy_draw::igs_document::DEFAULT_RESOLUTION)
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
            let commands = vec![poly_command(self.tool, &self.poly)];
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
                let (_, _, top) = select::text_extent(self.current().text_size, 1);
                TextEdit {
                    at: (point.0, point.1 + top),
                    text: Vec::new(),
                    index: None,
                }
            }
        };
        // New text is inserted after the selection while previewing through it.
        if edit.index.is_some() || !self.preview_to_selection {
            self.select_item(edit.index);
        }
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
        if self.animating() {
            self.stop_playback();
            return;
        }
        if !self.poly.is_empty()
            || self.text_edit.take().is_some()
            || self.drag.take().is_some()
            || self.shape_drag.take().is_some()
            || self.copy_source.take().is_some()
        {
            self.poly.clear();
        } else {
            self.select_item(None);
        }
    }

    fn open_palette_dialog(&mut self) {
        self.finish_pending();
        self.palette_dialog = Some(PaletteDialog::new(&self.palette, self.canvas.resolution));
    }

    /// Edits the user fill pattern command at `target`, or a new one for the tool's pattern slot.
    fn open_pattern_dialog(&mut self, target: Option<usize>) {
        self.finish_pending();
        self.commit_properties();
        let dialog = match target.and_then(|index| Some((index, self.document.command(index)?.clone()))) {
            Some((index, IgsCommand::LoadFillPattern { pattern, data })) => {
                let mut rows = [0; 16];
                for (row, word) in rows.iter_mut().zip(&data) {
                    *row = *word;
                }
                PatternDialog::new(pattern, rows, Some(index))
            }
            _ => {
                let slot = match self.current().pattern {
                    PatternType::UserDefined(slot) => slot.min(7),
                    _ => 0,
                };
                PatternDialog::new(slot, self.pattern_before(slot), None)
            }
        };
        self.pattern_dialog = Some(dialog);
    }

    /// Draws the user pattern of `slot` for a new pattern command.
    fn open_user_pattern(&mut self, slot: u8) {
        self.finish_pending();
        self.commit_properties();
        self.pattern_dialog = Some(PatternDialog::new(slot, self.pattern_before(slot), None));
    }

    /// The user pattern of `slot` where new commands are inserted.
    fn pattern_before(&self, slot: u8) -> [u16; 16] {
        pattern::pattern_before(self.document.items()[..self.insertion_index()].iter().filter_map(IgsItem::command), slot)
    }

    fn apply_pattern(&mut self, dialog: PatternDialog) {
        match dialog.target {
            Some(index) if matches!(self.document.command(index), Some(IgsCommand::LoadFillPattern { .. })) => self.replace_command(index, dialog.command()),
            Some(_) => {}
            None => {
                // The new pattern is loaded and then selected for fills, both as commands.
                self.add_commands(vec![dialog.command()]);
                let mut wanted = self.current();
                wanted.pattern = PatternType::UserDefined(dialog.slot());
                self.set_attributes(wanted);
            }
        }
    }

    /// The copy of the selected area placed with its top left corner at `at`.
    fn copy_command(&self, at: Point) -> Option<IgsCommand> {
        let ((x0, y0), (x1, y1)) = self.copy_source?;
        let (width, height) = (x1 - x0, y1 - y0);
        Some(IgsCommand::GrabScreen {
            operation: BlitOperation::ScreenToScreen {
                src_x1: x0,
                src_y1: y0,
                src_x2: x1,
                src_y2: y1,
                dest_x: at.0.min(self.canvas.width - 1 - width).max(0),
                dest_y: at.1.min(self.canvas.height - 1 - height).max(0),
            },
            mode: self.attributes.blit_mode,
        })
    }

    /// The lowest zone number not used yet.
    fn next_zone_id(&self) -> i32 {
        let used: Vec<i32> = self
            .document
            .items()
            .iter()
            .filter_map(|item| match item.command() {
                Some(IgsCommand::DefineZone { zone_id, .. }) => Some(*zone_id),
                _ => None,
            })
            .collect();
        (0..9997).find(|id| !used.contains(id)).unwrap_or(0)
    }

    fn zone_command(&self, from: Point, to: Point) -> IgsCommand {
        let v = IgsParameter::Value;
        let string = self.attributes.zone_host.bytes().filter(|byte| (0x20..0x7F).contains(byte)).collect::<Vec<_>>();
        IgsCommand::DefineZone {
            zone_id: self.next_zone_id(),
            x1: v(from.0.min(to.0)),
            y1: v(from.1.min(to.1)),
            x2: v(from.0.max(to.0)),
            y2: v(from.1.max(to.1)),
            length: string.len() as u16,
            string,
        }
    }

    fn add_zone(&mut self, from: Point, to: Point) {
        if (from.0 - to.0).abs() < 2 || (from.1 - to.1).abs() < 2 {
            return;
        }
        if self.attributes.zone_host.trim().is_empty() {
            self.error = Some(fl!("igs-zone-host-required"));
            return;
        }
        let command = self.zone_command(from, to);
        self.add_commands(vec![command]);
    }

    /// Plays a sound command with the sound table in effect before it.
    fn play_sound(&mut self, index: usize) {
        let Some(command) = self.document.command(index) else {
            return;
        };
        let sounds = SoundTable::before(self.document.items(), index).sounds(command);
        self.sound.play(sounds);
    }

    /// Whether the animation plays or is paused at a frame; editing waits until it stops.
    fn animating(&self) -> bool {
        self.transport.animating()
    }

    /// The item the canvas shows the drawing through: the animation frame, or the selection
    /// while previewing through it.
    fn frame(&self) -> Option<usize> {
        self.transport.frame(self.selected.filter(|_| self.preview_to_selection), self.document.len())
    }

    /// Selects the animation frame whenever it moves, so the command list follows playback.
    fn sync_selection(&mut self) {
        let frame = self.frame();
        if let Some(frame) = self.transport.follow(frame).filter(|frame| self.selected != Some(*frame)) {
            self.selected = Some(frame);
            self.editing = None;
            self.source = None;
        }
    }

    /// Turns a paused or playing animation into a preview through its frame, where the drawing
    /// can be edited, or turns the preview off.
    fn toggle_preview(&mut self) {
        if self.animating() {
            let frame = self.transport.end_for_preview(self.document.len());
            self.sound.stop();
            self.select_item(frame);
            self.preview_to_selection = frame.is_some();
        } else {
            self.preview_to_selection = !self.preview_to_selection;
        }
    }

    /// Applies a transport action, with the sounds of the items it reaches.
    fn transport_action(&mut self, action: Action, now: f64) {
        if action != Action::Stop {
            self.finish_pending();
            self.commit_properties();
        }
        let preview = self.selected.filter(|_| self.preview_to_selection);
        let outcome = self.transport.apply(action, &IgsTimeline(&self.document), preview, now);
        if outcome.silence {
            self.sound.stop();
        }
        if outcome.stopped {
            self.preview_to_selection = false;
        }
        if let Some(index) = outcome.select {
            self.select_item(Some(index));
        }
        if let Some(index) = outcome.started {
            self.sound_table = SoundTable::before(self.document.items(), index);
            self.enter_item(index);
        }
        if let Some(command) = outcome.stepped.and_then(|index| Some((index, self.document.command(index)?))) {
            let (index, command) = command;
            self.sound.play(SoundTable::before(self.document.items(), index).sounds(command));
        }
        if let Some(index) = outcome.jumped {
            self.sound_table = SoundTable::before(self.document.items(), index + 1);
        }
    }

    /// Starts at the next item after the paused frame, or pauses at the current frame.
    #[cfg(test)]
    fn toggle_playback(&mut self, now: f64) {
        self.transport_action(Action::PlayPause, now);
    }

    /// Ends the animation and the preview through the selection, showing the whole drawing.
    fn stop_playback(&mut self) {
        self.transport_action(Action::Stop, 0.0);
    }

    /// Seeks one item without replaying the preceding animation or its sounds.
    #[cfg(test)]
    fn step_playback(&mut self, forward: bool) {
        self.transport_action(if forward { Action::Next } else { Action::Previous }, 0.0);
    }

    /// Jumps to `index`, e.g. from the position slider. Playback continues from there without
    /// replaying earlier sounds; while paused, the frame is shown.
    fn seek_playback(&mut self, index: usize, now: f64) {
        self.transport_action(Action::Seek(index), now);
    }

    /// Plays the sound of an item the animation reached.
    fn enter_item(&mut self, index: usize) {
        if let Some(command) = self.document.command(index).cloned() {
            self.sound.play(self.sound_table.sounds(&command));
            self.sound_table.apply(&command);
        }
    }

    /// Advances playback to the next item that waits, or ends it after the last item.
    fn advance_playback(&mut self, context: &egui::Context) {
        let now = context.input(|input| input.time);
        let mut entered = Vec::new();
        let wait = self.transport.advance(&IgsTimeline(&self.document), now, &mut |index| entered.push(index));
        for index in entered {
            self.enter_item(index);
        }
        if let Some(wait) = wait {
            context.request_repaint_after(std::time::Duration::from_secs_f64(wait));
        }
    }

    /// The unfinished shape: a drag in progress, a path or new text.
    fn pending_commands(&self) -> Vec<IgsCommand> {
        if let Some(dialog) = &self.palette_dialog {
            return dialog.commands();
        }
        if let Some(dialog) = self.pattern_dialog.as_ref().filter(|dialog| dialog.target.is_none()) {
            return vec![dialog.command()];
        }
        if self.tool == Tool::CopyArea {
            return self.hover.and_then(|at| self.copy_command(at)).into_iter().collect();
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
                return vec![poly_command(self.tool, &points)];
            }
        }
        match self.drag {
            Some((from, to)) if self.tool.is_dragged() && from != to => self.shape_commands(from, to),
            _ => Vec::new(),
        }
    }

    fn preview_request(&self) -> PreviewRequest {
        let len = self.document.len();
        if let Some(index) = self.transport.frame(None, len) {
            return PreviewRequest {
                through: Some(index),
                replace: None,
                at: len,
                extra: Vec::new(),
                steps: self.transport.steps_shown(),
            };
        }
        let through = self.selected.filter(|_| self.preview_to_selection);
        let pattern = self.pattern_dialog.as_ref().and_then(|dialog| Some((dialog.target?, dialog.command())));
        let drag = self
            .shape_drag
            .as_ref()
            .filter(|drag| drag.current != drag.command)
            .map(|drag| (drag.index, drag.current.clone()));
        let text = self.text_edit.as_ref().and_then(|edit| {
            let index = edit
                .index
                .filter(|index| matches!(self.document.command(*index), Some(IgsCommand::WriteText { .. })))?;
            Some((index, edit.command()))
        });
        let draft = self
            .editing
            .as_ref()
            .filter(|(index, draft)| self.document.command(*index).is_some_and(|command| command != draft))
            .cloned();
        PreviewRequest {
            through,
            replace: pattern.or(drag).or(text).or(draft),
            at: self.insertion_index(),
            extra: self.pending_commands(),
            steps: None,
        }
    }

    fn render(&self, request: &PreviewRequest) -> Result<icy_draw::igs_document::IgsPreview, icy_draw::igs_document::IgsDocumentError> {
        let mut items = self.document.items().to_vec();
        if let Some((index, command)) = &request.replace {
            if let Some(IgsItem::Command(item)) = items.get_mut(*index) {
                item.set_command(command.clone());
            }
        }
        let at = request.at.min(items.len());
        items.splice(at..at, request.extra.iter().cloned().map(IgsItem::from));
        if let Some(through) = request.through {
            let mut end = through + 1;
            if at <= end {
                end += request.extra.len();
            }
            items.truncate(end);
        }
        IgsDocument::render_steps(&items, request.steps.filter(|_| request.through.is_some()))
    }

    fn refresh_preview(&mut self, context: &egui::Context) {
        let request = self.preview_request();
        let key = (self.document.revision(), request);
        if self.shown.as_ref() == Some(&key) && self.texture.is_some() {
            return;
        }
        let preview = self.render(&key.1);
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
fn command_row(ui: &mut egui::Ui, icons: &mut Icons, index: usize, item: &IgsItem, selected: bool, mark: RowMark, swatch: Option<Color32>) -> egui::Response {
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
    mark.paint_background(ui, rect, selected || response.hovered());
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
    let text = mark.text(text, selected);
    let weak = if selected {
        text.gamma_multiply(0.7)
    } else {
        mark.text(visuals.weak_text_color(), false)
    };
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
    fn transport(&mut self, ui: &mut egui::Ui) {
        let frame = self.frame();
        let now = ui.input(|input| input.time);
        if let Some(action) = self.transport.ui(ui, &mut self.icons, frame, self.document.len(), &IgsTimeline(&self.document)) {
            self.transport_action(action, now);
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_space(8.0);
            ui.add(self.icons.image(ui, self.tool.icon(), 18.0).tint(ui.visuals().text_color()));
            ui.label(icy_engine_gui::egui::appearance::bold(ui, self.tool.label()));
            widgets::divider(ui);
            // The drawing attributes are those where new commands go; changing one adds a command.
            let mut wanted = self.attribute_draft.unwrap_or_else(|| self.current());
            if self.tool.uses_line_style() {
                let colors = self.fill_colors(wanted.line_color);
                if let Some(style) = line::picker(ui, "igs-tool-line", wanted.line_style(), &self.user_patterns(), colors, 140.0) {
                    wanted.set_line_style(style);
                }
            }
            if self.tool.uses_fill() {
                let user = self.user_patterns();
                let colors = self.fill_colors(wanted.fill_color);
                let change = pattern::picker(ui, "igs-tool-pattern", wanted.pattern, wanted.border, &user, colors, 64.0);
                self.apply_picker(&mut wanted, change);
                if self.tool != Tool::FloodFill {
                    ui.checkbox(&mut wanted.border, fl!("igs-fill-border"));
                }
            }
            if matches!(self.tool, Tool::Marker | Tool::Spray) {
                let colors = self.fill_colors(wanted.marker_color);
                let change = marker::picker(ui, "igs-tool-marker", wanted.marker, wanted.marker_size, colors, 110.0);
                Self::apply_marker(&mut wanted, change);
            }
            let attributes = &mut self.attributes;
            if self.tool == Tool::Spray {
                ui.add(
                    egui::DragValue::new(&mut attributes.spray_density)
                        .range(1..=9999)
                        .prefix(fl!("igs-spray-density-prefix")),
                )
                .on_hover_text(fl!("igs-spray-density"));
            }
            if self.tool.has_angles() {
                ui.label(fl!("igs-start-angle"));
                ui.add(egui::DragValue::new(&mut attributes.start_angle).range(0..=360).suffix("°"));
                ui.label(fl!("igs-end-angle"));
                ui.add(egui::DragValue::new(&mut attributes.end_angle).range(0..=360).suffix("°"));
            }
            match self.tool {
                Tool::Text => {
                    ui.add(egui::DragValue::new(&mut wanted.text_size).range(1..=40).prefix(fl!("igs-size-prefix")));
                    properties::text_effects(ui, &mut wanted.text_effects);
                    properties::rotation(ui, &mut wanted.text_rotation);
                    ui.weak(if self.text_edit.is_some() {
                        fl!("igs-text-typing-hint")
                    } else {
                        fl!("igs-text-hint")
                    });
                }
                Tool::PolyLine | Tool::Polygon => {
                    ui.weak(format!("{} · {}", fl!("igs-poly-hint"), fl!("igs-shift-angle-hint")));
                }
                Tool::CopyArea => {
                    properties::blit_mode(ui, "igs-tool-blit-mode", &mut attributes.blit_mode);
                    ui.weak(if self.copy_source.is_some() {
                        fl!("igs-copy-place-hint")
                    } else {
                        fl!("igs-copy-hint")
                    });
                }
                Tool::Zone => {
                    ui.add(
                        icy_engine_gui::egui::appearance::text_edit(&mut attributes.zone_host)
                            .hint_text(fl!("igs-zone-host"))
                            .desired_width(180.0),
                    )
                    .on_hover_text(fl!("igs-zone-host-tooltip"));
                    ui.weak(fl!("igs-zone-hint"));
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
                tool => match tool.constraint() {
                    Some(Constraint::Angle) => {
                        ui.weak(fl!("igs-shift-angle-hint"));
                    }
                    Some(Constraint::Square) => {
                        ui.weak(fl!("igs-shift-square-hint"));
                    }
                    None => {}
                },
            }
            self.draft_attributes(wanted);
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

    /// The sidebar width that fits its labels in the current language: at least the three tool
    /// buttons, at most [`SIDEBAR_MAX_WIDTH`], beyond which labels are truncated.
    fn sidebar_width(context: &egui::Context) -> f32 {
        let style = context.style();
        let spacing = &style.spacing;
        let text = |text: String, text_style: egui::TextStyle| {
            let font = text_style.resolve(&style);
            context.fonts_mut(|fonts| fonts.layout_no_wrap(text, font, Color32::WHITE).size().x)
        };
        let button = |label: String| text(label, egui::TextStyle::Button) + 2.0 * spacing.button_padding.x;
        let combo = |label: String| button(label) + spacing.icon_spacing + spacing.icon_width;
        let pen = |label: String| text(label, egui::TextStyle::Body) + spacing.item_spacing.x + PEN_SWATCH.x;
        let widths = [TerminalResolution::Low, TerminalResolution::Medium, TerminalResolution::High]
            .map(|resolution| combo(resolution_name(resolution)))
            .into_iter()
            .chain(properties::MODES.map(|mode| combo(drawing_mode_name(mode))))
            .chain([fl!("igs-pen-line"), fl!("igs-pen-fill"), fl!("igs-pen-text"), fl!("igs-pen-marker")].map(pen))
            .chain([fl!("igs-palette-edit")].map(button))
            .chain(
                [fl!("igs-drawing-mode"), fl!("igs-fill-pattern-label"), fl!("igs-marker"), fl!("igs-line-type")]
                    .map(|label| text(label, egui::TextStyle::Body)),
            );
        let content = widths.fold(0.0, f32::max);
        (content + 2.0 * SIDEBAR_MARGIN).ceil().clamp(SIDEBAR_WIDTH, SIDEBAR_MAX_WIDTH)
    }

    fn sidebar(&mut self, context: &egui::Context, blocked: bool) {
        let panel_fill = context.style().visuals.panel_fill;
        // Content wider than a side panel is clipped but still takes its width, which is left
        // unpainted, so the panel is sized to its labels and they truncate beyond that.
        egui::SidePanel::left("igs-tools")
            .exact_width(Self::sidebar_width(context))
            .resizable(false)
            .frame(egui::Frame::new().fill(panel_fill).inner_margin(egui::Margin::same(SIDEBAR_MARGIN as i8)))
            .show(context, |ui| {
                ui.add_enabled_ui(!blocked, |ui| {
                    let start = self.start_resolution();
                    let mut chosen = start;
                    egui::ComboBox::from_id_salt("igs-resolution")
                        .width(ui.available_width())
                        .truncate()
                        .selected_text(resolution_name(chosen))
                        .show_ui(ui, |ui| {
                            for resolution in [TerminalResolution::Low, TerminalResolution::Medium, TerminalResolution::High] {
                                ui.selectable_value(&mut chosen, resolution, resolution_name(resolution));
                            }
                        })
                        .response
                        .on_hover_text(fl!("igs-resolution-tooltip"));
                    if chosen != start {
                        self.set_resolution(chosen);
                    }
                    let resolution = self.canvas.resolution;
                    // The pens (C), fill (A) and drawing mode (M) where new commands go; changing
                    // one adds its command there.
                    let mut wanted = self.attribute_draft.unwrap_or_else(|| self.current());
                    for (id, label, pen) in [
                        ("line", fl!("igs-pen-line"), PenType::Line),
                        ("fill", fl!("igs-pen-fill"), PenType::Fill),
                        ("text", fl!("igs-pen-text"), PenType::Text),
                        ("marker", fl!("igs-pen-marker"), PenType::Polymarker),
                    ] {
                        // Out-of-range pens from a file are shown clamped but only change when picked.
                        let shown = wanted.pen(pen).min(palette::pen_count(resolution) - 1);
                        let mut picked = shown;
                        let row = egui::vec2(ui.available_width(), PEN_SWATCH.y);
                        ui.allocate_ui_with_layout(row, egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            palette::pen_picker(ui, id, &self.palette, resolution, &mut picked, PEN_SWATCH);
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.add(egui::Label::new(label).truncate());
                            });
                        });
                        if picked != shown {
                            *wanted.pen_mut(pen) = picked;
                        }
                    }
                    let user = self.user_patterns();
                    ui.add(egui::Label::new(fl!("igs-line-type")).truncate());
                    let colors = self.fill_colors(wanted.line_color);
                    if let Some(style) = line::picker(ui, "igs-sidebar-line", wanted.line_style(), &user, colors, ui.available_width()) {
                        wanted.set_line_style(style);
                    }
                    ui.add(egui::Label::new(fl!("igs-fill-pattern-label")).truncate());
                    let colors = self.fill_colors(wanted.fill_color);
                    let change = pattern::picker(ui, "igs-sidebar-pattern", wanted.pattern, wanted.border, &user, colors, ui.available_width());
                    self.apply_picker(&mut wanted, change);
                    ui.add(egui::Label::new(fl!("igs-marker")).truncate());
                    let colors = self.fill_colors(wanted.marker_color);
                    let change = marker::picker(ui, "igs-sidebar-marker", wanted.marker, wanted.marker_size, colors, ui.available_width());
                    Self::apply_marker(&mut wanted, change);
                    ui.add(egui::Label::new(fl!("igs-drawing-mode")).truncate());
                    properties::drawing_mode(ui, "igs-tool-mode", &mut wanted.drawing_mode);
                    self.draft_attributes(wanted);
                    let full = egui::vec2(ui.available_width(), 26.0);
                    if ui
                        .add(egui::Button::new(fl!("igs-palette-edit")).truncate().min_size(full))
                        .on_hover_text(fl!("igs-palette-edit-tooltip"))
                        .clicked()
                    {
                        self.open_palette_dialog();
                    }
                    ui.separator();
                    let (columns, size) = tool_grid(ui.available_width(), ui.spacing().item_spacing.x);
                    egui::Grid::new("igs-tools-grid")
                        .num_columns(columns)
                        .spacing(egui::Vec2::splat(ui.spacing().item_spacing.x))
                        .show(ui, |ui| {
                            for (index, tool) in Tool::ALL.into_iter().enumerate() {
                                if self.icons.button_sized(ui, tool.icon(), &tool.label(), self.tool == tool, size).clicked() {
                                    self.select_tool(tool);
                                }
                                if index % columns == columns - 1 {
                                    ui.end_row();
                                }
                            }
                        });
                });
            });
    }

    fn command_list(&mut self, context: &egui::Context, blocked: bool, editing_blocked: bool) {
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
                        let selected = self.selected.filter(|index| *index < count && !editing_blocked);
                        let mut action = None;
                        let mut template = None;
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
                        ui.add_enabled_ui(selected.is_some(), |ui| {
                            if self.icons.button_sized(ui, "file_copy", &fl!("igs-editor-duplicate"), false, 26.0).clicked() {
                                action = Some(2);
                            }
                        });
                        let add = ui
                            .add_enabled_ui(!editing_blocked, |ui| self.icons.button_sized(ui, "add", &fl!("igs-editor-add"), false, 26.0))
                            .inner;
                        egui::Popup::menu(&add).id(egui::Id::new("igs-add-command")).show(|ui| {
                            ui.set_min_width(220.0);
                            ui.weak(fl!("igs-editor-add-hint"));
                            for (group, commands) in templates() {
                                ui.menu_button(group, |ui| {
                                    for (name, source) in commands {
                                        if ui.button(name).clicked() {
                                            template = Some(source);
                                            ui.close();
                                        }
                                    }
                                });
                            }
                        });
                        widgets::divider(ui);
                        let through = self.preview_to_selection && !self.animating();
                        if self
                            .icons
                            .button_sized(ui, "visibility", &fl!("igs-editor-preview-through"), through, 26.0)
                            .clicked()
                        {
                            self.toggle_preview();
                        }
                        match action {
                            Some(0) => self.delete_selected(),
                            Some(2) => self.duplicate_selected(),
                            Some(delta) => self.move_selected(delta),
                            None => {}
                        }
                        if let Some(source) = template {
                            self.insert_source(source);
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
                let mut double_clicked = None;
                let frame = self.frame();
                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    scroll.show_rows(ui, COMMAND_ROW_HEIGHT, count, |ui, rows| {
                        self.visible_rows = rows.clone();
                        for index in rows {
                            let Some(item) = self.document.items().get(index) else {
                                break;
                            };
                            let swatch = item_swatch(item, &self.palette, self.canvas.resolution);
                            let response = command_row(
                                ui,
                                &mut self.icons,
                                index,
                                item,
                                self.selected == Some(index),
                                RowMark::new(index, frame),
                                swatch,
                            );
                            if response.double_clicked() {
                                double_clicked = Some(index);
                            } else if response.clicked() {
                                clicked = Some(index);
                            }
                        }
                    });
                });
                if let Some(index) = double_clicked {
                    // Double-clicking runs the animation up to the item.
                    self.seek_playback(index, ui.input(|input| input.time));
                    self.sync_selection();
                    self.select_item(Some(index));
                } else if let Some(index) = clicked.filter(|_| editing_blocked) {
                    self.select_item(Some(index));
                } else if let Some(index) = clicked {
                    self.finish_text();
                    // Mouse zones are edited with their own tool, other shapes with the select tool.
                    let command = self.document.command(index);
                    let zone = command.is_some_and(is_zone);
                    let shape = command.is_some_and(|command| shape_tool(command).is_some());
                    if zone && self.tool != Tool::Zone {
                        self.select_tool(Tool::Zone);
                    } else if !zone && shape && self.tool == Tool::Zone {
                        self.select_tool(Tool::Select);
                    }
                    self.select_item(Some(index));
                }
                self.listed_selection = self.selected;
                if self.selected != previous {
                    self.editing = None;
                    self.source = None;
                }
                ui.add_enabled_ui(!editing_blocked, |ui| self.property_panel(ui));
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
        let mut open_pattern = false;
        let mut play = false;
        let mut stop_sound = false;
        match item.command() {
            Some(IgsCommand::LoadFillPattern { .. }) => {
                open_pattern = ui
                    .add(egui::Button::new(fl!("igs-pattern-edit")).min_size(egui::vec2(ui.available_width(), 28.0)))
                    .clicked();
            }
            Some(command) if !SoundTable::new().sounds(command).is_empty() => {
                ui.horizontal(|ui| {
                    let size = egui::vec2((ui.available_width() - ui.spacing().item_spacing.x) / 2.0, 28.0);
                    play = ui.add(egui::Button::new(fl!("igs-sound-play")).min_size(size)).clicked();
                    stop_sound = ui.add(egui::Button::new(fl!("igs-sound-stop")).min_size(size)).clicked();
                });
            }
            _ => {}
        }
        if matches!(item.command(), Some(IgsCommand::SetResolution { .. }))
            && self.document.items()[..index]
                .iter()
                .filter_map(IgsItem::command)
                .any(|command| shape_tool(command).is_some())
        {
            ui.colored_label(ui.visuals().warn_fg_color, fl!("igs-resolution-mid-warning"));
        }
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
        if open_pattern {
            self.open_pattern_dialog(Some(index));
            return;
        }
        if play {
            self.play_sound(index);
        }
        if stop_sound {
            self.sound.stop();
        }
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
        self.advance_playback(context);
        self.sync_selection();
        let blocked = blocked || self.palette_dialog.is_some() || self.pattern_dialog.is_some();
        let editing_blocked = blocked || self.animating();
        let panel_fill = context.style().visuals.panel_fill;
        egui::TopBottomPanel::top("igs-toolbar")
            .exact_height(TOOLBAR_HEIGHT)
            .frame(egui::Frame::new().fill(panel_fill))
            .show(context, |ui| {
                if editing_blocked {
                    ui.disable();
                }
                self.toolbar(ui);
            });
        egui::TopBottomPanel::top("igs-transport")
            .exact_height(42.0)
            .frame(egui::Frame::new().fill(panel_fill))
            .show(context, |ui| {
                if blocked {
                    ui.disable();
                }
                self.transport(ui);
            });
        self.sync_selection();
        self.sidebar(context, editing_blocked);
        // The command list follows and seeks the animation; only its editing waits.
        self.command_list(context, blocked, editing_blocked);
        if !editing_blocked {
            self.commit_attribute_draft(context);
            if let Some(slot) = self.pattern_request.take() {
                self.open_user_pattern(slot);
            }
        }
        if !blocked && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.cancel();
        }
        if !editing_blocked {
            self.type_text(context);
            if self.text_edit.is_none() && context.memory(|memory| memory.focused().is_none()) {
                self.clipboard(context);
            }
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
            self.canvas(ui, editing_blocked);
        });
        let (fill, background) = self.fill_colors(self.current().fill_color);
        // A fill pen that matches the background would show nothing.
        let colors = if fill == background {
            (Color32::BLACK, Color32::WHITE)
        } else {
            (fill, background)
        };
        if let Some(dialog) = &mut self.pattern_dialog {
            let document = &self.document;
            let end = dialog.target.unwrap_or(document.len()).min(document.len());
            let result = dialog.show(context, colors, |slot| {
                pattern::pattern_before(document.items()[..end].iter().filter_map(IgsItem::command), slot)
            });
            match result {
                PatternResult::Open => {}
                PatternResult::Cancel => self.pattern_dialog = None,
                PatternResult::Apply => {
                    if let Some(dialog) = self.pattern_dialog.take() {
                        self.apply_pattern(dialog);
                    }
                }
            }
        }
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

    /// Copy, cut, paste and duplicate (Ctrl+D) of the selected item as IGS source.
    fn clipboard(&mut self, context: &egui::Context) {
        let (copy, cut, pasted, duplicate) = context.input_mut(|input| {
            let mut copy = false;
            let mut cut = false;
            let mut pasted = None;
            input.events.retain(|event| match event {
                egui::Event::Copy => {
                    copy = true;
                    false
                }
                egui::Event::Cut => {
                    cut = true;
                    false
                }
                egui::Event::Paste(text) => {
                    pasted = Some(text.clone());
                    false
                }
                _ => true,
            });
            (copy, cut, pasted, input.consume_key(egui::Modifiers::COMMAND, egui::Key::D))
        });
        if copy || cut {
            if let Some(text) = self.copy_selected() {
                context.copy_text(text);
                if cut {
                    self.delete_selected();
                }
            }
        }
        if let Some(text) = pasted.filter(|text| !text.is_empty()) {
            self.paste(&text);
        }
        if duplicate {
            self.duplicate_selected();
        }
    }

    /// Canvas pixels per screen point, horizontally and vertically. Medium resolution pixels
    /// are twice as tall as wide, like on an Atari ST monitor.
    fn scale(&self, available: egui::Vec2) -> egui::Vec2 {
        let aspect = pixel_aspect(self.canvas.resolution);
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
            // The next polygon edge follows Shift like the click that ends it.
            if let (Some(hover), Some(last), true) = (self.hover, self.poly.last().copied(), self.tool.is_poly()) {
                let shift = ui.input(|input| input.modifiers.shift);
                self.hover = Some(self.constrained(shift, last, hover));
            }
            let editing = !self.animating();
            if !blocked && editing {
                self.canvas_input(ui, &response, &at, &on_screen, scale);
                // The texture is uploaded at the end of the frame, so the changed shape shows now.
                self.refresh_preview(ui.ctx());
            }
            let accent = ui.visuals().selection.stroke.color;
            if editing && !blocked && self.tool == Tool::Marker && self.drag.is_none() {
                self.paint_marker_preview(ui, &on_screen, scale);
            }
            if editing && self.tool == Tool::Zone {
                self.paint_zones(ui, &on_screen, scale);
            }
            if editing && self.tool == Tool::CopyArea {
                let area = |(from, to): (Point, Point)| {
                    egui::Rect::from_two_pos(on_screen((from.0.min(to.0), from.1.min(to.1))), on_screen((from.0.max(to.0), from.1.max(to.1))))
                        .expand2(scale * 0.5)
                };
                for (rect, strong) in [(self.copy_source.map(area), true), (self.drag.map(area), false)] {
                    if let Some(rect) = rect {
                        ui.painter()
                            .rect_stroke(rect, 0.0, Stroke::new(if strong { 1.5 } else { 1.0 }, accent), egui::StrokeKind::Middle);
                    }
                }
            }
            if editing && matches!(self.tool, Tool::Select | Tool::Zone) {
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
            if let Some(edit) = self.text_edit.as_ref().filter(|_| editing) {
                let size = match edit.index {
                    Some(index) => self.document.state_before(index).text.map_or(9, |(_, size, _)| size),
                    None => self.current().text_size,
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

    /// A see-through preview of the marker the next click plots at the pointer.
    fn paint_marker_preview(&self, ui: &egui::Ui, on_screen: &dyn Fn(Point) -> egui::Pos2, scale: egui::Vec2) {
        let Some((x, y)) = self.hover else {
            return;
        };
        let current = self.current();
        let color = palette::pen_color(&self.palette, self.canvas.resolution, current.marker_color).gamma_multiply(MARKER_PREVIEW_OPACITY);
        let (width, height) = (self.canvas.width, self.canvas.height);
        for (px, py) in marker::pixels(current.marker, current.marker_size, x, y) {
            if (0..width).contains(&px) && (0..height).contains(&py) {
                ui.painter().rect_filled(egui::Rect::from_center_size(on_screen((px, py)), scale), 0.0, color);
            }
        }
    }

    /// Outlines every mouse zone with its host string, and the one being dragged out.
    fn paint_zones(&self, ui: &egui::Ui, on_screen: &dyn Fn(Point) -> egui::Pos2, scale: egui::Vec2) {
        let color = Color32::from_rgb(0x40, 0xC8, 0xFF);
        let painter = ui.painter();
        let area = |from: Point, to: Point| {
            egui::Rect::from_two_pos(on_screen((from.0.min(to.0), from.1.min(to.1))), on_screen((from.0.max(to.0), from.1.max(to.1)))).expand2(scale * 0.5)
        };
        let selected = self.selected_shape();
        for index in 0..self.document.len() {
            let Some((IgsCommand::DefineZone { string, .. }, geometry)) = self.shape(index) else {
                continue;
            };
            let (x0, y0, x1, y1) = select::bounds(&geometry);
            let rect = area((x0, y0), (x1, y1));
            let chosen = selected == Some(index);
            painter.rect_filled(rect, 0.0, color.gamma_multiply(if chosen { 0.22 } else { 0.12 }));
            painter.rect_stroke(rect, 0.0, Stroke::new(if chosen { 2.0 } else { 1.0 }, color), egui::StrokeKind::Inside);
            let galley = painter.layout(latin1(&string), egui::FontId::proportional(11.0), Color32::WHITE, (rect.width() - 6.0).max(0.0));
            let tag = egui::Rect::from_min_size(rect.min, galley.size() + egui::vec2(6.0, 2.0)).intersect(rect);
            painter.rect_filled(tag, 0.0, color.gamma_multiply(0.85));
            painter.with_clip_rect(tag).galley(tag.min + egui::vec2(3.0, 1.0), galley, Color32::WHITE);
        }
        if let Some((from, to)) = self.drag {
            let rect = area(from, to);
            painter.rect_filled(rect, 0.0, color.gamma_multiply(0.18));
            painter.rect_stroke(rect, 0.0, Stroke::new(1.5, color), egui::StrokeKind::Inside);
        }
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
                if target.is_none() && self.tool == Tool::Zone {
                    let start = at(start);
                    self.drag = Some((start, start));
                }
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
        if let (Some((from, _)), Some(pointer)) = (self.drag, pointer) {
            let to = at(pointer);
            self.drag = Some((from, to));
            if response.drag_stopped() {
                self.drag = None;
                self.add_zone(from, to);
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

    /// Drag out the area to copy, then click where copies go; right-click picks a new area.
    fn copy_input(&mut self, ui: &egui::Ui, response: &egui::Response, at: &dyn Fn(egui::Pos2) -> Point) {
        let pointer = response.interact_pointer_pos();
        let origin = ui.input(|input| input.pointer.press_origin());
        if response.secondary_clicked() {
            self.copy_source = None;
            return;
        }
        if self.copy_source.is_some() {
            if response.clicked() {
                if let Some(command) = pointer.map(at).and_then(|point| self.copy_command(point)) {
                    self.add_commands(vec![command]);
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
                if (from.0 - to.0).abs() >= 1 && (from.1 - to.1).abs() >= 1 {
                    self.copy_source = Some(((from.0.min(to.0), from.1.min(to.1)), (from.0.max(to.0), from.1.max(to.1))));
                }
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
        let shift = ui.input(|input| input.modifiers.shift);
        if matches!(self.tool, Tool::Select | Tool::Zone) {
            self.select_input(ui, response, at, on_screen, scale);
            return;
        }
        if self.tool == Tool::CopyArea {
            self.copy_input(ui, response, at);
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
                    let point = self.poly.last().map_or(point, |last| self.constrained(shift, *last, point));
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
            let to = self.constrained(shift, from, at(pointer));
            self.drag = Some((from, to));
            if response.drag_stopped() {
                self.drag = None;
                self.add_shape(from, to);
            }
        } else if self.drag.is_some() && ui.input(|input| input.pointer.any_released()) {
            self.drag = None;
        }
    }

    /// `to` constrained from `from` by the tool while Shift is held.
    fn constrained(&self, shift: bool, from: Point, to: Point) -> Point {
        match self.tool.constraint() {
            Some(constraint) if shift => constrain(constraint, &self.canvas, from, to),
            _ => to,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_parser_core::BaudEmulation;

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

    /// Changes drawing attributes the way the tool controls do.
    fn set(editor: &mut IgsEditor, change: impl FnOnce(&mut Current)) {
        let mut wanted = editor.current();
        change(&mut wanted);
        editor.set_attributes(wanted);
    }

    fn commands(editor: &IgsEditor) -> Vec<IgsCommand> {
        editor.document.items().iter().filter_map(IgsItem::command).cloned().collect()
    }

    #[test]
    fn every_tool_adds_just_its_shape_as_one_undo_step() {
        for tool in Tool::ALL.into_iter().filter(|tool| tool.pen().is_some() && *tool != Tool::Text) {
            let mut editor = IgsEditor::new(TerminalResolution::Low);
            editor.select_tool(tool);
            set(&mut editor, |wanted| {
                wanted.line_color = 3;
                wanted.fill_color = 4;
                wanted.marker_color = 5;
            });
            let before = editor.document.len();
            editor.attributes.start_angle = 10;
            editor.attributes.end_angle = 120;
            if tool.is_poly() {
                editor.poly = vec![(100, 100), (180, 100), (160, 180)];
                editor.finish_poly();
            } else {
                editor.add_shape((100, 100), (160, 140));
            }
            assert!(editor.error.is_none(), "{tool:?}: {:?}", editor.error);
            assert_eq!(editor.document.len(), before + 1, "{tool:?}: no attribute commands come with the shape");
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
    fn changing_an_attribute_adds_its_command_at_once() {
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        let before = editor.document.len();
        set(&mut editor, |wanted| wanted.line_color = 1);
        assert_eq!(editor.document.len(), before, "the default pen needs no command");
        set(&mut editor, |wanted| wanted.line_color = 2);
        assert_eq!(editor.document.command(before), Some(&IgsCommand::ColorSet { pen: PenType::Line, color: 2 }));
        assert_eq!(editor.selected, Some(before));
        // Trying out colors changes that command instead of adding more.
        set(&mut editor, |wanted| wanted.line_color = 3);
        assert_eq!(editor.document.len(), before + 1);
        assert_eq!(editor.document.command(before), Some(&IgsCommand::ColorSet { pen: PenType::Line, color: 3 }));
        assert_eq!(editor.current().line_color, 3);
        editor.select_tool(Tool::Line);
        editor.add_shape((0, 0), (10, 10));
        set(&mut editor, |wanted| wanted.line_color = 4);
        assert_eq!(editor.document.len(), before + 3, "after a shape a new command follows it");
        editor.undo(false);
        assert_eq!(editor.current().line_color, 3);

        // Each attribute has its command.
        set(&mut editor, |wanted| {
            wanted.pattern = PatternType::Hatch(3);
            wanted.border = true;
            wanted.line_kind = LineKind::Dotted;
            wanted.line_thickness = 5;
            wanted.line_ends = (ArrowEnd::Arrow, ArrowEnd::Square);
            wanted.marker = PolymarkerKind::Star;
            wanted.drawing_mode = DrawingMode::Xor;
            wanted.text_size = 18;
        });
        let added = commands(&editor)[before + 2..].to_vec();
        assert_eq!(
            added,
            vec![
                IgsCommand::AttributeForFills {
                    pattern_type: PatternType::Hatch(3),
                    border: true
                },
                IgsCommand::SetLineOrMarkerStyle {
                    style: LineMarkerStyle::LineThickness(LineKind::Dotted, 1)
                },
                IgsCommand::SetLineOrMarkerStyle {
                    style: LineMarkerStyle::LineEndpoints(LineKind::Dotted, ArrowEnd::Arrow, ArrowEnd::Square)
                },
                IgsCommand::SetLineOrMarkerStyle {
                    style: LineMarkerStyle::PolyMarkerSize(PolymarkerKind::Star, 1)
                },
                IgsCommand::DrawingMode { mode: DrawingMode::Xor },
                IgsCommand::TextEffects {
                    effects: TextEffects::NORMAL,
                    size: 18,
                    rotation: TextRotation::Degrees0
                },
            ],
            "only IG's solid lines are wide"
        );
        let reopened = IgsDocument::from_bytes(&editor.document.to_bytes().unwrap()).unwrap();
        assert_eq!(
            reopened.items().iter().filter_map(IgsItem::command).cloned().collect::<Vec<_>>(),
            commands(&editor)
        );
    }

    #[test]
    fn toolbar_changes_wait_for_the_pointer_and_add_one_command() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        run(&context, &mut editor, vec![]);
        let before = editor.document.len();
        let mut wanted = editor.current();
        wanted.fill_color = 5;
        editor.draft_attributes(wanted);
        let _ = context.run(
            egui::RawInput {
                events: vec![egui::Event::PointerButton {
                    pos: egui::pos2(5.0, 5.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |context| editor.commit_attribute_draft(context),
        );
        assert_eq!(editor.document.len(), before, "nothing is added while the pointer is down");
        let _ = context.run(
            egui::RawInput {
                events: vec![egui::Event::PointerButton {
                    pos: egui::pos2(5.0, 5.0),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |_| {},
        );
        let _ = context.run(egui::RawInput::default(), |context| editor.commit_attribute_draft(context));
        assert_eq!(editor.document.command(before), Some(&IgsCommand::ColorSet { pen: PenType::Fill, color: 5 }));
        assert!(editor.attribute_draft.is_none());
    }

    #[test]
    fn dragged_boxes_render_filled_on_the_medium_resolution_canvas() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Medium);
        editor.select_tool(Tool::Rectangle);
        set(&mut editor, |wanted| wanted.fill_color = 2);
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
        set(&mut editor, |wanted| wanted.text_color = 3);
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
        assert_eq!(editor.preview_request().extra, changed);
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

    fn run_at(context: &egui::Context, editor: &mut IgsEditor, time: f64) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 820.0))),
                time: Some(time),
                ..Default::default()
            },
            |context| editor.show(context, false),
        );
    }

    #[test]
    fn every_template_inserts_one_item_that_saves() {
        for (name, source) in templates().into_iter().flat_map(|(_, commands)| commands) {
            let mut editor = IgsEditor::new(TerminalResolution::Low);
            editor.insert_source(source);
            assert!(editor.error.is_none(), "{name}: {:?}", editor.error);
            let item = &editor.document.items()[editor.document.len() - 1];
            assert_eq!(matches!(item, IgsItem::Text(_)), source.first() != Some(&b'G'), "{name}: {item:?}");
            let bytes = editor.document.to_bytes().unwrap();
            assert_eq!(IgsDocument::from_bytes(&bytes).unwrap().items().len(), editor.document.len(), "{name}");
            let context = egui::Context::default();
            editor.selected = Some(editor.document.len() - 1);
            run(&context, &mut editor, vec![]);
        }
    }

    #[test]
    fn attributes_changed_after_the_selection_apply_to_the_commands_after_it() {
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        editor.select_tool(Tool::Line);
        editor.add_shape((0, 0), (10, 10));
        let first = editor.document.len() - 1;
        editor.add_shape((10, 10), (20, 0));
        editor.preview_to_selection = true;
        editor.selected = Some(first);
        set(&mut editor, |wanted| wanted.line_color = 2);
        assert_eq!(editor.document.command(first + 1), Some(&IgsCommand::ColorSet { pen: PenType::Line, color: 2 }));
        assert_eq!(editor.selected, Some(first + 1), "new commands follow the attribute command");
        editor.add_shape((30, 30), (40, 40));
        assert_eq!(editor.selected, Some(first + 2));
        assert!(matches!(editor.document.command(first + 2), Some(IgsCommand::Line { .. })));
        // Explicit, as in IG's drawing program: the lines after it are drawn in the new pen too.
        assert_eq!(editor.document.state_before(first + 3).line_color, Some(2));
        assert_eq!(editor.document.len(), first + 4, "nothing is restored implicitly");
    }

    #[test]
    fn items_are_copied_pasted_and_duplicated_as_source() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        editor.select_tool(Tool::Rectangle);
        editor.add_shape((10, 10), (50, 50));
        let index = editor.document.len() - 1;
        let copied = editor.copy_selected().unwrap();
        assert_eq!(copied, "G#B>10,10,50,50,0:\n");
        editor.select_tool(Tool::Select);
        run(&context, &mut editor, vec![]);
        run(&context, &mut editor, vec![egui::Event::Paste(copied.clone())]);
        assert_eq!(editor.document.len(), index + 2);
        assert_eq!(editor.document.command(index + 1), editor.document.command(index));
        run(
            &context,
            &mut editor,
            vec![egui::Event::Key {
                key: egui::Key::D,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
        );
        assert_eq!(editor.document.len(), index + 3);
        assert_eq!(editor.selected, Some(index + 2));
        run(&context, &mut editor, vec![egui::Event::Cut]);
        assert_eq!(editor.document.len(), index + 2);
        editor.paste("\\q");
        assert!(editor.error.is_some());
    }

    #[test]
    fn user_fill_patterns_are_drawn_and_used_by_fills() {
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        editor.open_pattern_dialog(None);
        let mut dialog = editor.pattern_dialog.take().unwrap();
        assert_eq!(dialog.slot(), 0);
        for y in 0..16 {
            dialog.toggle(0, y);
        }
        editor.apply_pattern(dialog);
        let loaded = editor.document.len() - 2;
        assert!(matches!(editor.document.command(loaded), Some(IgsCommand::LoadFillPattern { pattern: 0, data }) if data[3] == 0x8000));
        assert_eq!(
            editor.document.command(loaded + 1),
            Some(&IgsCommand::AttributeForFills {
                pattern_type: PatternType::UserDefined(0),
                border: false
            }),
            "the new pattern is selected for fills with a command"
        );
        assert_eq!(editor.current().pattern, PatternType::UserDefined(0));
        editor.select_tool(Tool::FilledRectangle);
        editor.add_shape((0, 0), (63, 31));
        let preview = editor.document.preview().unwrap();
        let register = palette::pens(TerminalResolution::Low)[1];
        assert_eq!(preview.pixel_index(16, 5), Some(register));
        assert_eq!(preview.pixel_index(17, 5), Some(0));

        let index = editor
            .document
            .items()
            .iter()
            .position(|item| matches!(item.command(), Some(IgsCommand::LoadFillPattern { .. })))
            .unwrap();
        editor.open_pattern_dialog(Some(index));
        let mut dialog = editor.pattern_dialog.take().unwrap();
        dialog.toggle(1, 0);
        editor.apply_pattern(dialog);
        assert!(matches!(editor.document.command(index), Some(IgsCommand::LoadFillPattern { data, .. }) if data[0] == 0xC000));
    }

    #[test]
    fn areas_are_copied_to_where_they_are_placed() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        editor.select_tool(Tool::FilledRectangle);
        editor.add_shape((10, 10), (30, 30));
        editor.select_tool(Tool::CopyArea);
        run(&context, &mut editor, vec![]);
        drag(&context, &mut editor, (5, 5), (35, 35));
        assert_eq!(editor.copy_source, Some(((5, 5), (35, 35))));
        click(&context, &mut editor, (100, 100));
        assert!(matches!(
            editor.document.command(editor.document.len() - 1),
            Some(IgsCommand::GrabScreen {
                operation: BlitOperation::ScreenToScreen {
                    src_x1: 5,
                    dest_x: 100,
                    dest_y: 100,
                    ..
                },
                mode: BlitMode::Replace
            })
        ));
        let preview = editor.document.preview().unwrap();
        assert_eq!(preview.pixel_index(110, 110), preview.pixel_index(15, 15));
        assert_ne!(preview.pixel_index(110, 110), Some(0));
    }

    #[test]
    fn shift_constrains_lines_to_45_degrees_and_boxes_to_squares() {
        let low = Canvas::new(TerminalResolution::Low);
        assert_eq!(constrain(Constraint::Angle, &low, (100, 100), (150, 110)), (151, 100), "horizontal");
        assert_eq!(constrain(Constraint::Angle, &low, (100, 100), (104, 60)), (100, 60), "vertical");
        assert_eq!(constrain(Constraint::Angle, &low, (100, 100), (140, 135)), (138, 138), "diagonal");
        assert_eq!(constrain(Constraint::Angle, &low, (100, 100), (60, 64)), (62, 62), "diagonal the other way");
        assert_eq!(constrain(Constraint::Square, &low, (10, 10), (40, 20)), (40, 40));
        assert_eq!(constrain(Constraint::Square, &low, (50, 50), (20, 45)), (20, 20));
        assert_eq!(constrain(Constraint::Square, &low, (300, 10), (310, 60)), (319, 60), "kept on the canvas");

        // Medium resolution pixels are twice as tall, so 45° and squares use half the rows.
        let medium = Canvas::new(TerminalResolution::Medium);
        assert_eq!(constrain(Constraint::Angle, &medium, (100, 100), (138, 121)), (140, 120));
        assert_eq!(constrain(Constraint::Square, &medium, (10, 10), (50, 15)), (50, 30));
    }

    #[test]
    fn holding_shift_constrains_drawn_shapes() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        run(&context, &mut editor, vec![]);
        let shifted = |editor: &mut IgsEditor, events: Vec<egui::Event>| {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 820.0))),
                    events,
                    modifiers: egui::Modifiers::SHIFT,
                    ..Default::default()
                },
                |context| editor.show(context, false),
            );
        };
        let shifted_drag = |editor: &mut IgsEditor, from: Point, to: Point| {
            let (start, end) = (screen(editor, from), screen(editor, to));
            let press = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::SHIFT,
            };
            shifted(editor, vec![egui::Event::PointerMoved(start), press(start, true)]);
            shifted(editor, vec![egui::Event::PointerMoved(start.lerp(end, 0.5))]);
            shifted(editor, vec![egui::Event::PointerMoved(end)]);
            shifted(editor, vec![press(end, false)]);
            shifted(editor, vec![]);
        };

        editor.select_tool(Tool::Line);
        shifted_drag(&mut editor, (100, 100), (150, 106));
        let v = IgsParameter::Value;
        assert_eq!(
            commands(&editor).last(),
            Some(&IgsCommand::Line {
                x1: v(100),
                y1: v(100),
                x2: v(150),
                y2: v(100)
            })
        );

        editor.select_tool(Tool::Rectangle);
        shifted_drag(&mut editor, (20, 20), (60, 40));
        assert_eq!(
            commands(&editor).last(),
            Some(&IgsCommand::Box {
                x1: v(20),
                y1: v(20),
                x2: v(60),
                y2: v(60),
                rounded: false
            })
        );

        editor.select_tool(Tool::PolyLine);
        click(&context, &mut editor, (200, 50));
        let pos = screen(&editor, (240, 88));
        let press = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::SHIFT,
        };
        shifted(&mut editor, vec![egui::Event::PointerMoved(pos), press(true)]);
        shifted(&mut editor, vec![press(false)]);
        assert_eq!(editor.poly, vec![(200, 50), (239, 89)], "the vertex snaps to 45° from the last one");

        // Without Shift nothing is constrained.
        editor.finish_poly();
        editor.select_tool(Tool::Line);
        drag(&context, &mut editor, (100, 150), (150, 156));
        assert_eq!(
            commands(&editor).last(),
            Some(&IgsCommand::Line {
                x1: v(100),
                y1: v(150),
                x2: v(150),
                y2: v(156)
            })
        );
    }

    #[test]
    fn every_ig_blit_mode_can_be_chosen_for_copies() {
        for (number, mode) in properties::BLIT_MODES.into_iter().enumerate() {
            assert_eq!(mode as usize, number, "the modes are listed in IG's order");
        }
        let names: std::collections::HashSet<String> = properties::BLIT_MODES.into_iter().map(blit_mode_name).collect();
        assert_eq!(names.len(), 16, "every mode has its own name");
        let logic: std::collections::HashSet<&str> = properties::BLIT_MODES.into_iter().map(properties::blit_logic).collect();
        assert_eq!(logic.len(), 16);

        // Erase copies the source as a hole: a filled square erased onto itself becomes background.
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        editor.select_tool(Tool::FilledRectangle);
        editor.add_shape((10, 10), (30, 30));
        editor.select_tool(Tool::CopyArea);
        editor.attributes.blit_mode = BlitMode::Erase;
        run(&context, &mut editor, vec![]);
        drag(&context, &mut editor, (10, 10), (30, 30));
        click(&context, &mut editor, (10, 10));
        let last = editor.document.len() - 1;
        assert!(matches!(
            editor.document.command(last),
            Some(IgsCommand::GrabScreen { mode: BlitMode::Erase, .. })
        ));
        assert!(String::from_utf8(editor.document.item_source(last).unwrap()).unwrap().starts_with("G#G>0,4,"));
        let preview = editor.document.preview().unwrap();
        assert_eq!(preview.pixel_index(20, 20), preview.pixel_index(200, 150), "the square is erased");
    }

    #[test]
    fn mouse_zones_are_numbered_and_only_picked_with_their_tool() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        editor.select_tool(Tool::Zone);
        run(&context, &mut editor, vec![]);
        drag(&context, &mut editor, (10, 10), (60, 40));
        assert!(editor.error.is_some(), "a zone needs a host string");
        editor.attributes.zone_host = "MENU".into();
        drag(&context, &mut editor, (10, 10), (60, 40));
        drag(&context, &mut editor, (100, 10), (150, 40));
        let zones: Vec<_> = commands(&editor)
            .into_iter()
            .filter_map(|command| match command {
                IgsCommand::DefineZone { zone_id, string, length, .. } => Some((zone_id, string, length)),
                _ => None,
            })
            .collect();
        assert_eq!(zones, vec![(0, b"MENU".to_vec(), 4), (1, b"MENU".to_vec(), 4)]);
        let index = editor.document.len() - 1;
        assert!(editor.shape(index).is_some());
        editor.select_tool(Tool::Select);
        assert!(editor.shape(index).is_none());
        let reopened = IgsDocument::from_bytes(&editor.document.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.command(index), editor.document.command(index));
    }

    #[test]
    fn the_resolution_control_edits_the_first_resolution_command() {
        let mut editor = IgsEditor::new(TerminalResolution::Medium);
        editor.set_resolution(TerminalResolution::Low);
        assert_eq!(editor.document.resolution(), TerminalResolution::Low);
        assert!(matches!(
            editor.document.command(0),
            Some(IgsCommand::SetResolution {
                resolution: TerminalResolution::Low,
                ..
            })
        ));
        let mut editor = IgsEditor::from_document(IgsDocument::from_bytes(b"G#L>0,0,10,10:\r\n").unwrap());
        editor.set_resolution(TerminalResolution::High);
        assert!(matches!(
            editor.document.command(0),
            Some(IgsCommand::SetResolution {
                resolution: TerminalResolution::High,
                ..
            })
        ));
        assert_eq!(editor.document.len(), 2);
    }

    #[test]
    fn playback_waits_for_pauses_and_plays_sounds() {
        let context = egui::Context::default();
        let mut editor =
            IgsEditor::from_document(IgsDocument::from_bytes(b"G#R>0,2:\r\nG#b>20,0,0,3,0,0,500:\r\nG#b>0:\r\nG#t>1:\r\nG#L>0,0,10,10:\r\n").unwrap());
        editor.transport.speed = BaudEmulation::Off;
        assert!(matches!(editor.document.command(1), Some(IgsCommand::AlterSoundEffect { play: false, .. })));
        editor.toggle_playback(0.0);
        run_at(&context, &mut editor, 0.0);
        assert_eq!(editor.transport.run.as_ref().map(|playback| playback.index), Some(3), "stops at the pause");
        assert_eq!(editor.preview_request().through, Some(3));
        let Some(Sound::Gist(played)) = editor.sound.played.first() else {
            panic!("expected a sound: {:?}", editor.sound.played)
        };
        assert_eq!(played[3], 500, "the sound table was changed before it played");
        run_at(&context, &mut editor, 0.5);
        assert_eq!(editor.transport.run.as_ref().map(|playback| playback.index), Some(3));
        run_at(&context, &mut editor, 1.1);
        assert!(editor.transport.run.is_none(), "playback ends after the last item");

        editor.sound.played.clear();
        editor.play_sound(2);
        assert!(matches!(editor.sound.played.as_slice(), [Sound::Gist(data)] if data[3] == 500));
    }

    #[test]
    fn animation_transport_respects_transmission_time_and_seeks() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::from_document(IgsDocument::from_bytes(b"G#L>0,0,10,10:\r\nG#t>1:\r\nG#L>20,20,30,30:\r\n").unwrap());
        editor.toggle_playback(0.0);
        let first_delay = editor.document.item_source(1).unwrap().len() as f64 / 1200.0;
        assert!((editor.transport.run.as_ref().unwrap().next_at - first_delay).abs() < 1e-9);
        run_at(&context, &mut editor, first_delay / 2.0);
        assert_eq!(editor.transport.run.as_ref().unwrap().index, 0);
        run_at(&context, &mut editor, first_delay + 0.001);
        let pause = editor.transport.run.as_ref().unwrap();
        assert_eq!(pause.index, 1);
        let third_delay = editor.document.item_source(2).unwrap().len() as f64 / 1200.0;
        assert!((pause.next_at - first_delay - 1.0 - third_delay).abs() < 1e-9);

        editor.toggle_playback(first_delay + 0.001);
        assert_eq!(editor.preview_request().through, Some(1));
        editor.step_playback(false);
        assert_eq!(editor.preview_request().through, Some(0));
        editor.step_playback(true);
        assert_eq!(editor.preview_request().through, Some(1));
        editor.toggle_playback(2.0);
        assert_eq!(editor.transport.run.as_ref().unwrap().index, 2);
        run_at(&context, &mut editor, 2.1);
        assert!(editor.transport.run.is_none());
        assert_eq!(editor.preview_request().through, Some(2));
        editor.toggle_playback(3.0);
        assert_eq!(editor.transport.run.as_ref().unwrap().index, 0);
        editor.stop_playback();
        assert_eq!(editor.preview_request().through, None);
    }

    fn lines(count: usize) -> IgsEditor {
        let source: String = (0..count).map(|index| format!("G#L>{index},0,{index},50:\r\n")).collect();
        let mut editor = IgsEditor::from_document(IgsDocument::from_bytes(source.as_bytes()).unwrap());
        editor.transport.speed = BaudEmulation::Rate(300);
        editor
    }

    #[test]
    fn the_command_list_follows_and_scrolls_to_the_animation() {
        let context = egui::Context::default();
        let mut editor = lines(150);
        run_at(&context, &mut editor, 0.0);
        editor.toggle_playback(0.0);
        run_at(&context, &mut editor, 0.0);
        assert_eq!(editor.selected, Some(0));
        editor.seek_playback(120, 0.0);
        run_at(&context, &mut editor, 0.0);
        run_at(&context, &mut editor, 0.0);
        assert_eq!(editor.selected, Some(120));
        assert!(editor.visible_rows.contains(&120), "{:?}", editor.visible_rows);
        // Playing on selects the next item.
        let next = editor.transport.run.as_ref().unwrap().next_at;
        run_at(&context, &mut editor, next + 0.001);
        assert_eq!(editor.selected, Some(121));
        // A click selects without moving the paused animation.
        editor.toggle_playback(next + 0.002);
        editor.select_item(Some(5));
        run_at(&context, &mut editor, next + 0.003);
        assert_eq!((editor.selected, editor.frame()), (Some(5), Some(121)));
    }

    #[test]
    fn double_clicking_a_row_runs_the_animation_to_it() {
        let context = egui::Context::default();
        let mut editor = lines(60);
        let frame = |editor: &mut IgsEditor, time: f64, events: Vec<egui::Event>| {
            context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 820.0))),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |context| editor.show(context, false),
            )
        };
        let output = frame(&mut editor, 0.0, vec![]);
        let row = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "7" => Some(text.pos + text.galley.size() / 2.0 + egui::vec2(60.0, 0.0)),
                _ => None,
            })
            .expect("row 7 is listed");
        for (time, pressed) in [(1.0, true), (1.05, false), (1.1, true), (1.15, false)] {
            frame(
                &mut editor,
                time,
                vec![egui::Event::PointerMoved(row), button_event(row, egui::PointerButton::Primary, pressed)],
            );
        }
        frame(&mut editor, 1.2, vec![]);
        assert_eq!(editor.transport.playhead, Some(6), "the animation shows the drawing through item 7");
        assert_eq!(editor.selected, Some(6));
        assert_eq!(editor.preview_request().through, Some(6));
    }

    #[test]
    fn the_preview_through_the_selection_is_an_animation_position() {
        let mut editor = lines(10);
        editor.selected = Some(3);
        editor.toggle_preview();
        assert!(editor.preview_to_selection && !editor.animating());
        assert_eq!(editor.frame(), Some(3));
        // Stepping while previewing moves the selection and stays editable.
        editor.step_playback(true);
        assert_eq!((editor.selected, editor.frame(), editor.animating()), (Some(4), Some(4), false));
        editor.seek_playback(7, 0.0);
        assert_eq!((editor.selected, editor.animating()), (Some(7), false));
        // Play continues after the previewed item.
        editor.toggle_playback(0.0);
        assert_eq!(editor.transport.run.as_ref().unwrap().index, 8);
        // The eye turns the paused animation into an editable preview through its frame.
        editor.toggle_playback(0.1);
        assert_eq!(editor.transport.playhead, Some(8));
        editor.toggle_preview();
        assert!(!editor.animating() && editor.preview_to_selection);
        assert_eq!((editor.selected, editor.frame()), (Some(8), Some(8)));
        editor.stop_playback();
        assert!(!editor.preview_to_selection);
        assert_eq!(editor.frame(), None);
    }

    #[test]
    fn delayed_loops_play_iteration_by_iteration() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::from_document(IgsDocument::from_bytes(b"G#R>0,0:\r\nG#&>10,50,10,2,L,4,x,20,x,40:\r\nG#t>1:\r\n").unwrap());
        editor.transport.speed = BaudEmulation::Off;
        editor.toggle_playback(0.0);
        run_at(&context, &mut editor, 0.0);
        let request = editor.preview_request();
        assert_eq!((request.through, request.steps), (Some(1), Some(1)), "the loop starts with one iteration");
        let drawn = |editor: &IgsEditor| {
            let preview = editor.render(&editor.preview_request()).unwrap();
            [10, 20, 50].map(|x| preview.pixel_index(x, 30) != preview.pixel_index(5, 30))
        };
        assert_eq!(drawn(&editor), [true, false, false]);
        run_at(&context, &mut editor, 0.011);
        assert_eq!(editor.preview_request().steps, Some(2));
        assert_eq!(drawn(&editor), [true, true, false]);
        // After the last iteration's delay the next item follows.
        run_at(&context, &mut editor, 0.051);
        let request = editor.preview_request();
        assert_eq!((request.through, request.steps), (Some(2), None));
        assert_eq!(drawn(&editor), [true, true, true]);
    }

    #[test]
    fn the_position_slider_seeks_while_paused_and_playing() {
        let context = egui::Context::default();
        let mut editor =
            IgsEditor::from_document(IgsDocument::from_bytes(b"G#b>20,0,0,3,0,0,500:\r\nG#t>1:\r\nG#L>0,0,10,10:\r\nG#b>0:\r\nG#t>2:\r\n").unwrap());
        editor.transport.speed = BaudEmulation::Off;
        editor.seek_playback(2, 0.0);
        assert!(editor.transport.run.is_none());
        assert_eq!(editor.preview_request().through, Some(2));
        editor.seek_playback(99, 0.0);
        assert_eq!(editor.transport.playhead, Some(4));

        editor.stop_playback();
        editor.toggle_playback(10.0);
        editor.sound.played.clear();
        editor.seek_playback(3, 10.5);
        let playback = editor.transport.run.as_ref().unwrap();
        assert_eq!(playback.index, 3);
        assert_eq!(playback.next_at, 10.5, "the sound command waits for nothing");
        assert_eq!(editor.sound_table.effects[0][3], 500, "the sound table includes earlier changes");
        assert_eq!(editor.sound.played, vec![Sound::StopAll], "seeking does not replay sounds");
        run_at(&context, &mut editor, 10.6);
        assert_eq!(editor.transport.run.as_ref().unwrap().index, 4, "playback continues after the seek");
    }

    #[test]
    fn transmission_speed_applies_to_text_items_and_max_is_instant() {
        let mut editor = IgsEditor::from_document(IgsDocument::from_bytes(b"Hello G#L>0,0,10,10:\r\nG#b>0:\r\n").unwrap());
        editor.toggle_playback(0.0);
        assert!(editor.document.command(0).is_none());
        assert_eq!(editor.transport.run.as_ref().unwrap().index, 0);
        assert!(editor.transport.run.as_ref().unwrap().next_at > 0.0);
        editor.stop_playback();
        editor.transport.speed = BaudEmulation::Off;
        editor.toggle_playback(0.0);
        let context = egui::Context::default();
        run_at(&context, &mut editor, 0.0);
        assert_eq!(editor.preview_request().through, Some(editor.document.len() - 1));
    }

    #[test]
    fn sound_effect_property_is_saved_and_played() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::from_document(IgsDocument::from_bytes(b"G#b>0:\r\n").unwrap());
        editor.select_item(Some(0));
        run(&context, &mut editor, vec![]);
        let Some((_, IgsCommand::BellsAndWhistles { sound_effect })) = editor.editing.as_mut() else {
            panic!("expected a sound effect draft");
        };
        *sound_effect = icy_parser_core::SoundEffect::Landing;
        run(&context, &mut editor, vec![]);
        let reopened = IgsDocument::from_bytes(&editor.document.to_bytes().unwrap()).unwrap();
        assert!(matches!(
            reopened.command(0),
            Some(IgsCommand::BellsAndWhistles {
                sound_effect: icy_parser_core::SoundEffect::Landing
            })
        ));
        editor.play_sound(0);
        assert!(matches!(editor.sound.played.as_slice(), [Sound::Gist(data)] if data == icy_engine_gui::music::sound_effects::sound_data(19).unwrap()));
    }

    #[test]
    fn inserting_after_the_selection_previews_text_and_shapes_there() {
        let mut editor = IgsEditor::from_document(IgsDocument::from_bytes(b"G#C>1,1:L>0,0,10,10:\r\nG#s>4:\r\n").unwrap());
        editor.preview_to_selection = true;
        editor.selected = Some(1);
        editor.select_tool(Tool::Line);
        editor.drag = Some(((0, 50), (100, 50)));
        let request = editor.preview_request();
        assert_eq!((request.through, request.at), (Some(1), 2));
        assert!(request.extra.iter().any(|command| matches!(command, IgsCommand::Line { .. })));
        // The screen clear after the selection is not shown, the line being drawn is.
        let preview = editor.render(&request).unwrap();
        assert_ne!(preview.pixel_index(50, 50), Some(0));
        editor.drag = None;

        editor.select_tool(Tool::Text);
        editor.begin_text((20, 20));
        editor.text_edit.as_mut().unwrap().text = b"Hi".to_vec();
        editor.finish_text();
        assert!(matches!(editor.document.command(editor.selected.unwrap()), Some(IgsCommand::WriteText { .. })));
        assert!(editor.selected.unwrap() < editor.document.len() - 1, "the text is not at the end");
        assert!(matches!(
            editor.document.command(editor.document.len() - 1),
            Some(IgsCommand::ScreenClear { .. })
        ));
    }

    #[test]
    fn the_fill_pattern_picker_sets_patterns_and_opens_user_patterns() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        let mut wanted = editor.current();
        editor.apply_picker(
            &mut wanted,
            pattern::PickerChange {
                pattern: Some(PatternType::Hatch(4)),
                border: Some(true),
                edit: None,
            },
        );
        editor.set_attributes(wanted);
        assert_eq!(
            commands(&editor).last(),
            Some(&IgsCommand::AttributeForFills {
                pattern_type: PatternType::Hatch(4),
                border: true
            })
        );
        let mut wanted = editor.current();
        editor.apply_picker(
            &mut wanted,
            pattern::PickerChange {
                edit: Some(3),
                ..Default::default()
            },
        );
        run(&context, &mut editor, vec![]);
        assert_eq!(editor.pattern_dialog.as_ref().map(PatternDialog::slot), Some(3));
        let user = pattern::user_patterns(
            [IgsCommand::LoadFillPattern {
                pattern: 2,
                data: vec![0xAAAA; 16],
            }]
            .iter(),
        );
        assert_eq!(PatternType::UserDefined(2).fill_pattern(&user)[0], 0xAAAA);
    }

    #[test]
    fn the_sidebar_polymarker_picker_sets_the_marker_type_and_size() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        let frame = |editor: &mut IgsEditor, events: Vec<egui::Event>| {
            context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 820.0))),
                    events,
                    ..Default::default()
                },
                |context| editor.show(context, false),
            )
        };
        let find = |output: &egui::FullOutput, label: &str| {
            output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => Some(text.pos + text.galley.size() / 2.0),
                _ => None,
            })
        };
        let click = |editor: &mut IgsEditor, pos: egui::Pos2| {
            frame(
                editor,
                vec![egui::Event::PointerMoved(pos), button_event(pos, egui::PointerButton::Primary, true)],
            );
            frame(editor, vec![button_event(pos, egui::PointerButton::Primary, false)]);
            frame(editor, vec![])
        };

        let output = frame(&mut editor, vec![]);
        let swatch = find(&output, "× 1").expect("the sidebar shows the current polymarker and its size");
        click(&mut editor, swatch);
        // A popup grows over frames to what its content asks for.
        let output = (0..4).fold(frame(&mut editor, vec![]), |_, _| frame(&mut editor, vec![]));
        let star = find(&output, &fl!("igs-marker-star")).expect("the picker lists the marker types");
        let preview = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect) if rect.rect.height() == 60.0 && rect.rect.top() > star.y => Some(rect.rect.width()),
                _ => None,
            })
            .fold(0.0, f32::max);
        assert!(preview > 0.0 && preview < 400.0, "the popup is only as wide as its list, not {preview}");
        click(&mut editor, star);
        assert_eq!(
            commands(&editor).last(),
            Some(&IgsCommand::SetLineOrMarkerStyle {
                style: LineMarkerStyle::PolyMarkerSize(PolymarkerKind::Star, 1)
            })
        );

        let mut wanted = editor.current();
        IgsEditor::apply_marker(
            &mut wanted,
            marker::MarkerChange {
                size: Some(4),
                ..Default::default()
            },
        );
        editor.set_attributes(wanted);
        assert_eq!(
            commands(&editor).last(),
            Some(&IgsCommand::SetLineOrMarkerStyle {
                style: LineMarkerStyle::PolyMarkerSize(PolymarkerKind::Star, 4)
            })
        );
        assert_eq!(Tool::Marker.icon(), "polymarker");
    }

    #[test]
    fn the_marker_preview_sets_the_pixels_the_plotted_marker_does() {
        let document = IgsDocument::new(TerminalResolution::Low);
        let blank = document.preview().unwrap();
        for kind in marker::KINDS {
            for size in 1..=marker::MAX_SIZE {
                let plotted = document
                    .preview_with(
                        document.len(),
                        &[
                            IgsCommand::SetLineOrMarkerStyle {
                                style: LineMarkerStyle::PolyMarkerSize(kind, size),
                            },
                            IgsCommand::PolymarkerPlot {
                                x: IgsParameter::Value(160),
                                y: IgsParameter::Value(100),
                            },
                        ],
                    )
                    .unwrap();
                let mut drawn: Vec<(i32, i32)> = (0..plotted.height())
                    .flat_map(|y| (0..plotted.width()).map(move |x| (x, y)))
                    .filter(|&(x, y)| plotted.pixel_index(x, y) != blank.pixel_index(x, y))
                    .map(|(x, y)| (x as i32, y as i32))
                    .collect();
                drawn.sort_unstable();
                assert_eq!(marker::pixels(kind, size, 160, 100), drawn, "{kind:?} size {size}");
            }
        }
    }

    #[test]
    fn hovering_with_the_marker_tool_previews_the_marker() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        set(&mut editor, |wanted| {
            wanted.marker = PolymarkerKind::Star;
            wanted.marker_size = 2;
        });
        editor.select_tool(Tool::Marker);
        run(&context, &mut editor, vec![]);
        let preview = palette::pen_color(&editor.palette, editor.canvas.resolution, editor.current().marker_color).gamma_multiply(MARKER_PREVIEW_OPACITY);
        let count = |output: &egui::FullOutput| {
            output
                .shapes
                .iter()
                .filter(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.fill == preview))
                .count()
        };
        let pos = screen(&editor, (100, 80));
        let output = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 820.0))),
                events: vec![egui::Event::PointerMoved(pos)],
                ..Default::default()
            },
            |context| editor.show(context, false),
        );
        assert_eq!(count(&output), marker::pixels(PolymarkerKind::Star, 2, 100, 80).len());
        let before = editor.document.len();
        editor.select_tool(Tool::Line);
        let output = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 820.0))),
                events: vec![egui::Event::PointerMoved(pos)],
                ..Default::default()
            },
            |context| editor.show(context, false),
        );
        assert_eq!(count(&output), 0, "other tools show no marker preview");
        assert_eq!(editor.document.len(), before, "hovering adds nothing");
    }

    #[test]
    fn user_defined_lines_take_their_pattern_from_the_user_patterns() {
        // T 2,7,n keeps n as the line pattern: rows 1-16 of pattern 6, 17-32 of pattern 7.
        let mut rows6 = vec![0u16; 16];
        rows6[2] = 0xF0F0;
        let mut rows7 = vec![0u16; 16];
        rows7[0] = 0xCCCC;
        let mut document = IgsDocument::new(TerminalResolution::Low);
        document
            .append_many(vec![
                IgsCommand::LoadFillPattern { pattern: 6, data: rows6 },
                IgsCommand::LoadFillPattern { pattern: 7, data: rows7 },
            ])
            .unwrap();
        let reloaded = IgsDocument::from_bytes(b"G#T>2,7,20:\r\nG#T>2,3,5:\r\n").unwrap();
        assert_eq!(
            reloaded.command(0),
            Some(&IgsCommand::SetLineOrMarkerStyle {
                style: LineMarkerStyle::LineThickness(LineKind::UserDefined, 20)
            })
        );
        assert_eq!(
            reloaded.command(1),
            Some(&IgsCommand::SetLineOrMarkerStyle {
                style: LineMarkerStyle::LineThickness(LineKind::Dotted, 1)
            }),
            "other dashed lines stay one pixel wide"
        );

        let v = IgsParameter::Value;
        for (number, mask) in [(3, 0xF0F0u16), (17, 0xCCCC)] {
            let preview = document
                .preview_with(
                    document.len(),
                    &[
                        IgsCommand::SetLineOrMarkerStyle {
                            style: LineMarkerStyle::LineThickness(LineKind::UserDefined, number),
                        },
                        IgsCommand::Line {
                            x1: v(0),
                            y1: v(50),
                            x2: v(31),
                            y2: v(50),
                        },
                    ],
                )
                .unwrap();
            let background = preview.pixel_index(0, 100);
            let drawn: Vec<bool> = (0..32).map(|x| preview.pixel_index(x, 50) != background).collect();
            let expected: Vec<bool> = (0..32).map(|x| mask.rotate_left(x as u32 + 1) & 1 != 0).collect();
            assert_eq!(drawn, expected, "line pattern {number}");
        }
    }

    #[test]
    fn the_line_type_picker_writes_width_pattern_and_ends() {
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        let mut style = editor.current().line_style();
        style.kind = LineKind::UserDefined;
        style.pattern = 12;
        style.width = 9;
        set(&mut editor, |wanted| wanted.set_line_style(style));
        assert_eq!(
            commands(&editor).last(),
            Some(&IgsCommand::SetLineOrMarkerStyle {
                style: LineMarkerStyle::LineThickness(LineKind::UserDefined, 12)
            }),
            "user defined lines write their pattern, not a width"
        );
        let current = editor.current();
        assert_eq!((current.line_kind, current.line_pattern), (LineKind::UserDefined, 12));

        style.kind = LineKind::Solid;
        style.ends = (ArrowEnd::Arrow, ArrowEnd::Rounded);
        set(&mut editor, |wanted| wanted.set_line_style(style));
        let added = commands(&editor);
        assert_eq!(
            &added[added.len() - 2..],
            &[
                IgsCommand::SetLineOrMarkerStyle {
                    style: LineMarkerStyle::LineThickness(LineKind::Solid, 9)
                },
                IgsCommand::SetLineOrMarkerStyle {
                    style: LineMarkerStyle::LineEndpoints(LineKind::Solid, ArrowEnd::Arrow, ArrowEnd::Rounded)
                }
            ]
        );
        let current = editor.current().line_style();
        assert_eq!((current.kind, current.width, current.ends), (style.kind, style.width, style.ends));
    }

    #[test]
    fn the_sidebar_line_type_picker_sets_the_line_type() {
        let context = egui::Context::default();
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        let frame = |editor: &mut IgsEditor, events: Vec<egui::Event>| {
            context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 820.0))),
                    events,
                    ..Default::default()
                },
                |context| editor.show(context, false),
            )
        };
        let find = |output: &egui::FullOutput, label: &str| {
            output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => Some(egui::Rect::from_min_size(text.pos, text.galley.size())),
                _ => None,
            })
        };
        let click = |editor: &mut IgsEditor, pos: egui::Pos2| {
            frame(
                editor,
                vec![egui::Event::PointerMoved(pos), button_event(pos, egui::PointerButton::Primary, true)],
            );
            frame(editor, vec![button_event(pos, egui::PointerButton::Primary, false)]);
            frame(editor, vec![])
        };

        let output = frame(&mut editor, vec![]);
        let label = find(&output, &fl!("igs-line-type")).expect("the sidebar shows the line type");
        // The picker sits right below its label.
        click(&mut editor, egui::pos2(label.left() + 20.0, label.bottom() + 16.0));
        // A popup grows over frames to what its content asks for.
        let output = (0..4).fold(frame(&mut editor, vec![]), |_, _| frame(&mut editor, vec![]));
        let dotted = find(&output, &fl!("igs-line-dotted")).expect("the picker lists the line types");
        let arrows = find(&output, &format!("{} · {}", fl!("igs-end-arrow"), fl!("igs-end-arrow"))).expect("and the line ends");
        assert!(
            arrows.left() > dotted.right() && arrows.left() - dotted.left() < 400.0,
            "the ends are listed next to the types: {dotted:?} {arrows:?}"
        );
        let popup = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                // The preview below the columns is as wide as the popup's content.
                egui::Shape::Rect(rect) if rect.rect.height() == 48.0 && rect.rect.top() > dotted.bottom() => Some(rect.rect.width()),
                _ => None,
            })
            .fold(0.0, f32::max);
        assert!(popup > 0.0 && popup < 700.0, "the popup is only as wide as its columns, not {popup}");
        assert_eq!(
            icy_parser_core::user_line_mask(&editor.user_patterns(), 1),
            icy_parser_core::DEFAULT_USER_LINE_MASK,
            "without loaded patterns user lines show the default pattern they are drawn with"
        );
        click(&mut editor, dotted.center());
        assert_eq!(
            commands(&editor).last(),
            Some(&IgsCommand::SetLineOrMarkerStyle {
                style: LineMarkerStyle::LineThickness(LineKind::Dotted, 1)
            })
        );
    }

    #[test]
    fn attributes_show_the_state_where_commands_are_inserted() {
        let mut editor = IgsEditor::from_document(IgsDocument::from_bytes(b"G#C>1,2:\r\nG#L>0,0,10,10:\r\nG#C>1,5:\r\n").unwrap());
        assert_eq!(editor.current().line_color, 5);
        editor.preview_to_selection = true;
        editor.selected = Some(1);
        assert_eq!(editor.current().line_color, 2);
        editor.selected = Some(0);
        set(&mut editor, |wanted| wanted.line_color = 3);
        assert_eq!(
            editor.document.command(0),
            Some(&IgsCommand::ColorSet { pen: PenType::Line, color: 3 }),
            "the command right before the insertion point is changed"
        );
        assert_eq!(editor.document.len(), 3);
    }

    #[test]
    fn resolution_changes_keep_the_selection_and_show_the_first_command() {
        let mut editor = IgsEditor::from_document(IgsDocument::from_bytes(b"G#L>0,0,10,10:\r\nG#O>5,5,5:\r\n").unwrap());
        editor.selected = Some(1);
        editor.set_resolution(TerminalResolution::Low);
        assert_eq!(editor.selected, Some(2));
        assert!(matches!(editor.document.command(2), Some(IgsCommand::Circle { .. })));

        let editor = IgsEditor::from_document(IgsDocument::from_bytes(b"G#R>0,2:\r\nG#L>0,0,10,10:\r\nG#R>2,0:\r\n").unwrap());
        assert_eq!(editor.start_resolution(), TerminalResolution::Low);
        assert_eq!(editor.document.resolution(), TerminalResolution::High);
    }

    #[test]
    fn undo_closes_the_pattern_dialog_of_a_command() {
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        editor.open_pattern_dialog(None);
        let dialog = editor.pattern_dialog.take().unwrap();
        editor.apply_pattern(dialog);
        let index = editor.document.len() - 1;
        editor.open_pattern_dialog(Some(index));
        editor.undo(false);
        assert!(editor.pattern_dialog.is_none());
        let stale = PatternDialog::new(0, [0xFFFF; 16], Some(0));
        editor.apply_pattern(stale);
        assert!(matches!(editor.document.command(0), Some(IgsCommand::SetResolution { .. })));
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

    #[test]
    fn the_tool_grid_fills_the_sidebar() {
        for (width, spacing) in [(136.0, 8.0), (202.0, 8.0), (308.0, 6.0)] {
            let (columns, size) = tool_grid(width, spacing);
            let used = columns as f32 * size + (columns - 1) as f32 * spacing;
            assert!(size >= TOOL_BUTTON && width - used < columns as f32, "{width}: {columns} × {size} uses {used}");
        }
        assert_eq!(tool_grid(136.0, 8.0).0, 3, "the narrowest sidebar keeps three columns");
        assert_eq!(tool_grid(202.0, 8.0).0, 4, "a wider one adds columns rather than space");
    }

    #[test]
    fn the_tool_sidebar_widens_to_fit_its_labels() {
        let context = egui::Context::default();
        // Large text stands in for long translations such as "IGS-Palette bearbeiten…".
        context.style_mut(|style| {
            for font in style.text_styles.values_mut() {
                font.size *= 1.8;
            }
        });
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        let sidebar = || egui::containers::panel::PanelState::load(&context, egui::Id::new("igs-tools")).unwrap().rect;
        // The widest clip region along the left edge is where the sidebar is painted.
        let frame = |editor: &mut IgsEditor| {
            let output = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 820.0))),
                    ..Default::default()
                },
                |context| editor.show(context, false),
            );
            output
                .shapes
                .iter()
                .map(|shape| shape.clip_rect)
                .filter(|clip| clip.min.x <= 0.5 && clip.max.x < 640.0 && clip.height() > 400.0)
                .map(|clip| clip.width())
                .fold(0.0, f32::max)
        };
        frame(&mut editor);
        frame(&mut editor);
        let painted = frame(&mut editor);
        let panel = sidebar();
        assert!(panel.width() > SIDEBAR_WIDTH, "{panel:?}");
        assert_eq!(painted, panel.width(), "the sidebar paints all of the width it takes");
        assert!(editor.canvas_rect.unwrap().min.x >= panel.max.x);
    }

    #[test]
    fn line_ends_are_set_with_their_own_command() {
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        set(&mut editor, |wanted| wanted.line_thickness = 7);
        set(&mut editor, |wanted| wanted.line_ends = (ArrowEnd::Square, ArrowEnd::Arrow));
        let arrow = IgsCommand::SetLineOrMarkerStyle {
            style: LineMarkerStyle::LineEndpoints(LineKind::Solid, ArrowEnd::Square, ArrowEnd::Arrow),
        };
        assert_eq!(commands(&editor).last(), Some(&arrow));
        assert_eq!(editor.current().line_thickness, 7, "the ends keep the width");
        set(&mut editor, |wanted| wanted.line_ends = (ArrowEnd::Rounded, ArrowEnd::Arrow));
        assert_eq!(
            commands(&editor)
                .iter()
                .filter(|command| matches!(
                    command,
                    IgsCommand::SetLineOrMarkerStyle {
                        style: LineMarkerStyle::LineEndpoints(..)
                    }
                ))
                .count(),
            1
        );
        editor.select_tool(Tool::Line);
        editor.add_shape((0, 20), (40, 20));
        assert!(matches!(commands(&editor).last(), Some(IgsCommand::Line { .. })));
    }

    #[test]
    fn spray_areas_are_drawn_moved_and_limited_to_255_pixels() {
        let mut editor = IgsEditor::new(TerminalResolution::Low);
        editor.select_tool(Tool::Spray);
        editor.attributes.spray_density = 50;
        set(&mut editor, |wanted| wanted.marker = PolymarkerKind::Plus);
        editor.add_shape((20, 30), (80, 60));
        let index = editor.document.len() - 1;
        assert_eq!(
            editor.document.command(index),
            Some(&IgsCommand::SprayPaint {
                x: IgsParameter::Value(20),
                y: IgsParameter::Value(30),
                width: IgsParameter::Value(60),
                height: IgsParameter::Value(30),
                density: IgsParameter::Value(50),
            })
        );
        assert!(commands(&editor).contains(&IgsCommand::SetLineOrMarkerStyle {
            style: LineMarkerStyle::PolyMarkerSize(PolymarkerKind::Plus, 1)
        }));
        let (command, geometry) = editor.shape(index).unwrap();
        let moved = select::apply(&command, &select::translate(&geometry, &editor.canvas, 10, 5), &editor.canvas);
        assert!(matches!(
            moved,
            IgsCommand::SprayPaint {
                x: IgsParameter::Value(30),
                y: IgsParameter::Value(35),
                width: IgsParameter::Value(60),
                ..
            }
        ));
        let wide = select::apply(&command, &Geometry::Rect { x0: 0, y0: 0, x1: 319, y1: 10 }, &editor.canvas);
        assert!(matches!(
            wide,
            IgsCommand::SprayPaint {
                width: IgsParameter::Value(255),
                ..
            }
        ));

        // The color rotation switch is not an area.
        let document = IgsDocument::from_bytes(b"G#X>0,1,0,0,0,0:\r\n").unwrap();
        assert_eq!(shape_tool(document.command(0).unwrap()), None);
        assert_eq!(item_name(&document.items()[0]), fl!("igs-command-spray-rotation"));
    }

    #[test]
    fn delayed_color_rotation_plays_one_shift_per_step() {
        let document = IgsDocument::from_bytes(b"G#R>0,2:X>1,1,1792:X>1,2,112:X>1,3,7:X>8,1,3,2,10:").unwrap();
        let rotation = document.command(document.len() - 1).unwrap();
        assert_eq!((steps(rotation), delay_ms(rotation)), (2, 50));
        let register = |steps| {
            let preview = IgsDocument::render_steps(document.items(), steps).unwrap();
            preview.screen().palette().rgb(1)
        };
        assert_eq!(register(Some(1)), (0, 0, 238), "the first shift brings the last color to the front");
        assert_eq!(register(Some(2)), (0, 238, 0));
        assert_eq!(register(None), (0, 238, 0));
    }

    #[test]
    fn extended_commands_have_property_panels() {
        let context = egui::Context::default();
        let palette = icy_engine::Palette::default();
        for source in [
            &b"G#X>0,1,0,0,0,0:"[..],
            b"G#X>8,1,15,20,10:",
            b"G#X>1,4,1911:",
            b"G#n>0,0,15,40,50,0:",
            b"G#b>23,2:",
            b"G#k>0:",
            b"G#v>1:",
            b"G#c>1,3:",
            b"G#<>1,0,1:",
        ] {
            let document = IgsDocument::from_bytes(source).unwrap();
            assert_eq!(document.len(), 1, "{}", String::from_utf8_lossy(source));
            let mut command = document.command(0).unwrap().clone();
            let _ = context.run(egui::RawInput::default(), |context| {
                egui::CentralPanel::default().show(context, |ui| {
                    assert!(
                        properties::command(ui, &mut command, &palette, TerminalResolution::Low),
                        "{}",
                        String::from_utf8_lossy(source)
                    );
                });
            });
            assert_eq!(&command, document.command(0).unwrap(), "showing the panel changes nothing");
        }
    }
}
