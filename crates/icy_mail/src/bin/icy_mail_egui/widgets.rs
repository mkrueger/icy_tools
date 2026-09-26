use std::collections::HashMap;

use eframe::egui::{self, Color32, FontId, Rect, Response, Sense, Stroke};
use icy_engine_gui::egui::appearance;
use icy_mail::{
    reader::SortDirection,
    text::{find_ignore_case, HeaderText},
};

pub const ROW_HEIGHT: f32 = 24.0;
pub const NAV_HEIGHT: f32 = 28.0;
pub const TOOL_SIZE: egui::Vec2 = egui::vec2(30.0, 28.0);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Icon {
    Open,
    Copy,
    Up,
    Down,
    Menu,
    Reply,
    Forward,
    Compose,
    Export,
    Inbox,
    Personal,
    Drafts,
    Lock,
    Read,
    Unread,
    Search,
    Close,
    Threads,
    List,
    Info,
    Delete,
    Sort,
    Warning,
    Check,
    Mailbox,
    Contacts,
    PersonAdd,
    Shuffle,
    Tag,
}

impl Icon {
    fn svg(self) -> &'static [u8] {
        match self {
            Self::Open => include_bytes!("../../../data/icons/folder_open.svg"),
            Self::Copy => include_bytes!("../../../data/icons/content_copy.svg"),
            Self::Up => include_bytes!("../../../data/icons/arrow_upward.svg"),
            Self::Down => include_bytes!("../../../data/icons/arrow_downward.svg"),
            Self::Menu => include_bytes!("../../../data/icons/menu.svg"),
            Self::Reply => include_bytes!("../../../data/icons/reply.svg"),
            Self::Forward => include_bytes!("../../../data/icons/forward.svg"),
            Self::Compose => include_bytes!("../../../data/icons/edit_square.svg"),
            Self::Export => include_bytes!("../../../data/icons/outbox.svg"),
            Self::Inbox => include_bytes!("../../../data/icons/inbox.svg"),
            Self::Personal => include_bytes!("../../../data/icons/person.svg"),
            Self::Drafts => include_bytes!("../../../data/icons/draft.svg"),
            Self::Lock => include_bytes!("../../../data/icons/lock.svg"),
            Self::Read => include_bytes!("../../../data/icons/mark_email_read.svg"),
            Self::Unread => include_bytes!("../../../data/icons/mark_email_unread.svg"),
            Self::Search => include_bytes!("../../../data/icons/search.svg"),
            Self::Close => include_bytes!("../../../data/icons/close.svg"),
            Self::Threads => include_bytes!("../../../data/icons/forum.svg"),
            Self::List => include_bytes!("../../../data/icons/view_list.svg"),
            Self::Info => include_bytes!("../../../data/icons/info.svg"),
            Self::Delete => include_bytes!("../../../data/icons/delete.svg"),
            Self::Sort => include_bytes!("../../../data/icons/sort.svg"),
            Self::Warning => include_bytes!("../../../data/icons/warning.svg"),
            Self::Check => include_bytes!("../../../data/icons/check_circle.svg"),
            Self::Mailbox => include_bytes!("../../../data/icons/markunread_mailbox.svg"),
            Self::Contacts => include_bytes!("../../../data/icons/contacts.svg"),
            Self::PersonAdd => include_bytes!("../../../data/icons/person_add.svg"),
            Self::Shuffle => include_bytes!("../../../data/icons/shuffle.svg"),
            Self::Tag => include_bytes!("../../../data/icons/sell.svg"),
        }
    }
}

/// Bundled Material Symbols, rendered once per window and tinted when drawn.
#[derive(Default)]
pub struct Icons(HashMap<Icon, egui::TextureHandle>);

impl Icons {
    pub fn image(&mut self, context: &egui::Context, icon: Icon, size: f32) -> egui::Image<'static> {
        let texture = self.0.entry(icon).or_insert_with(|| {
            let tree = resvg::usvg::Tree::from_data(icon.svg(), &Default::default()).expect("bundled icon");
            let mut pixels = resvg::tiny_skia::Pixmap::new(48, 48).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::from_scale(48.0 / tree.size().width(), 48.0 / tree.size().height()),
                &mut pixels.as_mut(),
            );
            context.load_texture(
                format!("mail-icon-{icon:?}"),
                egui::ColorImage::from_rgba_premultiplied([48, 48], pixels.data()),
                egui::TextureOptions::LINEAR,
            )
        });
        egui::Image::new((texture.id(), egui::Vec2::splat(size)))
    }

    /// Icon in the given colour, for list flags and headings.
    pub fn paint(&mut self, ui: &egui::Ui, icon: Icon, rect: Rect, color: Color32) {
        self.image(ui.ctx(), icon, rect.width()).tint(color).paint_at(ui, rect);
    }

    /// Flat, square icon button that only shows its frame while hovered or selected.
    pub fn button(&mut self, ui: &mut egui::Ui, icon: Icon, tooltip: &str, enabled: bool) -> Response {
        self.tool(ui, icon, None, tooltip, enabled, false)
    }

    pub fn toggle(&mut self, ui: &mut egui::Ui, icon: Icon, tooltip: &str, selected: bool) -> Response {
        self.tool(ui, icon, None, tooltip, true, selected)
    }

    /// Toolbar action with an icon and, unless `label` is `None`, a caption.
    pub fn tool(&mut self, ui: &mut egui::Ui, icon: Icon, label: Option<&str>, tooltip: &str, enabled: bool, selected: bool) -> Response {
        let image = self.image(ui.ctx(), icon, 18.0);
        let button = match label {
            Some(label) => egui::Button::image_and_text(image, label),
            None => egui::Button::image(image),
        }
        .selected(selected)
        .frame_when_inactive(selected)
        .image_tint_follows_text_color(true)
        .stroke(Stroke::NONE)
        .corner_radius(6)
        .min_size(TOOL_SIZE);
        ui.scope(|ui| {
            ui.spacing_mut().button_padding = egui::vec2(6.0, 4.0);
            ui.add_enabled(enabled, button)
        })
        .inner
        .on_hover_text(tooltip)
        .on_disabled_hover_text(tooltip)
    }
}

/// Accent used for unread markers and counts.
pub fn accent(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(92, 164, 236)
    } else {
        appearance::PRIMARY
    }
}

pub fn warning(ui: &egui::Ui) -> Color32 {
    ui.visuals().warn_fg_color
}

/// Rounded input surface holding an icon and a frameless text edit; the border follows the focus.
pub fn field(ui: &mut egui::Ui, width: f32, id: egui::Id, add: impl FnOnce(&mut egui::Ui) -> Response) -> Response {
    let focused = ui.memory(|memory| memory.has_focus(id));
    let visuals = ui.visuals().clone();
    let stroke = if focused {
        Stroke::new(1.0, visuals.selection.stroke.color)
    } else {
        visuals.widgets.inactive.bg_stroke
    };
    ui.allocate_ui_with_layout(egui::vec2(width, 30.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        egui::Frame::new()
            .fill(visuals.extreme_bg_color)
            .stroke(stroke)
            .corner_radius(6)
            .inner_margin(egui::Margin {
                left: 6,
                right: 4,
                top: 0,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.set_min_size(egui::vec2(ui.available_width(), 28.0));
                ui.spacing_mut().item_spacing.x = 2.0;
                add(ui)
            })
            .inner
    })
    .inner
}

/// Frameless drop-down for the status bar.
pub fn status_menu(ui: &mut egui::Ui, label: egui::RichText, tooltip: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(8.0, 2.0);
        let button = egui::Button::new(label)
            .right_text(egui::RichText::new("\u{23f7}").size(10.0))
            .frame_when_inactive(false)
            .stroke(Stroke::NONE)
            .min_size(egui::vec2(0.0, 20.0));
        egui::containers::menu::MenuButton::from_button(button).ui(ui, content).0
    })
    .inner
    .on_hover_text(tooltip);
}

/// Rounded toggle chip, filled with the selection colour when on.
pub fn pill(ui: &mut egui::Ui, on: bool, label: &str) -> Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(10.0, 2.0);
        let color = if on {
            ui.visuals().selection.stroke.color
        } else {
            ui.visuals().weak_text_color()
        };
        let button = egui::Button::new(egui::RichText::new(label).size(12.0).color(color))
            .selected(on)
            .corner_radius(11)
            .min_size(egui::vec2(0.0, 22.0));
        let response = ui.add(button);
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), on, label));
        response
    })
    .inner
}

/// Small uppercase heading above a group of sidebar entries.
pub fn section(ui: &mut egui::Ui, title: &str) {
    section_with(ui, title, |_| {});
}

/// Section heading with controls at its right edge.
pub fn section_with(ui: &mut egui::Ui, title: &str, right: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.set_min_height(20.0);
        ui.add_space(12.0);
        ui.label(egui::RichText::new(title.to_uppercase()).size(11.0).color(ui.visuals().weak_text_color()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            right(ui);
        });
    });
    ui.add_space(2.0);
}

/// Count shown at the right of a sidebar entry: accented pill for unread messages, weak total otherwise.
pub enum Badge {
    None,
    Unread(usize),
    Total(usize),
    Warning(usize),
}

/// Sidebar entry with an optional icon, a label and a count badge.
pub fn nav_row(ui: &mut egui::Ui, image: Option<egui::Image<'static>>, label: &str, badge: Badge, selected: bool, focused: bool) -> Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, NAV_HEIGHT), Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let visuals = ui.visuals();
    let inner = rect.shrink2(egui::vec2(6.0, 1.0));
    let fill = if selected && focused {
        visuals.selection.bg_fill
    } else if selected {
        visuals.widgets.active.weak_bg_fill
    } else if response.hovered() {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(inner, 6.0, fill);
    let color = if selected && focused {
        visuals.selection.stroke.color
    } else {
        visuals.text_color()
    };
    let mut left = inner.left() + 8.0;
    if let Some(image) = image {
        let icon = Rect::from_center_size(egui::pos2(left + 9.0, inner.center().y), egui::Vec2::splat(18.0));
        image.tint(color).paint_at(ui, icon);
        left += 26.0;
    }
    let (badge_text, badge_color, strong) = match badge {
        Badge::None => (String::new(), color, false),
        Badge::Unread(count) => (count.to_string(), accent(ui), true),
        Badge::Total(count) => (count.to_string(), visuals.weak_text_color(), false),
        Badge::Warning(count) => (count.to_string(), warning(ui), true),
    };
    let mut right = inner.right() - 8.0;
    if !badge_text.is_empty() {
        let font = FontId::proportional(11.0);
        let text_color = if strong { Color32::WHITE } else { badge_color };
        let galley = ui.painter().layout_no_wrap(badge_text, font, text_color);
        let size = egui::vec2((galley.size().x + 12.0).max(22.0), 18.0);
        let pill = Rect::from_min_size(egui::pos2(right - size.x, inner.center().y - size.y / 2.0), size);
        if strong {
            ui.painter().rect_filled(pill, 9.0, badge_color);
        }
        ui.painter().galley(pill.center() - galley.size() / 2.0, galley, text_color);
        right = pill.left() - 6.0;
    }
    let family = if matches!(badge, Badge::Unread(_)) {
        appearance::bold_family(ui)
    } else {
        egui::FontFamily::Proportional
    };
    let text = Rect::from_min_max(egui::pos2(left, inner.top()), egui::pos2(right.max(left), inner.bottom()));
    paint_text(ui, text, label, FontId::new(13.5, family), color, egui::Align::Min);
    response
}

/// One cell of a list row.
#[derive(Clone, Copy)]
pub enum CellText<'a> {
    Plain(&'a str),
    Header(&'a HeaderText),
}

#[derive(Clone, Copy)]
pub struct Cell<'a> {
    pub text: CellText<'a>,
    pub prefix: &'a str,
    pub strong: bool,
    pub weak: bool,
    pub indent: f32,
    pub right: bool,
    /// Search text to highlight.
    pub highlight: &'a str,
}

impl<'a> Cell<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text: CellText::Plain(text),
            prefix: "",
            strong: false,
            weak: false,
            indent: 0.0,
            right: false,
            highlight: "",
        }
    }

    pub fn header(text: &'a HeaderText) -> Self {
        Self {
            text: CellText::Header(text),
            ..Self::new("")
        }
    }

    pub fn prefix(mut self, prefix: &'a str) -> Self {
        self.prefix = prefix;
        self
    }

    pub fn strong(mut self, strong: bool) -> Self {
        self.strong = strong;
        self
    }

    pub fn weak(mut self) -> Self {
        self.weak = true;
        self
    }

    pub fn indent(mut self, indent: f32) -> Self {
        self.indent = indent;
        self
    }

    pub fn right(mut self) -> Self {
        self.right = true;
        self
    }

    pub fn highlight(mut self, needle: &'a str) -> Self {
        self.highlight = needle;
        self
    }
}

/// Column rectangles of a row, left to right.
pub fn columns(rect: Rect, widths: &[f32]) -> Vec<Rect> {
    let mut left = rect.left();
    widths
        .iter()
        .map(|width| {
            let cell = Rect::from_min_size(egui::pos2(left, rect.top()), egui::vec2(*width, rect.height()));
            left += width;
            cell
        })
        .collect()
}

pub struct RowColors {
    pub text: Color32,
    pub weak: Color32,
    pub selected: bool,
}

/// Clickable list row; returns the response, the cell rectangles and the text colours in use.
pub fn row(ui: &mut egui::Ui, widths: &[f32], cells: &[Cell], selected: bool, focused: bool) -> (Response, Vec<Rect>, RowColors) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(widths.iter().sum(), ROW_HEIGHT), Sense::click());
    let visuals = ui.visuals();
    let fill = if selected && focused {
        visuals.selection.bg_fill
    } else if selected {
        visuals.widgets.active.weak_bg_fill
    } else if response.hovered() {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 0, fill);
    let (text, weak) = if selected && focused {
        let color = visuals.selection.stroke.color;
        (color, color.gamma_multiply(0.8))
    } else {
        (visuals.text_color(), visuals.weak_text_color())
    };
    let rects = columns(rect, widths);
    let bold = appearance::bold_family(ui);
    for (cell, value) in rects.iter().zip(cells) {
        let family = if value.strong { bold.clone() } else { egui::FontFamily::Proportional };
        let mut area = cell.shrink2(egui::vec2(6.0, 0.0));
        area.min.x = (area.min.x + value.indent).min(area.max.x);
        let color = if value.weak { weak } else { text };
        let align = if value.right { egui::Align::Max } else { egui::Align::Min };
        let font = FontId::new(13.0, family);
        match value.text {
            CellText::Plain(cell_text) if value.highlight.is_empty() => paint_text(ui, area, cell_text, font, color, align),
            CellText::Plain(cell_text) => {
                let mut job = egui::text::LayoutJob::simple_singleline(cell_text.to_owned(), font, color);
                highlight(&mut job, 0, value.highlight, ui);
                paint_job(ui, area, job, color, align);
            }
            CellText::Header(header) => {
                let mut job = header_job(ui, header, value.prefix, font, color, value.strong);
                highlight(&mut job, value.prefix.len(), value.highlight, ui);
                paint_job(ui, area, job, color, align);
            }
        }
    }
    (
        response,
        rects,
        RowColors {
            text,
            weak,
            selected: selected && focused,
        },
    )
}

/// Column captions; clicking one returns its index so the caller can sort by it.
pub fn header(ui: &mut egui::Ui, widths: &[f32], labels: &[&str], active: Option<(usize, SortDirection)>, enabled: bool) -> Option<usize> {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(widths.iter().sum(), 26.0), Sense::hover());
    let visuals = ui.visuals().clone();
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
    );
    let mut clicked = None;
    for (index, (cell, label)) in columns(rect, widths).into_iter().zip(labels).enumerate() {
        if label.is_empty() {
            continue;
        }
        let response = ui.interact(cell, ui.id().with(("header", index)), if enabled { Sense::click() } else { Sense::hover() });
        let sorted = active.filter(|(column, _)| *column == index);
        let text = match sorted {
            Some((_, direction)) => format!("{label} {}", arrow(direction)),
            None => label.to_string(),
        };
        let color = if sorted.is_some() || (enabled && response.hovered()) {
            visuals.strong_text_color()
        } else {
            visuals.weak_text_color()
        };
        paint_text(
            ui,
            cell.shrink2(egui::vec2(6.0, 0.0)),
            &text,
            FontId::proportional(12.0),
            color,
            egui::Align::Min,
        );
        if response.clicked() {
            clicked = Some(index);
        }
    }
    clicked
}

/// Sort indicator that the bundled UI font can draw.
pub fn arrow(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Ascending => "\u{2191}",
        SortDirection::Descending => "\u{2193}",
    }
}

/// Single line of text clipped to `rect`, vertically centred.
pub fn paint_text(ui: &egui::Ui, rect: Rect, text: &str, font: FontId, color: Color32, align: egui::Align) {
    if text.is_empty() || rect.width() <= 0.0 {
        return;
    }
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let x = match align {
        egui::Align::Max => (rect.right() - galley.size().x).max(rect.left()),
        _ => rect.left(),
    };
    ui.painter()
        .with_clip_rect(ui.clip_rect().intersect(rect))
        .galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley, color);
}

pub fn header_job(ui: &egui::Ui, header: &HeaderText, prefix: &str, font: FontId, color: Color32, strong: bool) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    append_header(&mut job, ui, header, prefix, font, color, strong);
    job
}

pub fn append_header(job: &mut egui::text::LayoutJob, ui: &egui::Ui, header: &HeaderText, prefix: &str, font: FontId, color: Color32, strong: bool) {
    let base = egui::TextFormat {
        font_id: font.clone(),
        color,
        ..Default::default()
    };
    if !prefix.is_empty() {
        job.append(prefix, 0.0, base.clone());
    }
    let Some(spans) = header.styled() else {
        job.append(header.as_str(), 0.0, base);
        return;
    };
    for span in spans {
        let span_color = span.foreground.map_or(color, |[red, green, blue]| Color32::from_rgb(red, green, blue));
        job.append(
            &span.text,
            0.0,
            egui::TextFormat {
                font_id: if strong || span.bold {
                    FontId::new(font.size, appearance::bold_family(ui))
                } else {
                    font.clone()
                },
                color: span_color,
                background: span
                    .background
                    .map_or(Color32::TRANSPARENT, |[red, green, blue]| Color32::from_rgb(red, green, blue)),
                italics: span.italic,
                underline: if span.underline { Stroke::new(1.0, span_color) } else { Stroke::NONE },
                strikethrough: if span.strikethrough { Stroke::new(1.0, span_color) } else { Stroke::NONE },
                ..Default::default()
            },
        );
    }
}

/// Marks the search matches in the job's text from byte `start` on, like the find highlight of a browser.
pub fn highlight(job: &mut egui::text::LayoutJob, start: usize, needle: &str, ui: &egui::Ui) {
    let Some(text) = job.text.get(start..) else {
        return;
    };
    let matches: Vec<_> = find_ignore_case(text, needle.trim())
        .into_iter()
        .map(|found| found.start + start..found.end + start)
        .collect();
    if matches.is_empty() {
        return;
    }
    let background = if ui.visuals().dark_mode {
        Color32::from_rgb(230, 180, 40)
    } else {
        Color32::from_rgb(255, 214, 80)
    };
    let mut sections = Vec::with_capacity(job.sections.len() + matches.len() * 2);
    for section in std::mem::take(&mut job.sections) {
        let mut start = section.byte_range.start;
        let mut leading_space = section.leading_space;
        let mut push = |sections: &mut Vec<egui::text::LayoutSection>, range: std::ops::Range<usize>, marked: bool| {
            if range.is_empty() {
                return;
            }
            let mut format = section.format.clone();
            if marked {
                format.background = background;
                format.color = Color32::BLACK;
            }
            sections.push(egui::text::LayoutSection {
                leading_space: std::mem::take(&mut leading_space),
                byte_range: range,
                format,
            });
        };
        for found in &matches {
            let (from, to) = (found.start.max(start), found.end.min(section.byte_range.end));
            if from >= to {
                continue;
            }
            push(&mut sections, start..from, false);
            push(&mut sections, from..to, true);
            start = to;
        }
        push(&mut sections, start..section.byte_range.end, false);
    }
    job.sections = sections;
}

fn paint_job(ui: &egui::Ui, rect: Rect, job: egui::text::LayoutJob, color: Color32, align: egui::Align) {
    if job.text.is_empty() || rect.width() <= 0.0 {
        return;
    }
    let galley = ui.painter().layout_job(job);
    let x = match align {
        egui::Align::Max => (rect.right() - galley.size().x).max(rect.left()),
        _ => rect.left(),
    };
    ui.painter()
        .with_clip_rect(ui.clip_rect().intersect(rect))
        .galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley, color);
}

/// Small filled circle marking unread messages.
pub fn unread_dot(ui: &egui::Ui, rect: Rect, color: Color32) {
    ui.painter().circle_filled(rect.center(), 3.5, color);
}

/// Circle with the initial of `name`, coloured by the name, like the avatars of most mail clients.
pub fn avatar(ui: &mut egui::Ui, name: &str, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(size), Sense::hover());
    let hash = name.bytes().fold(0x811c_9dc5u32, |hash, byte| {
        (hash ^ u32::from(byte.to_ascii_lowercase())).wrapping_mul(0x0100_0193)
    });
    let color = egui::ecolor::Hsva::new((hash % 360) as f32 / 360.0, 0.45, if ui.visuals().dark_mode { 0.55 } else { 0.7 }, 1.0);
    ui.painter().circle_filled(rect.center(), size / 2.0, Color32::from(color));
    let initial = name.trim().chars().next().map_or_else(|| "?".to_string(), |ch| ch.to_uppercase().to_string());
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initial,
        FontId::new(size * 0.45, appearance::bold_family(ui)),
        Color32::WHITE,
    );
}

/// Scroll offset that keeps row `position` visible.
pub fn reveal(position: usize, offset: f32, height: f32) -> f32 {
    let top = position as f32 * ROW_HEIGHT;
    let bottom = top + ROW_HEIGHT;
    if top < offset || height <= 0.0 {
        top
    } else if bottom > offset + height {
        (bottom - height).max(0.0)
    } else {
        offset
    }
}

/// Centred placeholder for empty panes.
pub fn empty(ui: &mut egui::Ui, image: egui::Image<'static>, title: &str, detail: &str) {
    ui.vertical_centered(|ui| {
        let top = ((ui.available_height() - 110.0) / 2.0).max(8.0);
        ui.add_space(top);
        ui.add(image.tint(ui.visuals().weak_text_color()));
        ui.add_space(8.0);
        ui.label(egui::RichText::new(title).size(15.0).color(ui.visuals().text_color()));
        if !detail.is_empty() {
            ui.add_space(2.0);
            ui.label(egui::RichText::new(detail).size(12.0).color(ui.visuals().weak_text_color()));
        }
    });
}

/// A row of a selectable list in a dialog, painted with the selection or hover background.
pub fn list_row(ui: &mut egui::Ui, selected: bool, height: f32) -> (egui::Rect, Response) {
    // Not focusable: the arrow keys move the selection, so they must not move the focus onto rows.
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::CLICK);
    let visuals = ui.visuals();
    if selected {
        ui.painter().rect_filled(rect, 4, visuals.selection.bg_fill);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 4, visuals.widgets.hovered.weak_bg_fill);
    }
    (rect, response)
}

/// Text color on a [`list_row`].
pub fn list_text(ui: &egui::Ui, selected: bool) -> Color32 {
    if selected {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals().text_color()
    }
}

/// Moves `selected` through the `visible` entries with the arrow and page keys, which a
/// filter field above the list does not need; returns whether it moved.
/// Keeps the arrow keys in a list's filter field, so they move the list selection instead of the focus.
pub fn keep_list_focus(ui: &egui::Ui, response: &egui::Response) {
    if response.has_focus() {
        let filter = egui::EventFilter {
            horizontal_arrows: true,
            vertical_arrows: true,
            ..Default::default()
        };
        ui.memory_mut(|memory| memory.set_focus_lock_filter(response.id, filter));
    }
}

pub fn list_keys(context: &egui::Context, visible: &[usize], selected: &mut Option<usize>) -> bool {
    if visible.is_empty() {
        return false;
    }
    let current = selected.and_then(|index| visible.iter().position(|&entry| entry == index));
    let last = visible.len() - 1;
    let next = context.input_mut(|input| {
        let mut next = None;
        for (key, target) in [
            (egui::Key::ArrowUp, current.map_or(0, |current| current.saturating_sub(1))),
            (egui::Key::ArrowDown, current.map_or(0, |current| (current + 1).min(last))),
            (egui::Key::PageUp, current.map_or(0, |current| current.saturating_sub(10))),
            (egui::Key::PageDown, current.map_or(0, |current| (current + 10).min(last))),
        ] {
            if input.consume_key(egui::Modifiers::NONE, key) {
                next = Some(target);
            }
        }
        next
    });
    match next {
        Some(next) => {
            *selected = Some(visible[next]);
            true
        }
        None => false,
    }
}

/// One line of text in `rect`, cut off with an ellipsis, with the matches of `needle` highlighted.
pub fn paint_line(ui: &egui::Ui, rect: Rect, text: &str, font: FontId, color: Color32, needle: &str) {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat {
            font_id: font,
            color,
            ..Default::default()
        },
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width());
    highlight(&mut job, 0, needle, ui);
    paint_job(ui, rect, job, color, egui::Align::Min);
}
