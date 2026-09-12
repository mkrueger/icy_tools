//! Editor chrome that mirrors the original icy_ui layout: colour switcher and
//! tool column on the left, minimap and layers on the right, moebius status bar.

use super::{Dialog, DrawApp};
use eframe::egui::{self, Color32};
use icy_draw::brush::BrushPrimaryMode;
use icy_engine::{Position, Rectangle, RenderOptions, TextBuffer, TextPane};
use icy_engine_edit::tools::Tool;

/// Width of the original left bar (`LEFT_BAR_WIDTH`).
pub const SIDEBAR_WIDTH: f32 = 52.0;
/// Width of the original right panel (`RIGHT_PANEL_BASE_WIDTH`).
pub const PANEL_WIDTH: f32 = 320.0;
/// Original `TOP_CONTROL_TOTAL_HEIGHT`.
pub const TOOLBAR_HEIGHT: f32 = 52.0;
/// Original `TOOL_ICON_SIZE`.
const TOOL_ICON: f32 = 42.0;
const LAYER_PREVIEW: [usize; 2] = [96, 60];
const MINIMAP_PIXELS: usize = 512;

pub const TOOLS: [Tool; 12] = [
    Tool::Click,
    Tool::Select,
    Tool::Pencil,
    Tool::Line,
    Tool::RectangleOutline,
    Tool::RectangleFilled,
    Tool::EllipseOutline,
    Tool::EllipseFilled,
    Tool::Fill,
    Tool::Pipette,
    Tool::Font,
    Tool::Tag,
];

#[derive(Default)]
pub struct Chrome {
    minimap: Option<(u64, egui::TextureHandle)>,
    previews: Vec<(u64, Option<egui::TextureHandle>)>,
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
            let (mut red, mut green, mut blue, mut count) = (0u32, 0u32, 0u32, 0u32);
            for y in row * step..((row + 1) * step).min(height) {
                for x in column * step..((column + 1) * step).min(width) {
                    let offset = (y * width + x) * 4;
                    red += u32::from(rgba[offset]);
                    green += u32::from(rgba[offset + 1]);
                    blue += u32::from(rgba[offset + 2]);
                    count += 1;
                }
            }
            if count > 0 {
                image[(column, row)] = Color32::from_rgb((red / count) as u8, (green / count) as u8, (blue / count) as u8);
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
    let mut copy = layer.clone();
    copy.set_offset(Position::default());
    copy.set_is_visible(true);
    preview.layers.clear();
    preview.layers.push(copy);
    render(&preview, LAYER_PREVIEW[0].max(LAYER_PREVIEW[1]) * 2)
}

impl DrawApp {
    /// Original colour switcher: overlapping fg/bg swatches, swap and default corners.
    fn color_switcher(&mut self, ui: &mut egui::Ui) {
        let (foreground, background) = self.document.with_state(|state| {
            let palette = &state.get_buffer().palette;
            let attribute = state.get_caret().attribute;
            (palette.rgb(attribute.foreground()), palette.rgb(attribute.background()))
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
        let swap = egui::Rect::from_min_size(egui::pos2(rect.right() - 15.0, rect.top() + 1.0), egui::Vec2::splat(13.0));
        let default = egui::Rect::from_min_size(egui::pos2(rect.left() + 1.0, rect.bottom() - 14.0), egui::Vec2::splat(13.0));
        let accent = ui.visuals().text_color();
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
        let arrow = egui::Stroke::new(1.5, accent);
        painter.line_segment([egui::pos2(swap.left(), swap.bottom()), egui::pos2(swap.right(), swap.top())], arrow);
        painter.line_segment([egui::pos2(swap.right() - 5.0, swap.top()), egui::pos2(swap.right(), swap.top())], arrow);
        painter.line_segment([egui::pos2(swap.right(), swap.top()), egui::pos2(swap.right(), swap.top() + 5.0)], arrow);
        if response.clicked() {
            match hovered {
                Some(point) if swap.expand(3.0).contains(point) => {
                    self.document.with_state(|state| state.swap_caret_colors());
                }
                Some(point) if default.expand(3.0).contains(point) => self.document.with_state(|state| {
                    state.set_caret_foreground(7);
                    state.set_caret_background(0);
                }),
                _ => {}
            }
        }
        response.on_hover_text("Foreground and background — click the arrow to swap, the small squares to reset");
    }

    /// Original left bar: palette grid above the tool column.
    pub(super) fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
        egui::ScrollArea::vertical().id_salt("sidebar").show(ui, |ui| {
            self.palette_grid(ui, SIDEBAR_WIDTH);
            ui.add_space(4.0);
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
            for tool in TOOLS {
                if self
                    .icons
                    .button_sized(ui, tool.icon(), tool.name(), self.document.tool == tool, TOOL_ICON)
                    .clicked()
                {
                    self.select_tool(tool);
                }
            }
        });
    }

    pub(super) fn select_tool(&mut self, tool: Tool) {
        self.document.finish();
        self.document.tool = tool;
        self.canvas_focus = true;
        if tool == Tool::Tag {
            self.dialog = Some(Dialog::Tags);
        }
        if tool == Tool::Font && self.text_fonts.is_none() {
            self.text_fonts = Some(icy_draw::text_art_fonts::TextArtFontLibrary::create_shared());
        }
    }

    /// 16 colours as 8×2 squares like the original palette grid.
    fn palette_grid(&mut self, ui: &mut egui::Ui, width: f32) {
        let (palette, foreground, background) = self.document.with_state(|state| {
            let attribute = state.get_caret().attribute;
            (state.get_buffer().palette.clone(), attribute.foreground(), attribute.background())
        });
        let count = palette.len().min(256);
        let columns = if count <= 16 { 2 } else { (width / 16.0).floor().max(2.0) as usize };
        let cell = width / columns as f32;
        let rows = count.div_ceil(columns);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, cell * rows as f32), egui::Sense::click());
        let painter = ui.painter();
        for index in 0..count {
            let origin = rect.min + egui::vec2((index % columns) as f32 * cell, (index / columns) as f32 * cell);
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
        let hovered = response.hover_pos().map(|point| {
            let cell_position = (point - rect.min) / cell;
            (cell_position.y as usize * columns + cell_position.x as usize).min(count.saturating_sub(1))
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
            self.color_switcher(ui);
            ui.separator();
            ui.vertical(|ui| self.tool_options(ui, context));
        });
    }

    /// Right panel: minimap over the layer list, split like the original pane grid.
    pub(super) fn panel(&mut self, ui: &mut egui::Ui) {
        let signature = self.document.with_state(|state| signature(state.get_buffer()));
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
                .map(|layer| (layer.properties.title.clone(), layer.is_visible(), layer.properties.is_locked))
                .collect::<Vec<_>>()
        });
        ui.horizontal(|ui| {
            ui.label("Layers");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.icons.button(ui, "delete", "Delete Layer", false).clicked() && rows.len() > 1 {
                    self.edit(|state| state.remove_layer(current));
                }
                if self.icons.button(ui, "move_down", "Lower Layer", false).clicked() {
                    self.edit(|state| state.lower_layer(current));
                }
                if self.icons.button(ui, "move_up", "Raise Layer", false).clicked() && current + 1 < rows.len() {
                    self.edit(|state| state.raise_layer(current));
                }
                if self.icons.button(ui, "file_copy", "Duplicate Layer", false).clicked() {
                    self.edit(|state| state.duplicate_layer(current));
                }
                if self.icons.button(ui, "anchor", "Anchor Pasted Layer", false).clicked() {
                    self.edit(|state| state.anchor_layer());
                }
                if self.icons.button(ui, "add_layer", "Add Layer", false).clicked() {
                    self.edit(|state| state.add_new_layer(current));
                }
            });
        });
        if self.chrome.previews.first().is_some_and(|(key, _)| *key != signature) || self.chrome.previews.len() != rows.len() {
            self.chrome.previews.clear();
        }
        egui::ScrollArea::vertical().id_salt("layers").show(ui, |ui| {
            for (index, (title, visible, locked)) in rows.iter().enumerate().rev() {
                if self.chrome.previews.len() <= index {
                    let image = self.document.with_state(|state| layer_preview(state.get_buffer(), index));
                    let texture = image.map(|image| ui.ctx().load_texture(format!("layer-{index}"), image, egui::TextureOptions::LINEAR));
                    self.chrome.previews.push((signature, texture));
                }
                let selected = index == current;
                let response = egui::Frame::new()
                    .fill(if selected { ui.visuals().selection.bg_fill } else { Color32::TRANSPARENT })
                    .inner_margin(2)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if self
                                .icons
                                .button(ui, if *visible { "visibility" } else { "visibility_off" }, "Toggle Visibility", false)
                                .clicked()
                            {
                                self.edit(|state| state.toggle_layer_visibility(index));
                            }
                            let preview = egui::vec2(LAYER_PREVIEW[0] as f32, LAYER_PREVIEW[1] as f32);
                            match self.chrome.previews.get(index).and_then(|(_, texture)| texture.as_ref()) {
                                Some(texture) => {
                                    let [width, height] = texture.size();
                                    let scale = (preview.x / width as f32).min(preview.y / height as f32);
                                    ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(width as f32, height as f32) * scale));
                                }
                                None => {
                                    ui.allocate_exact_size(preview, egui::Sense::hover());
                                }
                            }
                            ui.label(title);
                            if *locked {
                                ui.weak("locked");
                            }
                        });
                    })
                    .response
                    .interact(egui::Sense::click());
                if response.clicked() {
                    self.document.with_state(|state| state.set_current_layer(index));
                }
                response.context_menu(|ui| {
                    if ui.button(if *locked { "Unlock Layer" } else { "Lock Layer" }).clicked() {
                        let mut properties = self.document.with_state(|state| state.get_buffer().layers[index].properties.clone());
                        properties.is_locked = !properties.is_locked;
                        self.edit(|state| state.update_layer_properties(index, properties));
                        ui.close();
                    }
                    if ui.button("New Layer").clicked() {
                        self.edit(|state| state.add_new_layer(index));
                        ui.close();
                    }
                    if ui.button("Duplicate Layer").clicked() {
                        self.edit(|state| state.duplicate_layer(index));
                        ui.close();
                    }
                    if index > 0 && ui.button("Merge Down").clicked() {
                        self.edit(|state| state.merge_layer_down(index));
                        ui.close();
                    }
                    if rows.len() > 1 && ui.button("Delete Layer").clicked() {
                        self.edit(|state| state.remove_layer(index));
                        ui.close();
                    }
                    if ui.button("Clear Layer").clicked() {
                        self.edit(|state| state.clear_layer(index));
                        ui.close();
                    }
                });
            }
        });
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
        ui.horizontal(|ui| {
            if ui.selectable_label(false, if ice { "ICE" } else { "BLINK" }).clicked() {
                let mode = if ice { icy_engine::IceMode::Blink } else { icy_engine::IceMode::Ice };
                self.edit(|state| state.set_ice_mode(mode));
            }
            ui.separator();
            if ui.selectable_label(false, if spacing { "9 px" } else { "8 px" }).clicked() {
                self.edit(|state| state.set_use_letter_spacing(!spacing));
            }
            ui.separator();
            if ui.selectable_label(false, if aspect { "ASPECT RATIO" } else { "SQUARE" }).clicked() {
                self.edit(|state| state.set_use_aspect_ratio(!aspect));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.selectable_label(false, &font).on_hover_text("Select Font").clicked() {
                    self.dialog = Some(Dialog::FontSelect);
                }
                ui.separator();
                if let Some(bounds) = selection {
                    ui.label(format!(
                        "({},{})–({},{}) {}×{}",
                        bounds.left(),
                        bounds.top(),
                        bounds.right() - 1,
                        bounds.bottom() - 1,
                        bounds.width(),
                        bounds.height()
                    ));
                } else {
                    ui.label(format!("({},{})", caret.x, caret.y));
                }
                if self.picker {
                    ui.spinner();
                }
                let rect = ui.available_rect_before_wrap();
                if rect.width() > 260.0 {
                    ui.painter().with_clip_rect(rect).text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("{}    {}×{}", self.tool_hint(), size.width, size.height),
                        egui::TextStyle::Body.resolve(ui.style()),
                        ui.visuals().weak_text_color(),
                    );
                }
            });
        });
    }

    fn tool_hint(&self) -> String {
        let brush = match self.document.brush.primary {
            BrushPrimaryMode::Char => format!("Char mode — paints '{}'", self.document.brush.paint_char),
            BrushPrimaryMode::HalfBlock => "Half-block mode (2× vertical resolution)".into(),
            BrushPrimaryMode::Shading => "Shading mode (LMB lighter, RMB darker)".into(),
            BrushPrimaryMode::Replace => "Replace mode (recolors existing characters)".into(),
            BrushPrimaryMode::Blink => "Blink mode (toggles blink attribute)".into(),
            BrushPrimaryMode::Colorize => "Colorize mode (changes only colors)".into(),
        };
        match self.document.tool {
            Tool::Click => "Click  •  Type characters or drag a rectangular selection".into(),
            Tool::Select => "Select  •  Drag to select, Shift to add, Alt to subtract".into(),
            Tool::Pencil => format!("Pencil  •  {brush}"),
            Tool::Line => format!("Line  •  {brush}"),
            Tool::RectangleOutline => format!("Rectangle  •  {brush}"),
            Tool::RectangleFilled => format!("Filled rectangle  •  {brush}"),
            Tool::EllipseOutline => format!("Ellipse  •  {brush}"),
            Tool::EllipseFilled => format!("Filled ellipse  •  {brush}"),
            Tool::Fill => format!("Fill  •  {brush}"),
            Tool::Pipette => "Color picker  •  Click to sample fg/bg/char".into(),
            Tool::Font => "Font  •  Place a TDF/Figlet caret, then type".into(),
            Tool::Tag => "Tag  •  Click to place an expandable tag".into(),
        }
    }
}
