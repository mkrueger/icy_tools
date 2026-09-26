//! Editor chrome: palette and tools on the left, tool options on top, minimap
//! and layers on the right and a compact status bar.

use super::{widgets, Dialog, DrawApp};
use eframe::egui::{self, Color32};
use icy_engine::{LayerProperties, Position, Rectangle, RenderOptions, Role, TextBuffer, TextPane};
use icy_engine_edit::tools::{Tool, ToolPair};
use icy_engine_gui::egui::appearance::{self, labels, Dialog as SharedDialog, DialogButton, DialogSize, PRIMARY};

/// Width of the left palette and tool rail.
pub const SIDEBAR_WIDTH: f32 = 52.0;
/// Width of the right sidebar with minimap and layers.
pub const PANEL_WIDTH: f32 = 320.0;
/// Height of the full-width tool options bar.
pub const TOOLBAR_HEIGHT: f32 = 44.0;
pub const STATUS_HEIGHT: f32 = 28.0;
const TOOL_ICON: f32 = 36.0;
const LAYER_PREVIEW: [usize; 2] = [56, 36];
const LAYER_ROW_HEIGHT: f32 = 48.0;
const MINIMAP_PIXELS: usize = 512;
/// Horizontal padding of the right sidebar sections.
const SECTION_MARGIN: i8 = 12;

const TOOL_SLOTS: [ToolPair; 10] = [
    ToolPair::single(Tool::Click),
    ToolPair::single(Tool::Select),
    ToolPair::single(Tool::Pencil),
    ToolPair::single(Tool::Line),
    ToolPair::new(Tool::RectangleOutline, Tool::RectangleFilled),
    ToolPair::new(Tool::EllipseOutline, Tool::EllipseFilled),
    ToolPair::single(Tool::Fill),
    ToolPair::single(Tool::Pipette),
    ToolPair::single(Tool::Font),
    ToolPair::single(Tool::Tag),
];

#[derive(Default)]
pub struct Chrome {
    minimap: Option<(u64, egui::TextureHandle)>,
    previews: Vec<Option<(u64, Option<egui::TextureHandle>)>>,
    layer_properties: Option<(usize, LayerProperties)>,
    outline_preview: Option<(u64, usize, egui::TextureHandle)>,
    outline_style: usize,
}

enum LayerAction {
    Select(usize),
    Properties(usize),
    Visibility(usize),
    Lock(usize),
    Add(usize),
    Remove(usize),
    Duplicate(usize),
    Raise(usize),
    Lower(usize),
    Merge(usize),
    Clear(usize),
}

fn mix(hash: &mut u64, value: u64) {
    *hash ^= value.wrapping_add(0x9e37_79b9_7f4a_7c15).wrapping_add(*hash << 6).wrapping_add(*hash >> 2);
}

pub(super) fn color_sample(ui: &mut egui::Ui, label: &str, index: u32, (red, green, blue): (u8, u8, u8), selected: bool) {
    const LABEL_WIDTH: f32 = 48.0;
    const GAP: f32 = 6.0;
    const SWATCH_WIDTH: f32 = 82.0;
    const HEIGHT: f32 = 30.0;

    let (rect, response) = ui.allocate_exact_size(egui::vec2(LABEL_WIDTH + GAP + SWATCH_WIDTH, HEIGHT), egui::Sense::hover());
    let label_rect = egui::Rect::from_min_size(rect.min, egui::vec2(LABEL_WIDTH, HEIGHT));
    let swatch_rect = egui::Rect::from_min_size(rect.min + egui::vec2(LABEL_WIDTH + GAP, 0.0), egui::vec2(SWATCH_WIDTH, HEIGHT));
    let visuals = ui.visuals();
    let label_color = if selected {
        visuals.strong_text_color()
    } else {
        visuals.weak_text_color().gamma_multiply(0.55)
    };
    ui.painter().text(
        label_rect.right_center(),
        egui::Align2::RIGHT_CENTER,
        format!("{label} {index}"),
        egui::FontId::monospace(13.0),
        label_color,
    );

    let color = Color32::from_rgb(red, green, blue);
    let text = if 299 * u32::from(red) + 587 * u32::from(green) + 114 * u32::from(blue) > 186_000 {
        Color32::BLACK
    } else {
        Color32::WHITE
    };
    ui.painter().rect_filled(swatch_rect, 4, color);
    ui.painter()
        .rect_stroke(swatch_rect, 4, egui::Stroke::new(1.0, visuals.strong_text_color()), egui::StrokeKind::Inside);
    ui.painter().text(
        swatch_rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("#{red:02X}{green:02X}{blue:02X}"),
        egui::FontId::monospace(13.0),
        text,
    );
    response.on_hover_text(format!("{label} {index}: #{red:02X}{green:02X}{blue:02X}"));
}

/// Cache key built on the engine's buffer version, like the original layer view.
fn signature(buffer: &TextBuffer) -> u64 {
    let mut hash = buffer.version();
    for value in [
        buffer.width() as u64,
        buffer.height() as u64,
        buffer.layers.len() as u64,
        buffer.use_letter_spacing() as u64,
        buffer.use_aspect_ratio() as u64,
        buffer.palette.len() as u64,
    ] {
        mix(&mut hash, value);
    }
    for layer in &buffer.layers {
        for value in [layer.is_visible() as u64, layer.offset().x as u64, layer.offset().y as u64] {
            mix(&mut hash, value);
        }
    }
    hash
}

/// Box-filtered downscale that fits the longer side into `target` pixels.
fn downsample(width: usize, height: usize, rgba: &[u8], target: usize) -> Option<egui::ColorImage> {
    downsample_by(width, height, rgba, width.max(height).div_ceil(target.max(1)).max(1))
}

/// Box-filtered downscale by an integer `step` so small previews stay readable.
fn downsample_by(width: usize, height: usize, rgba: &[u8], step: usize) -> Option<egui::ColorImage> {
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        return None;
    }
    let step = step.max(1);
    let (columns, rows) = (width.div_ceil(step), height.div_ceil(step));
    let mut image = egui::ColorImage::filled([columns, rows], Color32::BLACK);
    for row in 0..rows {
        for column in 0..columns {
            let (mut red, mut green, mut blue, mut alpha, mut count) = (0u64, 0u64, 0u64, 0u64, 0u64);
            for y in row * step..((row + 1) * step).min(height) {
                for x in column * step..((column + 1) * step).min(width) {
                    let offset = (y * width + x) * 4;
                    let opacity = u64::from(rgba[offset + 3]);
                    red += u64::from(rgba[offset]) * opacity;
                    green += u64::from(rgba[offset + 1]) * opacity;
                    blue += u64::from(rgba[offset + 2]) * opacity;
                    alpha += opacity;
                    count += 1;
                }
            }
            if count > 0 {
                image[(column, row)] = Color32::from_rgba_unmultiplied(
                    (red / alpha.max(1)) as u8,
                    (green / alpha.max(1)) as u8,
                    (blue / alpha.max(1)) as u8,
                    (alpha / count) as u8,
                );
            }
        }
    }
    Some(image)
}

fn render(buffer: &TextBuffer, target: usize) -> Option<egui::ColorImage> {
    let size = buffer.size();
    if size.width <= 0 || size.height <= 0 {
        return None;
    }
    let options: RenderOptions = Rectangle::from(0, 0, size.width, size.height).into();
    let (pixels, rgba) = buffer.render_to_rgba(&options, false);
    downsample(pixels.width.max(0) as usize, pixels.height.max(0) as usize, &rgba, target)
}

/// Renders the minimap fitted to `MINIMAP_PIXELS` in width, so tall documents keep
/// their detail; only documents taller than `max_side` are reduced further.
fn render_minimap(buffer: &TextBuffer, max_side: usize) -> Option<egui::ColorImage> {
    let size = buffer.size();
    if size.width <= 0 || size.height <= 0 {
        return None;
    }
    let options: RenderOptions = Rectangle::from(0, 0, size.width, size.height).into();
    let (pixels, rgba) = buffer.render_to_rgba(&options, false);
    let (width, height) = (pixels.width.max(0) as usize, pixels.height.max(0) as usize);
    let step = width.div_ceil(MINIMAP_PIXELS).max(height.div_ceil(max_side.max(1)));
    downsample_by(width, height, &rgba, step)
}

/// Minimap geometry: the image fills the available width and, when taller than the
/// available area, scrolls proportionally with the canvas so the viewport frame stays visible.
struct MinimapLayout {
    area: egui::Rect,
    image: egui::Rect,
    frame: egui::Rect,
    /// Minimap pixels per canvas scroll unit.
    scale: egui::Vec2,
    /// Screen y of the frame's top edge at canvas offset zero.
    base: f32,
    /// Screen movement of the frame's top edge per vertical canvas scroll unit.
    track: f32,
}

impl MinimapLayout {
    fn new(area: egui::Rect, texture: [usize; 2], content: egui::Vec2, viewport: egui::Vec2, offset: egui::Vec2, max_offset: egui::Vec2) -> Option<Self> {
        if texture[0] == 0 || texture[1] == 0 || content.x <= 0.0 || content.y <= 0.0 || area.width() <= 0.0 {
            return None;
        }
        let size = egui::vec2(area.width(), area.width() * texture[1] as f32 / texture[0] as f32);
        let overflow = (size.y - area.height()).max(0.0);
        let scroll_rate = if overflow > 0.0 && max_offset.y > 0.0 { overflow / max_offset.y } else { 0.0 };
        let base = if overflow > 0.0 { area.top() } else { area.center().y - size.y / 2.0 };
        let scroll = (offset.y * scroll_rate).clamp(0.0, overflow);
        let image = egui::Rect::from_min_size(egui::pos2(area.left(), base - scroll), size);
        let scale = size / content;
        let frame = egui::Rect::from_min_size(image.min + offset * scale, (viewport / content).min(egui::Vec2::splat(1.0)) * size);
        Some(Self {
            area,
            image,
            frame,
            scale,
            base,
            track: scale.y - scroll_rate,
        })
    }

    /// Canvas offset that centers the viewport on a minimap point.
    fn offset_at(&self, point: egui::Pos2, viewport: egui::Vec2) -> egui::Vec2 {
        (point - self.image.min) / self.scale - viewport / 2.0
    }

    /// Screen position of the frame's top-left corner for a canvas offset.
    fn frame_corner(&self, offset: egui::Vec2) -> egui::Pos2 {
        if self.track > f32::EPSILON {
            egui::pos2(self.area.left() + offset.x * self.scale.x, self.base + offset.y * self.track)
        } else {
            self.image.min + offset * self.scale
        }
    }

    /// Canvas offset that moves the frame's top-left corner to `corner`, so dragging the
    /// frame behaves like a scrollbar thumb even while the minimap scrolls underneath it.
    fn offset_for_frame(&self, corner: egui::Pos2) -> egui::Vec2 {
        if self.track > f32::EPSILON {
            egui::vec2((corner.x - self.area.left()) / self.scale.x, (corner.y - self.base) / self.track)
        } else {
            (corner - self.image.min) / self.scale
        }
    }
}

fn layer_preview(buffer: &TextBuffer, index: usize) -> Option<egui::ColorImage> {
    let layer = buffer.layers.get(index)?;
    let size = layer.size();
    if size.width <= 0 || size.height <= 0 {
        return None;
    }
    let mut preview = TextBuffer::create(size);
    preview.palette = buffer.palette.clone();
    preview.set_font_table(buffer.font_table());
    preview.ice_mode = buffer.ice_mode;
    preview.set_use_letter_spacing(buffer.use_letter_spacing());
    preview.set_use_aspect_ratio(buffer.use_aspect_ratio());
    let mut copy = layer.clone();
    copy.set_offset(Position::default());
    copy.set_is_visible(true);
    preview.layers.clear();
    preview.layers.push(copy);
    let columns = size.width.min(80);
    let rows = size.height.min(25);
    let options: RenderOptions = Rectangle::from(0, 0, columns, rows).into();
    let dimensions = preview.font_dimensions();
    let region = Rectangle::from(0, 0, columns * (dimensions.width + 1), rows * dimensions.height * 2);
    let (pixels, rgba) = preview.render_region_to_rgba(region, &options, false);
    downsample(pixels.width.max(0) as usize, pixels.height.max(0) as usize, &rgba, LAYER_PREVIEW[0] * 2)
}

impl DrawApp {
    /// Overlapping foreground/background swatches with swap and reset corners; opens a palette popup.
    pub(super) fn color_switcher(&mut self, ui: &mut egui::Ui) {
        let (foreground, background) = self.document.with_state(|state| {
            let palette = &state.get_buffer().palette;
            let attribute = state.get_caret().attribute;
            (
                attribute.foreground_color().as_rgb().unwrap_or_else(|| palette.rgb(attribute.foreground())),
                attribute.background_color().as_rgb().unwrap_or_else(|| palette.rgb(attribute.background())),
            )
        });
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(40.0), egui::Sense::hover());
        let swatch = 24.0;
        let foreground_rect = egui::Rect::from_min_size(rect.min, egui::Vec2::splat(swatch));
        let background_rect = egui::Rect::from_min_size(rect.max - egui::Vec2::splat(swatch), egui::Vec2::splat(swatch));
        let swap = egui::Rect::from_min_size(egui::pos2(rect.right() - 14.0, rect.top()), egui::Vec2::splat(14.0));
        let default = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.bottom() - 14.0), egui::Vec2::splat(14.0));
        let colors = ui
            .interact(foreground_rect.union(background_rect), ui.id().with("caret-colors"), egui::Sense::click())
            .on_hover_text("Foreground / Background – click to pick a palette color");
        let swap_response = ui
            .interact(swap, ui.id().with("swap-colors"), egui::Sense::click())
            .on_hover_text("Swap Foreground and Background");
        let default_response = ui
            .interact(default, ui.id().with("default-colors"), egui::Sense::click())
            .on_hover_text("Default Colors");

        let visuals = ui.visuals().clone();
        let painter = ui.painter();
        let border = visuals.widgets.inactive.fg_stroke.color.gamma_multiply(0.6);
        let plate = |target: egui::Rect, (red, green, blue): (u8, u8, u8)| {
            painter.rect_filled(target.expand(2.0), 6, visuals.panel_fill);
            painter.rect_filled(target, 4, Color32::from_rgb(red, green, blue));
            painter.rect_stroke(target, 4, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);
        };
        plate(background_rect, background);
        plate(foreground_rect, foreground);
        let tint = |response: &egui::Response| {
            if response.hovered() {
                visuals.strong_text_color()
            } else {
                visuals.weak_text_color()
            }
        };
        self.icons
            .image(ui, "swap", 12.0)
            .tint(tint(&swap_response))
            .paint_at(ui, egui::Rect::from_center_size(swap.center(), egui::Vec2::splat(12.0)));
        let reset_color = tint(&default_response);
        let back = egui::Rect::from_min_size(default.min + egui::vec2(5.0, 5.0), egui::Vec2::splat(7.0));
        let front = egui::Rect::from_min_size(default.min + egui::vec2(1.0, 1.0), egui::Vec2::splat(7.0));
        ui.painter().rect_filled(back, 1, Color32::BLACK);
        ui.painter().rect_stroke(back, 1, egui::Stroke::new(1.0, reset_color), egui::StrokeKind::Inside);
        ui.painter().rect_filled(front, 1, Color32::from_gray(170));
        ui.painter()
            .rect_stroke(front, 1, egui::Stroke::new(1.0, reset_color), egui::StrokeKind::Inside);

        if swap_response.clicked() {
            let result = self.document.swap_caret_colors();
            self.result(result);
        }
        if default_response.clicked() {
            let result = self.document.reset_caret_colors();
            self.result(result);
        }
        egui::Popup::from_toggle_button_response(&colors)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .align(egui::RectAlign::RIGHT_END)
            .align_alternatives(&[egui::RectAlign::RIGHT_START, egui::RectAlign::RIGHT])
            .show(|ui| {
                ui.set_width(208.0);
                ui.weak("Left click: foreground · Right click: background");
                self.palette_grid(ui, 208.0);
                if ui.button("Edit Palette…").clicked() {
                    self.open_palette_editor(true);
                    ui.close();
                }
            });
    }

    fn open_palette_editor(&mut self, foreground: bool) {
        self.palette_edit = self.document.with_state(|state| state.get_buffer().palette.clone());
        self.palette_index = self.document.with_state(|state| {
            let attribute = state.get_caret().attribute;
            if foreground {
                attribute.foreground()
            } else {
                attribute.background()
            }
        }) as usize;
        self.palette_index = self.palette_index.min(self.palette_edit.len().saturating_sub(1));
        self.dialog = Some(Dialog::Palette);
    }

    /// Left rail: the persistent palette followed by grouped tools.
    pub(super) fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            let palette_width = ui.available_width();
            self.palette_grid(ui, palette_width);
            rail_divider(ui);
            egui::ScrollArea::vertical()
                .id_salt("sidebar")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let mut last_group = None;
                        for (slot, pair) in TOOL_SLOTS.into_iter().enumerate() {
                            if self.document.outline_font && !matches!(pair.primary, Tool::Click | Tool::Select) {
                                continue;
                            }
                            if self.charfont.is_some() && pair.primary == Tool::Tag {
                                continue;
                            }
                            let group = TOOL_GROUPS.iter().position(|&end| slot < end);
                            if last_group.is_some_and(|last| Some(last) != group) {
                                rail_divider(ui);
                            }
                            last_group = group;
                            self.tool_button(ui, pair);
                        }
                    });
                });
        });
    }

    fn tool_button(&mut self, ui: &mut egui::Ui, pair: ToolPair) {
        let selected = pair.contains(self.document.tool);
        let tool = if selected { self.document.tool } else { pair.primary };
        let label = format!("{} – {}", tool_label(tool), tool_hint(tool));
        let response = self.icons.button_sized(ui, tool.icon(), &label, selected, TOOL_ICON);
        if response.clicked() {
            self.select_tool(if selected { pair.toggle(tool) } else { pair.primary });
        }
        if pair.primary != pair.secondary {
            let corner = response.rect.right_bottom() - egui::vec2(4.0, 4.0);
            let color = if selected { Color32::WHITE } else { ui.visuals().weak_text_color() };
            ui.painter().add(egui::Shape::convex_polygon(
                vec![corner, corner - egui::vec2(4.0, 0.0), corner - egui::vec2(0.0, 4.0)],
                color,
                egui::Stroke::NONE,
            ));
            response.context_menu(|ui| {
                for variant in [pair.primary, pair.secondary] {
                    if ui.selectable_label(self.document.tool == variant, tool_label(variant)).clicked() {
                        self.select_tool(variant);
                        ui.close();
                    }
                }
            });
        }
    }

    pub(super) fn select_tool(&mut self, tool: Tool) {
        if self.document.paste_active() {
            return;
        }
        if self.document.outline_font && !matches!(tool, Tool::Click | Tool::Select) {
            return;
        }
        self.document.finish();
        self.document.tool = tool;
        self.canvas_focus = true;
        if tool == Tool::Font && self.text_fonts.is_none() {
            self.text_fonts = Some(icy_draw::text_art_fonts::TextArtFontLibrary::create_shared());
        }
    }

    /// Selection, layer border and caret overlays the way the original editor renders them.
    pub(super) fn editor_markers(&mut self) -> icy_engine_gui::EditorMarkers {
        use icy_engine::AddType;
        use icy_engine_gui::selection_colors;

        let mut markers = icy_engine_gui::EditorMarkers::default();
        let floating = self.document.paste_active();
        markers.paste_mode = floating;
        markers.show_layer_bounds = floating || self.show_layer_bounds;
        markers.layer_border_animated = floating;
        markers.marker_settings = Some(icy_engine_gui::MarkerSettings::default());
        if let Some(image) = &mut self.reference_image {
            if image.visible {
                image.load_and_cache();
            }
            markers.reference_image = Some(image.clone());
        }
        self.document.with_state(|state| {
            let dimensions = state.get_buffer().font_dimensions();
            let (font_width, font_height) = (dimensions.width as f32, dimensions.height as f32);
            let to_pixels = |(columns, rows): (i32, i32)| (columns as f32 * font_width, rows as f32 * font_height);
            markers.raster = self.raster.filter(|_| self.show_raster).map(to_pixels);
            markers.guide = self.guide.filter(|_| self.show_guide).map(to_pixels);
            let pixels_of = |position: Position, size: icy_engine::Size| {
                (
                    position.x as f32 * font_width,
                    position.y as f32 * font_height,
                    size.width as f32 * font_width,
                    size.height as f32 * font_height,
                )
            };
            let selection = state.selection();
            markers.selection_color = match selection.map(|selection| selection.add_type) {
                Some(AddType::Add) => selection_colors::ADD,
                Some(AddType::Subtract) => selection_colors::SUBTRACT,
                _ => selection_colors::DEFAULT,
            };
            markers.selection_rect = selection.map(|selection| {
                let rect = selection.as_rectangle();
                pixels_of(rect.start, rect.size())
            });
            if !icy_engine::Screen::selection_mask(state).is_empty() {
                let (width, height) = (state.get_buffer().width().max(1) as u32, state.get_buffer().height().max(1) as u32);
                let mut mask = vec![255u8; (width * height * 4) as usize];
                for y in 0..height {
                    for x in 0..width {
                        let value = if state.get_is_mask_selected(Position::new(x as i32, y as i32)) {
                            255
                        } else {
                            0
                        };
                        let offset = ((y * width + x) * 4) as usize;
                        mask[offset..offset + 3].fill(value);
                    }
                }
                markers.selection_mask_data = Some((mask, width, height));
            }
            if let Some(layer) = state.get_current_layer().ok().and_then(|index| state.get_buffer().layers.get(index)) {
                let bounds = pixels_of(layer.offset(), layer.size());
                markers.caret_origin_px = (bounds.0, bounds.1);
                markers.layer_bounds = Some(bounds);
            }
        });
        markers
    }

    /// Row numbers beside and column digits above and below the document, highlighting the caret or selection like the original editor.
    pub(super) fn line_numbers(&mut self, ui: &egui::Ui, painter: &egui::Painter, clip: egui::Rect, origin: egui::Pos2, step: egui::Vec2) {
        if step.x <= 0.0 || step.y <= 0.0 {
            return;
        }
        let (width, height, caret, selection) = self.document.with_state(|state| {
            let buffer = state.get_buffer();
            (
                buffer.width(),
                buffer.height(),
                state.layer_to_document_position(state.get_caret().position()),
                state.selection().map(|selection| selection.as_rectangle()),
            )
        });
        let (rows, columns) = match selection {
            Some(rect) => (rect.top()..rect.bottom(), rect.left()..rect.right()),
            None => (caret.y..caret.y + 1, caret.x..caret.x + 1),
        };
        let document = egui::Rect::from_min_size(origin, egui::vec2(width as f32 * step.x, height as f32 * step.y));
        let font = egui::FontId::monospace((12.0 * step.y / 16.0).clamp(8.0, 16.0));
        let visuals = ui.visuals();
        let (normal, active) = (visuals.weak_text_color(), visuals.strong_text_color());
        let background = visuals.extreme_bg_color.gamma_multiply(0.85);
        let digit = painter.layout_no_wrap("0".to_owned(), font.clone(), normal).size();
        let gap = 4.0;

        let gutter = digit.x * height.max(1).to_string().len() as f32 + gap;
        let first_row = (((clip.top() - origin.y) / step.y).floor() as i32).max(0);
        let last_row = (((clip.bottom() - origin.y) / step.y).ceil() as i32).min(height);
        // Keep the numbers on screen: they sit outside the document when there is room and overlay its edge otherwise.
        let left = (document.left() - gap).max(clip.left() + gutter);
        let right = (document.right() + gap).min(clip.right() - gutter);
        for (edge, align) in [(left, egui::Align2::RIGHT_CENTER), (right, egui::Align2::LEFT_CENTER)] {
            let strip = if align == egui::Align2::RIGHT_CENTER {
                egui::Rect::from_x_y_ranges(edge - gutter + gap / 2.0..=edge + gap / 2.0, document.y_range())
            } else {
                egui::Rect::from_x_y_ranges(edge - gap / 2.0..=edge + gutter - gap / 2.0, document.y_range())
            };
            if strip.intersects(document) {
                painter.rect_filled(strip.intersect(clip), 0, background);
            }
            for row in first_row..last_row {
                let y = origin.y + (row as f32 + 0.5) * step.y;
                let color = if rows.contains(&row) { active } else { normal };
                painter.text(egui::pos2(edge, y), align, (row + 1).to_string(), font.clone(), color);
            }
        }

        if step.x < digit.x * 0.9 {
            return;
        }
        let first_column = (((clip.left() - origin.x) / step.x).floor() as i32).max(0);
        let last_column = (((clip.right() - origin.x) / step.x).ceil() as i32).min(width);
        let top = (document.top() - gap / 2.0).max(clip.top() + digit.y);
        let bottom = (document.bottom() + gap / 2.0).min(clip.bottom() - digit.y);
        for (edge, align) in [(top, egui::Align2::CENTER_BOTTOM), (bottom, egui::Align2::CENTER_TOP)] {
            let strip = if align == egui::Align2::CENTER_BOTTOM {
                egui::Rect::from_x_y_ranges(document.x_range(), edge - digit.y..=edge)
            } else {
                egui::Rect::from_x_y_ranges(document.x_range(), edge..=edge + digit.y)
            };
            if strip.intersects(document) {
                painter.rect_filled(strip.intersect(clip), 0, background);
            }
            for column in first_column..last_column {
                let x = origin.x + (column as f32 + 0.5) * step.x;
                let color = if columns.contains(&column) { active } else { normal };
                painter.text(egui::pos2(x, edge), align, ((column + 1) % 10).to_string(), font.clone(), color);
            }
        }
    }

    /// Row-major palette grid: left click sets the foreground, right click the background.
    fn palette_grid(&mut self, ui: &mut egui::Ui, width: f32) {
        let (palette, foreground, background) = self.document.with_state(|state| {
            let attribute = state.get_caret().attribute;
            (state.get_buffer().palette.clone(), attribute.foreground(), attribute.background())
        });
        let count = palette.len().min(256);
        if count == 0 {
            return;
        }
        let columns = if count <= 16 {
            if width < 100.0 {
                2
            } else {
                8
            }
        } else {
            16
        }
        .min(count);
        let cell = width / columns as f32;
        let rows = count.div_ceil(columns);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, cell * rows as f32), egui::Sense::click());
        let gap = if cell >= 16.0 { 3.0 } else { 1.0 };
        let rounding = if cell >= 16.0 { 4 } else { 1 };
        let edge = ui.visuals().widgets.noninteractive.bg_stroke.color;
        let hovered = response.hover_pos().and_then(|point| {
            let local = point - rect.min;
            let (column, row) = ((local.x / cell) as usize, (local.y / cell) as usize);
            (local.x >= 0.0 && local.y >= 0.0 && column < columns && row * columns + column < count).then_some(row * columns + column)
        });
        let painter = ui.painter();
        for index in 0..count {
            let (column, row) = (index % columns, index / columns);
            let target = egui::Rect::from_min_size(rect.min + egui::vec2(column as f32 * cell, row as f32 * cell), egui::Vec2::splat(cell)).shrink(gap / 2.0);
            let (red, green, blue) = palette.rgb(index as u32);
            painter.rect_filled(target, rounding, Color32::from_rgb(red, green, blue));
            painter.rect_stroke(target, rounding, egui::Stroke::new(1.0, edge), egui::StrokeKind::Inside);
            if index as u32 == foreground {
                painter.rect_stroke(target, rounding, egui::Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Inside);
                painter.rect_stroke(target.shrink(2.0), rounding, egui::Stroke::new(1.0, Color32::BLACK), egui::StrokeKind::Inside);
            } else if hovered == Some(index) {
                painter.rect_stroke(
                    target,
                    rounding,
                    egui::Stroke::new(1.0, ui.visuals().strong_text_color()),
                    egui::StrokeKind::Inside,
                );
            }
            if index as u32 == background {
                let radius = (cell * 0.14).clamp(2.0, 3.5);
                painter.circle_filled(target.center(), radius, Color32::WHITE);
                painter.circle_stroke(target.center(), radius, egui::Stroke::new(1.0, Color32::BLACK));
            }
        }
        if let Some(index) = hovered {
            if response.clicked() {
                let result = self.document.set_caret_foreground(index as u32);
                self.result(result);
            }
            if response.secondary_clicked() {
                let result = self.document.set_caret_background(index as u32);
                self.result(result);
            }
            let (red, green, blue) = palette.rgb(index as u32);
            response.on_hover_text(format!("{index}: #{red:02X}{green:02X}{blue:02X}"));
        }
    }

    /// Full-width tool options bar: the active tool's name followed by its options.
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(6.0);
            self.color_switcher(ui);
            widgets::divider(ui);
            let (icon, name) = if self.document.paste_active() {
                ("anchor", "Paste")
            } else {
                (self.document.tool.icon(), tool_label(self.document.tool))
            };
            let wide = ui.available_width() >= 760.0;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(if wide { 150.0 } else { 28.0 }, widgets::CONTROL_HEIGHT), egui::Sense::hover());
            let icon_rect = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.center().y - 14.0), egui::Vec2::splat(28.0));
            ui.painter().rect_filled(icon_rect, 6, ui.visuals().widgets.inactive.weak_bg_fill);
            self.icons
                .image(ui, icon, 18.0)
                .tint(ui.visuals().strong_text_color())
                .paint_at(ui, egui::Rect::from_center_size(icon_rect.center(), egui::Vec2::splat(18.0)));
            if wide {
                ui.painter().with_clip_rect(rect).text(
                    egui::pos2(icon_rect.right() + 8.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    name,
                    egui::FontId::proportional(14.0),
                    ui.visuals().strong_text_color(),
                );
            }
            widgets::divider(ui);
            egui::ScrollArea::horizontal()
                .id_salt("tool-options")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_height(TOOLBAR_HEIGHT);
                    ui.horizontal_centered(|ui| self.tool_options(ui, context));
                });
        });
    }

    /// Right sidebar: minimap and layers as separated sections.
    pub(super) fn panel(&mut self, ui: &mut egui::Ui) {
        let signature = self.document.with_state(|state| signature(state.get_buffer()));
        if self.charfont.is_some() {
            section(ui, |ui| self.charfont_section(ui));
        }
        if self.document.outline_font {
            section(ui, |ui| {
                let style = &mut self.chrome.outline_style;
                widgets::section_header(ui, "Outline Preview", |ui| {
                    egui::ComboBox::from_id_salt("outline-preview-style")
                        .width(90.0)
                        .selected_text(format!("Style {}", *style + 1))
                        .show_ui(ui, |ui| {
                            for index in 0..19 {
                                ui.selectable_value(style, index, format!("Style {}", index + 1));
                            }
                        });
                });
                if self
                    .chrome
                    .outline_preview
                    .as_ref()
                    .is_none_or(|(key, style, _)| *key != signature || *style != self.chrome.outline_style)
                {
                    let buffer = icy_draw::charfont::outline_preview(&self.document, self.chrome.outline_style);
                    if let Some(image) = render(&buffer, MINIMAP_PIXELS) {
                        self.chrome.outline_preview = Some((
                            signature,
                            self.chrome.outline_style,
                            ui.ctx().load_texture("outline-preview", image, egui::TextureOptions::NEAREST),
                        ));
                    }
                }
                if let Some((_, _, texture)) = &self.chrome.outline_preview {
                    ui.add(egui::Image::new(texture).corner_radius(4).fit_to_exact_size(egui::vec2(
                        ui.available_width(),
                        ui.available_width() * texture.size()[1] as f32 / texture.size()[0] as f32,
                    )));
                }
            });
            return;
        }
        if self.collab.active {
            // Moebius documents have a single layer, so the session only shows the minimap.
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: SECTION_MARGIN,
                    right: SECTION_MARGIN,
                    top: 2,
                    bottom: 10,
                })
                .show(ui, |ui| self.minimap(ui, signature));
            return;
        }
        if self.charfont.is_none() {
            egui::TopBottomPanel::top("minimap")
                .resizable(true)
                .default_height(340.0)
                .height_range(160.0..=520.0)
                .frame(egui::Frame::new().inner_margin(egui::Margin {
                    left: SECTION_MARGIN,
                    right: SECTION_MARGIN,
                    top: 2,
                    bottom: 10,
                }))
                .show_inside(ui, |ui| self.minimap(ui, signature));
        }
        self.layers(ui, signature);
    }

    fn minimap(&mut self, ui: &mut egui::Ui, signature: u64) {
        widgets::section_header(ui, "Minimap", |_| {});
        if self.chrome.minimap.as_ref().is_none_or(|(key, _)| *key != signature) {
            let max_side = ui.ctx().input(|input| input.max_texture_side);
            if let Some(image) = self.document.with_state(|state| render_minimap(state.get_buffer(), max_side)) {
                let texture = ui.ctx().load_texture("minimap", image, egui::TextureOptions::LINEAR);
                self.chrome.minimap = Some((signature, texture));
            }
        }
        let Some((_, texture)) = self.chrome.minimap.clone() else {
            return;
        };
        let (well, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
        ui.painter().rect_filled(well, 6, ui.visuals().extreme_bg_color);
        let area = well.shrink(6.0);
        let viewport = self.canvas_rect.size();
        let max_offset = self.view.max_offset;
        let Some(layout) = MinimapLayout::new(area, texture.size(), self.content_size(), viewport, self.view.offset, max_offset) else {
            return;
        };
        let id = ui.id().with("minimap-image");
        let response = ui.interact(layout.image.intersect(area), id, egui::Sense::click_and_drag());
        let painter = ui.painter_at(area);
        painter.image(
            texture.id(),
            layout.image,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        let frame = layout.frame.intersect(layout.image);
        let shows_frame = layout.frame.width() < layout.image.width() - 1.0 || layout.frame.height() < layout.image.height() - 1.0;
        if shows_frame {
            painter.rect_filled(frame, 0, PRIMARY.gamma_multiply(0.15));
            painter.rect_stroke(frame, 0, egui::Stroke::new(1.5, PRIMARY), egui::StrokeKind::Inside);
        }
        let clamp = |offset: egui::Vec2| offset.max(egui::Vec2::ZERO).min(max_offset);
        let grab_id = id.with("grab");
        let mut target = None;
        if let Some(point) = response.interact_pointer_pos() {
            if response.drag_started() || response.clicked() {
                // Dragging the frame keeps the grab point; anywhere else jumps there first.
                let grab = if shows_frame && !response.clicked() && frame.contains(point) {
                    point - layout.frame.min
                } else {
                    let offset = clamp(layout.offset_at(point, viewport));
                    target = Some(offset);
                    point - layout.frame_corner(offset)
                };
                ui.data_mut(|data| data.insert_temp(grab_id, grab));
            } else if response.dragged() {
                let grab = ui.data(|data| data.get_temp(grab_id)).unwrap_or(layout.frame.size() / 2.0);
                target = Some(clamp(layout.offset_for_frame(point - grab)));
            }
        }
        if response.hovered() && target.is_none() {
            let delta = ui.input(|input| input.smooth_scroll_delta);
            if delta != egui::Vec2::ZERO {
                target = Some(clamp(self.view.offset - delta));
            }
        }
        if response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if shows_frame && response.hover_pos().is_some_and(|pos| frame.contains(pos)) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if target.is_some() {
            self.view.scroll_to = target;
        }
    }

    /// Canvas content size in scroll units, matching `ScreenView`'s offsets.
    fn content_size(&self) -> egui::Vec2 {
        let info = self.view.terminal.render_info.read();
        let cell = egui::vec2(info.font_width, info.font_height * if info.scan_lines { 2.0 } else { 1.0 }) * self.view.zoom;
        let size = self.document.with_state(|state| state.get_buffer().size());
        egui::vec2(size.width as f32 * cell.x, size.height as f32 * cell.y)
    }

    fn layers(&mut self, ui: &mut egui::Ui, signature: u64) {
        let current = self.document.with_state(|state| state.get_current_layer().unwrap_or(0));
        let rows = self.document.with_state(|state| {
            state
                .get_buffer()
                .layers
                .iter()
                .map(|layer| (layer.properties.clone(), layer.size(), layer.role))
                .collect::<Vec<_>>()
        });
        let mut action = None;
        let current_unlocked = rows.get(current).is_some_and(|(properties, _, _)| !properties.is_locked);
        let can_merge = |index: usize| {
            index > 0
                && rows
                    .get(index)
                    .is_some_and(|(properties, _, role)| !properties.is_locked && *role != Role::Image)
                && !rows[index - 1].0.is_locked
        };
        egui::Frame::new().inner_margin(egui::Margin::symmetric(SECTION_MARGIN, 0)).show(ui, |ui| {
            widgets::section_header(ui, "Layers", |ui| {
                ui.weak(rows.len().to_string());
            });
        });
        egui::TopBottomPanel::bottom("layer-actions")
            .exact_height(40.0)
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(8, 0)))
            .show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if self.icons.button(ui, "add_layer", "Add Layer", false).clicked() {
                        action = Some(LayerAction::Add(current));
                    }
                    if self.icons.button(ui, "file_copy", "Duplicate Layer", false).clicked() {
                        action = Some(LayerAction::Duplicate(current));
                    }
                    ui.add_enabled_ui(current_unlocked && current + 1 < rows.len(), |ui| {
                        if self.icons.button(ui, "move_up", "Raise Layer", false).clicked() {
                            action = Some(LayerAction::Raise(current));
                        }
                    });
                    ui.add_enabled_ui(current_unlocked && current > 0, |ui| {
                        if self.icons.button(ui, "move_down", "Lower Layer", false).clicked() {
                            action = Some(LayerAction::Lower(current));
                        }
                    });
                    ui.add_enabled_ui(can_merge(current), |ui| {
                        if self.icons.button(ui, "anchor", "Merge Down", false).clicked() {
                            action = Some(LayerAction::Merge(current));
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.add_enabled_ui(current_unlocked && rows.len() > 1, |ui| {
                            if self.icons.button(ui, "delete", "Delete Layer", false).clicked() {
                                action = Some(LayerAction::Remove(current));
                            }
                        });
                        if self.icons.button(ui, "measure", "Layer Properties", false).clicked() {
                            action = Some(LayerAction::Properties(current));
                        }
                    });
                });
            });
        self.chrome.previews.resize_with(rows.len(), || None);
        let row_pitch = LAYER_ROW_HEIGHT + 2.0;
        egui::ScrollArea::vertical()
            .id_salt("layers")
            .auto_shrink([false, false])
            .show_rows(ui, row_pitch, rows.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for row in range {
                    let index = rows.len() - row - 1;
                    let (properties, dimensions, _) = &rows[index];
                    self.ensure_layer_preview(ui.ctx(), index, signature);
                    let selected = index == current;
                    let (outer, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), LAYER_ROW_HEIGHT), egui::Sense::click());
                    let rect = outer.shrink2(egui::vec2(6.0, 0.0));
                    let fill = if selected {
                        ui.visuals().selection.bg_fill
                    } else if response.hovered() {
                        ui.visuals().widgets.hovered.weak_bg_fill
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(rect, 6, fill);
                    let text_color = if selected {
                        ui.visuals().selection.stroke.color
                    } else {
                        ui.visuals().text_color()
                    };
                    let muted = text_color.gamma_multiply(0.65);
                    let faded = !properties.is_visible;
                    ui.push_id(index, |ui| {
                        ui.scope_builder(
                            egui::UiBuilder::new()
                                .max_rect(rect.shrink2(egui::vec2(4.0, 0.0)))
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                            |ui| {
                                ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                                ui.spacing_mut().item_spacing.x = 6.0;
                                if self
                                    .icons
                                    .button_sized(
                                        ui,
                                        if properties.is_visible { "visibility" } else { "visibility_off" },
                                        if properties.is_visible { "Hide Layer" } else { "Show Layer" },
                                        false,
                                        26.0,
                                    )
                                    .clicked()
                                {
                                    action = Some(LayerAction::Visibility(index));
                                }
                                let preview = egui::vec2(LAYER_PREVIEW[0] as f32, LAYER_PREVIEW[1] as f32);
                                let (preview_rect, _) = ui.allocate_exact_size(preview, egui::Sense::hover());
                                let painter = ui.painter().with_clip_rect(preview_rect.intersect(ui.clip_rect()));
                                for checker_row in 0..(LAYER_PREVIEW[1] / 6) {
                                    for column in 0..(LAYER_PREVIEW[0] / 6 + 1) {
                                        let cell = egui::Rect::from_min_size(
                                            preview_rect.min + egui::vec2(column as f32 * 6.0, checker_row as f32 * 6.0),
                                            egui::Vec2::splat(6.0),
                                        );
                                        painter.rect_filled(cell, 0, Color32::from_gray(if (checker_row + column) % 2 == 0 { 48 } else { 64 }));
                                    }
                                }
                                if let Some((_, Some(texture))) = &self.chrome.previews[index] {
                                    let [width, height] = texture.size();
                                    let scale = (preview.x / width as f32).min(preview.y / height as f32);
                                    painter.image(
                                        texture.id(),
                                        egui::Rect::from_center_size(preview_rect.center(), egui::vec2(width as f32, height as f32) * scale),
                                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                        if faded { Color32::from_white_alpha(90) } else { Color32::WHITE },
                                    );
                                }
                                ui.painter().rect_stroke(
                                    preview_rect,
                                    3,
                                    egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                                    egui::StrokeKind::Outside,
                                );
                                let lock_width = 26.0;
                                let text_width = (ui.available_width() - lock_width - 6.0).max(0.0);
                                ui.allocate_ui_with_layout(egui::vec2(text_width, LAYER_ROW_HEIGHT), egui::Layout::top_down(egui::Align::Min), |ui| {
                                    ui.set_width(text_width);
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    ui.add_space(7.0);
                                    let title = egui::RichText::new(&properties.title).color(if faded { muted } else { text_color });
                                    ui.add(egui::Label::new(if selected { title.strong() } else { title }).truncate().selectable(false))
                                        .on_hover_text(&properties.title);
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(format!(
                                                "{} × {}  ·  {}, {}",
                                                dimensions.width, dimensions.height, properties.offset.x, properties.offset.y
                                            ))
                                            .size(11.0)
                                            .color(muted),
                                        )
                                        .truncate()
                                        .selectable(false),
                                    );
                                });
                                let lock = self.icons.subtle_button(
                                    ui,
                                    if properties.is_locked { "lock" } else { "lock_open" },
                                    if properties.is_locked { "Unlock Layer" } else { "Lock Layer" },
                                    !properties.is_locked,
                                    lock_width,
                                );
                                if lock.clicked() {
                                    action = Some(LayerAction::Lock(index));
                                }
                            },
                        );
                    });
                    if response.double_clicked() {
                        action = Some(LayerAction::Properties(index));
                    } else if response.clicked() && action.is_none() {
                        action = Some(LayerAction::Select(index));
                    }
                    response.context_menu(|ui| {
                        if ui.button("Layer Properties...").clicked() {
                            action = Some(LayerAction::Properties(index));
                            ui.close();
                        }
                        if ui.button(if properties.is_locked { "Unlock Layer" } else { "Lock Layer" }).clicked() {
                            action = Some(LayerAction::Lock(index));
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("New Layer").clicked() {
                            action = Some(LayerAction::Add(index));
                            ui.close();
                        }
                        if ui.button("Duplicate Layer").clicked() {
                            action = Some(LayerAction::Duplicate(index));
                            ui.close();
                        }
                        if ui.add_enabled(can_merge(index), egui::Button::new("Merge Down")).clicked() {
                            action = Some(LayerAction::Merge(index));
                            ui.close();
                        }
                        if ui
                            .add_enabled(!properties.is_locked && rows.len() > 1, egui::Button::new("Delete Layer"))
                            .clicked()
                        {
                            action = Some(LayerAction::Remove(index));
                            ui.close();
                        }
                        if ui.add_enabled(!properties.is_locked, egui::Button::new("Clear Layer")).clicked() {
                            action = Some(LayerAction::Clear(index));
                            ui.close();
                        }
                    });
                }
            });
        if let Some(action) = action {
            self.layer_action(action);
        }
    }

    fn ensure_layer_preview(&mut self, context: &egui::Context, index: usize, signature: u64) {
        if self.chrome.previews[index].as_ref().is_none_or(|(key, _)| *key != signature) {
            let image = self.document.with_state(|state| layer_preview(state.get_buffer(), index));
            let texture = image.map(|image| context.load_texture(format!("layer-{index}"), image, egui::TextureOptions::LINEAR));
            self.chrome.previews[index] = Some((signature, texture));
        }
    }

    fn layer_action(&mut self, action: LayerAction) {
        self.document.finish();
        match action {
            LayerAction::Select(index) => {
                self.document.with_state(|state| state.set_current_layer(index));
            }
            LayerAction::Properties(index) => {
                self.chrome.layer_properties = self
                    .document
                    .with_state(|state| state.get_buffer().layers.get(index).map(|layer| (index, layer.properties.clone())));
                self.canvas_focus = false;
            }
            LayerAction::Visibility(index) => self.edit(|state| state.toggle_layer_visibility(index)),
            LayerAction::Lock(index) => self.edit(|state| {
                let mut properties = state.get_buffer().layers[index].properties.clone();
                properties.is_locked = !properties.is_locked;
                state.update_layer_properties(index, properties)
            }),
            LayerAction::Add(index) => self.edit(|state| state.add_new_layer(index)),
            LayerAction::Remove(index) => self.edit(|state| state.remove_layer(index)),
            LayerAction::Duplicate(index) => self.edit(|state| state.duplicate_layer(index)),
            LayerAction::Raise(index) => self.edit(|state| state.raise_layer(index)),
            LayerAction::Lower(index) => self.edit(|state| state.lower_layer(index)),
            LayerAction::Merge(index) => self.edit(|state| state.merge_layer_down(index)),
            LayerAction::Clear(index) => self.edit(|state| state.clear_layer(index)),
        }
    }

    pub(super) fn layer_properties_open(&self) -> bool {
        self.chrome.layer_properties.is_some()
    }

    pub(super) fn layer_properties_dialog(&mut self, context: &egui::Context) {
        let Some((index, mut properties)) = self.chrome.layer_properties.take() else {
            return;
        };
        #[derive(Clone, Copy)]
        enum Action {
            Cancel,
            Apply,
        }
        let response = SharedDialog::new("layer-properties")
            .size(DialogSize::Small)
            .confirm_on_enter(true)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    appearance::group(ui, "", |ui| {
                        ui.label("Name");
                        ui.add(egui::TextEdit::singleline(&mut properties.title).desired_width(f32::INFINITY));
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut properties.is_visible, "Visible");
                            ui.checkbox(&mut properties.is_locked, "Locked");
                        });
                        ui.separator();
                        ui.checkbox(&mut properties.is_position_locked, "Lock Position");
                        ui.add_enabled_ui(!properties.is_position_locked, |ui| {
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut properties.offset.x).prefix("X "));
                                ui.add(egui::DragValue::new(&mut properties.offset.y).prefix("Y "));
                            });
                        });
                        ui.checkbox(&mut properties.has_alpha_channel, "Transparency");
                        ui.add_enabled_ui(properties.has_alpha_channel, |ui| {
                            ui.checkbox(&mut properties.is_alpha_channel_locked, "Lock Transparency");
                        });
                        egui::ComboBox::from_label("Mode")
                            .selected_text(format!("{:?}", properties.mode))
                            .show_ui(ui, |ui| {
                                for (mode, label) in [
                                    (icy_engine::Mode::Normal, "Normal"),
                                    (icy_engine::Mode::Chars, "Characters"),
                                    (icy_engine::Mode::Attributes, "Attributes"),
                                ] {
                                    ui.selectable_value(&mut properties.mode, mode, label);
                                }
                            });
                    });
                });
                dialog.buttons([
                    DialogButton::cancel(labels::cancel(), Action::Cancel),
                    DialogButton::primary("Apply", Action::Apply),
                ]);
            });
        match response.action {
            Some(Action::Apply) => {
                self.edit(|state| state.update_layer_properties(index, properties));
                self.canvas_focus = true;
            }
            Some(Action::Cancel) => self.canvas_focus = true,
            None if !response.dismissed => self.chrome.layer_properties = Some((index, properties)),
            None => self.canvas_focus = true,
        }
    }

    /// Status bar: document facts on the left, clickable document settings on the right.
    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        let (size, caret, selection, ice, spacing, aspect, font) = self.document.with_state(|state| {
            let buffer = state.get_buffer();
            let font = buffer
                .font(state.get_caret().attribute.font_page())
                .map(|font| font.name.to_string())
                .unwrap_or_else(|| "Unknown".into());
            (
                buffer.size(),
                state.get_caret().position(),
                state.selection().map(|selection| selection.as_rectangle()),
                buffer.ice_mode.has_high_bg_colors(),
                buffer.use_letter_spacing(),
                buffer.use_aspect_ratio(),
                font,
            )
        });
        let compact = ui.available_width() < 650.0;
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_space(8.0);
            let small = |text: String| egui::RichText::new(text).size(12.0);
            ui.label(small(format!("{} × {}", size.width, size.height)))
                .on_hover_text("Canvas size in characters");
            ui.label(small("·".into()).weak());
            if let Some(bounds) = selection {
                ui.label(small(format!("Selection {} × {}", bounds.width(), bounds.height())).weak())
                    .on_hover_text(format!(
                        "Selection: {}, {} to {}, {}",
                        bounds.left(),
                        bounds.top(),
                        bounds.right() - 1,
                        bounds.bottom() - 1
                    ));
            } else {
                ui.label(small(format!("{}, {}", caret.x, caret.y)).weak())
                    .on_hover_text("Caret position (column, row)");
            }
            if self.picker {
                ui.spinner();
            }
            if self.collab.in_session() {
                ui.label(small("·".into()).weak());
                self.collaboration_status(ui);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.add_space(6.0);
                let zoom = widgets::status_button(
                    ui,
                    &format!("{:.0}%", self.view.zoom * 100.0),
                    &format!("Zoom: {}", super::menus::zoom_label(self.settings.monitor_settings.scaling_mode)),
                );
                egui::Popup::menu(&zoom).show(|ui| {
                    if ui.button("Zoom In").clicked() {
                        self.zoom_step(1);
                    }
                    if ui.button("Zoom Out").clicked() {
                        self.zoom_step(-1);
                    }
                    ui.separator();
                    for (label, mode) in [
                        ("Fit to Window", icy_engine_gui::ScalingMode::Auto),
                        ("Fit Width", icy_engine_gui::ScalingMode::FitWidth),
                        ("50%", icy_engine_gui::ScalingMode::Manual(0.5)),
                        ("100%", icy_engine_gui::ScalingMode::Manual(1.0)),
                        ("200%", icy_engine_gui::ScalingMode::Manual(2.0)),
                        ("400%", icy_engine_gui::ScalingMode::Manual(4.0)),
                    ] {
                        if ui.selectable_label(self.settings.monitor_settings.scaling_mode == mode, label).clicked() {
                            self.settings.monitor_settings.scaling_mode = mode;
                            ui.close();
                        }
                    }
                });
                status_separator(ui);
                let font_label = if compact {
                    "Font".to_owned()
                } else if font.chars().count() > 26 {
                    format!("{}…", font.chars().take(25).collect::<String>())
                } else {
                    font.clone()
                };
                if widgets::status_button(ui, &font_label, &format!("Font: {font}\nClick to choose a different font")).clicked() {
                    self.dialog = Some(Dialog::FontSelect);
                }
                status_separator(ui);
                let aspect_label = match (aspect, compact) {
                    (true, true) => "4:3",
                    (true, false) => "DOS Aspect",
                    (false, true) => "1:1",
                    (false, false) => "Square Pixels",
                };
                let aspect_tip = if aspect {
                    "Pixels are stretched like on a 4:3 DOS monitor.\nClick to use square pixels."
                } else {
                    "Pixels are square.\nClick to stretch them like on a 4:3 DOS monitor."
                };
                if widgets::status_button(ui, aspect_label, aspect_tip).clicked() {
                    self.edit(|state| state.set_use_aspect_ratio(!aspect));
                }
                let spacing_label = match (spacing, compact) {
                    (true, true) => "9px",
                    (true, false) => "9 px Font",
                    (false, true) => "8px",
                    (false, false) => "8 px Font",
                };
                let spacing_tip = if spacing {
                    "Characters are 9 pixels wide (VGA letter spacing).\nClick to use 8 pixel wide characters."
                } else {
                    "Characters are 8 pixels wide.\nClick to add the 9th pixel column of VGA text mode."
                };
                if widgets::status_button(ui, spacing_label, spacing_tip).clicked() {
                    self.edit(|state| state.set_use_letter_spacing(!spacing));
                }
                let ice_label = match (ice, compact) {
                    (true, _) => "iCE Colors",
                    (false, true) => "Blink",
                    (false, false) => "Blinking",
                };
                let ice_tip = if ice {
                    "The blink bit selects 8 additional bright background colors (iCE colors).\nClick to make it blink instead."
                } else {
                    "The blink bit makes characters blink.\nClick to use it for 8 bright background colors (iCE colors)."
                };
                if widgets::status_button(ui, ice_label, ice_tip).clicked() {
                    let mode = if ice { icy_engine::IceMode::Blink } else { icy_engine::IceMode::Ice };
                    self.edit(|state| state.set_ice_mode(mode));
                }
            });
        });
    }
}

/// Display name of a tool in this frontend.
pub(super) fn tool_label(tool: Tool) -> &'static str {
    match tool {
        Tool::Click => "Text",
        Tool::Pipette => "Color Picker",
        Tool::Font => "Text Art",
        tool => tool.name(),
    }
}

/// One line description shown in the tool rail tooltips.
fn tool_hint(tool: Tool) -> &'static str {
    match tool {
        Tool::Click => "Type text, move the caret and use F-key characters",
        Tool::Select => "Select areas, characters or colors",
        Tool::Pencil => "Paint with the brush",
        Tool::Pipette => "Pick colors from the canvas",
        Tool::Font => "Type with TheDraw and FIGlet fonts",
        Tool::Tag => "Place and edit annotation tags",
        tool => tool.tooltip(),
    }
}

/// End indices (exclusive) of the tool groups in `TOOL_SLOTS`: navigation, drawing, utilities.
const TOOL_GROUPS: [usize; 3] = [2, 7, 10];

/// Padded sidebar section followed by a full-width separator line.
fn section(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .inner_margin(egui::Margin {
            left: SECTION_MARGIN,
            right: SECTION_MARGIN,
            top: 2,
            bottom: 12,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    let y = ui.cursor().top();
    ui.painter().hline(ui.max_rect().x_range(), y, ui.visuals().widgets.noninteractive.bg_stroke);
}

fn rail_divider(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 9.0), egui::Sense::hover());
    ui.painter().hline(
        egui::Rangef::new(rect.center().x - 12.0, rect.center().x + 12.0),
        rect.center().y,
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
}

fn status_separator(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(9.0, 14.0), egui::Sense::hover());
    ui.painter()
        .vline(rect.center().x, rect.y_range(), ui.visuals().widgets.noninteractive.bg_stroke);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimap_fills_width_and_follows_tall_documents() {
        let area = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(300.0, 300.0));
        let (content, viewport) = (egui::vec2(640.0, 6400.0), egui::vec2(640.0, 320.0));
        let max_offset = content - viewport;
        let layout = |offset_y: f32| MinimapLayout::new(area, [320, 3200], content, viewport, egui::vec2(0.0, offset_y), max_offset).unwrap();

        let top = layout(0.0);
        assert_eq!(top.image.width(), area.width());
        assert_eq!(top.image.height(), 3000.0);
        assert_eq!(top.image.top(), area.top());
        assert_eq!(top.frame.top(), area.top());

        let bottom = layout(max_offset.y);
        assert!((bottom.image.bottom() - area.bottom()).abs() < 0.01);
        assert!((bottom.frame.bottom() - area.bottom()).abs() < 0.01);

        for offset_y in [100.0, 2500.0, 5000.0] {
            let middle = layout(offset_y);
            assert!(area.expand(0.01).contains_rect(middle.frame), "{offset_y}: {:?}", middle.frame);
            let offset = egui::vec2(0.0, offset_y);
            assert!((middle.frame_corner(offset) - middle.frame.min).length() < 0.01);
            // Dragging the frame is a stable scrollbar mapping: geometry from any offset agrees.
            let corner = egui::pos2(area.left(), area.top() + 123.0);
            assert!((middle.offset_for_frame(corner) - top.offset_for_frame(corner)).length() < 0.01);
            assert!((middle.offset_for_frame(middle.frame_corner(offset)) - offset).length() < 0.01);
        }
    }

    #[test]
    fn minimap_centers_documents_that_fit() {
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 300.0));
        let layout = MinimapLayout::new(
            area,
            [640, 400],
            egui::vec2(640.0, 400.0),
            egui::vec2(320.0, 200.0),
            egui::Vec2::ZERO,
            egui::vec2(320.0, 200.0),
        )
        .unwrap();
        assert_eq!(layout.image.size(), egui::vec2(300.0, 187.5));
        assert_eq!(layout.image.center(), area.center());
        let point = layout.image.center();
        assert!((layout.offset_at(point, egui::vec2(320.0, 200.0)) - egui::vec2(160.0, 100.0)).length() < 0.01);
        assert!(MinimapLayout::new(area, [0, 400], egui::vec2(640.0, 400.0), egui::Vec2::ZERO, egui::Vec2::ZERO, egui::Vec2::ZERO).is_none());
    }

    #[test]
    fn minimap_texture_keeps_width_for_tall_documents() {
        let short = render_minimap(&TextBuffer::create(icy_engine::Size::new(80, 25)), 8192).unwrap();
        let tall = render_minimap(&TextBuffer::create(icy_engine::Size::new(80, 500)), 8192).unwrap();
        assert_eq!(short.size[0], tall.size[0]);
        assert!(tall.size[0] >= MINIMAP_PIXELS / 2);
        let capped = render_minimap(&TextBuffer::create(icy_engine::Size::new(80, 500)), 1024).unwrap();
        assert!(capped.size[1] <= 1024);
    }
    #[test]
    fn shape_slots_toggle_like_legacy_registry() {
        assert_eq!(TOOL_SLOTS.len(), 10);
        for (slot, outline, filled) in [
            (4, Tool::RectangleOutline, Tool::RectangleFilled),
            (5, Tool::EllipseOutline, Tool::EllipseFilled),
        ] {
            assert!(TOOL_SLOTS[slot].contains(outline));
            assert!(TOOL_SLOTS[slot].contains(filled));
            assert_eq!(TOOL_SLOTS[slot].toggle(outline), filled);
            assert_eq!(TOOL_SLOTS[slot].toggle(filled), outline);
        }
    }

    #[test]
    fn palette_grid_is_row_major_and_accepts_both_mouse_buttons() {
        let context = egui::Context::default();
        let mut app = DrawApp::new();
        let mut origin = egui::Pos2::ZERO;
        let mut draw = |app: &mut DrawApp, events: Vec<egui::Event>| {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(200.0, 300.0))),
                    events,
                    ..Default::default()
                },
                |context| {
                    egui::CentralPanel::default().show(context, |ui| {
                        origin = ui.cursor().min;
                        app.palette_grid(ui, 208.0);
                    });
                },
            );
            origin
        };
        let origin = draw(&mut app, vec![]);
        for (column, row, index, button) in [
            (1, 0, 1, egui::PointerButton::Primary),
            (0, 1, 8, egui::PointerButton::Primary),
            (7, 1, 15, egui::PointerButton::Secondary),
        ] {
            let point = origin + egui::vec2(column as f32 * 26.0 + 13.0, row as f32 * 26.0 + 13.0);
            for pressed in [true, false] {
                draw(
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            let attribute = app.document.with_state(|state| state.get_caret().attribute);
            assert_eq!(
                if button == egui::PointerButton::Primary {
                    attribute.foreground()
                } else {
                    attribute.background()
                },
                index
            );
        }
    }

    #[test]
    fn layer_previews_keep_their_indices_and_reset_on_replace() {
        let context = egui::Context::default();
        let mut app = DrawApp::new();
        app.replace(icy_draw::document::Document::new(icy_engine::Size::new(2, 2)));
        app.document.with_state(|state| {
            state.add_new_layer(0).unwrap();
            let buffer = state.get_buffer_mut();
            for (index, background) in [4, 1].into_iter().enumerate() {
                for row in 0..2 {
                    for column in 0..2 {
                        buffer.layers[index].set_char(
                            (column, row),
                            icy_engine::AttributedChar::new(' ', icy_engine::TextAttribute::from_color(7, background)),
                        );
                    }
                }
            }
            buffer.mark_dirty();
        });
        let signature = app.document.with_state(|state| signature(state.get_buffer()));
        app.chrome.previews.resize_with(2, || None);
        let output = context.run(Default::default(), |context| {
            app.ensure_layer_preview(context, 1, signature);
            app.ensure_layer_preview(context, 0, signature);
        });
        let ids: Vec<_> = app
            .chrome
            .previews
            .iter()
            .map(|entry| entry.as_ref().unwrap().1.as_ref().unwrap().id())
            .collect();
        assert_ne!(ids[0], ids[1]);
        for (index, palette_index) in [4, 1].into_iter().enumerate() {
            let (_, delta) = output.textures_delta.set.iter().find(|(id, _)| *id == ids[index]).unwrap();
            let egui::ImageData::Color(image) = &delta.image;
            let (red, green, blue) = app.document.with_state(|state| state.get_buffer().palette.rgb(palette_index));
            assert!(image.pixels.contains(&Color32::from_rgb(red, green, blue)));
        }
        app.ensure_layer_preview(&context, 1, signature);
        assert_eq!(app.chrome.previews[1].as_ref().unwrap().1.as_ref().unwrap().id(), ids[1]);
        app.chrome.layer_properties = Some((0, LayerProperties::default()));
        app.replace(icy_draw::document::Document::new(icy_engine::Size::new(2, 2)));
        assert!(app.chrome.previews.is_empty());
        assert!(app.chrome.minimap.is_none());
        assert!(!app.layer_properties_open());
    }

    #[test]
    fn layer_actions_keep_selection_and_undo_structure() {
        let mut app = DrawApp::new();
        app.layer_action(LayerAction::Add(0));
        assert_eq!(app.document.with_state(|state| state.get_current_layer().unwrap()), 1);
        app.layer_action(LayerAction::Duplicate(1));
        app.layer_action(LayerAction::Lower(2));
        assert_eq!(app.document.with_state(|state| state.get_current_layer().unwrap()), 1);
        app.layer_action(LayerAction::Visibility(1));
        assert!(!app.document.with_state(|state| state.get_buffer().layers[1].is_visible()));
        app.layer_action(LayerAction::Lock(1));
        assert!(app.document.with_state(|state| state.get_buffer().layers[1].properties.is_locked));
        app.layer_action(LayerAction::Lock(1));
        app.layer_action(LayerAction::Remove(1));
        assert_eq!(app.document.with_state(|state| state.get_buffer().layers.len()), 2);
        app.document.undo().unwrap();
        assert_eq!(app.document.with_state(|state| state.get_buffer().layers.len()), 3);
    }
}
