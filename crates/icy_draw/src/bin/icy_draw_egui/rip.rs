use eframe::egui::{self, Color32, Stroke};
use icy_draw::{fl, rip_document::RipDocument};
use icy_engine::Screen;
use icy_parser_core::{FillStyle, LineStyle, RipCommand};
use std::path::Path;

use super::widgets::{self, Icons};

#[path = "rip_button.rs"]
mod button;
use button::{ButtonDialog, ButtonKind, ButtonOptions, ButtonTarget, DialogResult};

const WIDTH: u16 = 640;
const HEIGHT: u16 = 350;
const TOOLBAR_HEIGHT: f32 = 44.0;
/// Screen distance in points within which a Bézier handle is picked up.
const HANDLE_RADIUS: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Pixel,
    Line,
    Rectangle,
    Bar,
    Circle,
    Oval,
    FilledOval,
    Text,
    Bezier,
    Button,
}

impl Tool {
    const ALL: [Self; 10] = [
        Self::Pixel,
        Self::Line,
        Self::Rectangle,
        Self::Bar,
        Self::Circle,
        Self::Oval,
        Self::FilledOval,
        Self::Text,
        Self::Bezier,
        Self::Button,
    ];

    fn label(self) -> String {
        match self {
            Self::Pixel => fl!("rip-editor-pixel"),
            Self::Line => fl!("rip-editor-line"),
            Self::Rectangle => fl!("rip-editor-rectangle"),
            Self::Bar => fl!("rip-editor-bar"),
            Self::Circle => fl!("rip-editor-circle"),
            Self::Oval => fl!("rip-editor-outline-oval"),
            Self::FilledOval => fl!("rip-editor-oval"),
            Self::Text => fl!("rip-editor-text"),
            Self::Bezier => fl!("rip-editor-bezier"),
            Self::Button => fl!("rip-editor-button"),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Pixel => "pencil",
            Self::Line => "line",
            Self::Rectangle => "rectangle_outline",
            Self::Bar => "rectangle_filled",
            Self::Circle | Self::Oval => "ellipse_outline",
            Self::FilledOval => "ellipse_filled",
            Self::Text => "text",
            Self::Bezier => "bezier",
            Self::Button => "rip_button",
        }
    }

    fn uses_line_style(self) -> bool {
        matches!(self, Self::Line | Self::Rectangle | Self::Circle | Self::Oval | Self::Bezier)
    }

    fn uses_fill(self) -> bool {
        matches!(self, Self::Bar | Self::FilledOval)
    }

    /// Whether the drawn shape needs a drag rather than a click.
    fn is_dragged(self) -> bool {
        !matches!(self, Self::Pixel | Self::Text)
    }

    /// The shape command itself, without the color and style commands before it.
    fn command(self, from: (u16, u16), to: (u16, u16), text: &str) -> RipCommand {
        let ((x0, y0), (x1, y1)) = (from, to);
        match self {
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
        }
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

/// A Bézier curve whose four points can still be moved.
#[derive(Clone, Copy, Debug, PartialEq)]
struct BezierEdit {
    points: [(u16, u16); 4],
    moving: Option<usize>,
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
        RipCommand::Circle { x_center, y_center, radius } => format!("{x_center}, {y_center} · r {radius}"),
        RipCommand::Oval { x, y, x_rad, y_rad, .. } | RipCommand::FilledOval { x, y, x_rad, y_rad } => {
            format!("{x}, {y} · {x_rad} × {y_rad}")
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

/// A readable list entry; buttons show their label.
fn list_label(command: &RipCommand) -> String {
    match command {
        RipCommand::Button { text, .. } => {
            let label = text.split("<>").nth(1).unwrap_or(text);
            let details = describe(command).unwrap_or_default();
            format!("{} \"{label}\" · {details}", fl!("rip-editor-button"))
        }
        RipCommand::ButtonStyle { .. } => fl!("rip-button-style-entry"),
        _ => format!("{command:?}"),
    }
}

fn command_field(ui: &mut egui::Ui, name: &str, value: &mut u16, maximum: u16) {
    ui.horizontal(|ui| {
        ui.label(name);
        ui.add(egui::DragValue::new(value).range(0..=maximum));
    });
}

fn command_properties(ui: &mut egui::Ui, command: &mut RipCommand) -> bool {
    match command {
        RipCommand::Color { c } => command_field(ui, "c", c, 15),
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
    active_color: ColorRole,
    icons: Icons,
    text: String,
    line_style: LineStyle,
    line_thickness: u16,
    fill_pattern: FillStyle,
    text_font: u16,
    text_size: u16,
    text_direction: u16,
    bezier_segments: u16,
    button: ButtonOptions,
    button_dialog: Option<ButtonDialog>,
    bezier: Option<BezierEdit>,
    drag: Option<((u16, u16), (u16, u16))>,
    hover: Option<(u16, u16)>,
    palette: icy_engine::Palette,
    texture: Option<egui::TextureHandle>,
    preview_dirty: bool,
    /// The unfinished shape the preview currently shows.
    shown_pending: Vec<RipCommand>,
    error: Option<String>,
    #[cfg(test)]
    canvas_rect: Option<egui::Rect>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ColorRole {
    Draw,
    Border,
    Fill,
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
            active_color: ColorRole::Draw,
            icons: Icons::default(),
            text: String::new(),
            line_style: LineStyle::Solid,
            line_thickness: 1,
            fill_pattern: FillStyle::Solid,
            text_font: 0,
            text_size: 1,
            text_direction: 0,
            bezier_segments: 32,
            button: ButtonOptions::default(),
            button_dialog: None,
            bezier: None,
            drag: None,
            hover: None,
            palette: icy_engine::Palette::dos_default(),
            texture: None,
            preview_dirty: true,
            shown_pending: Vec::new(),
            error: None,
            #[cfg(test)]
            canvas_rect: None,
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.document.path()
    }

    pub fn save(&mut self, path: &Path, overwrite: bool) -> Result<(), String> {
        self.finish_bezier();
        self.document.save_as(path, overwrite).map_err(|error| error.to_string())
    }

    pub fn modified(&self) -> bool {
        self.document.is_dirty() || self.bezier.is_some()
    }

    pub fn recovery_snapshot(&self) -> Result<icy_draw::recovery::Snapshot, String> {
        self.document.recovery_snapshot().map_err(|error| error.to_string())
    }

    pub fn from_recovery(snapshot: &icy_draw::recovery::Snapshot) -> Result<Self, String> {
        Ok(Self::from_document(RipDocument::from_recovery(snapshot).map_err(|error| error.to_string())?))
    }

    pub fn can_undo(&self) -> bool {
        self.document.can_undo() || self.bezier.is_some()
    }

    pub fn can_redo(&self) -> bool {
        self.document.can_redo()
    }

    pub fn undo(&mut self, redo: bool) {
        // Undo first drops an unfinished curve.
        if !redo && self.bezier.take().is_some() {
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
        self.finish_bezier();
        self.tool = tool;
        self.drag = None;
        self.preview_dirty = true;
    }

    /// Color and style commands that make the scene's state match the tool settings.
    fn state_commands(&self, tool: Tool) -> Vec<RipCommand> {
        let state = DrawingState::at_end(self.document.commands());
        let mut commands = Vec::new();
        let color = match tool {
            Tool::Bar | Tool::Button => None,
            Tool::Rectangle | Tool::Circle | Tool::Oval | Tool::FilledOval => Some(self.border_color),
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
        let shape = match self.tool {
            Tool::Text if self.text.is_empty() => return Err(fl!("rip-editor-label-required")),
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
            tool => tool.command(from, to, &self.text),
        };
        let mut commands = self.state_commands(self.tool);
        commands.push(shape);
        Ok(commands)
    }

    fn bezier_commands(&self, edit: &BezierEdit) -> Vec<RipCommand> {
        let mut commands = self.state_commands(Tool::Bezier);
        commands.push(bezier_command(edit.points, self.bezier_segments));
        commands
    }

    /// The unfinished shape: a drag in progress or a Bézier curve being adjusted.
    fn pending_commands(&self) -> Vec<RipCommand> {
        if let Some(edit) = &self.bezier {
            return self.bezier_commands(edit);
        }
        match self.drag {
            Some((from, to)) if self.tool.is_dragged() => self.shape_commands(from, to).unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn refresh_preview(&mut self, context: &egui::Context) {
        let pending = if self.preview_to_selection { Vec::new() } else { self.pending_commands() };
        if pending != self.shown_pending {
            self.shown_pending = pending;
            self.preview_dirty = true;
        }
        if !self.preview_dirty {
            return;
        }
        let preview = if self.preview_to_selection {
            self.document.preview_through(self.selected)
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
            Ok(commands) => self.add_commands(commands),
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
        if self.bezier.take().is_some() || self.drag.take().is_some() {
            self.preview_dirty = true;
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
            match self.tool {
                Tool::Text => {
                    ui.add(
                        icy_engine_gui::egui::appearance::text_edit(&mut self.text)
                            .hint_text(fl!("rip-editor-label"))
                            .desired_width(180.0),
                    );
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
                }
                Tool::Bezier => {
                    ui.weak(fl!("rip-bezier-segments"));
                    ui.add(egui::DragValue::new(&mut self.bezier_segments).range(2..=200));
                    if self.bezier.is_some() {
                        ui.weak(fl!("rip-bezier-adjust-hint"));
                    }
                }
                Tool::Button => {
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
            let parameters = self
                .shown_pending
                .last()
                .and_then(describe)
                .or_else(|| self.hover.map(|(x, y)| format!("{x}, {y}")));
            if let Some(parameters) = parameters {
                widgets::divider(ui);
                ui.label(egui::RichText::new(parameters).monospace().color(ui.visuals().weak_text_color()));
            }
        });
    }

    fn command_list(&mut self, context: &egui::Context, blocked: bool) {
        egui::SidePanel::right("rip-commands")
            .default_width(260.0)
            .min_width(180.0)
            .show(context, |ui| {
                if blocked {
                    ui.disable();
                }
                ui.heading(fl!("rip-editor-commands"));
                if ui.checkbox(&mut self.preview_to_selection, fl!("rip-editor-preview-through")).changed() {
                    self.preview_dirty = true;
                }
                if self.document.preserved_commands() > 0 {
                    ui.weak(fl!("rip-editor-preserved"));
                }
                ui.horizontal(|ui| {
                    let selected = self.selected;
                    let editable = selected.is_some_and(|index| index >= self.document.preserved_commands());
                    if ui
                        .add_enabled(
                            editable && selected.is_some_and(|index| index > self.document.preserved_commands()),
                            egui::Button::new("↑"),
                        )
                        .on_hover_text(fl!("rip-editor-up"))
                        .clicked()
                    {
                        let index = selected.unwrap();
                        match self.document.move_command(index, index - 1) {
                            Ok(()) => {
                                self.selected = Some(index - 1);
                                self.editing = None;
                                self.preview_dirty = true;
                            }
                            Err(error) => self.error = Some(error.to_string()),
                        }
                    }
                    if ui
                        .add_enabled(
                            editable && selected.is_some_and(|index| index + 1 < self.document.commands().len()),
                            egui::Button::new("↓"),
                        )
                        .on_hover_text(fl!("rip-editor-down"))
                        .clicked()
                    {
                        let index = selected.unwrap();
                        match self.document.move_command(index, index + 1) {
                            Ok(()) => {
                                self.selected = Some(index + 1);
                                self.editing = None;
                                self.preview_dirty = true;
                            }
                            Err(error) => self.error = Some(error.to_string()),
                        }
                    }
                    if ui
                        .add_enabled(editable, egui::Button::new("×"))
                        .on_hover_text(fl!("rip-editor-delete"))
                        .clicked()
                    {
                        match self.document.delete(selected.unwrap()) {
                            Ok(_) => {
                                self.selected = None;
                                self.editing = None;
                                self.preview_dirty = true;
                            }
                            Err(error) => self.error = Some(error.to_string()),
                        }
                    }
                });
                ui.separator();
                let previous = self.selected;
                let selected_button = self.selected.and_then(|index| ButtonTarget::owning(self.document.commands(), index));
                egui::ScrollArea::vertical()
                    .max_height((ui.available_height() - 240.0).max(120.0))
                    .show(ui, |ui| {
                        for (index, command) in self.document.commands().iter().enumerate() {
                            let name = format!("{}. {}", index + 1, list_label(command));
                            let label: String = name.chars().take(72).collect();
                            // A button's style and font commands are highlighted with it.
                            let part_of_button =
                                selected_button.is_some_and(|target| target.button == index || target.style == Some(index) || target.font == Some(index));
                            let response = ui.selectable_label(self.selected == Some(index) || part_of_button, label).on_hover_text(name);
                            if response.clicked() {
                                self.selected = Some(index);
                            }
                            if index < self.document.preserved_commands() {
                                response.on_hover_text(fl!("rip-editor-preserved"));
                            }
                        }
                    });
                if self.selected != previous {
                    self.editing = None;
                    if self.preview_to_selection {
                        self.preview_dirty = true;
                    }
                }
                if self.editing.is_none() {
                    self.editing = self
                        .selected
                        .and_then(|index| self.document.commands().get(index).cloned().map(|cmd| (index, cmd)));
                }
                let button_target = self.selected.and_then(|index| ButtonTarget::owning(self.document.commands(), index));
                let mut open_button = None;
                if let Some((index, draft)) = self.editing.as_mut() {
                    ui.separator();
                    egui::ScrollArea::vertical().id_salt("rip-properties").show(ui, |ui| {
                        ui.label(fl!("rip-editor-properties"));
                        if *index < self.document.preserved_commands() {
                            ui.weak(fl!("rip-editor-preserved"));
                        } else if let Some(target) = button_target {
                            if ui.button(fl!("rip-button-edit")).clicked() {
                                open_button = Some(target);
                            }
                            ui.weak(fl!("rip-button-edit-hint"));
                        } else if command_properties(ui, draft) {
                            if ui.button(fl!("rip-editor-apply")).clicked() {
                                match self.document.replace(*index, draft.clone()) {
                                    Ok(()) => self.preview_dirty = true,
                                    Err(error) => self.error = Some(error.to_string()),
                                }
                            }
                        } else {
                            ui.weak(fl!("rip-editor-unsupported-properties"));
                        }
                    });
                }
                if let Some(target) = open_button {
                    self.open_button_dialog(Some(target));
                }
            });
    }

    fn sidebar(&mut self, context: &egui::Context, blocked: bool) {
        let panel_fill = context.style().visuals.panel_fill;
        egui::SidePanel::left("rip-tools")
            .exact_width(136.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(panel_fill).inner_margin(egui::Margin::symmetric(6, 6)))
            .show(context, |ui| {
                ui.add_enabled_ui(!blocked, |ui| {
                    ui.vertical(|ui| {
                        for (role, label, color) in [
                            (ColorRole::Draw, fl!("rip-editor-color"), self.draw_color),
                            (ColorRole::Border, fl!("rip-editor-border-color"), self.border_color),
                            (ColorRole::Fill, fl!("rip-editor-fill-color"), self.fill_color),
                        ] {
                            ui.horizontal(|ui| {
                                if ui.selectable_label(self.active_color == role, label).clicked()
                                    || ui
                                        .add_sized([18.0, 18.0], egui::Button::new("").fill(button::color32(&self.palette, color)))
                                        .clicked()
                                {
                                    self.active_color = role;
                                }
                            });
                        }
                    });
                    egui::Grid::new("rip-colors").spacing(egui::vec2(3.0, 3.0)).show(ui, |ui| {
                        for color in 0..16u16 {
                            let selected = match self.active_color {
                                ColorRole::Draw => self.draw_color,
                                ColorRole::Border => self.border_color,
                                ColorRole::Fill => self.fill_color,
                            } == color;
                            let response = ui.add_sized(
                                [24.0, 24.0],
                                egui::Button::new("")
                                    .fill(button::color32(&self.palette, color))
                                    .stroke(Stroke::new(if selected { 2.0 } else { 0.0 }, ui.visuals().strong_text_color())),
                            );
                            if response.clicked() {
                                *match self.active_color {
                                    ColorRole::Draw => &mut self.draw_color,
                                    ColorRole::Border => &mut self.border_color,
                                    ColorRole::Fill => &mut self.fill_color,
                                } = color;
                            }
                            if color % 4 == 3 {
                                ui.end_row();
                            }
                        }
                    });
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
                });
            });
    }

    pub fn show(&mut self, context: &egui::Context, blocked: bool) {
        let blocked = blocked || self.button_dialog.is_some();
        let panel_fill = context.style().visuals.panel_fill;
        egui::TopBottomPanel::top("rip-toolbar")
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
            if self.bezier.is_some() && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
                self.finish_bezier();
            }
        }
        egui::CentralPanel::default().show(context, |ui| {
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            self.canvas(ui, blocked);
        });
        if let Some(dialog) = &mut self.button_dialog {
            match dialog.show(context, &self.palette) {
                DialogResult::Open => {}
                DialogResult::Cancel => self.button_dialog = None,
                DialogResult::Apply(options) => {
                    let target = dialog.target;
                    self.button_dialog = None;
                    self.apply_button(target, options);
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

    fn canvas_input(&mut self, ui: &egui::Ui, response: &egui::Response, at: &dyn Fn(egui::Pos2) -> (u16, u16), on_screen: &dyn Fn((u16, u16)) -> egui::Pos2) {
        let pointer = response.interact_pointer_pos();
        let origin = ui.input(|input| input.pointer.press_origin());
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
    fn drawing_tools_create_pixel_coordinate_commands() {
        assert_eq!(Tool::Pixel.command((12, 34), (12, 34), ""), RipCommand::Pixel { x: 12, y: 34 });
        assert_eq!(
            Tool::Line.command((12, 34), (56, 78), ""),
            RipCommand::Line {
                x0: 12,
                y0: 34,
                x1: 56,
                y1: 78
            }
        );
        assert_eq!(
            Tool::Rectangle.command((12, 34), (56, 78), ""),
            RipCommand::Rectangle {
                x0: 12,
                y0: 34,
                x1: 56,
                y1: 78
            }
        );
        assert_eq!(
            Tool::Bar.command((12, 34), (56, 78), ""),
            RipCommand::Bar {
                x0: 12,
                y0: 34,
                x1: 56,
                y1: 78
            }
        );
        assert_eq!(
            Tool::Circle.command((12, 34), (15, 38), ""),
            RipCommand::Circle {
                x_center: 12,
                y_center: 34,
                radius: 5
            }
        );
        assert_eq!(
            Tool::FilledOval.command((12, 34), (56, 78), ""),
            RipCommand::FilledOval {
                x: 12,
                y: 34,
                x_rad: 44,
                y_rad: 44
            }
        );
        assert_eq!(
            Tool::Oval.command((12, 34), (56, 78), ""),
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
        editor.text = "Hello".into();
        editor.add_shape((80, 80), (80, 80));
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
        assert!(matches!(Tool::Bezier.command((0, 0), (30, 30), ""), RipCommand::Bezier { x2: 10, y2: 10, .. }));
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
        editor.button.label = "Next".into();
        editor.add_shape((200, 100), (260, 130));
        assert!(matches!(editor.document.commands()[3..], [RipCommand::Button { .. }]));
    }
}
