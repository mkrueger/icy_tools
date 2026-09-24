//! Editor chrome that mirrors the original icy_ui layout: colour switcher and
//! tool column on the left, minimap and layers on the right, moebius status bar.

use super::{Dialog, DrawApp};
use eframe::egui::{self, Color32};
use icy_engine::{LayerProperties, Position, Rectangle, RenderOptions, Role, TextBuffer, TextPane};
use icy_engine_edit::tools::{Tool, ToolPair};
use icy_engine_gui::egui::appearance::{self, labels, Dialog as SharedDialog, DialogButton, DialogSize};

/// Width of the original left bar (`LEFT_BAR_WIDTH`).
pub const SIDEBAR_WIDTH: f32 = 52.0;
/// Width of the original right panel (`RIGHT_PANEL_BASE_WIDTH`).
pub const PANEL_WIDTH: f32 = 320.0;
/// Original `TOP_CONTROL_TOTAL_HEIGHT`.
pub const TOOLBAR_HEIGHT: f32 = 52.0;
/// Original `TOOL_ICON_SIZE`.
const TOOL_ICON: f32 = 42.0;
const LAYER_PREVIEW: [usize; 2] = [128, 80];
const LAYER_ROW_HEIGHT: f32 = 86.0;
const MINIMAP_PIXELS: usize = 512;

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

/// Box-filtered downscale so small previews stay readable.
fn downsample(width: usize, height: usize, rgba: &[u8], target: usize) -> Option<egui::ColorImage> {
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        return None;
    }
    let step = width.max(height).div_ceil(target.max(1)).max(1);
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
    /// Original colour switcher: overlapping fg/bg swatches, swap and default corners.
    fn color_switcher(&mut self, ui: &mut egui::Ui) {
        let (foreground, background) = self.document.with_state(|state| {
            let palette = &state.get_buffer().palette;
            let attribute = state.get_caret().attribute;
            (
                attribute.foreground_color().as_rgb().unwrap_or_else(|| palette.rgb(attribute.foreground())),
                attribute.background_color().as_rgb().unwrap_or_else(|| palette.rgb(attribute.background())),
            )
        });
        let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(SIDEBAR_WIDTH), egui::Sense::click());
        let painter = ui.painter();
        let swatch = 26.0;
        let foreground_rect = egui::Rect::from_min_size(rect.min + egui::vec2(2.0, 2.0), egui::Vec2::splat(swatch));
        let background_rect = egui::Rect::from_min_size(rect.max - egui::vec2(swatch + 2.0, swatch + 2.0), egui::Vec2::splat(swatch));
        let plate = |target: egui::Rect, (red, green, blue): (u8, u8, u8)| {
            painter.rect_filled(target, 0, Color32::BLACK);
            painter.rect_filled(target.shrink(1.0), 0, Color32::WHITE);
            painter.rect_filled(target.shrink(2.0), 0, Color32::from_rgb(red, green, blue));
        };
        plate(background_rect, background);
        plate(foreground_rect, foreground);
        let swap = egui::Rect::from_min_size(egui::pos2(rect.right() - 23.0, rect.top()), egui::Vec2::splat(23.0));
        let default = egui::Rect::from_min_size(egui::pos2(rect.left() + 1.0, rect.bottom() - 14.0), egui::Vec2::splat(13.0));
        let hovered = response.hover_pos();
        painter.rect_filled(egui::Rect::from_min_size(default.min, egui::Vec2::splat(9.0)), 0, Color32::from_gray(170));
        painter.rect_stroke(
            egui::Rect::from_min_size(default.min + egui::vec2(4.0, 4.0), egui::Vec2::splat(9.0)),
            0,
            egui::Stroke::new(1.0, Color32::from_gray(170)),
            egui::StrokeKind::Inside,
        );
        painter.rect_filled(
            egui::Rect::from_min_size(default.min + egui::vec2(4.0, 4.0), egui::Vec2::splat(9.0)),
            0,
            Color32::BLACK,
        );
        self.icons.image(ui, "swap", 23.0).paint_at(ui, swap);
        if response.clicked() {
            match hovered {
                Some(point) if swap.contains(point) => {
                    self.document.with_state(|state| state.swap_caret_colors());
                }
                Some(point) if default.expand(3.0).contains(point) => self.document.with_state(|state| {
                    state.set_caret_foreground(7);
                    state.set_caret_background(0);
                }),
                Some(point) if foreground_rect.contains(point) || background_rect.contains(point) => {
                    let foreground = foreground_rect.contains(point);
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
                _ => {}
            }
        }
        let hint = match hovered {
            Some(point) if swap.contains(point) => "Swap Foreground and Background",
            Some(point) if default.expand(3.0).contains(point) => "Default Colors",
            Some(point) if foreground_rect.contains(point) => "Edit Foreground Palette Color",
            _ => "Edit Background Palette Color",
        };
        response.on_hover_text(hint);
    }

    /// Original left bar: palette grid above the tool column.
    pub(super) fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
        egui::ScrollArea::vertical().id_salt("sidebar").show(ui, |ui| {
            self.palette_grid(ui, SIDEBAR_WIDTH);
            ui.add_space(4.0);
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
            ui.vertical_centered(|ui| {
                for pair in TOOL_SLOTS {
                    if self.document.outline_font && !matches!(pair.primary, Tool::Click | Tool::Select) {
                        continue;
                    }
                    if self.charfont.is_some() && pair.primary == Tool::Tag {
                        continue;
                    }
                    let selected = pair.contains(self.document.tool);
                    let tool = if selected { self.document.tool } else { pair.primary };
                    let response = self.icons.button_sized(ui, tool.icon(), tool.name(), selected, TOOL_ICON);
                    if response.clicked() {
                        self.select_tool(if selected { pair.toggle(tool) } else { pair.primary });
                    }
                    if pair.primary != pair.secondary {
                        let corner = response.rect.right_bottom() - egui::vec2(4.0, 4.0);
                        ui.painter().add(egui::Shape::convex_polygon(
                            vec![corner, corner - egui::vec2(5.0, 0.0), corner - egui::vec2(0.0, 5.0)],
                            ui.visuals().text_color(),
                            egui::Stroke::NONE,
                        ));
                        response.context_menu(|ui| {
                            for variant in [pair.primary, pair.secondary] {
                                if ui.selectable_label(self.document.tool == variant, variant.name()).clicked() {
                                    self.select_tool(variant);
                                    ui.close();
                                }
                            }
                        });
                    }
                }
            });
        });
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
        self.document.with_state(|state| {
            let dimensions = state.get_buffer().font_dimensions();
            let (font_width, font_height) = (dimensions.width as f32, dimensions.height as f32);
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

    /// 16 colours as 8×2 squares like the original palette grid.
    fn palette_grid(&mut self, ui: &mut egui::Ui, width: f32) {
        let (palette, foreground, background) = self.document.with_state(|state| {
            let attribute = state.get_caret().attribute;
            (state.get_buffer().palette.clone(), attribute.foreground(), attribute.background())
        });
        let count = palette.len().min(256);
        if count == 0 {
            return;
        }
        let columns = if count == 8 {
            1
        } else if count <= 16 {
            2
        } else {
            (width / 12.0).floor().max(2.0) as usize
        };
        let cell = width / columns.max(2) as f32;
        let inset = (width - cell * columns as f32) / 2.0;
        let rows = count.div_ceil(columns);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, cell * rows as f32), egui::Sense::click());
        let painter = ui.painter();
        for index in 0..count {
            let (column, row) = if count == 16 {
                (index / 8, index % 8)
            } else {
                (index % columns, index / columns)
            };
            let origin = rect.min + egui::vec2(inset + column as f32 * cell, row as f32 * cell);
            let target = egui::Rect::from_min_size(origin, egui::Vec2::splat(cell));
            let (red, green, blue) = palette.rgb(index as u32);
            painter.rect_filled(target, 0, Color32::from_rgb(red, green, blue));
            if index as u32 == foreground {
                painter.rect_stroke(target.shrink(1.0), 0, egui::Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Inside);
                painter.rect_stroke(target, 0, egui::Stroke::new(1.0, Color32::BLACK), egui::StrokeKind::Inside);
            }
            if index as u32 == background {
                painter.circle_filled(target.center(), 3.0, Color32::WHITE);
                painter.circle_stroke(target.center(), 3.0, egui::Stroke::new(1.0, Color32::BLACK));
            }
        }
        let hovered = response.hover_pos().and_then(|point| {
            let local = point - rect.min - egui::vec2(inset, 0.0);
            if local.x < 0.0 || local.x >= columns as f32 * cell {
                return None;
            }
            let column = (local.x / cell) as usize;
            let row = (local.y / cell) as usize;
            let index = if count == 16 { column * 8 + row } else { row * columns + column };
            (index < count).then_some(index)
        });
        if let Some(index) = hovered {
            if response.clicked() {
                self.document.with_state(|state| state.set_caret_foreground(index as u32));
            }
            if response.secondary_clicked() {
                self.document.with_state(|state| state.set_caret_background(index as u32));
            }
            let (red, green, blue) = palette.rgb(index as u32);
            response.on_hover_text(format!("{index}: #{red:02X}{green:02X}{blue:02X}"));
        }
    }

    /// Original top toolbar: colour switcher plus the active tool's options.
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            self.color_switcher(ui);
            ui.separator();
            egui::ScrollArea::horizontal()
                .id_salt("tool-options")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_height(TOOLBAR_HEIGHT);
                    self.tool_options(ui, context);
                });
        });
    }

    /// Right panel: minimap over the layer list, split like the original pane grid.
    pub(super) fn panel(&mut self, ui: &mut egui::Ui) {
        let signature = self.document.with_state(|state| signature(state.get_buffer()));
        if self.document.outline_font {
            ui.horizontal(|ui| {
                ui.label("Outline Preview");
                egui::ComboBox::from_id_salt("outline-preview-style")
                    .selected_text(format!("Style {}", self.chrome.outline_style + 1))
                    .show_ui(ui, |ui| {
                        for style in 0..19 {
                            ui.selectable_value(&mut self.chrome.outline_style, style, format!("Style {}", style + 1));
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
                ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(
                    ui.available_width(),
                    ui.available_width() * texture.size()[1] as f32 / texture.size()[0] as f32,
                )));
            }
            return;
        }
        egui::TopBottomPanel::top("minimap")
            .resizable(true)
            .default_height(200.0)
            .height_range(80.0..=420.0)
            .show_inside(ui, |ui| self.minimap(ui, signature));
        self.layers(ui, signature);
    }

    fn minimap(&mut self, ui: &mut egui::Ui, signature: u64) {
        ui.label("Minimap");
        if self.chrome.minimap.as_ref().is_none_or(|(key, _)| *key != signature) {
            if let Some(image) = self.document.with_state(|state| render(state.get_buffer(), MINIMAP_PIXELS)) {
                let texture = ui.ctx().load_texture("minimap", image, egui::TextureOptions::LINEAR);
                self.chrome.minimap = Some((signature, texture));
            }
        }
        let Some((_, texture)) = self.chrome.minimap.clone() else {
            return;
        };
        let available = ui.available_size();
        let [width, height] = texture.size();
        let scale = (available.x / width as f32).min(available.y / height as f32).max(0.01);
        let size = egui::vec2(width as f32 * scale, height as f32 * scale);
        let (outer, response) = ui.allocate_exact_size(egui::vec2(available.x, size.y), egui::Sense::click_and_drag());
        let rect = egui::Rect::from_center_size(outer.center(), size);
        ui.painter().image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        let content = self.content_size();
        if content.x > 0.0 && content.y > 0.0 {
            let viewport = self.canvas_rect.size();
            let visible = egui::Rect::from_min_size(
                rect.min + egui::vec2(self.view.offset.x / content.x * size.x, self.view.offset.y / content.y * size.y),
                egui::vec2((viewport.x / content.x).min(1.0) * size.x, (viewport.y / content.y).min(1.0) * size.y),
            );
            ui.painter().rect_stroke(
                visible.intersect(rect),
                0,
                egui::Stroke::new(1.0, ui.visuals().selection.bg_fill),
                egui::StrokeKind::Inside,
            );
            if let Some(point) = (response.clicked() || response.dragged()).then(|| response.interact_pointer_pos()).flatten() {
                let normalized = (point - rect.min) / size;
                let offset = egui::vec2(normalized.x * content.x, normalized.y * content.y) - viewport / 2.0;
                self.view.scroll_to = Some(offset.max(egui::Vec2::ZERO).min(self.view.max_offset));
            }
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
        ui.horizontal(|ui| {
            ui.strong("Layers");
            ui.weak(rows.len().to_string());
        });
        egui::TopBottomPanel::bottom("layer-actions").exact_height(38.0).show_inside(ui, |ui| {
            ui.horizontal_centered(|ui| {
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
                if self.icons.button(ui, "measure", "Layer Properties", false).clicked() {
                    action = Some(LayerAction::Properties(current));
                }
                ui.add_enabled_ui(current_unlocked && rows.len() > 1, |ui| {
                    if self.icons.button(ui, "delete", "Delete Layer", false).clicked() {
                        action = Some(LayerAction::Remove(current));
                    }
                });
            });
        });
        self.chrome.previews.resize_with(rows.len(), || None);
        egui::ScrollArea::vertical()
            .id_salt("layers")
            .auto_shrink([false, false])
            .show_rows(ui, LAYER_ROW_HEIGHT, rows.len(), |ui, range| {
                for row in range {
                    let index = rows.len() - row - 1;
                    let (properties, dimensions, _) = &rows[index];
                    self.ensure_layer_preview(ui.ctx(), index, signature);
                    let selected = index == current;
                    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), LAYER_ROW_HEIGHT), egui::Sense::click());
                    let fill = if selected {
                        ui.visuals().selection.bg_fill
                    } else if response.hovered() {
                        ui.visuals().widgets.hovered.bg_fill
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(rect, 0, fill);
                    if selected {
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(rect.min, egui::vec2(3.0, rect.height())),
                            0,
                            ui.visuals().selection.stroke.color,
                        );
                    }
                    ui.push_id(index, |ui| {
                        ui.scope_builder(
                            egui::UiBuilder::new()
                                .max_rect(rect.shrink2(egui::vec2(5.0, 3.0)))
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                            |ui| {
                                ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                                if self
                                    .icons
                                    .button_sized(
                                        ui,
                                        if properties.is_visible { "visibility" } else { "visibility_off" },
                                        "Toggle Visibility",
                                        false,
                                        24.0,
                                    )
                                    .clicked()
                                {
                                    action = Some(LayerAction::Visibility(index));
                                }
                                let preview = egui::vec2(LAYER_PREVIEW[0] as f32, LAYER_PREVIEW[1] as f32);
                                let (preview_rect, _) = ui.allocate_exact_size(preview, egui::Sense::hover());
                                for row in 0..10 {
                                    for column in 0..16 {
                                        let cell = egui::Rect::from_min_size(
                                            preview_rect.min + egui::vec2(column as f32 * 8.0, row as f32 * 8.0),
                                            egui::Vec2::splat(8.0),
                                        );
                                        ui.painter()
                                            .rect_filled(cell, 0, Color32::from_gray(if (row + column) % 2 == 0 { 45 } else { 58 }));
                                    }
                                }
                                if let Some((_, Some(texture))) = &self.chrome.previews[index] {
                                    let [width, height] = texture.size();
                                    let scale = (preview.x / width as f32).min(preview.y / height as f32);
                                    ui.painter().image(
                                        texture.id(),
                                        egui::Rect::from_center_size(preview_rect.center(), egui::vec2(width as f32, height as f32) * scale),
                                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                        Color32::WHITE,
                                    );
                                }
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    ui.spacing_mut().interact_size.y = 18.0;
                                    ui.add(egui::Label::new(&properties.title).truncate()).on_hover_text(&properties.title);
                                    ui.small(format!("{} x {}", dimensions.width, dimensions.height));
                                    ui.weak(format!("{}, {}", properties.offset.x, properties.offset.y));
                                    let mut locked = properties.is_locked;
                                    if ui.checkbox(&mut locked, "Lock").changed() {
                                        action = Some(LayerAction::Lock(index));
                                    }
                                });
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

    /// Moebius-style status bar from the original main window.
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
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if ui.selectable_label(false, if ice { "ICE" } else { "BLINK" }).clicked() {
                let mode = if ice { icy_engine::IceMode::Blink } else { icy_engine::IceMode::Ice };
                self.edit(|state| state.set_ice_mode(mode));
            }
            ui.separator();
            if ui.selectable_label(false, if spacing { "9 px" } else { "8 px" }).clicked() {
                self.edit(|state| state.set_use_letter_spacing(!spacing));
            }
            ui.separator();
            if ui
                .selectable_label(false, if aspect { "ASPECT" } else { "SQUARE" })
                .on_hover_text("Pixel Aspect Ratio")
                .clicked()
            {
                self.edit(|state| state.set_use_aspect_ratio(!aspect));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button(format!("{:.0}%", self.view.zoom * 100.0), |ui| {
                    for (label, mode) in [
                        ("Fit", icy_engine_gui::ScalingMode::Auto),
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
                let font_clicked = if compact {
                    self.icons.button_sized(ui, "font", &format!("Select Font: {font}"), false, 20.0).clicked()
                } else {
                    ui.add_sized([150.0, 20.0], egui::Button::new(egui::RichText::new(&font).small()).truncate().frame(false))
                        .on_hover_text(format!("Select Font: {font}"))
                        .clicked()
                };
                if font_clicked {
                    self.dialog = Some(Dialog::FontSelect);
                }
                ui.separator();
                if let Some(bounds) = selection {
                    ui.label(format!("{} x {}", bounds.width(), bounds.height())).on_hover_text(format!(
                        "Selection: {}, {} to {}, {}",
                        bounds.left(),
                        bounds.top(),
                        bounds.right() - 1,
                        bounds.bottom() - 1
                    ));
                } else {
                    ui.label(format!("({},{})", caret.x, caret.y));
                }
                if self.picker {
                    ui.spinner();
                }
                let rect = ui.available_rect_before_wrap();
                if rect.width() > 180.0 {
                    ui.painter().with_clip_rect(rect).text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("{}    {} x {}", self.document.tool.name(), size.width, size.height),
                        egui::TextStyle::Body.resolve(ui.style()),
                        ui.visuals().weak_text_color(),
                    );
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn palette_columns_match_legacy_and_accept_both_mouse_buttons() {
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
                        app.palette_grid(ui, SIDEBAR_WIDTH);
                    });
                },
            );
            origin
        };
        let origin = draw(&mut app, vec![]);
        for (column, row, index, button) in [
            (0, 1, 1, egui::PointerButton::Primary),
            (1, 0, 8, egui::PointerButton::Primary),
            (1, 7, 15, egui::PointerButton::Secondary),
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
