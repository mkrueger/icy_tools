use std::collections::HashMap;

use eframe::egui::{self, Color32, FontId, Rect, Response, Sense, Stroke};
use icy_engine_gui::egui::appearance;
use icy_mail::reader::SortDirection;

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
#[derive(Clone, Copy, Default)]
pub struct Cell<'a> {
    pub text: &'a str,
    pub strong: bool,
    pub weak: bool,
    pub indent: f32,
    pub right: bool,
}

impl<'a> Cell<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, ..Default::default() }
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
        paint_text(
            ui,
            area,
            value.text,
            FontId::new(13.0, family),
            if value.weak { weak } else { text },
            if value.right { egui::Align::Max } else { egui::Align::Min },
        );
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
