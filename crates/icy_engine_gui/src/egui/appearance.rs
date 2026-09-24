use ::egui::{self, Color32, CornerRadius, FontId, Stroke, TextStyle};

pub use super::dialog::{button_row, dialog_frame, labels, ButtonKind, Dialog, DialogButton, DialogResponse, DialogSize, DialogUi, MessageBox, MessageKind};

pub const PRIMARY: Color32 = Color32::from_rgb(32, 100, 160);

/// Shared padding so every single-line input ends up the same height.
pub const FIELD_MARGIN: egui::Vec2 = egui::Vec2::new(8.0, 6.0);

pub fn text_edit(value: &mut String) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(value).margin(FIELD_MARGIN)
}

pub fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(title).strong().size(14.0));
    ui.add_space(2.0);
}

const BOLD_FAMILY: &str = "icy-sans-bold";

/// The semibold face, or the regular one when [`apply`] has not installed the fonts.
pub fn bold_family(ui: &egui::Ui) -> egui::FontFamily {
    let family = egui::FontFamily::Name(BOLD_FAMILY.into());
    if ui.fonts(|fonts| fonts.definitions().families.contains_key(&family)) {
        family
    } else {
        egui::FontFamily::Proportional
    }
}

pub fn bold(ui: &egui::Ui, text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).strong().family(bold_family(ui))
}

/// Titled, rounded box around a set of rows, like the grouped panes of the macOS preferences.
pub fn group<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    group_with_spacing(ui, title, 4.0, 12, 14.0, 8.0, add)
}

/// The same grouped surface with tighter spacing for dense, read-only information.
pub fn compact_group<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    group_with_spacing(ui, title, 2.0, 6, 4.0, 3.0, add)
}

fn group_with_spacing<R>(
    ui: &mut egui::Ui,
    title: &str,
    top_spacing: f32,
    vertical_margin: i8,
    bottom_spacing: f32,
    row_spacing: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.add_space(top_spacing);
    if !title.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(bold(ui, title).size(14.0));
        });
        ui.add_space(2.0);
    }
    let visuals = ui.visuals();
    let fill = if visuals.dark_mode {
        visuals.window_fill.lerp_to_gamma(visuals.faint_bg_color, 0.35)
    } else {
        // Slightly darker than the window, so white text fields stand out inside the box.
        visuals.window_fill.lerp_to_gamma(visuals.widgets.inactive.bg_fill, 0.4)
    };
    let border = visuals.widgets.noninteractive.bg_stroke.color.lerp_to_gamma(visuals.window_fill, 0.3);
    let inner = egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(16, vertical_margin))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = row_spacing;
            add(ui)
        })
        .inner;
    ui.add_space(bottom_spacing);
    inner
}

/// Square check box without a caption, filled with the accent colour when set.
pub fn check(ui: &mut egui::Ui, value: &mut bool) -> egui::Response {
    let side = 18.0;
    let (rect, mut response) = ui.allocate_exact_size(egui::vec2(side, side.max(ui.spacing().interact_size.y - 6.0)), egui::Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, ""));
    if ui.is_rect_visible(rect) {
        let square = egui::Rect::from_center_size(rect.center(), egui::vec2(side, side));
        let painter = ui.painter();
        if *value {
            let fill = if response.hovered() { PRIMARY.gamma_multiply(1.2) } else { PRIMARY };
            painter.rect_filled(square, 4.0, fill);
            let point = |x: f32, y: f32| square.min + egui::vec2(x, y) * side;
            painter.add(egui::Shape::line(
                vec![point(0.26, 0.52), point(0.43, 0.69), point(0.75, 0.33)],
                Stroke::new(2.0, Color32::WHITE),
            ));
        } else {
            let stroke = if response.hovered() {
                ui.visuals().widgets.hovered.bg_stroke
            } else {
                ui.visuals().widgets.inactive.bg_stroke
            };
            painter.rect(square, 4.0, ui.visuals().extreme_bg_color, stroke, egui::StrokeKind::Inside);
        }
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Form row with the caption in the label column and a [`check`] box in the control column.
pub fn check_row(ui: &mut egui::Ui, label: &str, value: &mut bool) -> egui::Response {
    ui.push_id(label, |ui| {
        if ui.available_width() < FORM_STACK_WIDTH {
            // Stacking would leave the box alone on its own line, so keep it beside the caption.
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(label).wrap());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| check(ui, value)).inner
            })
            .inner
        } else {
            let mut response = None;
            form_row(ui, label, |ui| response = Some(check(ui, value)));
            response.unwrap()
        }
    })
    .inner
}

/// Flat track with a filled leading part and a round handle.
pub fn slider(ui: &mut egui::Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>, width: f32) -> egui::Response {
    let (min, max) = (*range.start(), *range.end());
    let (rect, mut response) = ui.allocate_exact_size(egui::vec2(width, ui.spacing().interact_size.y), egui::Sense::click_and_drag());
    let radius = 8.0;
    let (left, right) = (rect.left() + radius, (rect.right() - radius).max(rect.left() + radius + 1.0));
    let before = *value;
    if let Some(position) = response.interact_pointer_pos() {
        let t = ((position.x - left) / (right - left)).clamp(0.0, 1.0);
        *value = ((min + t * (max - min)) * 100.0).round() / 100.0;
    }
    if response.has_focus() {
        let step = (max - min) / 100.0;
        let (decrease, increase) = ui.input(|input| {
            (
                input.key_pressed(egui::Key::ArrowLeft) || input.key_pressed(egui::Key::ArrowDown),
                input.key_pressed(egui::Key::ArrowRight) || input.key_pressed(egui::Key::ArrowUp),
            )
        });
        if decrease {
            *value = (*value - step).clamp(min, max);
        }
        if increase {
            *value = (*value + step).clamp(min, max);
        }
    }
    if *value != before {
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), f64::from(*value), ""));
    if ui.is_rect_visible(rect) {
        let t = if max > min { ((*value - min) / (max - min)).clamp(0.0, 1.0) } else { 0.0 };
        let x = left + t * (right - left);
        let y = rect.center().y;
        let painter = ui.painter();
        let track = egui::Rect::from_x_y_ranges(left..=right, (y - 2.0)..=(y + 2.0));
        painter.rect_filled(track, 2.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
        painter.rect_filled(egui::Rect::from_x_y_ranges(left..=x, track.y_range()), 2.0, PRIMARY);
        let active = response.hovered() || response.dragged() || response.has_focus();
        let handle = if active { radius + 1.0 } else { radius };
        painter.circle(egui::pos2(x, y), handle - 1.0, Color32::WHITE, Stroke::new(2.0, PRIMARY));
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Pill shaped page selector: the current page is filled with the accent colour.
pub fn tab<Value: PartialEq>(ui: &mut egui::Ui, current: &mut Value, value: Value, label: &str) -> egui::Response {
    let selected = *current == value;
    let galley = ui.fonts_mut(|fonts| fonts.layout_no_wrap(label.to_owned(), FontId::proportional(14.0), Color32::PLACEHOLDER));
    let (rect, response) = ui.allocate_exact_size(egui::vec2(galley.size().x + 20.0, 30.0), egui::Sense::click());
    let pressed = response.is_pointer_button_down_on();
    let hovered = response.hovered();
    let fill = if selected {
        Some(PRIMARY)
    } else if pressed {
        Some(PRIMARY.gamma_multiply(0.55))
    } else if hovered {
        Some(PRIMARY.gamma_multiply(0.3))
    } else {
        None
    };
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, 6.0, fill);
    }
    let color = if selected { Color32::WHITE } else { ui.visuals().text_color() };
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    if response.clicked() {
        *current = value;
        ui.ctx().request_repaint();
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn chip(ui: &mut egui::Ui, label: &str, color: Color32) {
    let font = FontId::proportional(10.0);
    let width = ui.fonts_mut(|fonts| fonts.layout_no_wrap(label.to_owned(), font.clone(), color).size().x);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width + 10.0, 15.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 3.0, color.gamma_multiply(0.22));
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, font, color);
}

pub fn metric_tile(ui: &mut egui::Ui, label: &str, value: &str) {
    tile(ui, label, value, None);
}

/// Metric tile with an exact outer width so a row of tiles keeps its size while the values change.
pub fn metric_tile_sized(ui: &mut egui::Ui, label: &str, value: &str, width: f32) {
    tile(ui, label, value, Some(width));
}

fn tile(ui: &mut egui::Ui, label: &str, value: &str, outer_width: Option<f32>) {
    let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
    // A frame reserves its margins and its stroke around the content.
    let content_width = outer_width.map(|width| (width - 20.0 - stroke.width * 2.0).max(8.0));
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(stroke)
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.set_min_width(content_width.unwrap_or(64.0));
                if let Some(width) = content_width {
                    ui.set_max_width(width);
                }
                ui.add(egui::Label::new(egui::RichText::new(value).strong().size(15.0)).truncate());
                ui.add(egui::Label::new(egui::RichText::new(label).weak().size(11.0)).truncate());
            });
        });
}

/// Rounded status pill with a leading dot, used for transfer and connection state.
pub fn status_badge(ui: &mut egui::Ui, label: &str, color: Color32) {
    let font = FontId::proportional(11.0);
    let width = ui.fonts_mut(|fonts| fonts.layout_no_wrap(label.to_owned(), font.clone(), color).size().x);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width + 26.0, 20.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 10.0, color.gamma_multiply(0.18));
    ui.painter().circle_filled(egui::pos2(rect.left() + 10.0, rect.center().y), 3.5, color);
    ui.painter()
        .text(egui::pos2(rect.left() + 18.0, rect.center().y), egui::Align2::LEFT_CENTER, label, font, color);
}

pub fn combo_row(ui: &mut egui::Ui, label: &str, selected: impl Into<egui::WidgetText>, choices: impl FnOnce(&mut egui::Ui)) {
    ui.push_id(label, |ui| {
        form_row(ui, label, |ui| {
            egui::ComboBox::from_id_salt("value")
                .width(ui.available_width().min(280.0))
                .selected_text(selected)
                .show_ui(ui, choices);
        });
    });
}

pub fn slider_row(ui: &mut egui::Ui, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>) {
    form_row(ui, label, |ui| {
        ui.horizontal(|ui| slider_with_value(ui, value, range));
    });
}

fn slider_with_value(ui: &mut egui::Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>) {
    // The value box has a fixed width, otherwise a long value would widen the whole dialog.
    let box_width = 64.0;
    let spacing = ui.spacing().item_spacing.x;
    let track = (ui.available_width() - box_width - spacing * 2.0).max(40.0);
    slider(ui, value, range.clone(), track);
    ui.add_space(spacing);
    let speed = f64::from((*range.end() - *range.start()) / 200.0);
    ui.add_sized(
        [box_width, ui.spacing().interact_size.y],
        egui::DragValue::new(value).range(range).max_decimals(2).speed(speed),
    );
}

pub fn primary_button(label: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(label.into()).color(Color32::WHITE)).fill(PRIMARY)
}

pub fn value_row(ui: &mut egui::Ui, label: &str, value: &str) {
    value_row_with_note(ui, label, value, "");
}

pub fn value_row_with_note(ui: &mut egui::Ui, label: &str, value: &str, note: &str) {
    let label_width = (ui.available_width() * 0.48).min(180.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.allocate_ui_with_layout(egui::vec2(label_width, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_min_width(label_width);
            ui.add(egui::Label::new(egui::RichText::new(label).weak()).wrap());
        });
        ui.add(egui::Label::new(value).wrap().selectable(true));
        if !note.is_empty() {
            ui.add(egui::Label::new(egui::RichText::new(note).weak().size(12.0)).wrap().selectable(true));
        }
    });
}

/// Below this width [`form_row`] puts the caption above its control.
const FORM_STACK_WIDTH: f32 = 280.0;

pub fn form_row(ui: &mut egui::Ui, label: &str, control: impl FnOnce(&mut egui::Ui)) {
    if ui.available_width() < FORM_STACK_WIDTH {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.add(egui::Label::new(label).wrap());
            control(ui);
        });
    } else {
        let label_width = (ui.available_width() * 0.42).min(210.0);
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(label_width, 28.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_min_width(label_width);
                ui.add(egui::Label::new(label).wrap());
            });
            control(ui);
        });
    }
}

pub fn apply(context: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "icy-sans".into(),
        egui::FontData::from_static(include_bytes!("../../data/fonts/FiraSans-Regular.ttf")).into(),
    );
    fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "icy-sans".into());
    fonts.font_data.insert(
        BOLD_FAMILY.into(),
        egui::FontData::from_static(include_bytes!("../../data/fonts/FiraSans-SemiBold.ttf")).into(),
    );
    let mut bold = vec![BOLD_FAMILY.to_string()];
    bold.extend(fonts.families[&egui::FontFamily::Proportional].iter().cloned());
    fonts.families.insert(egui::FontFamily::Name(BOLD_FAMILY.into()), bold);
    context.set_fonts(fonts);
    context.all_styles_mut(|style| {
        style.text_styles.insert(TextStyle::Heading, FontId::proportional(17.0));
        style.text_styles.insert(TextStyle::Body, FontId::proportional(14.0));
        style.text_styles.insert(TextStyle::Button, FontId::proportional(14.0));
        style.text_styles.insert(TextStyle::Small, FontId::proportional(12.0));
        style.spacing.interact_size = egui::vec2(28.0, 30.0);
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.window_margin = egui::Margin::same(16);
        style.spacing.menu_margin = egui::Margin::same(6);
        style.visuals.window_corner_radius = CornerRadius::same(6);
        style.visuals.menu_corner_radius = CornerRadius::same(4);
        let dark = style.visuals.dark_mode;
        let (surface, input, control, hover, border, text, muted, accent) = if dark {
            (
                Color32::from_rgb(35, 36, 39),
                Color32::from_rgb(27, 28, 31),
                Color32::from_rgb(47, 49, 53),
                Color32::from_rgb(62, 65, 71),
                Color32::from_rgb(71, 74, 81),
                Color32::from_rgb(235, 237, 240),
                Color32::from_rgb(168, 174, 184),
                Color32::from_rgb(110, 180, 240),
            )
        } else {
            (
                Color32::from_rgb(250, 250, 252),
                Color32::WHITE,
                Color32::from_rgb(236, 238, 241),
                Color32::from_rgb(224, 231, 239),
                Color32::from_rgb(201, 206, 214),
                Color32::from_rgb(34, 38, 44),
                Color32::from_rgb(98, 107, 120),
                Color32::from_rgb(29, 102, 173),
            )
        };
        let visuals = &mut style.visuals;
        visuals.window_highlight_topmost = false;
        visuals.panel_fill = surface;
        visuals.window_fill = surface;
        visuals.extreme_bg_color = input;
        visuals.faint_bg_color = control;
        visuals.weak_text_color = Some(muted);
        visuals.window_stroke = Stroke::new(1.0, border);
        visuals.selection.bg_fill = if dark {
            Color32::from_rgb(43, 70, 96)
        } else {
            Color32::from_rgb(214, 231, 249)
        };
        visuals.selection.stroke = Stroke::new(1.0, accent);
        visuals.hyperlink_color = accent;
        for widget in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.corner_radius = CornerRadius::same(4);
            widget.fg_stroke = Stroke::new(1.0, text);
            widget.bg_stroke = Stroke::new(1.0, border);
            widget.expansion = 0.0;
        }
        visuals.widgets.noninteractive.bg_fill = surface;
        visuals.widgets.noninteractive.weak_bg_fill = surface;
        visuals.widgets.inactive.bg_fill = control;
        visuals.widgets.inactive.weak_bg_fill = control;
        visuals.widgets.hovered.bg_fill = hover;
        visuals.widgets.hovered.weak_bg_fill = hover;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, accent);
        visuals.widgets.active.bg_fill = visuals.selection.bg_fill;
        visuals.widgets.active.weak_bg_fill = visuals.selection.bg_fill;
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, accent);
        visuals.widgets.open = visuals.widgets.active;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(context: &egui::Context, page: &mut u8, events: Vec<egui::Event>) -> egui::FullOutput {
        context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 200.0))),
                events,
                ..Default::default()
            },
            |context| {
                egui::CentralPanel::default().show(context, |ui| {
                    ui.horizontal(|ui| {
                        tab(ui, page, 0, "First");
                        tab(ui, page, 1, "Second");
                    });
                });
            },
        )
    }

    fn label_center(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => Some(text.pos + text.galley.size() / 2.0),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing tab: {label}"))
    }

    /// The panel background also covers the point, so only tab-sized rectangles count.
    fn has_highlight(output: &egui::FullOutput, position: egui::Pos2) -> bool {
        output.shapes.iter().any(|shape| match &shape.shape {
            egui::Shape::Rect(rect) => rect.rect.contains(position) && rect.rect.height() <= 30.0,
            _ => false,
        })
    }

    fn click(position: egui::Pos2) -> Vec<Vec<egui::Event>> {
        let button = |pressed| egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        vec![vec![egui::Event::PointerMoved(position)], vec![button(true)], vec![button(false)]]
    }

    /// Renders a check box above a slider and returns their rectangles.
    fn controls(context: &egui::Context, events: Vec<egui::Event>, checked: &mut bool, value: &mut f32) -> (egui::Rect, egui::Rect) {
        let mut rects = (egui::Rect::NOTHING, egui::Rect::NOTHING);
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 200.0))),
                events,
                ..Default::default()
            },
            |context| {
                egui::CentralPanel::default().show(context, |ui| {
                    rects.0 = check(ui, checked).rect;
                    rects.1 = slider(ui, value, 0.0..=100.0, 200.0).rect;
                });
            },
        );
        rects
    }

    #[test]
    fn check_toggles_and_slider_follows_the_pointer() {
        let context = egui::Context::default();
        let (mut checked, mut value) = (false, 0.0f32);
        let (check_rect, slider_rect) = controls(&context, vec![], &mut checked, &mut value);
        for events in click(check_rect.center()) {
            controls(&context, events, &mut checked, &mut value);
        }
        assert!(checked, "clicking the box must set it");
        for events in click(egui::pos2(slider_rect.right(), slider_rect.center().y)) {
            controls(&context, events, &mut checked, &mut value);
        }
        assert_eq!(value, 100.0, "clicking the end of the track must select the maximum");
        for events in click(slider_rect.center()) {
            controls(&context, events, &mut checked, &mut value);
        }
        assert!((value - 50.0).abs() < 0.5, "the middle of the track must select the midpoint, got {value}");
    }

    #[test]
    fn tabs_highlight_on_hover_and_switch_on_click() {
        let context = egui::Context::default();
        let mut page = 0u8;
        let output = render(&context, &mut page, vec![]);
        let second = label_center(&output, "Second");
        assert!(!has_highlight(&output, second), "an untouched tab must stay flat");

        render(&context, &mut page, vec![egui::Event::PointerMoved(second)]);
        let output = render(&context, &mut page, vec![]);
        assert!(has_highlight(&output, second), "hovering must draw a highlight");
        assert_eq!(page, 0, "hovering must not switch the page");

        let button = |pressed| egui::Event::PointerButton {
            pos: second,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        render(&context, &mut page, vec![button(true)]);
        render(&context, &mut page, vec![button(false)]);
        assert_eq!(page, 1, "clicking a tab must select it");
    }
}
