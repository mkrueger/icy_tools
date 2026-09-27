//! Font selector in the classic Icy Draw split layout: filter and grouped font list
//! on the left, all 256 glyphs of the selected font on the right.

use std::path::Path;

use eframe::egui::{self, Color32, Stroke, StrokeKind};
use icy_draw::fl;
use icy_engine::{get_sauce_font_names, AttributedChar, BitFont, FileFormat, FontMode, RenderOptions, TextAttribute, TextBuffer};
use icy_engine_edit::EditState;
use icy_engine_gui::egui::appearance::{self, labels, Dialog, DialogButton, DialogSize};

const LIST_WIDTH: f32 = 280.0;
/// Tall enough for an 8 × 16 font sheet at twice its size.
const BODY_HEIGHT: f32 = 600.0;
const HEADER_HEIGHT: f32 = 28.0;
const ITEM_HEIGHT: f32 = 24.0;
const ITEM_INDENT: f32 = 24.0;
const PREVIEW_PADDING: f32 = 8.0;
/// Largest preview magnification; smaller dialogs scale the preview down to fit.
const PREVIEW_SCALE: f32 = 2.0;
pub const FONT_EXTENSIONS: &[&str] = &["psf", "psf2", "psfu", "yaff", "xb", "f08", "f14", "f16", "f19"];

pub enum Action {
    Load,
    Apply(Box<BitFont>),
    Cancel,
}

#[derive(Clone)]
enum Button {
    Load,
    Cancel,
    Apply,
    Back,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Category {
    Sauce,
    Ansi,
}

impl Category {
    const ALL: [Category; 2] = [Category::Sauce, Category::Ansi];

    fn label(self) -> String {
        match self {
            Category::Sauce => fl!("font-selector-sauce-fonts"),
            Category::Ansi => fl!("font-selector-ansi-fonts"),
        }
    }
}

struct Entry {
    font: BitFont,
    category: Category,
}

enum Row {
    Header(Category, usize),
    Font(usize),
}

pub struct FontSelector {
    entries: Vec<Entry>,
    /// Only fonts with the height of the font in use are offered, like the classic editor.
    height: i32,
    selected: usize,
    filter: String,
    collapsed: [bool; 2],
    preview: Option<(usize, egui::TextureHandle)>,
    /// Fonts of an XBin file with more than one font, waiting for a choice.
    xbin_fonts: Option<Vec<BitFont>>,
    error: Option<String>,
    reveal: bool,
    focus_filter: bool,
    page_rows: usize,
}

impl Default for FontSelector {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            height: 16,
            selected: 0,
            filter: String::new(),
            collapsed: [false; 2],
            preview: None,
            xbin_fonts: None,
            error: None,
            reveal: true,
            focus_filter: true,
            page_rows: 16,
        }
    }
}

fn font_key(font: &BitFont) -> (String, i32, i32) {
    let size = font.size();
    (font.name().to_string(), size.width, size.height)
}

impl FontSelector {
    /// Lists the SAUCE fonts, the ANSI font pages unless the document is SAUCE-only,
    /// and the document's own fonts, with the font of the caret's page selected.
    pub fn new(state: &EditState) -> Self {
        let buffer = state.get_buffer();
        let sauce_only = matches!(buffer.font_mode, FontMode::Sauce);
        let mut entries: Vec<Entry> = Vec::new();
        let add = |entries: &mut Vec<Entry>, font: BitFont, category: Category| -> usize {
            let key = font_key(&font);
            if let Some(index) = entries.iter().position(|entry| font_key(&entry.font) == key) {
                return index;
            }
            entries.push(Entry { font, category });
            entries.len() - 1
        };
        for name in get_sauce_font_names() {
            if let Ok(font) = BitFont::from_sauce_name(name) {
                add(&mut entries, font, Category::Sauce);
            }
        }
        if !sauce_only {
            for page in 0..icy_engine::ANSI_FONTS {
                if let Some(font) = BitFont::from_ansi_font_page(page as u8, 16) {
                    add(&mut entries, font.clone(), Category::Ansi);
                }
            }
        }
        let page = state.get_caret().font_page();
        let mut selected = 0;
        let mut height = 16;
        for (slot, font) in buffer.font_iter() {
            let index = add(&mut entries, font.clone(), Category::Ansi);
            if *slot == page {
                selected = index;
                height = font.size().height;
            }
        }
        Self {
            entries,
            height,
            selected,
            ..Default::default()
        }
    }

    pub fn selected_font(&self) -> Option<&BitFont> {
        self.entries.get(self.selected).map(|entry| &entry.font)
    }

    #[cfg(test)]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    #[cfg(test)]
    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_string();
        self.keep_selection_visible();
    }

    fn matches(&self, entry: &Entry) -> bool {
        entry.font.size().height == self.height && entry.font.name().to_lowercase().contains(&self.filter.trim().to_lowercase())
    }

    fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        for (index, category) in Category::ALL.into_iter().enumerate() {
            let fonts: Vec<_> = (0..self.entries.len())
                .filter(|&font| self.entries[font].category == category && self.matches(&self.entries[font]))
                .collect();
            if fonts.is_empty() {
                continue;
            }
            rows.push(Row::Header(category, fonts.len()));
            if !self.collapsed[index] {
                rows.extend(fonts.into_iter().map(Row::Font));
            }
        }
        rows
    }

    /// Fonts that can be selected, in list order.
    fn selectable(&self) -> Vec<usize> {
        self.rows()
            .into_iter()
            .filter_map(|row| match row {
                Row::Font(index) => Some(index),
                Row::Header(..) => None,
            })
            .collect()
    }

    fn keep_selection_visible(&mut self) {
        let selectable = self.selectable();
        if !selectable.contains(&self.selected) {
            if let Some(&first) = selectable.first() {
                self.selected = first;
            }
        }
        self.reveal = true;
    }

    fn toggle(&mut self, category: Category) {
        let index = Category::ALL.iter().position(|item| *item == category).unwrap_or(0);
        self.collapsed[index] = !self.collapsed[index];
        self.keep_selection_visible();
    }

    /// Moves the selection by `delta` rows; `isize::MIN`/`isize::MAX` jump to the ends.
    pub fn step(&mut self, delta: isize) {
        let selectable = self.selectable();
        let Some(last) = selectable.len().checked_sub(1) else {
            return;
        };
        let position = selectable.iter().position(|&index| index == self.selected);
        let target = match position {
            Some(position) => position.saturating_add_signed(delta).min(last),
            None if delta < 0 => last,
            None => 0,
        };
        self.selected = selectable[target];
        self.reveal = true;
    }

    /// Loads `path`; a single font is returned for applying right away, several fonts of an
    /// XBin file are offered for a choice first.
    pub fn load(&mut self, path: &Path) -> Option<BitFont> {
        match load_fonts(path) {
            Ok(mut fonts) if fonts.len() == 1 => fonts.pop(),
            Ok(fonts) => {
                self.xbin_fonts = Some(fonts);
                None
            }
            Err(error) => {
                self.error = Some(error);
                None
            }
        }
    }

    /// `blocked` is set while a file picker is open.
    pub fn show(&mut self, context: &egui::Context, blocked: bool) -> Option<Action> {
        if self.xbin_fonts.is_some() {
            return self.show_xbin_choice(context, blocked);
        }
        let mut action = None;
        if !blocked {
            action = self.keyboard(context);
        }
        let body_height = (context.content_rect().height() - 180.0).clamp(160.0, BODY_HEIGHT);
        let response = Dialog::new("font-select").size(DialogSize::Width(800.0)).scroll(false).show(context, |dialog| {
            dialog.content(|ui| {
                ui.horizontal_top(|ui| {
                    let list_width = if ui.available_width() < 2.0 * LIST_WIDTH {
                        (ui.available_width() * 0.5).max(120.0)
                    } else {
                        LIST_WIDTH
                    };
                    ui.allocate_ui_with_layout(egui::vec2(list_width, body_height), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_min_size(egui::vec2(list_width, body_height));
                        if self.list(ui) {
                            action = self.selected_font().cloned().map(|font| Action::Apply(Box::new(font)));
                        }
                    });
                    ui.add_space(12.0);
                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), body_height), egui::Layout::top_down(egui::Align::Min), |ui| {
                        self.preview(ui)
                    });
                });
            });
            dialog.buttons([
                DialogButton::secondary(fl!("set-font-load-font"), Button::Load).leading().enabled(!blocked),
                DialogButton::cancel(labels::cancel(), Button::Cancel),
                DialogButton::primary(labels::ok(), Button::Apply).enabled(self.selected_font().is_some()),
            ]);
        });
        match response.action {
            Some(Button::Load) if !blocked => action = Some(Action::Load),
            Some(Button::Apply) if !blocked => action = self.selected_font().cloned().map(|font| Action::Apply(Box::new(font))),
            Some(Button::Cancel) if !blocked => action = Some(Action::Cancel),
            _ if response.dismissed && !blocked => action = Some(Action::Cancel),
            _ => {}
        }
        action
    }

    fn keyboard(&mut self, context: &egui::Context) -> Option<Action> {
        let page = self.page_rows.max(1) as isize;
        let typing = context.wants_keyboard_input();
        let pressed = |key| context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key));
        if pressed(egui::Key::ArrowUp) {
            self.step(-1);
        }
        if pressed(egui::Key::ArrowDown) {
            self.step(1);
        }
        if pressed(egui::Key::PageUp) {
            self.step(-page);
        }
        if pressed(egui::Key::PageDown) {
            self.step(page);
        }
        // Home and End stay with the filter field while it has the focus.
        if !typing && pressed(egui::Key::Home) {
            self.step(isize::MIN);
        }
        if !typing && pressed(egui::Key::End) {
            self.step(isize::MAX);
        }
        if pressed(egui::Key::Enter) {
            return self.selected_font().cloned().map(|font| Action::Apply(Box::new(font)));
        }
        None
    }

    /// Returns true when a font was double clicked.
    fn list(&mut self, ui: &mut egui::Ui) -> bool {
        let response = ui.add(
            appearance::text_edit(&mut self.filter)
                .hint_text(fl!("font-selector-filter-placeholder"))
                .desired_width(f32::INFINITY),
        );
        if std::mem::take(&mut self.focus_filter) {
            response.request_focus();
        }
        if response.changed() {
            self.keep_selection_visible();
        }
        ui.add_space(8.0);
        let rows = self.rows();
        if rows.is_empty() {
            ui.weak(fl!("font-selector-no-fonts-match"));
            return false;
        }
        self.page_rows = (ui.available_height() / ITEM_HEIGHT) as usize;
        let reveal = std::mem::take(&mut self.reveal);
        let mut toggle = None;
        let mut activated = false;
        egui::ScrollArea::vertical()
            .id_salt("font-select-list")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for row in &rows {
                    match *row {
                        Row::Header(category, count) => {
                            let index = Category::ALL.iter().position(|item| *item == category).unwrap_or(0);
                            if header(ui, &format!("{} ({count})", category.label()), !self.collapsed[index]).clicked() {
                                toggle = Some(category);
                            }
                        }
                        Row::Font(index) => {
                            let response = item(ui, self.entries[index].font.name(), &self.filter, index == self.selected);
                            if reveal && index == self.selected {
                                response.scroll_to_me(None);
                            }
                            if response.clicked() {
                                self.selected = index;
                            }
                            if response.double_clicked() {
                                self.selected = index;
                                activated = true;
                            }
                        }
                    }
                }
            });
        if let Some(category) = toggle {
            self.toggle(category);
        }
        activated
    }

    fn preview(&mut self, ui: &mut egui::Ui) {
        let Some(font) = self.selected_font().cloned() else {
            ui.centered_and_justified(|ui| ui.weak(fl!("font-selector-no-selection")));
            return;
        };
        ui.label(egui::RichText::new(font.name()).size(14.0));
        let size = font.size();
        ui.weak(format!("{} × {}", size.width, size.height));
        if let Some(error) = &self.error {
            ui.colored_label(icy_engine_gui::egui::dialog::DANGER, error);
        }
        ui.add_space(8.0);
        if self.preview.as_ref().is_none_or(|(index, _)| *index != self.selected) {
            self.preview = glyph_sheet(&font).map(|image| {
                (
                    self.selected,
                    ui.ctx().load_texture("font-select-preview", image, egui::TextureOptions::NEAREST),
                )
            });
        }
        let Some((_, texture)) = &self.preview else {
            return;
        };
        let image = texture.size_vec2();
        let room = ui.available_size() - egui::Vec2::splat(PREVIEW_PADDING * 2.0);
        let fit = (room.x / image.x).min(room.y / image.y).max(0.1);
        // Whole-number magnification keeps the glyph pixels crisp whenever there is room.
        let scale = if fit >= 1.0 { fit.floor().min(PREVIEW_SCALE) } else { fit };
        let frame = egui::Rect::from_center_size(
            egui::pos2(ui.max_rect().center().x, ui.cursor().top() + (image.y * scale) / 2.0 + PREVIEW_PADDING),
            image * scale + egui::Vec2::splat(PREVIEW_PADDING * 2.0),
        );
        ui.allocate_rect(frame, egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(frame, 6, ui.visuals().extreme_bg_color);
        painter.rect_stroke(frame, 6, ui.visuals().widgets.noninteractive.bg_stroke, StrokeKind::Inside);
        painter.image(
            texture.id(),
            egui::Rect::from_center_size(frame.center(), image * scale),
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    fn show_xbin_choice(&mut self, context: &egui::Context, blocked: bool) -> Option<Action> {
        let fonts = self.xbin_fonts.clone().unwrap_or_default();
        let mut action = None;
        let response = Dialog::new("font-select-xbin").size(DialogSize::Small).show(context, |dialog| {
            dialog.content(|ui| {
                ui.label(appearance::bold(ui, fl!("set-font-xbin-select-title")).size(16.0));
                ui.add_space(12.0);
                for (index, font) in fonts.iter().enumerate() {
                    let size = font.size();
                    let slot = (index + 1) as i64;
                    let label = format!("{}: {} ({}×{})", fl!("set-font-xbin-font", slot = slot), font.name(), size.width, size.height);
                    if ui.add_sized([ui.available_width(), 28.0], egui::Button::new(label)).clicked() {
                        action = Some(Action::Apply(Box::new(font.clone())));
                    }
                }
            });
            dialog.buttons([DialogButton::cancel(labels::cancel(), Button::Back)]);
        });
        if matches!(response.action, Some(Button::Back)) || response.dismissed {
            self.xbin_fonts = None;
        }
        if blocked {
            return None;
        }
        action
    }
}

/// All 256 glyphs of `font` in a 16 × 16 grid, light gray on black.
fn glyph_sheet(font: &BitFont) -> Option<egui::ColorImage> {
    let mut buffer = TextBuffer::new((16, 16));
    buffer.set_font(0, font.clone());
    for code in 0..256u32 {
        let position = ((code % 16) as i32, (code / 16) as i32);
        let ch = char::from_u32(code).unwrap_or(' ');
        buffer.layers[0].set_char(position, AttributedChar::new(ch, TextAttribute::default()));
    }
    let size = font.size();
    let region = icy_engine::Rectangle::from(0, 0, 16 * size.width, 16 * size.height);
    let (pixels, rgba) = buffer.render_region_to_rgba(region, &RenderOptions::default(), false);
    let (width, height) = (pixels.width.max(0) as usize, pixels.height.max(0) as usize);
    (width > 0 && height > 0 && rgba.len() >= width * height * 4)
        .then(|| egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba[..width * height * 4]))
}

fn header(ui: &mut egui::Ui, label: &str, expanded: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), HEADER_HEIGHT), egui::Sense::click());
    let visuals = ui.visuals();
    let fill = if response.hovered() {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        visuals.widgets.inactive.weak_bg_fill
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 0, fill);
    let center = egui::pos2(rect.left() + 14.0, rect.center().y);
    let points = if expanded {
        vec![center + egui::vec2(-5.0, -3.0), center + egui::vec2(5.0, -3.0), center + egui::vec2(0.0, 4.0)]
    } else {
        vec![center + egui::vec2(-3.0, -5.0), center + egui::vec2(4.0, 0.0), center + egui::vec2(-3.0, 5.0)]
    };
    painter.add(egui::Shape::convex_polygon(points, visuals.text_color(), Stroke::NONE));
    painter.text(
        egui::pos2(rect.left() + 28.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.0),
        visuals.strong_text_color(),
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn item(ui: &mut egui::Ui, name: &str, filter: &str, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ITEM_HEIGHT), egui::Sense::click());
    let visuals = ui.visuals();
    if selected {
        ui.painter().rect_filled(rect, 0, visuals.selection.bg_fill);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 0, visuals.widgets.hovered.weak_bg_fill);
    }
    let color = if selected { visuals.selection.stroke.color } else { visuals.text_color() };
    let base = egui::TextFormat {
        font_id: egui::FontId::proportional(14.0),
        color,
        ..Default::default()
    };
    let highlight = egui::TextFormat {
        background: Color32::from_rgb(230, 174, 55),
        color: Color32::BLACK,
        ..base.clone()
    };
    let mut job = egui::text::LayoutJob::default();
    let mut offset = 0;
    for range in super::filter_match_ranges(name, filter) {
        job.append(&name[offset..range.start], 0.0, base.clone());
        job.append(&name[range.clone()], 0.0, highlight.clone());
        offset = range.end;
    }
    job.append(&name[offset..], 0.0, base);
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let position = egui::pos2(rect.left() + ITEM_INDENT, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(position, galley, color);
    response
}

/// Reads PSF, YAFF and raw DOS fonts, or every font of an XBin file.
pub fn load_fonts(path: &Path) -> Result<Vec<BitFont>, String> {
    let load_error = |error: &dyn std::fmt::Display| format!("{}: {error}", fl!("set-font-load-error"));
    let data = std::fs::read(path).map_err(|error| load_error(&error))?;
    let stem = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("Font");
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension == "xb" || extension == "xbin" {
        let document = FileFormat::XBin.from_bytes(&data, None).map_err(|error| load_error(&error))?;
        let fonts: Vec<_> = document
            .screen
            .buffer
            .font_iter()
            .map(|(slot, font)| {
                let mut font = font.clone();
                if font.name().is_empty() || font.name() == "Font" {
                    font.set_name(&format!("{stem} ({})", slot + 1));
                }
                font
            })
            .collect();
        if fonts.is_empty() {
            return Err(fl!("set-font-xbin-no-fonts"));
        }
        return Ok(fonts);
    }
    BitFont::from_bytes(stem.to_string(), &data)
        .map(|font| vec![font])
        .map_err(|error| load_error(&error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_draw::document::Document;
    use icy_engine::Size;

    fn selector() -> FontSelector {
        Document::new(Size::new(80, 25)).with_state(|state| FontSelector::new(state))
    }

    #[test]
    fn lists_fonts_of_the_current_height_grouped_by_category() {
        let selector = selector();
        assert_eq!(selector.selected_font().unwrap().name(), "IBM VGA");
        let rows = selector.rows();
        assert!(matches!(rows[0], Row::Header(Category::Sauce, _)));
        assert!(selector.selectable().iter().all(|&index| selector.entries[index].font.size().height == 16));
        let names: Vec<_> = selector.entries.iter().map(|entry| font_key(&entry.font)).collect();
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(names.len(), unique.len(), "fonts are listed once");
    }

    #[test]
    fn filter_collapse_and_keyboard_keep_a_visible_selection() {
        let mut selector = selector();
        selector.set_filter("vga 85");
        let selectable = selector.selectable();
        assert!(!selectable.is_empty());
        assert_eq!(selector.selected, selectable[0]);
        assert!(selectable
            .iter()
            .all(|&index| selector.entries[index].font.name().to_lowercase().contains("vga 85")));
        selector.step(1);
        assert_eq!(selector.selected, selectable[1]);
        selector.step(isize::MAX);
        assert_eq!(selector.selected, *selectable.last().unwrap());
        selector.step(isize::MIN);
        assert_eq!(selector.selected, selectable[0]);

        selector.set_filter("");
        let selected = selector.selected;
        selector.toggle(Category::Sauce);
        assert!(selector.selectable().iter().all(|&index| selector.entries[index].category == Category::Ansi));
        assert!(matches!(selector.rows()[0], Row::Header(Category::Sauce, _)), "collapsed headers stay visible");
        selector.toggle(Category::Sauce);
        assert!(selector.selectable().contains(&selected));

        selector.set_filter("no such font");
        assert!(selector.rows().is_empty());
    }

    #[test]
    fn sauce_documents_only_offer_sauce_fonts() {
        let document = Document::new(Size::new(80, 25));
        let selector = document.with_state(|state| {
            state.get_buffer_mut().font_mode = FontMode::Sauce;
            FontSelector::new(state)
        });
        assert!(selector.entries.iter().all(|entry| entry.category == Category::Sauce));
    }

    #[test]
    fn loads_single_fonts_and_asks_for_xbin_choice() {
        let directory = std::env::temp_dir().join(format!("icy_draw_font_select_{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let font = BitFont::from_sauce_name("IBM VGA 850").unwrap();
        let path = directory.join("custom.psf");
        std::fs::write(&path, font.to_psf2_bytes().unwrap()).unwrap();

        let mut selector = selector();
        let loaded = selector.load(&path).unwrap();
        assert_eq!(loaded.size(), font.size());
        assert!(selector.xbin_fonts.is_none());

        assert!(selector.load(&directory.join("missing.psf")).is_none());
        assert!(selector.error().is_some());
        let _ = std::fs::remove_dir_all(directory);
    }
}
