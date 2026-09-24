use super::{
    browser::Browser,
    colors,
    icons::{Icon, Icons},
    text,
};
use eframe::egui;
use icy_view::{
    sauce_loader::{SauceLoader, SauceRequest, SharedSauceCache},
    sort_order::SortOrder,
};
use std::sync::Arc;

pub const ROW_HEIGHT: f32 = 24.0;

pub struct FileList {
    loader: SauceLoader,
    cache: SharedSauceCache,
    source: Option<(u64, String)>,
    pub viewport_height: f32,
}

pub struct Response {
    pub activate: Option<(usize, bool)>,
    pub sort: Option<SortOrder>,
}

impl FileList {
    pub fn new(context: &egui::Context) -> Self {
        let (loader, mut receiver, cache) = SauceLoader::spawn();
        let context = context.clone();
        std::thread::spawn(move || {
            while receiver.blocking_recv().is_some() {
                context.request_repaint();
            }
        });
        Self {
            loader,
            cache,
            source: None,
            viewport_height: 0.0,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, browser: &Browser, icons: &mut Icons, sauce_mode: bool, ensure_selected: bool) -> Response {
        let source = (browser.revision(), browser.location.point.path.clone());
        if self.source.as_ref() != Some(&source) {
            *self = Self::new(ui.ctx());
            self.source = Some(source);
        }
        let mut response = Response { activate: None, sort: None };
        let visible = browser.visible();
        let widths: Vec<f32> = if sauce_mode {
            vec![286.0, 280.0, 160.0, 160.0]
        } else {
            vec![(ui.available_width() - 80.0).max(160.0), 76.0]
        };
        let headers = if sauce_mode {
            vec![text("header-name"), text("header-title"), text("header-author"), text("header-group")]
        } else {
            vec![text("header-name"), text("sauce-field-file-size")]
        };
        egui::ScrollArea::horizontal()
            .id_salt(("file-columns", &browser.location.point.path))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(widths.iter().sum());
                ui.spacing_mut().item_spacing.y = 0.0;
                let (header, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_HEIGHT), egui::Sense::hover());
                ui.painter().rect_filled(header, 0.0, ui.visuals().faint_bg_color);
                let mut left = header.left();
                for (column, width) in widths.iter().enumerate() {
                    let rect = egui::Rect::from_min_size(egui::pos2(left, header.top()), egui::vec2(*width, ROW_HEIGHT));
                    let sortable = column == 0 || (!sauce_mode && column == 1);
                    let clicked = ui.interact(
                        rect,
                        ui.id().with(("header", column)),
                        if sortable { egui::Sense::click() } else { egui::Sense::hover() },
                    );
                    if clicked.clicked() {
                        response.sort = Some(if column == 0 {
                            if browser.sort == SortOrder::NameAsc {
                                SortOrder::NameDesc
                            } else {
                                SortOrder::NameAsc
                            }
                        } else if browser.sort == SortOrder::SizeAsc {
                            SortOrder::SizeDesc
                        } else {
                            SortOrder::SizeAsc
                        });
                    }
                    let ascending = match (column, browser.sort) {
                        (0, SortOrder::NameAsc) => Some(true),
                        (0, SortOrder::NameDesc) => Some(false),
                        (1, SortOrder::SizeAsc) if !sauce_mode => Some(true),
                        (1, SortOrder::SizeDesc) if !sauce_mode => Some(false),
                        _ => None,
                    };
                    let color = if clicked.hovered() && sortable {
                        ui.visuals().text_color()
                    } else {
                        ui.visuals().weak_text_color()
                    };
                    let label = cell(ui, rect, egui::RichText::new(&headers[column]).size(12.0).strong().color(color));
                    if let Some(ascending) = ascending {
                        let center = egui::pos2((label.right() + 8.0).min(rect.right() - 6.0), rect.center().y);
                        let (tip, base) = if ascending { (-3.0, 2.0) } else { (3.0, -2.0) };
                        ui.painter().add(egui::Shape::convex_polygon(
                            vec![center + egui::vec2(0.0, tip), center + egui::vec2(4.0, base), center + egui::vec2(-4.0, base)],
                            color,
                            egui::Stroke::NONE,
                        ));
                    }
                    if column > 0 {
                        ui.painter().vline(
                            rect.left(),
                            rect.y_range().shrink(5.0),
                            egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                        );
                    }
                    left += width;
                }
                ui.painter()
                    .hline(header.x_range(), header.bottom() - 0.5, ui.visuals().widgets.noninteractive.bg_stroke);
                self.viewport_height = ui.available_height();
                let mut scroll = egui::ScrollArea::vertical()
                    .id_salt(("file-rows", &browser.location.point.path, &browser.filter))
                    .auto_shrink([false, false]);
                if ensure_selected {
                    if let Some(row) = visible.iter().position(|index| Some(*index) == browser.selected) {
                        scroll = scroll.vertical_scroll_offset((row as f32 * ROW_HEIGHT - self.viewport_height * 0.5).max(0.0));
                    }
                }
                scroll.show_rows(ui, ROW_HEIGHT, visible.len(), |ui, rows| {
                    let dark = ui.visuals().dark_mode;
                    let sauce_colors = colors::Sauce::new(dark);
                    for row in rows {
                        let index = visible[row];
                        let item = &browser.items[index];
                        let path = item.get_full_path().unwrap_or_else(|| item.get_file_path());
                        if sauce_mode && !item.is_container() {
                            self.loader.load(SauceRequest {
                                item: Arc::from(item.clone_box()),
                            });
                        }
                        let cached = self.cache.read().get(&path);
                        let loaded = cached.is_some();
                        let metadata = cached.flatten();
                        let (rect, row_response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_HEIGHT), egui::Sense::click());
                        let selected = browser.selected == Some(index);
                        if selected || row_response.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                0.0,
                                if selected {
                                    ui.visuals().selection.bg_fill
                                } else {
                                    ui.visuals().widgets.hovered.bg_fill
                                },
                            );
                        } else if row % 2 == 1 {
                            ui.painter().rect_filled(rect, 0.0, ui.visuals().faint_bg_color.gamma_multiply(0.25));
                        }
                        icons
                            .image(ui.ctx(), Icon::File(item.get_file_icon()), 18.0)
                            .paint_at(ui, egui::Rect::from_min_size(rect.min + egui::vec2(4.0, 3.0), egui::Vec2::splat(18.0)));
                        let name_rect = egui::Rect::from_min_max(rect.min + egui::vec2(26.0, 0.0), egui::pos2(rect.left() + widths[0], rect.bottom()));
                        let label = item.get_label();
                        let color = colors::file_name(dark, &label, item.is_container(), ui.visuals().text_color());
                        cell(ui, name_rect, highlighted(&label, &browser.filter, color, colors::highlight(dark)));
                        let values: Vec<(String, egui::Color32)> = if sauce_mode {
                            let fields = [
                                (metadata.as_ref().map(|sauce| sauce.title.to_string()), sauce_colors.title),
                                (metadata.as_ref().map(|sauce| sauce.author.to_string()), sauce_colors.author),
                                (metadata.as_ref().map(|sauce| sauce.group.to_string()), sauce_colors.group),
                            ];
                            fields
                                .into_iter()
                                .map(|(value, color)| match value {
                                    Some(value) if !value.trim().is_empty() => (value, color),
                                    // A loaded record without this field shows the original placeholder.
                                    _ if loaded && !item.is_container() => ("-".to_owned(), sauce_colors.empty),
                                    _ => (String::new(), sauce_colors.empty),
                                })
                                .collect()
                        } else {
                            vec![(item.size().map(super::app::format_size).unwrap_or_default(), ui.visuals().weak_text_color())]
                        };
                        let mut left = rect.left() + widths[0];
                        for (column, (value, color)) in values.iter().enumerate() {
                            cell(
                                ui,
                                egui::Rect::from_min_size(egui::pos2(left, rect.top()), egui::vec2(widths[column + 1], ROW_HEIGHT)),
                                egui::RichText::new(value).monospace().size(14.0).color(*color),
                            );
                            left += widths[column + 1];
                        }
                        if row_response.double_clicked() {
                            response.activate = Some((index, true));
                        } else if row_response.clicked() {
                            response.activate = Some((index, false));
                        }
                        row_response.on_hover_ui(|ui| {
                            ui.label(&label);
                            if let Some(metadata) = &metadata {
                                for value in [&metadata.title, &metadata.author, &metadata.group] {
                                    if !value.trim().is_empty() {
                                        ui.label(value.as_ref());
                                    }
                                }
                            }
                        });
                    }
                });
                if visible.is_empty() && !browser.loading {
                    ui.label(text("filter-no-items-found"));
                }
            });
        response
    }
}

impl Drop for FileList {
    fn drop(&mut self) {
        self.loader.cancel_all();
    }
}

fn cell(ui: &mut egui::Ui, rect: egui::Rect, value: impl Into<egui::WidgetText>) -> egui::Rect {
    let rect = rect.shrink2(egui::vec2(4.0, 2.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    child.set_clip_rect(ui.clip_rect().intersect(rect));
    child.add(egui::Label::new(value).truncate().selectable(false)).rect
}

fn highlighted(label: &str, filter: &str, color: egui::Color32, highlight: egui::Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let mut folded = String::new();
    let mut boundaries = Vec::new();
    for (offset, character) in label.char_indices() {
        let lower = character.to_lowercase().collect::<String>();
        boundaries.extend(std::iter::repeat_n((offset, offset + character.len_utf8()), lower.len()));
        folded.push_str(&lower);
    }
    let filter = filter.to_lowercase();
    let mut last = 0;
    let format = egui::TextFormat {
        font_id: egui::FontId::monospace(14.0),
        color,
        ..Default::default()
    };
    if !filter.is_empty() {
        for (offset, matched) in folded.match_indices(&filter) {
            let start = boundaries[offset].0;
            let end = boundaries[offset + matched.len() - 1].1;
            if start < last {
                continue;
            }
            job.append(&label[last..start], 0.0, format.clone());
            job.append(
                &label[start..end],
                0.0,
                egui::TextFormat {
                    color: highlight,
                    ..format.clone()
                },
            );
            last = end;
        }
    }
    job.append(&label[last..], 0.0, format);
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires live Sixteen Colors access"]
    fn live_web_sauce_loads_into_list_and_dialog() {
        use icy_view::items::{Item, NavPoint, SixteenColorsFile};
        use std::time::{Duration, Instant};

        let fixture = crate::tests::Fixture::new();
        let context = egui::Context::default();
        icy_engine_gui::egui::appearance::apply(&context);
        let item = SixteenColorsFile::new(
            "arl-evoke.ans".into(),
            "/pack/impure91/raw/arl-evoke.ans".into(),
            "/pack/impure91/raw/arl-evoke.ans".into(),
            String::new(),
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let data = runtime.block_on(async { tokio::time::timeout(Duration::from_secs(15), item.read_data()).await.unwrap().unwrap() });
        let sauce = icy_sauce::SauceRecord::from_bytes(&data).unwrap().expect("web file has no SAUCE");
        let title = sauce.title().to_string();
        assert!(!title.trim().is_empty());
        let mut browser = Browser::new(fixture.0.clone(), Default::default()).unwrap();
        browser.location.point = NavPoint::web("2025/impure91");
        browser.items.push(Box::new(item));
        let mut list = FileList::new(&context);
        let mut icons = Icons::default();
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))),
            ..Default::default()
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let output = context.run(input(), |context| {
                egui::CentralPanel::default().show(context, |ui| {
                    list.show(ui, &browser, &mut icons, true, false);
                });
            });
            if output
                .shapes
                .iter()
                .any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == title))
            {
                break;
            }
            assert!(Instant::now() < deadline, "SAUCE title missing from file list: {title}");
            std::thread::yield_now();
        }
        let mut preview = crate::preview::Preview::new(&context).unwrap();
        preview.load("arl-evoke.ans".into(), data, false, &context);
        crate::tests::wait_preview(&mut preview, &context);
        assert_eq!(preview.sauce.as_ref().unwrap().title().to_string(), title);
        let mut dialogs = crate::dialogs::Dialogs::default();
        let mut options = icy_view::Options::default();
        dialogs.open(crate::dialogs::Mode::Sauce, &options, &preview);
        for _ in 0..2 {
            let _ = context.run(input(), |context| dialogs.show(context, &mut options, &mut preview));
        }
        let output = context.run(input(), |context| dialogs.show(context, &mut options, &mut preview));
        assert!(
            output
                .shapes
                .iter()
                .any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == title)),
            "SAUCE title missing from dialog"
        );
        println!("Web SAUCE loaded in list and dialog: {title} / {} / {}", sauce.author(), sauce.group());
    }

    #[test]
    fn filter_highlighting_preserves_unicode_filename() {
        let job = highlighted("Ärt-İCY.ans", "äRT", egui::Color32::WHITE, egui::Color32::BLUE);
        assert_eq!(job.text, "Ärt-İCY.ans");
        assert!(job
            .sections
            .iter()
            .any(|section| section.format.color == egui::Color32::BLUE && &job.text[section.byte_range.clone()] == "Ärt"));
    }
}
