use eframe::egui::{self, Color32, Stroke};
use icy_draw::{
    fl,
    skypix_document::{SkypixDocument, SkypixItem},
};
use icy_engine::Screen;
use icy_parser_core::{DisplayMode, FillMode, SkypixCommand};
use std::path::Path;

use super::{
    command_list::{CommandRow, SwatchLayout, Tone, ROW_HEIGHT},
    playback::RowMark,
    widgets::{self, Icons},
};

const WIDTH: i32 = 640;
const HEIGHT: i32 = 200;
type Point = (i32, i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PreviewKey {
    revision: u64,
    count: Option<usize>,
    dragged_line: Option<(Point, Point)>,
}

#[derive(Clone, Copy)]
struct CanvasTransform {
    rect: egui::Rect,
}

impl CanvasTransform {
    fn scale(self) -> egui::Vec2 {
        self.rect.size() / egui::vec2(WIDTH as f32, HEIGHT as f32)
    }

    fn pixel(self, position: egui::Pos2) -> Point {
        let pixel = (position - self.rect.min) / self.scale();
        (
            pixel.x.floor().clamp(0.0, (WIDTH - 1) as f32) as i32,
            pixel.y.floor().clamp(0.0, (HEIGHT - 1) as f32) as i32,
        )
    }

    fn position(self, point: Point) -> egui::Pos2 {
        self.rect.min + egui::vec2(point.0 as f32 + 0.5, point.1 as f32 + 0.5) * self.scale()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Select,
    Pixel,
    Line,
    Rectangle,
    FilledRectangle,
    Ellipse,
    FilledEllipse,
    Fill,
    Text,
    GrabBrush,
    Stamp,
}

impl Tool {
    const ALL: [Self; 11] = [
        Self::Select,
        Self::Pixel,
        Self::Line,
        Self::Rectangle,
        Self::FilledRectangle,
        Self::Ellipse,
        Self::FilledEllipse,
        Self::Fill,
        Self::Text,
        Self::GrabBrush,
        Self::Stamp,
    ];

    fn label(self) -> String {
        match self {
            Self::Select => fl!("skypix-editor-select"),
            Self::Pixel => fl!("skypix-editor-pixel"),
            Self::Line => fl!("skypix-editor-line"),
            Self::Rectangle => fl!("skypix-editor-rectangle"),
            Self::FilledRectangle => fl!("skypix-editor-filled-rectangle"),
            Self::Ellipse => fl!("skypix-editor-ellipse"),
            Self::FilledEllipse => fl!("skypix-editor-filled-ellipse"),
            Self::Fill => fl!("skypix-editor-fill"),
            Self::Text => fl!("skypix-editor-text"),
            Self::GrabBrush => fl!("skypix-editor-grab-brush"),
            Self::Stamp => fl!("skypix-editor-stamp"),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Select => "cursor",
            Self::Pixel => "pencil",
            Self::Line => "line",
            Self::Rectangle => "box_outline",
            Self::FilledRectangle => "rectangle_filled",
            Self::Ellipse => "ellipse_outline",
            Self::FilledEllipse => "ellipse_filled",
            Self::Fill => "fill",
            Self::Text => "text",
            Self::GrabBrush => "select",
            Self::Stamp => "paint_brush",
        }
    }

    fn dragged(self) -> bool {
        matches!(
            self,
            Self::Select | Self::Line | Self::Rectangle | Self::FilledRectangle | Self::Ellipse | Self::FilledEllipse | Self::GrabBrush
        )
    }
}

fn text_string(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&byte| match byte {
            b'\r' | b'\n' | b'\t' => char::from(byte),
            _ => icy_engine::BufferType::CP437.convert_to_unicode(char::from(byte)),
        })
        .collect()
}

fn item_name(item: &SkypixItem) -> String {
    match item {
        SkypixItem::Text(_) => fl!("skypix-editor-text"),
        SkypixItem::Raw(_) => fl!("skypix-command-preserved"),
        SkypixItem::Command(command) => match command {
            SkypixCommand::SetPixel { .. } => Tool::Pixel.label(),
            SkypixCommand::DrawLine { .. } => Tool::Line.label(),
            SkypixCommand::RectangleFill { .. } => Tool::FilledRectangle.label(),
            SkypixCommand::Ellipse { .. } => Tool::Ellipse.label(),
            SkypixCommand::FilledEllipse { .. } => Tool::FilledEllipse.label(),
            SkypixCommand::AreaFill { .. } => Tool::Fill.label(),
            SkypixCommand::GrabBrush { .. } => Tool::GrabBrush.label(),
            SkypixCommand::UseBrush { .. } => Tool::Stamp.label(),
            SkypixCommand::NewPalette { .. } => fl!("skypix-editor-palette"),
            SkypixCommand::ResetPalette => fl!("skypix-editor-reset-palette"),
            SkypixCommand::SetFont { .. } => fl!("skypix-editor-font"),
            SkypixCommand::ResetFont => fl!("skypix-command-reset-font"),
            SkypixCommand::SetPenA { .. } => fl!("skypix-editor-pen"),
            SkypixCommand::SetPenB { .. } => fl!("skypix-editor-background"),
            SkypixCommand::PositionCursor { .. } => fl!("skypix-command-text-position"),
            SkypixCommand::MovePen { .. } => fl!("skypix-command-move-pen"),
            SkypixCommand::SetDisplayMode { .. } => fl!("skypix-editor-display-mode"),
            SkypixCommand::Delay { .. } => fl!("skypix-command-delay"),
            SkypixCommand::Comment { .. } => fl!("skypix-command-comment"),
            SkypixCommand::PlaySample { .. } => fl!("skypix-command-sample"),
            SkypixCommand::CrcTransfer { .. } => fl!("skypix-command-transfer"),
            SkypixCommand::ControllerReturn { .. } => fl!("skypix-command-controller"),
            SkypixCommand::DefineGadget { .. } => fl!("skypix-command-gadget"),
            SkypixCommand::EndSkypix => fl!("skypix-command-end"),
        },
    }
}

fn item_summary(items: &[SkypixItem], index: usize) -> String {
    match &items[index] {
        SkypixItem::Text(bytes) => format!("\"{}\"", text_string(bytes).escape_debug()),
        SkypixItem::Raw(bytes) => fl!("skypix-command-bytes", count = bytes.len()),
        SkypixItem::Command(command) => match command {
            SkypixCommand::SetPixel { x, y } | SkypixCommand::MovePen { x, y } | SkypixCommand::PositionCursor { x, y } => format!("{x}, {y}"),
            SkypixCommand::DrawLine { x, y } => {
                let (start_x, start_y) = pen_before(items, index);
                format!("{start_x}, {start_y} → {x}, {y}")
            }
            SkypixCommand::RectangleFill { x1, y1, x2, y2 } => format!(
                "{}, {} · {} × {}",
                x1.min(x2),
                y1.min(y2),
                u64::from(x1.abs_diff(*x2)) + 1,
                u64::from(y1.abs_diff(*y2)) + 1
            ),
            SkypixCommand::Ellipse { x, y, a, b } | SkypixCommand::FilledEllipse { x, y, a, b } => format!("{x}, {y} · {a} × {b}"),
            SkypixCommand::AreaFill { mode, x, y } => format!(
                "{x}, {y} · {}",
                match mode {
                    FillMode::Color => fl!("skypix-editor-fill-color"),
                    FillMode::Outline => fl!("skypix-editor-fill-outline"),
                }
            ),
            SkypixCommand::GrabBrush { x1, y1, width, height } => format!("{x1}, {y1} · {width} × {height}"),
            SkypixCommand::UseBrush {
                src_x,
                src_y,
                dst_x,
                dst_y,
                width,
                height,
                ..
            } => format!("{src_x}, {src_y} → {dst_x}, {dst_y} · {width} × {height}"),
            SkypixCommand::SetPenA { color } | SkypixCommand::SetPenB { color } => color.to_string(),
            SkypixCommand::SetFont { name, size } => format!("{name} · {size} px"),
            SkypixCommand::SetDisplayMode { mode } => match mode {
                DisplayMode::EightColors => fl!("skypix-editor-eight-colors"),
                DisplayMode::SixteenColors => fl!("skypix-editor-sixteen-colors"),
            },
            SkypixCommand::NewPalette { colors } => fl!("skypix-command-colors", count = colors.len()),
            SkypixCommand::Delay { jiffies } => format!("{jiffies} · {:.2} s", f64::from(*jiffies) / 60.0),
            SkypixCommand::Comment { text } => format!("\"{}\"", text.escape_debug()),
            SkypixCommand::PlaySample { speed, start, end, loops } => format!("{speed} · {start}–{end} · ×{loops}"),
            SkypixCommand::CrcTransfer { mode, filename, width, height } => format!("{filename} · {mode} · {width} × {height}"),
            SkypixCommand::ControllerReturn { c, x, y } => format!("{c} · {x}, {y}"),
            SkypixCommand::DefineGadget { num, cmd, x1, y1, x2, y2 } => format!("#{num} · {cmd} · {x1}, {y1} → {x2}, {y2}"),
            SkypixCommand::ResetFont | SkypixCommand::ResetPalette | SkypixCommand::EndSkypix => String::new(),
        },
    }
}

fn item_icon(item: &SkypixItem) -> &'static str {
    match item {
        SkypixItem::Text(_) => "text",
        SkypixItem::Raw(_) => "file_copy",
        SkypixItem::Command(command) => match command {
            SkypixCommand::SetPixel { .. } => Tool::Pixel.icon(),
            SkypixCommand::DrawLine { .. } => Tool::Line.icon(),
            SkypixCommand::RectangleFill { .. } => Tool::FilledRectangle.icon(),
            SkypixCommand::Ellipse { .. } => Tool::Ellipse.icon(),
            SkypixCommand::FilledEllipse { .. } => Tool::FilledEllipse.icon(),
            SkypixCommand::AreaFill { .. } => Tool::Fill.icon(),
            SkypixCommand::GrabBrush { .. } => Tool::GrabBrush.icon(),
            SkypixCommand::UseBrush { .. } => Tool::Stamp.icon(),
            SkypixCommand::SetFont { .. } | SkypixCommand::ResetFont => "font",
            SkypixCommand::MovePen { .. } => "move",
            SkypixCommand::PositionCursor { .. } => "cursor",
            SkypixCommand::Comment { .. } => "tag",
            SkypixCommand::Delay { .. } => "pause",
            SkypixCommand::PlaySample { .. } => "play",
            SkypixCommand::CrcTransfer { .. } => "navigate_next",
            SkypixCommand::ControllerReturn { .. } => "rip_mouse",
            SkypixCommand::DefineGadget { .. } => "rip_button",
            SkypixCommand::EndSkypix => "stop",
            SkypixCommand::NewPalette { .. }
            | SkypixCommand::ResetPalette
            | SkypixCommand::SetPenA { .. }
            | SkypixCommand::SetPenB { .. }
            | SkypixCommand::SetDisplayMode { .. } => "paint_brush",
        },
    }
}

fn item_swatch(item: &SkypixItem, palette: &icy_engine::Palette) -> Option<Color32> {
    match item {
        SkypixItem::Command(SkypixCommand::SetPenA { color } | SkypixCommand::SetPenB { color }) if (0..16).contains(color) => {
            let (r, g, b) = palette.rgb(*color as u32);
            Some(Color32::from_rgb(r, g, b))
        }
        _ => None,
    }
}

fn pen_before(items: &[SkypixItem], count: usize) -> Point {
    let mut pen = (0, 0);
    for item in &items[..count] {
        if let SkypixItem::Command(SkypixCommand::MovePen { x, y } | SkypixCommand::DrawLine { x, y } | SkypixCommand::PositionCursor { x, y }) = item {
            pen = (*x, *y);
        }
    }
    pen
}

fn bounds(items: &[SkypixItem], index: usize) -> Option<(Point, Point)> {
    let SkypixItem::Command(command) = items.get(index)? else {
        return None;
    };
    Some(match command {
        SkypixCommand::SetPixel { x, y } | SkypixCommand::PositionCursor { x, y } => ((*x, *y), (*x, *y)),
        SkypixCommand::DrawLine { x, y } => (pen_before(items, index), (*x, *y)),
        SkypixCommand::RectangleFill { x1, y1, x2, y2 } => ((*x1, *y1), (*x2, *y2)),
        SkypixCommand::Ellipse { x, y, a, b } | SkypixCommand::FilledEllipse { x, y, a, b } => {
            ((x.saturating_sub(*a), y.saturating_sub(*b)), (x.saturating_add(*a), y.saturating_add(*b)))
        }
        SkypixCommand::GrabBrush { x1, y1, width, height } => (
            (*x1, *y1),
            (x1.saturating_add(*width).saturating_sub(1), y1.saturating_add(*height).saturating_sub(1)),
        ),
        SkypixCommand::UseBrush {
            dst_x, dst_y, width, height, ..
        } => (
            (*dst_x, *dst_y),
            (dst_x.saturating_add(*width).saturating_sub(1), dst_y.saturating_add(*height).saturating_sub(1)),
        ),
        _ => return None,
    })
}

fn coordinate(ui: &mut egui::Ui, label: &str, x: &mut i32, y: &mut i32, size: bool) {
    ui.horizontal(|ui| {
        ui.label(label);
        let low = if size { 1 } else { 0 };
        ui.add(egui::DragValue::new(x).range(low..=if size { WIDTH } else { WIDTH - 1 }).prefix("X: "));
        ui.add(egui::DragValue::new(y).range(low..=if size { HEIGHT } else { HEIGHT - 1 }).prefix("Y: "));
    });
}

pub struct SkypixEditor {
    pub(super) document: SkypixDocument,
    tool: Tool,
    pen: u8,
    background: u8,
    fill_mode: FillMode,
    text: String,
    fonts: Vec<(String, i32)>,
    font: Option<usize>,
    selected: Option<usize>,
    command_filter: String,
    listed_selection: Option<usize>,
    visible_rows: std::ops::Range<usize>,
    draft: Option<SkypixItem>,
    line_start: Option<Point>,
    text_draft: String,
    preview_selection: bool,
    palette: icy_engine::Palette,
    palette_draft: [[u8; 3]; 16],
    palette_original: [[u8; 3]; 16],
    palette_revision: Option<u64>,
    colors: usize,
    texture: Option<egui::TextureHandle>,
    shown: Option<PreviewKey>,
    drag: Option<(Point, Point)>,
    drag_handle: Option<bool>,
    zoom: f32,
    fit: bool,
    aspect: bool,
    canvas_rect: egui::Rect,
    icons: Icons,
    error: Option<String>,
    warnings: Vec<String>,
}

impl SkypixEditor {
    pub fn new() -> Self {
        Self::from_document(SkypixDocument::new())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        Ok(Self::from_document(SkypixDocument::load(path).map_err(|error| error.to_string())?))
    }

    fn from_document(document: SkypixDocument) -> Self {
        let fonts = icy_engine::skypix::load_amiga_fonts()
            .into_iter()
            .map(|(name, size, _, _)| (name, size))
            .collect();
        Self {
            document,
            tool: Tool::Line,
            pen: 7,
            background: 0,
            fill_mode: FillMode::Color,
            text: String::new(),
            fonts,
            font: None,
            selected: None,
            command_filter: String::new(),
            listed_selection: None,
            visible_rows: 0..0,
            draft: None,
            line_start: None,
            text_draft: String::new(),
            preview_selection: false,
            palette: icy_engine::Palette::from_slice(&icy_engine::SKYPIX_PALETTE),
            palette_draft: [[0; 3]; 16],
            palette_original: [[0; 3]; 16],
            palette_revision: None,
            colors: 16,
            texture: None,
            shown: None,
            drag: None,
            drag_handle: None,
            zoom: 1.0,
            fit: true,
            aspect: true,
            canvas_rect: egui::Rect::NOTHING,
            icons: Icons::default(),
            error: None,
            warnings: Vec::new(),
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.document.path()
    }

    pub(super) fn apply_ai_items(&mut self, items: Vec<SkypixItem>) -> Result<(), String> {
        let edited = self
            .selected
            .zip(self.draft.as_ref())
            .is_some_and(|(index, item)| self.document.items().get(index) != Some(item));
        let text_edited = self.text_draft != self.draft.as_ref().and_then(SkypixItem::as_text).map(text_string).unwrap_or_default();
        let palette_edited = self.palette_revision.is_some() && self.palette_draft != self.palette_original;
        let line_edited = self
            .line_start
            .is_some_and(|start| self.selected.is_none_or(|index| start != pen_before(self.document.items(), index)));
        if edited || text_edited || palette_edited || line_edited || self.drag.is_some() || self.drag_handle.is_some() {
            return Err(fl!("ai-chat-apply-stale-skypix"));
        }
        self.document.set_items(items).map_err(|error| error.to_string())?;
        self.select(None);
        self.listed_selection = None;
        self.line_start = None;
        self.shown = None;
        self.palette_revision = None;
        self.error = None;
        Ok(())
    }

    pub fn save(&mut self, path: &Path, overwrite: bool) -> Result<(), String> {
        self.document.save_as(path, overwrite).map_err(|error| error.to_string())
    }

    pub fn modified(&self) -> bool {
        self.document.modified()
    }

    pub fn can_undo(&self) -> bool {
        self.document.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.document.can_redo()
    }

    pub fn undo(&mut self, redo: bool) {
        if redo {
            self.document.redo();
        } else {
            self.document.undo();
        }
        self.select(None);
        self.drag = None;
        self.drag_handle = None;
        self.error = None;
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(0.25, 8.0);
        self.fit = false;
    }

    pub fn canvas_rect(&self) -> egui::Rect {
        self.canvas_rect
    }

    pub fn recovery_snapshot(&self) -> Result<icy_draw::recovery::Snapshot, String> {
        self.document.recovery_snapshot()
    }

    pub fn from_recovery(snapshot: &icy_draw::recovery::Snapshot) -> Result<Self, String> {
        Ok(Self::from_document(SkypixDocument::from_recovery(snapshot)?))
    }

    fn select(&mut self, index: Option<usize>) {
        self.selected = index;
        self.draft = index.and_then(|index| self.document.items().get(index)).cloned();
        self.line_start = index
            .filter(|_| matches!(self.draft, Some(SkypixItem::Command(SkypixCommand::DrawLine { .. }))))
            .map(|index| pen_before(self.document.items(), index));
        self.text_draft = match &self.draft {
            Some(SkypixItem::Text(bytes)) => text_string(bytes),
            _ => String::new(),
        };
    }

    fn append(&mut self, items: Vec<SkypixItem>) {
        match self.document.append(items) {
            Ok(()) => {
                self.error = None;
                self.preview_selection = false;
                self.select(self.document.items().len().checked_sub(1));
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn shape_items(&self, from: Point, to: Point) -> Result<Vec<SkypixItem>, String> {
        let cmd = SkypixItem::command;
        let ((x0, y0), (x1, y1)) = (from, to);
        let (left, top, right, bottom) = (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
        let mut items = vec![cmd(SkypixCommand::SetPenA { color: self.pen.into() })];
        match self.tool {
            Tool::Select => return Ok(Vec::new()),
            Tool::Pixel => items.push(cmd(SkypixCommand::SetPixel { x: x1, y: y1 })),
            Tool::Line => items.extend([cmd(SkypixCommand::MovePen { x: x0, y: y0 }), cmd(SkypixCommand::DrawLine { x: x1, y: y1 })]),
            Tool::Rectangle => items.extend([
                cmd(SkypixCommand::MovePen { x: left, y: top }),
                cmd(SkypixCommand::DrawLine { x: right, y: top }),
                cmd(SkypixCommand::DrawLine { x: right, y: bottom }),
                cmd(SkypixCommand::DrawLine { x: left, y: bottom }),
                cmd(SkypixCommand::DrawLine { x: left, y: top }),
            ]),
            Tool::FilledRectangle => items.push(cmd(SkypixCommand::RectangleFill {
                x1: left,
                y1: top,
                x2: right,
                y2: bottom,
            })),
            Tool::Ellipse | Tool::FilledEllipse => {
                let (x, y, a, b) = ((left + right) / 2, (top + bottom) / 2, (right - left) / 2, (bottom - top) / 2);
                if a == 0 || b == 0 {
                    return Err(fl!("skypix-editor-degenerate-ellipse"));
                }
                items.push(cmd(if self.tool == Tool::Ellipse {
                    SkypixCommand::Ellipse { x, y, a, b }
                } else {
                    SkypixCommand::FilledEllipse { x, y, a, b }
                }));
            }
            Tool::Fill => items.push(cmd(SkypixCommand::AreaFill {
                mode: self.fill_mode,
                x: x1,
                y: y1,
            })),
            Tool::Text => {
                if self.text.is_empty() {
                    return Err(fl!("skypix-editor-text-hint"));
                }
                items.push(cmd(SkypixCommand::SetPenB { color: self.background.into() }));
                items.push(cmd(match self.font {
                    Some(index) => {
                        let (name, size) = &self.fonts[index];
                        SkypixCommand::SetFont {
                            name: name.clone(),
                            size: *size,
                        }
                    }
                    None => SkypixCommand::ResetFont,
                }));
                items.push(cmd(SkypixCommand::PositionCursor { x: x1, y: y1 }));
                if self.font.is_none() {
                    items.push(cmd(SkypixCommand::ResetFont));
                }
                items.push(SkypixItem::text(&self.text).map_err(|error| error.to_string())?);
            }
            Tool::GrabBrush => {
                items.push(cmd(SkypixCommand::GrabBrush {
                    x1: left,
                    y1: top,
                    width: right - left + 1,
                    height: bottom - top + 1,
                }));
            }
            Tool::Stamp => {
                let brush = self.document.items().iter().rev().find_map(|item| match item {
                    SkypixItem::Command(SkypixCommand::GrabBrush { width, height, .. }) => Some((*width, *height)),
                    _ => None,
                });
                let Some((width, height)) = brush else {
                    return Err(fl!("skypix-editor-brush-missing"));
                };
                items.push(cmd(SkypixCommand::UseBrush {
                    src_x: 0,
                    src_y: 0,
                    dst_x: x1,
                    dst_y: y1,
                    width,
                    height,
                    minterm: 192,
                    mask: 255,
                }));
            }
        }
        Ok(items)
    }

    fn add_shape(&mut self, from: Point, to: Point) {
        match self.shape_items(from, to) {
            Ok(items) if !items.is_empty() => self.append(items),
            Ok(_) => {}
            Err(error) => self.error = Some(error),
        }
    }

    fn refresh_preview(&mut self, context: &egui::Context) {
        let count = self.selected.filter(|_| self.preview_selection).map(|index| index + 1);
        let revision = self.document.revision();
        let dragged_line = self.dragged_line();
        let key = PreviewKey { revision, count, dragged_line };
        if self.shown == Some(key) {
            return;
        }
        let result = match dragged_line.and_then(|(from, to)| self.line_replacement(from, to)) {
            Some((items, _)) => self.document.preview_with_items(items),
            None => match count {
                Some(count) => self.document.preview_through(count),
                None => self.document.preview(),
            },
        };
        match result {
            Ok(preview) => {
                self.warnings = preview.warnings().to_vec();
                self.palette = preview.screen().palette().clone();
                if self.palette_revision != Some(revision) {
                    for (index, rgb) in self.palette_draft.iter_mut().enumerate() {
                        let (r, g, b) = self.palette.rgb(index as u32);
                        *rgb = [r / 17, g / 17, b / 17];
                    }
                    self.palette_revision = Some(revision);
                    self.palette_original = self.palette_draft;
                }
                self.colors = 16;
                for item in self.document.items().iter().take(count.unwrap_or(self.document.items().len())) {
                    if let SkypixItem::Command(SkypixCommand::SetDisplayMode { mode }) = item {
                        self.colors = match mode {
                            DisplayMode::EightColors => 8,
                            DisplayMode::SixteenColors => 16,
                        };
                    }
                }
                self.pen = self.pen.min((self.colors - 1) as u8);
                self.background = self.background.min((self.colors - 1) as u8);
                let image = egui::ColorImage::from_rgba_unmultiplied([preview.width(), preview.height()], &preview.rgba());
                if let Some(texture) = &mut self.texture {
                    texture.set(image, egui::TextureOptions::NEAREST);
                } else {
                    self.texture = Some(context.load_texture("skypix-editor-canvas", image, egui::TextureOptions::NEAREST));
                }
            }
            Err(error) => {
                self.error = Some(error.to_string());
                self.warnings.clear();
                self.texture = None;
            }
        }
        self.shown = Some(key);
    }

    fn swatches(&mut self, ui: &mut egui::Ui, background: bool) {
        egui::Grid::new(if background { "skypix-background" } else { "skypix-pen" })
            .spacing(egui::vec2(3.0, 3.0))
            .show(ui, |ui| {
                for index in 0..self.colors {
                    let (r, g, b) = self.palette.rgb(index as u32);
                    let selected = if background { self.background } else { self.pen } == index as u8;
                    let color = Color32::from_rgb(r, g, b);
                    let response = ui.add(
                        egui::Button::new(
                            egui::RichText::new(index.to_string()).color(if u16::from(r) + u16::from(g) + u16::from(b) > 384 {
                                Color32::BLACK
                            } else {
                                Color32::WHITE
                            }),
                        )
                        .fill(color)
                        .stroke(Stroke::new(if selected { 3.0 } else { 0.5 }, ui.visuals().selection.stroke.color))
                        .min_size(egui::vec2(27.0, 27.0)),
                    );
                    if response.clicked() {
                        if background {
                            self.background = index as u8;
                        } else {
                            self.pen = index as u8;
                        }
                    }
                    if index % 4 == 3 {
                        ui.end_row();
                    }
                }
            });
    }

    fn sidebar(&mut self, context: &egui::Context, blocked: bool) {
        egui::SidePanel::left("skypix-tools").default_width(192.0).resizable(true).show(context, |ui| {
            if blocked {
                ui.disable();
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading(fl!("skypix-editor-tools"));
                egui::Grid::new("skypix-tool-grid").num_columns(4).show(ui, |ui| {
                    for (index, tool) in Tool::ALL.into_iter().enumerate() {
                        if self.icons.button(ui, tool.icon(), &tool.label(), self.tool == tool).clicked() {
                            self.tool = tool;
                            self.drag = None;
                            self.preview_selection = false;
                        }
                        if index % 4 == 3 {
                            ui.end_row();
                        }
                    }
                });
                ui.separator();
                ui.label(fl!("skypix-editor-pen"));
                self.swatches(ui, false);
                if self.tool == Tool::Text {
                    ui.label(fl!("skypix-editor-background"));
                    self.swatches(ui, true);
                    ui.label(fl!("skypix-editor-font"));
                    egui::ComboBox::from_id_salt("skypix-font")
                        .width(160.0)
                        .selected_text(match self.font {
                            Some(index) => format!("{} {}", self.fonts[index].0, self.fonts[index].1),
                            None => fl!("skypix-editor-default-font"),
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.font, None, fl!("skypix-editor-default-font"));
                            for (index, (name, size)) in self.fonts.iter().enumerate() {
                                ui.selectable_value(&mut self.font, Some(index), format!("{name} {size}"));
                            }
                        });
                    ui.add(egui::TextEdit::multiline(&mut self.text).desired_width(160.0).desired_rows(3));
                    ui.add(egui::Label::new(fl!("skypix-editor-text-hint")).wrap());
                }
                if self.tool == Tool::Fill {
                    ui.label(fl!("skypix-editor-fill-mode"));
                    ui.radio_value(&mut self.fill_mode, FillMode::Color, fl!("skypix-editor-fill-color"));
                    ui.radio_value(&mut self.fill_mode, FillMode::Outline, fl!("skypix-editor-fill-outline"));
                }
                ui.separator();
                ui.label(fl!("skypix-editor-display-mode"));
                for (mode, label) in [
                    (DisplayMode::EightColors, fl!("skypix-editor-eight-colors")),
                    (DisplayMode::SixteenColors, fl!("skypix-editor-sixteen-colors")),
                ] {
                    if ui
                        .selectable_label(self.colors == if mode == DisplayMode::EightColors { 8 } else { 16 }, label)
                        .clicked()
                    {
                        self.append(vec![SkypixItem::command(SkypixCommand::SetDisplayMode { mode })]);
                    }
                }
                ui.separator();
                ui.collapsing(fl!("skypix-editor-palette"), |ui| {
                    for (slot, rgb) in self.palette_draft.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(slot.to_string());
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 0, Color32::from_rgb(rgb[0] * 17, rgb[1] * 17, rgb[2] * 17));
                            for component in rgb {
                                ui.add(egui::DragValue::new(component).range(0..=15));
                            }
                        });
                    }
                    if ui.button(fl!("skypix-editor-apply-palette")).clicked() {
                        let colors = self
                            .palette_draft
                            .iter()
                            .map(|rgb| i32::from(rgb[0]) | (i32::from(rgb[1]) << 4) | (i32::from(rgb[2]) << 8))
                            .collect();
                        self.append(vec![SkypixItem::command(SkypixCommand::NewPalette { colors })]);
                    }
                    if ui.button(fl!("skypix-editor-reset-palette")).clicked() {
                        self.append(vec![SkypixItem::command(SkypixCommand::ResetPalette)]);
                    }
                });
            });
        });
    }

    fn command_panel(&mut self, context: &egui::Context, blocked: bool) {
        egui::SidePanel::right("skypix-commands")
            .default_width(300.0)
            .min_width(220.0)
            .resizable(true)
            .show(context, |ui| {
                if blocked {
                    ui.disable();
                }
                let count = self.document.items().len();
                let filter = self.command_filter.trim().to_lowercase();
                let rows: Vec<usize> = self
                    .document
                    .items()
                    .iter()
                    .enumerate()
                    .filter(|(index, item)| {
                        filter.is_empty()
                            || format!("{} {}", item_name(item), item_summary(self.document.items(), *index))
                                .to_lowercase()
                                .contains(&filter)
                    })
                    .map(|(index, _)| index)
                    .collect();
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(icy_engine_gui::egui::appearance::bold(ui, fl!("skypix-editor-commands")));
                    ui.weak(if filter.is_empty() {
                        count.to_string()
                    } else {
                        format!("{} / {count}", rows.len())
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let selected = self.selected.filter(|&index| index < count);
                        let mut action = None;
                        for (enabled, icon, label, value) in [
                            (selected.is_some(), "delete", fl!("skypix-editor-delete"), 0),
                            (selected.is_some_and(|index| index + 1 < count), "move_down", fl!("skypix-editor-down"), 1),
                            (selected.is_some_and(|index| index > 0), "move_up", fl!("skypix-editor-up"), -1),
                            (selected.is_some(), "file_copy", fl!("skypix-editor-duplicate"), 2),
                        ] {
                            if ui
                                .add_enabled_ui(enabled, |ui| self.icons.button_sized(ui, icon, &label, false, 26.0))
                                .inner
                                .clicked()
                            {
                                action = Some(value);
                            }
                        }
                        widgets::divider(ui);
                        if ui
                            .add_enabled_ui(selected.is_some() || self.preview_selection, |ui| {
                                self.icons
                                    .button_sized(ui, "visibility", &fl!("skypix-editor-preview-selection"), self.preview_selection, 26.0)
                            })
                            .inner
                            .clicked()
                        {
                            self.preview_selection = !self.preview_selection;
                        }
                        if let Some(index) = selected {
                            match action {
                                Some(0) => self.delete_selected(),
                                Some(1) => self.move_item(index, index + 1),
                                Some(-1) => self.move_item(index, index - 1),
                                Some(2) => self.duplicate_selected(),
                                _ => {}
                            }
                        }
                    });
                });
                ui.add(
                    icy_engine_gui::egui::appearance::text_edit(&mut self.command_filter)
                        .hint_text(fl!("skypix-editor-filter"))
                        .desired_width(ui.available_width()),
                );
                ui.separator();
                let mut clicked = None;
                let mut double_clicked = None;
                let reserved = if self.selected.is_some() { 280.0 } else { 8.0 };
                let list_height = (ui.available_height() - reserved).max(120.0);
                let mut scroll = egui::ScrollArea::vertical()
                    .id_salt("skypix-command-list")
                    .auto_shrink([false, false])
                    .max_height(list_height);
                if self.selected != self.listed_selection {
                    if let Some(position) = self
                        .selected
                        .and_then(|index| rows.iter().position(|&row| row == index))
                        .filter(|position| !self.visible_rows.contains(position))
                    {
                        scroll = scroll.vertical_scroll_offset((position as f32 * ROW_HEIGHT - list_height / 2.0).max(0.0));
                    }
                }
                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    scroll.show_rows(ui, ROW_HEIGHT, rows.len(), |ui, positions| {
                        self.visible_rows = positions.clone();
                        for index in positions.map(|position| rows[position]) {
                            let Some(item) = self.document.items().get(index) else { break };
                            let name = item_name(item);
                            let summary = item_summary(self.document.items(), index);
                            let response = CommandRow {
                                index,
                                name: &name,
                                summary: &summary,
                                icon: Some(item_icon(item)),
                                swatch: item_swatch(item, &self.palette),
                                swatch_layout: SwatchLayout::ReplaceIcon,
                                selected: self.selected == Some(index),
                                related: false,
                                tone: if matches!(item, SkypixItem::Raw(_)) { Tone::Muted } else { Tone::Normal },
                                mark: RowMark::new(index, self.selected.filter(|_| self.preview_selection)),
                            }
                            .show(ui, &mut self.icons);
                            if response.double_clicked() {
                                double_clicked = Some(index);
                            } else if response.clicked() {
                                clicked = Some(index);
                            }
                        }
                    });
                });
                if let Some(index) = double_clicked {
                    self.select(Some(index));
                    self.preview_selection = true;
                } else if let Some(index) = clicked {
                    self.select(Some(index));
                }
                self.listed_selection = self.selected;
                if let Some(index) = self.selected.filter(|&index| index < self.document.items().len()) {
                    ui.separator();
                    let item = &self.document.items()[index];
                    ui.horizontal(|ui| {
                        ui.add(self.icons.image(ui, item_icon(item), 16.0));
                        ui.label(icy_engine_gui::egui::appearance::bold(ui, item_name(item)));
                        ui.weak(format!("#{}", index + 1));
                    });
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical()
                        .id_salt("skypix-properties")
                        .max_height((ui.available_height() - 8.0).max(60.0))
                        .show(ui, |ui| self.properties(ui));
                }
            });
    }

    fn duplicate_selected(&mut self) {
        let Some(index) = self.selected else { return };
        let mut items = self.document.items().to_vec();
        let Some(item) = items.get(index).cloned() else { return };
        items.insert(index + 1, item);
        match self.document.set_items(items) {
            Ok(()) => {
                self.select(Some(index + 1));
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn move_item(&mut self, from: usize, to: usize) {
        match self.document.move_item(from, to) {
            Ok(()) => {
                self.select(Some(to));
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn delete_selected(&mut self) {
        if let Some(index) = self.selected {
            match self.document.delete(index) {
                Ok(()) => {
                    self.select(None);
                    self.error = None;
                }
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }
    fn properties(&mut self, ui: &mut egui::Ui) {
        let Some(draft) = &mut self.draft else {
            ui.add(egui::Label::new(fl!("skypix-editor-no-selection")).wrap());
            return;
        };
        let editable = match draft {
            SkypixItem::Raw(bytes) => {
                ui.add(egui::Label::new(fl!("skypix-editor-preserved")).wrap());
                let mut source = bytes.escape_ascii().to_string();
                ui.add(
                    egui::TextEdit::multiline(&mut source)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(3)
                        .interactive(false),
                );
                false
            }
            SkypixItem::Text(_) => {
                ui.add(egui::TextEdit::multiline(&mut self.text_draft).desired_width(f32::INFINITY).desired_rows(3));
                true
            }
            SkypixItem::Command(command) => {
                match command {
                    SkypixCommand::SetPixel { x, y } | SkypixCommand::MovePen { x, y } | SkypixCommand::PositionCursor { x, y } => {
                        coordinate(ui, &fl!("skypix-editor-position"), x, y, false)
                    }
                    SkypixCommand::DrawLine { x, y } => {
                        if let Some((start_x, start_y)) = &mut self.line_start {
                            coordinate(ui, &fl!("skypix-editor-start"), start_x, start_y, false);
                        }
                        coordinate(ui, &fl!("skypix-editor-end"), x, y, false);
                    }
                    SkypixCommand::RectangleFill { x1, y1, x2, y2 } => {
                        coordinate(ui, &fl!("skypix-editor-position"), x1, y1, false);
                        coordinate(ui, &fl!("skypix-editor-end"), x2, y2, false);
                    }
                    SkypixCommand::Ellipse { x, y, a, b } | SkypixCommand::FilledEllipse { x, y, a, b } => {
                        coordinate(ui, &fl!("skypix-editor-position"), x, y, false);
                        coordinate(ui, &fl!("skypix-editor-radius"), a, b, true);
                    }
                    SkypixCommand::AreaFill { mode, x, y } => {
                        coordinate(ui, &fl!("skypix-editor-position"), x, y, false);
                        ui.radio_value(mode, FillMode::Color, fl!("skypix-editor-fill-color"));
                        ui.radio_value(mode, FillMode::Outline, fl!("skypix-editor-fill-outline"));
                    }
                    SkypixCommand::GrabBrush { x1, y1, width, height } => {
                        coordinate(ui, &fl!("skypix-editor-position"), x1, y1, false);
                        coordinate(ui, &fl!("skypix-editor-size"), width, height, true);
                    }
                    SkypixCommand::UseBrush {
                        src_x,
                        src_y,
                        dst_x,
                        dst_y,
                        width,
                        height,
                        minterm,
                        mask,
                    } => {
                        coordinate(ui, &fl!("skypix-editor-source"), src_x, src_y, false);
                        coordinate(ui, &fl!("skypix-editor-position"), dst_x, dst_y, false);
                        coordinate(ui, &fl!("skypix-editor-size"), width, height, true);
                        ui.horizontal(|ui| {
                            ui.label("Minterm");
                            ui.add(egui::DragValue::new(minterm).range(0..=255));
                            ui.label("Mask");
                            ui.add(egui::DragValue::new(mask).range(0..=255));
                        });
                    }
                    SkypixCommand::SetPenA { color } | SkypixCommand::SetPenB { color } => {
                        ui.add(egui::DragValue::new(color).range(0..=15));
                    }
                    SkypixCommand::SetFont { name, size } => {
                        egui::ComboBox::from_id_salt("skypix-property-font")
                            .selected_text(format!("{name} {size}"))
                            .show_ui(ui, |ui| {
                                for (font_name, font_size) in &self.fonts {
                                    if ui
                                        .selectable_label(name == font_name && size == font_size, format!("{font_name} {font_size}"))
                                        .clicked()
                                    {
                                        *name = font_name.clone();
                                        *size = *font_size;
                                    }
                                }
                            });
                    }
                    SkypixCommand::SetDisplayMode { mode } => {
                        ui.radio_value(mode, DisplayMode::EightColors, fl!("skypix-editor-eight-colors"));
                        ui.radio_value(mode, DisplayMode::SixteenColors, fl!("skypix-editor-sixteen-colors"));
                    }
                    SkypixCommand::NewPalette { colors } => {
                        for (slot, color) in colors.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(slot.to_string());
                                ui.add(egui::DragValue::new(color).range(0..=4095).hexadecimal(3, false, true));
                            });
                        }
                    }
                    SkypixCommand::Delay { jiffies } => {
                        ui.add(egui::DragValue::new(jiffies).range(0..=65535));
                    }
                    SkypixCommand::Comment { text } => {
                        ui.add(egui::TextEdit::multiline(text).desired_width(f32::INFINITY).desired_rows(3));
                    }
                    SkypixCommand::ResetFont | SkypixCommand::ResetPalette | SkypixCommand::EndSkypix => {
                        return;
                    }
                    SkypixCommand::PlaySample { .. }
                    | SkypixCommand::CrcTransfer { .. }
                    | SkypixCommand::ControllerReturn { .. }
                    | SkypixCommand::DefineGadget { .. } => {
                        ui.add(egui::Label::new(fl!("skypix-editor-preserved-preview")).wrap());
                        return;
                    }
                }
                true
            }
        };
        if editable && ui.button(fl!("skypix-editor-apply")).clicked() {
            if let Some(SkypixItem::Command(SkypixCommand::DrawLine { x, y })) = self.draft.clone() {
                if let Some(start) = self.line_start {
                    self.replace_line(start, (x, y));
                }
                return;
            }
            let replacement = match &self.draft {
                Some(SkypixItem::Text(_)) => SkypixItem::text(&self.text_draft),
                Some(item) => Ok(item.clone()),
                None => return,
            };
            match replacement.and_then(|item| self.document.replace(self.selected.expect("draft has selection"), item)) {
                Ok(()) => {
                    self.select(self.selected);
                    self.error = None;
                }
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }

    fn hit(&self, point: Point) -> Option<usize> {
        self.document.items().iter().enumerate().rev().find_map(|(index, _)| {
            let (a, b) = bounds(self.document.items(), index)?;
            let near = if matches!(self.document.items()[index], SkypixItem::Command(SkypixCommand::DrawLine { .. })) {
                let start = egui::pos2(a.0 as f32, a.1 as f32);
                let end = egui::pos2(b.0 as f32, b.1 as f32);
                let point = egui::pos2(point.0 as f32, point.1 as f32);
                let segment = end - start;
                let fraction = if segment.length_sq() > 0.0 {
                    ((point - start).dot(segment) / segment.length_sq()).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                point.distance(start + segment * fraction) <= 3.0
            } else {
                point.0 >= a.0.min(b.0).saturating_sub(3)
                    && point.0 <= a.0.max(b.0).saturating_add(3)
                    && point.1 >= a.1.min(b.1).saturating_sub(3)
                    && point.1 <= a.1.max(b.1).saturating_add(3)
            };
            near.then_some(index)
        })
    }

    fn translate_selected(&mut self, delta: Point) {
        let Some(index) = self.selected else {
            return;
        };
        let Some((a, b)) = bounds(self.document.items(), index) else {
            return;
        };
        let (min_x, max_x) = (a.0.min(b.0).saturating_neg(), (WIDTH - 1).saturating_sub(a.0.max(b.0)));
        let (min_y, max_y) = (a.1.min(b.1).saturating_neg(), (HEIGHT - 1).saturating_sub(a.1.max(b.1)));
        if min_x > max_x || min_y > max_y {
            self.error = Some(fl!("skypix-editor-move-too-large"));
            return;
        }
        let dx = delta.0.clamp(min_x, max_x);
        let dy = delta.1.clamp(min_y, max_y);
        if dx == 0 && dy == 0 {
            return;
        }
        if matches!(self.document.items()[index], SkypixItem::Command(SkypixCommand::DrawLine { .. })) {
            self.replace_line((a.0 + dx, a.1 + dy), (b.0 + dx, b.1 + dy));
            return;
        }
        let mut items = self.document.items().to_vec();
        let SkypixItem::Command(command) = &mut items[index] else {
            return;
        };
        match command {
            SkypixCommand::SetPixel { x, y }
            | SkypixCommand::PositionCursor { x, y }
            | SkypixCommand::Ellipse { x, y, .. }
            | SkypixCommand::FilledEllipse { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            SkypixCommand::RectangleFill { x1, y1, x2, y2 } => {
                *x1 += dx;
                *x2 += dx;
                *y1 += dy;
                *y2 += dy;
            }
            SkypixCommand::GrabBrush { x1, y1, .. } => {
                *x1 += dx;
                *y1 += dy;
            }
            SkypixCommand::UseBrush { dst_x, dst_y, .. } => {
                *dst_x += dx;
                *dst_y += dy;
            }
            _ => return,
        }
        match self.document.set_items(items) {
            Ok(()) => {
                self.select(Some(index));
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn dragged_line(&self) -> Option<(Point, Point)> {
        if self.tool != Tool::Select || self.preview_selection {
            return None;
        }
        let end = self.drag_handle?;
        let (from, to) = self.drag?;
        let (a, b) = bounds(self.document.items(), self.selected?)?;
        let anchor = if end { b } else { a };
        let moved = (
            anchor.0.saturating_add(to.0 - from.0).clamp(0, WIDTH - 1),
            anchor.1.saturating_add(to.1 - from.1).clamp(0, HEIGHT - 1),
        );
        Some((if end { a } else { moved }, if end { moved } else { b }))
    }

    fn line_replacement(&self, from: Point, to: Point) -> Option<(Vec<SkypixItem>, usize)> {
        let index = self
            .selected
            .filter(|&index| matches!(self.document.items().get(index), Some(SkypixItem::Command(SkypixCommand::DrawLine { .. }))))?;
        let (old_from, old_to) = bounds(self.document.items(), index)?;
        if (old_from, old_to) == (from, to) {
            return None;
        }
        let mut items = self.document.items().to_vec();
        items[index] = SkypixItem::command(SkypixCommand::DrawLine { x: to.0, y: to.1 });
        // A following pen-relative segment keeps its original start.
        let dependent = items[index + 1..]
            .iter()
            .find_map(|item| match item {
                SkypixItem::Command(SkypixCommand::DrawLine { .. }) => Some(true),
                SkypixItem::Command(SkypixCommand::MovePen { .. } | SkypixCommand::PositionCursor { .. }) => Some(false),
                _ => None,
            })
            .unwrap_or(false);
        if dependent && to != old_to {
            items.insert(index + 1, SkypixItem::command(SkypixCommand::MovePen { x: old_to.0, y: old_to.1 }));
        }
        let selected = if index > 0 && matches!(items[index - 1], SkypixItem::Command(SkypixCommand::MovePen { .. })) {
            items[index - 1] = SkypixItem::command(SkypixCommand::MovePen { x: from.0, y: from.1 });
            index
        } else if from != old_from {
            items.insert(index, SkypixItem::command(SkypixCommand::MovePen { x: from.0, y: from.1 }));
            index + 1
        } else {
            index
        };
        Some((items, selected))
    }

    fn replace_line(&mut self, from: Point, to: Point) {
        let Some((items, selected)) = self.line_replacement(from, to) else {
            return;
        };
        match self.document.set_items(items) {
            Ok(()) => {
                self.select(Some(selected));
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui, blocked: bool) {
        self.refresh_preview(ui.ctx());
        let Some(texture) = self.texture.clone() else {
            return;
        };
        let aspect = if self.aspect { 2.0 } else { 1.0 };
        let logical = egui::vec2(WIDTH as f32, HEIGHT as f32 * aspect);
        if self.fit {
            self.zoom = (ui.available_width() / logical.x).min(ui.available_height() / logical.y).clamp(0.25, 8.0);
        }
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            let response = ui.add(
                egui::Image::new(&texture)
                    .maintain_aspect_ratio(false)
                    .fit_to_exact_size(logical * self.zoom)
                    .sense(egui::Sense::click_and_drag()),
            );
            self.canvas_rect = response.rect;
            let canvas = CanvasTransform { rect: response.rect };
            let at = |position| canvas.pixel(position);
            let point = response.interact_pointer_pos().map(at);
            if !blocked && !self.preview_selection {
                if response.drag_started() {
                    if let Some(position) = ui.input(|input| input.pointer.press_origin()) {
                        let point = at(position);
                        self.drag_handle = None;
                        if self.tool == Tool::Select {
                            if let Some((a, b)) = self
                                .selected
                                .filter(|&index| matches!(self.document.items().get(index), Some(SkypixItem::Command(SkypixCommand::DrawLine { .. }))))
                                .and_then(|index| bounds(self.document.items(), index))
                            {
                                self.drag_handle = [(false, a), (true, b)]
                                    .into_iter()
                                    .find(|(_, point)| canvas.position(*point).distance(position) <= 8.0)
                                    .map(|(end, _)| end);
                            }
                            if self.drag_handle.is_none() {
                                self.select(self.hit(point));
                            }
                        }
                        self.drag = Some((point, point));
                    }
                }
                if response.dragged() || response.drag_stopped() {
                    if let (Some((start, _)), Some(end)) = (self.drag, point) {
                        self.drag = Some((start, end));
                    }
                }
                if response.drag_stopped() {
                    let dragged_line = self.dragged_line();
                    if let Some((from, to)) = self.drag.take() {
                        if self.tool == Tool::Select {
                            if self.drag_handle.take().is_some() {
                                if let Some((a, b)) = dragged_line {
                                    self.replace_line(a, b);
                                }
                            } else {
                                self.translate_selected((to.0 - from.0, to.1 - from.1));
                            }
                        } else {
                            self.add_shape(from, to);
                        }
                    }
                } else if response.clicked() {
                    if let Some(point) = point {
                        if self.tool == Tool::Select {
                            self.select(self.hit(point));
                        } else if !self.tool.dragged() {
                            self.add_shape(point, point);
                        }
                    }
                }
                self.refresh_preview(ui.ctx());
            }
            let on_screen = |point| canvas.position(point);
            if let Some((from, to)) = self.drag {
                let stroke = Stroke::new(1.0, ui.visuals().selection.stroke.color);
                if self.tool == Tool::Line {
                    ui.painter().line_segment([on_screen(from), on_screen(to)], stroke);
                } else if self.tool != Tool::Select {
                    ui.painter()
                        .rect_stroke(egui::Rect::from_two_pos(on_screen(from), on_screen(to)), 0, stroke, egui::StrokeKind::Middle);
                }
            }
            if self.tool == Tool::Select {
                if let Some(index) = self.selected {
                    if let Some((a, b)) = self.dragged_line().or_else(|| bounds(self.document.items(), index)) {
                        if matches!(self.document.items()[index], SkypixItem::Command(SkypixCommand::DrawLine { .. })) {
                            widgets::paint_vertices(ui, [on_screen(a), on_screen(b)], ui.visuals().selection.stroke.color);
                        } else {
                            ui.painter().rect_stroke(
                                egui::Rect::from_two_pos(on_screen(a), on_screen(b)).expand(3.0),
                                0,
                                Stroke::new(1.0, ui.visuals().selection.stroke.color),
                                egui::StrokeKind::Middle,
                            );
                        }
                    }
                }
            }
        });
    }

    pub fn show(&mut self, context: &egui::Context, blocked: bool) {
        let cancel_drag = !blocked && self.drag.is_some() && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if blocked || cancel_drag {
            self.drag = None;
            self.drag_handle = None;
            if cancel_drag {
                self.select(None);
                context.request_repaint();
            }
        }
        self.refresh_preview(context);
        egui::TopBottomPanel::top("skypix-toolbar").exact_height(44.0).show(context, |ui| {
            if blocked {
                ui.disable();
            }
            ui.horizontal_centered(|ui| {
                ui.strong(fl!("skypix-editor-title"));
                ui.separator();
                if ui.button(fl!("skypix-editor-fit")).clicked() {
                    self.fit = true;
                }
                if ui.button("1:1").clicked() {
                    self.set_zoom(1.0);
                }
                let mut zoom = self.zoom * 100.0;
                if ui.add(egui::DragValue::new(&mut zoom).range(25.0..=800.0).suffix("%")).changed() {
                    self.set_zoom(zoom / 100.0);
                }
                ui.checkbox(&mut self.aspect, fl!("skypix-editor-aspect"));
            });
        });
        self.sidebar(context, blocked);
        self.command_panel(context, blocked);
        egui::TopBottomPanel::bottom("skypix-status").show(context, |ui| {
            ui.add(egui::Label::new(egui::RichText::new(fl!("skypix-editor-unsupported")).weak()).wrap());
        });
        egui::CentralPanel::default().show(context, |ui| {
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            if !self.warnings.is_empty() {
                egui::CollapsingHeader::new(fl!("skypix-editor-preview-warnings", count = self.warnings.len()))
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("skypix-preview-warnings")
                            .max_height(100.0)
                            .show(ui, |ui| {
                                for warning in &self.warnings {
                                    ui.add(egui::Label::new(egui::RichText::new(warning).color(ui.visuals().warn_fg_color)).wrap());
                                }
                            });
                    });
            }
            self.canvas(ui, blocked);
        });
        if !blocked && !context.wants_keyboard_input() {
            if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                self.drag = None;
                self.drag_handle = None;
                self.select(None);
                context.request_repaint();
            }
            if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Delete)) {
                self.delete_selected();
            }
            if self.tool == Tool::Select && self.selected.is_some() {
                for (key, delta) in [
                    (egui::Key::ArrowLeft, (-1, 0)),
                    (egui::Key::ArrowRight, (1, 0)),
                    (egui::Key::ArrowUp, (0, -1)),
                    (egui::Key::ArrowDown, (0, 1)),
                ] {
                    if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key)) {
                        self.translate_selected(delta);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ai_apply_accepts_ordinary_selection_and_rejects_pending_edits() {
        let mut editor = super::SkypixEditor::new();
        editor
            .document
            .append(vec![icy_draw::skypix_document::SkypixItem::text("LABEL").unwrap()])
            .unwrap();
        editor.select(Some(0));
        let items = editor.document.items().to_vec();
        editor.apply_ai_items(items.clone()).unwrap();
        editor.select(Some(0));
        editor.text_draft.push('!');
        assert!(editor.apply_ai_items(items.clone()).is_err());
        editor.select(None);
        editor.palette_revision = Some(editor.document.revision());
        for (index, rgb) in editor.palette_draft.iter_mut().enumerate() {
            let (r, g, b) = editor.palette.rgb(index as u32);
            *rgb = [r / 17, g / 17, b / 17];
        }
        editor.palette_original = editor.palette_draft;
        editor.apply_ai_items(items.clone()).unwrap();
        editor.palette_revision = Some(editor.document.revision());
        editor.palette_draft[0][0] ^= 1;
        assert!(editor.apply_ai_items(items).is_err());
        let mut editor = super::SkypixEditor::new();
        editor
            .document
            .append(vec![icy_draw::skypix_document::SkypixItem::command(icy_parser_core::SkypixCommand::DrawLine {
                x: 20,
                y: 20,
            })])
            .unwrap();
        editor.select(Some(0));
        editor.line_start = Some((10, 10));
        assert!(editor.apply_ai_items(editor.document.items().to_vec()).is_err());
    }
    use super::*;

    fn frame(context: &egui::Context, editor: &mut SkypixEditor, events: Vec<egui::Event>, blocked: bool) -> egui::FullOutput {
        let time = context.input(|input| input.time) + 0.05;
        context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 820.0))),
                time: Some(time),
                events,
                ..Default::default()
            },
            |context| editor.show(context, blocked),
        )
    }

    fn canvas_image<'a>(output: &'a egui::FullOutput, editor: &SkypixEditor) -> &'a egui::ColorImage {
        let id = editor.texture.as_ref().unwrap().id();
        let (_, delta) = output
            .textures_delta
            .set
            .iter()
            .find(|(texture, _)| *texture == id)
            .expect("canvas texture update");
        let egui::ImageData::Color(image) = &delta.image;
        image
    }

    #[test]
    fn skypix_tools_render_save_and_undo() {
        let mut editor = SkypixEditor::new();
        editor.tool = Tool::FilledRectangle;
        editor.pen = 3;
        editor.add_shape((10, 10), (20, 20));
        assert!(editor.modified());
        assert_eq!(editor.document.preview().unwrap().pixel_index(15, 15), Some(3));
        let bytes = editor.document.to_bytes().unwrap();
        let reloaded = SkypixDocument::from_bytes(&bytes).unwrap();
        assert_eq!(reloaded.preview().unwrap().pixel_index(15, 15), Some(3));
        editor.undo(false);
        assert!(!editor.modified());
        editor.undo(true);
        assert_eq!(editor.document.preview().unwrap().pixel_index(15, 15), Some(3));
    }

    #[test]
    fn skypix_all_art_tools_generate_roundtrippable_items() {
        for tool in Tool::ALL.into_iter().filter(|tool| *tool != Tool::Select) {
            let mut editor = SkypixEditor::new();
            editor.text = "SkyPix".to_owned();
            if tool == Tool::Stamp {
                editor.tool = Tool::GrabBrush;
                editor.add_shape((0, 0), (5, 5));
            }
            editor.tool = tool;
            editor.add_shape((10, 10), (30, 30));
            assert!(editor.error.is_none(), "{tool:?}: {:?}", editor.error);
            let bytes = editor.document.to_bytes().unwrap();
            let loaded = SkypixDocument::from_bytes(&bytes).unwrap();
            assert_eq!(editor.document.preview().unwrap().rgba(), loaded.preview().unwrap().rgba(), "{tool:?}");
        }
    }

    #[test]
    fn skypix_move_line_preserves_following_segment_and_is_one_undo() {
        let mut editor = SkypixEditor::new();
        editor.add_shape((10, 10), (20, 10));
        editor.append(vec![SkypixItem::command(SkypixCommand::DrawLine { x: 20, y: 20 })]);
        editor.select(Some(2));
        let before = editor.document.to_bytes().unwrap();
        editor.translate_selected((10, 10));
        let preview = editor.document.preview().unwrap();
        assert_eq!(preview.pixel_index(25, 20), Some(7));
        assert_eq!(preview.pixel_index(20, 15), Some(7));
        editor.undo(false);
        assert_eq!(editor.document.to_bytes().unwrap(), before);
    }

    #[test]
    fn skypix_headless_editor_shows_canvas_and_respects_blocking() {
        let context = egui::Context::default();
        icy_engine_gui::egui::appearance::apply(&context);
        let mut editor = SkypixEditor::new();
        frame(&context, &mut editor, vec![], true);
        assert!(editor.canvas_rect().is_positive());
        assert!(!editor.modified());
    }

    #[test]
    fn skypix_canvas_click_maps_amiga_pixels_and_modal_blocks_drawing() {
        let context = egui::Context::default();
        icy_engine_gui::egui::appearance::apply(&context);
        let mut editor = SkypixEditor::new();
        editor.set_zoom(1.0);
        editor.tool = Tool::Pixel;
        editor.pen = 5;
        frame(&context, &mut editor, vec![], false);
        let point = editor.canvas_rect().min + egui::vec2(10.5, 41.0);
        for blocked in [true, false] {
            for pressed in [true, false] {
                frame(
                    &context,
                    &mut editor,
                    vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    blocked,
                );
            }
            assert_eq!(editor.modified(), !blocked);
        }
        assert_eq!(editor.document.preview().unwrap().pixel_index(10, 20), Some(5));
    }

    #[test]
    fn skypix_brush_captures_inclusive_drag_rectangle() {
        let mut editor = SkypixEditor::new();
        editor.tool = Tool::FilledRectangle;
        editor.pen = 2;
        editor.add_shape((10, 10), (12, 12));
        editor.tool = Tool::GrabBrush;
        editor.add_shape((10, 10), (12, 12));
        editor.tool = Tool::Stamp;
        editor.add_shape((20, 20), (20, 20));
        let preview = editor.document.preview().unwrap();
        assert_eq!(preview.pixel_index(20, 20), Some(2));
        assert_eq!(preview.pixel_index(22, 22), Some(2));
        assert_eq!(preview.pixel_index(23, 22), Some(0));
    }

    #[test]
    fn skypix_canvas_drag_uses_press_origin_and_release_position() {
        let context = egui::Context::default();
        icy_engine_gui::egui::appearance::apply(&context);
        let mut editor = SkypixEditor::new();
        editor.set_zoom(1.0);
        editor.tool = Tool::FilledRectangle;
        editor.pen = 4;
        frame(&context, &mut editor, vec![], false);
        let origin = editor.canvas_rect().min;
        let press = origin + egui::vec2(10.5, 21.0);
        let release = origin + egui::vec2(30.5, 61.0);
        frame(
            &context,
            &mut editor,
            vec![
                egui::Event::PointerMoved(press),
                egui::Event::PointerButton {
                    pos: press,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            false,
        );
        frame(&context, &mut editor, vec![egui::Event::PointerMoved(origin + egui::vec2(20.5, 41.0))], false);
        frame(
            &context,
            &mut editor,
            vec![
                egui::Event::PointerMoved(release),
                egui::Event::PointerButton {
                    pos: release,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            false,
        );
        let preview = editor.document.preview().unwrap();
        assert_eq!(preview.pixel_index(10, 10), Some(4));
        assert_eq!(preview.pixel_index(30, 30), Some(4));
        assert_eq!(preview.pixel_index(31, 30), Some(0));
        editor.undo(false);
        assert!(!editor.modified());
    }

    #[test]
    fn skypix_default_and_amiga_font_text_roundtrip() {
        let mut editor = SkypixEditor::new();
        editor.tool = Tool::Text;
        editor.text = "Hello".to_owned();
        editor.background = 3;
        editor.add_shape((10, 10), (10, 10));
        assert!(editor.error.is_none(), "{:?}", editor.error);
        assert_eq!(editor.document.preview().unwrap().pixel_index(10, 10), Some(3));
        editor.font = editor.fonts.iter().position(|(name, size)| name == "Topaz.font" && *size == 8);
        assert!(editor.font.is_some());
        editor.add_shape((10, 40), (10, 40));
        let bytes = editor.document.to_bytes().unwrap();
        assert_eq!(
            editor.document.preview().unwrap().rgba(),
            SkypixDocument::from_bytes(&bytes).unwrap().preview().unwrap().rgba()
        );
    }

    #[test]
    fn skypix_preserved_commands_show_file_specific_preview_warnings() {
        let bytes = b"\x1b[777;123!";
        let document = SkypixDocument::from_bytes(bytes).unwrap();
        let mut editor = SkypixEditor::from_document(document);
        let context = egui::Context::default();
        icy_engine_gui::egui::appearance::apply(&context);
        frame(&context, &mut editor, vec![], false);
        assert!(!editor.warnings.is_empty());
        assert!(editor.texture.is_some());
        assert_eq!(editor.document.to_bytes().unwrap(), bytes);
        assert!(!editor.modified());
    }

    #[test]
    fn skypix_image_aspect_matches_input_and_selection_pixel_centers() {
        let context = egui::Context::default();
        let mut editor = SkypixEditor::new();
        editor.set_zoom(1.1447);
        for aspect in [true, false] {
            editor.aspect = aspect;
            frame(&context, &mut editor, vec![], false);
            let rect = editor.canvas_rect();
            assert!((rect.width() - 640.0 * editor.zoom()).abs() < 0.05, "actual {rect:?}, zoom {}", editor.zoom());
            assert!(
                (rect.height() - if aspect { 400.0 } else { 200.0 } * editor.zoom()).abs() < 0.05,
                "actual {rect:?}, aspect {aspect}, zoom {}",
                editor.zoom()
            );
            let canvas = CanvasTransform { rect };
            for point in [(0, 0), (117, 16), (302, 86), (639, 199)] {
                let center = canvas.position(point);
                assert_eq!(canvas.pixel(center), point);
                let normalized = (center - rect.min) / rect.size();
                assert!((normalized.x - (point.0 as f32 + 0.5) / 640.0).abs() < 0.0001);
                assert!((normalized.y - (point.1 as f32 + 0.5) / 200.0).abs() < 0.0001);
            }
        }
    }

    #[test]
    fn skypix_lines_show_only_endpoint_handles_and_only_in_selection_tool() {
        let context = egui::Context::default();
        let mut editor = SkypixEditor::new();
        editor.set_zoom(1.0);
        editor.add_shape((117, 16), (302, 86));
        let accent = context.style().visuals.selection.stroke.color;
        for tool in [Tool::Line, Tool::Select] {
            editor.tool = tool;
            let output = frame(&context, &mut editor, vec![], false);
            let canvas = CanvasTransform { rect: editor.canvas_rect() };
            let rects: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect) if rect.stroke.color == accent && rect.rect.intersects(canvas.rect) => Some(rect.rect),
                    _ => None,
                })
                .collect();
            assert!(
                !rects.iter().any(|rect| rect.width() > 40.0 && rect.height() > 40.0),
                "line must not have a bounding frame"
            );
            for point in [(117, 16), (302, 86)] {
                let handle = egui::Rect::from_center_size(canvas.position(point), egui::Vec2::splat(7.0));
                assert_eq!(rects.contains(&handle), tool == Tool::Select);
            }
        }
    }

    #[test]
    fn skypix_line_endpoint_drag_preserves_following_segments_and_undo() {
        let context = egui::Context::default();
        let mut editor = SkypixEditor::new();
        editor.set_zoom(1.0);
        editor.add_shape((10, 10), (20, 10));
        editor.append(vec![SkypixItem::command(SkypixCommand::DrawLine { x: 20, y: 20 })]);
        editor.select(Some(2));
        editor.tool = Tool::Select;
        let before = editor.document.to_bytes().unwrap();
        frame(&context, &mut editor, vec![], false);
        let canvas = CanvasTransform { rect: editor.canvas_rect() };
        let press = canvas.position((20, 10));
        let release = canvas.position((30, 30));
        frame(
            &context,
            &mut editor,
            vec![
                egui::Event::PointerMoved(press),
                egui::Event::PointerButton {
                    pos: press,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            false,
        );
        let revision = editor.document.revision();
        for endpoint in [(25, 20), (30, 30)] {
            let output = frame(&context, &mut editor, vec![egui::Event::PointerMoved(canvas.position(endpoint))], false);
            let image = canvas_image(&output, &editor);
            let (r, g, b) = editor.palette.rgb(7);
            assert_eq!(image[(endpoint.0 as usize, endpoint.1 as usize)], Color32::from_rgb(r, g, b));
            assert_eq!(image[(15, 10)], Color32::BLACK, "the original line must disappear during dragging");
            assert_eq!(image[(20, 15)], Color32::from_rgb(r, g, b), "the following segment must stay fixed");
            let handle = egui::Rect::from_center_size(canvas.position(endpoint), egui::Vec2::splat(7.0));
            assert!(output
                .shapes
                .iter()
                .any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect == handle)));
            assert_eq!(editor.document.to_bytes().unwrap(), before);
            assert_eq!(editor.document.revision(), revision);
        }
        frame(
            &context,
            &mut editor,
            vec![
                egui::Event::PointerMoved(release),
                egui::Event::PointerButton {
                    pos: release,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            false,
        );
        assert_eq!(bounds(editor.document.items(), editor.selected.unwrap()), Some(((10, 10), (30, 30))));
        let preview = editor.document.preview().unwrap();
        assert_eq!(preview.pixel_index(25, 25), Some(7));
        assert_eq!(preview.pixel_index(20, 15), Some(7));
        editor.undo(false);
        assert_eq!(editor.document.to_bytes().unwrap(), before);
    }

    #[test]
    fn skypix_start_endpoint_live_preview_cancels_without_editing() {
        for blocked in [false, true] {
            let context = egui::Context::default();
            let mut editor = SkypixEditor::new();
            editor.aspect = false;
            editor.set_zoom(1.1447);
            editor.add_shape((10, 10), (20, 10));
            editor.tool = Tool::Select;
            let before = editor.document.to_bytes().unwrap();
            let revision = editor.document.revision();
            frame(&context, &mut editor, vec![], false);
            let canvas = CanvasTransform { rect: editor.canvas_rect() };
            let press = canvas.position((10, 10)) + egui::vec2(2.0, 1.0);
            frame(
                &context,
                &mut editor,
                vec![
                    egui::Event::PointerMoved(press),
                    egui::Event::PointerButton {
                        pos: press,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                false,
            );
            let moved = press + canvas.scale() * egui::vec2(0.0, 10.0);
            let output = frame(&context, &mut editor, vec![egui::Event::PointerMoved(moved)], false);
            let (r, g, b) = editor.palette.rgb(7);
            assert_eq!(canvas_image(&output, &editor)[(10, 20)], Color32::from_rgb(r, g, b));
            assert_eq!(canvas_image(&output, &editor)[(10, 10)], Color32::BLACK);
            assert_eq!(editor.dragged_line(), Some(((10, 20), (20, 10))));
            let events = if blocked {
                vec![]
            } else {
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }]
            };
            let cancelled = frame(&context, &mut editor, events, blocked);
            let output = frame(&context, &mut editor, vec![], blocked);
            assert!(editor.drag.is_none(), "drag must be cancelled (blocked={blocked})");
            assert_eq!(editor.document.to_bytes().unwrap(), before);
            let id = editor.texture.as_ref().unwrap().id();
            let restored = if output.textures_delta.set.iter().any(|(texture, _)| *texture == id) {
                &output
            } else {
                &cancelled
            };
            let image = canvas_image(restored, &editor);
            let original = editor.document.preview().unwrap();
            let expected = egui::ColorImage::from_rgba_unmultiplied([original.width(), original.height()], &original.rgba());
            assert!(
                image.pixels == expected.pixels,
                "cancellation must restore the original raster (blocked={blocked}, origin {:?}, expected {:?})",
                image[(10, 10)],
                expected[(10, 10)]
            );
            frame(
                &context,
                &mut editor,
                vec![egui::Event::PointerButton {
                    pos: moved,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                false,
            );
            assert!(editor.drag.is_none());
            assert_eq!(editor.document.to_bytes().unwrap(), before);
            assert_eq!(editor.document.revision(), revision);
        }
    }

    #[test]
    fn skypix_line_moves_do_not_accumulate_redundant_pen_commands() {
        let mut editor = SkypixEditor::new();
        editor.add_shape((10, 10), (20, 20));
        for _ in 0..10 {
            editor.translate_selected((1, 1));
        }
        assert_eq!(editor.document.items().len(), 3);
        assert_eq!(bounds(editor.document.items(), editor.selected.unwrap()), Some(((20, 20), (30, 30))));
        for _ in 0..10 {
            editor.undo(false);
            editor.select(Some(2));
        }
        assert_eq!(bounds(editor.document.items(), 2), Some(((10, 10), (20, 20))));
    }

    #[test]
    fn skypix_command_summaries_are_compact_and_describe_the_actual_line() {
        let mut editor = SkypixEditor::new();
        editor.add_shape((117, 16), (302, 86));
        assert_eq!(item_summary(editor.document.items(), 2), "117, 16 → 302, 86");
        for (index, item) in editor.document.items().iter().enumerate() {
            assert!(!item_name(item).contains('{'));
            assert!(!item_summary(editor.document.items(), index).contains('{'));
            assert!(!item_name(item).contains("DrawLine"));
        }
        assert_eq!(item_swatch(&editor.document.items()[0], &editor.palette), Some(Color32::from_rgb(204, 0, 238)));
    }

    #[test]
    fn skypix_command_list_follows_canvas_selection_and_filters_without_editing() {
        let context = egui::Context::default();
        let mut editor = SkypixEditor::new();
        editor.append((0..60).map(|x| SkypixItem::command(SkypixCommand::SetPixel { x, y: 10 })).collect());
        editor.select(Some(59));
        let bytes = editor.document.to_bytes().unwrap();
        for _ in 0..2 {
            frame(&context, &mut editor, vec![], false);
        }
        assert!(editor.visible_rows.contains(&59));
        editor.command_filter = "59, 10".to_owned();
        for _ in 0..2 {
            frame(&context, &mut editor, vec![], false);
        }
        assert_eq!(editor.visible_rows, 0..1);
        assert_eq!(editor.document.to_bytes().unwrap(), bytes);
        assert_eq!(editor.selected, Some(59));
        editor.duplicate_selected();
        assert_eq!(editor.document.items()[59], editor.document.items()[60]);
        assert_eq!(editor.selected, Some(60));
        editor.undo(false);
        assert_eq!(editor.document.to_bytes().unwrap(), bytes);
    }

    #[test]
    fn skypix_line_properties_apply_both_endpoints_in_one_undo_step() {
        let context = egui::Context::default();
        let mut editor = SkypixEditor::new();
        editor.add_shape((10, 10), (20, 10));
        editor.append(vec![SkypixItem::command(SkypixCommand::DrawLine { x: 20, y: 20 })]);
        editor.select(Some(2));
        let before = editor.document.to_bytes().unwrap();
        editor.line_start = Some((11, 12));
        editor.draft = Some(SkypixItem::command(SkypixCommand::DrawLine { x: 30, y: 30 }));
        let output = frame(&context, &mut editor, vec![], false);
        let apply = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == fl!("skypix-editor-apply") => Some(text.pos + text.galley.size() * 0.5),
                _ => None,
            })
            .expect("line property controls have an Apply button");
        for pressed in [true, false] {
            frame(
                &context,
                &mut editor,
                vec![
                    egui::Event::PointerMoved(apply),
                    egui::Event::PointerButton {
                        pos: apply,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                false,
            );
        }
        assert_eq!(bounds(editor.document.items(), editor.selected.unwrap()), Some(((11, 12), (30, 30))));
        assert_eq!(editor.document.preview().unwrap().pixel_index(20, 15), Some(7));
        editor.undo(false);
        assert_eq!(editor.document.to_bytes().unwrap(), before);
    }

    #[test]
    fn skypix_command_row_double_click_previews_through_that_item() {
        let context = egui::Context::default();
        let mut editor = SkypixEditor::new();
        editor.append(vec![
            SkypixItem::command(SkypixCommand::SetPenA { color: 7 }),
            SkypixItem::command(SkypixCommand::SetPixel { x: 10, y: 10 }),
            SkypixItem::command(SkypixCommand::SetPixel { x: 20, y: 20 }),
        ]);
        editor.select(None);
        let output = frame(&context, &mut editor, vec![], false);
        let label = format!("{}{}", item_name(&editor.document.items()[1]), item_summary(editor.document.items(), 1));
        let row = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => Some(text.pos + text.galley.size() * 0.5),
                _ => None,
            })
            .expect("shared command row renders its name and compact summary");
        for _ in 0..2 {
            for pressed in [true, false] {
                frame(
                    &context,
                    &mut editor,
                    vec![
                        egui::Event::PointerMoved(row),
                        egui::Event::PointerButton {
                            pos: row,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    false,
                );
            }
        }
        assert_eq!(editor.selected, Some(1));
        assert!(editor.preview_selection);
        assert_eq!(
            editor.shown,
            Some(PreviewKey {
                revision: editor.document.revision(),
                count: Some(2),
                dragged_line: None,
            })
        );
    }
}
