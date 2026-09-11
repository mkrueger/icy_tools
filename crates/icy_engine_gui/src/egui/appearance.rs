use ::egui::{self, Color32, CornerRadius, FontId, Stroke, TextStyle};

pub const PRIMARY: Color32 = Color32::from_rgb(32, 100, 160);

/// Shared padding so every single-line input ends up the same height.
pub const FIELD_MARGIN: egui::Vec2 = egui::Vec2::new(8.0, 6.0);

pub fn text_edit(value: &mut String) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(value).margin(FIELD_MARGIN)
}

pub fn dialog_frame(context: &egui::Context) -> egui::Frame {
    egui::Frame::window(&context.style()).inner_margin(16)
}

pub fn dialog_header(ui: &mut egui::Ui, title: &str) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        let width = ui.available_width() - 32.0;
        ui.allocate_ui_with_layout(egui::vec2(width, 28.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(width);
            ui.add(egui::Label::new(egui::RichText::new(title).strong().size(17.0)).wrap());
        });
        close = ui
            .add_sized([24.0, 24.0], egui::Button::new(egui::RichText::new("\u{00d7}").size(20.0)).frame(false))
            .on_hover_text(i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "dialog-close-button"))
            .clicked();
    });
    ui.separator();
    close
}

pub fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(title).strong().size(14.0));
    ui.add_space(2.0);
}

pub struct DialogResponse {
    pub closed: bool,
}

pub fn chip(ui: &mut egui::Ui, label: &str, color: Color32) {
    let font = FontId::proportional(10.0);
    let width = ui.fonts_mut(|fonts| fonts.layout_no_wrap(label.to_owned(), font.clone(), color).size().x);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width + 10.0, 15.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 3.0, color.gamma_multiply(0.22));
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, font, color);
}

pub fn metric_tile(ui: &mut egui::Ui, label: &str, value: &str) {
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.set_min_width(64.0);
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

/// Shared modal shell so every dialog gets the same frame, header, body and action row.
pub struct Dialog {
    id: &'static str,
    title: String,
    max_width: f32,
    max_height: f32,
    scroll: bool,
}

/// Body and action row of a [`Dialog`]. Call `content` first, then `actions`.
pub struct DialogUi<'a> {
    ui: &'a mut egui::Ui,
    id: &'static str,
    body_height: f32,
    scroll: bool,
}

impl DialogUi<'_> {
    pub fn content(&mut self, add: impl FnOnce(&mut egui::Ui)) {
        if self.scroll {
            egui::ScrollArea::vertical()
                .id_salt((self.id, "dialog-body"))
                .auto_shrink([false, true])
                .min_scrolled_height(0.0)
                .max_height(self.body_height)
                .show(self.ui, add);
        } else {
            add(self.ui);
        }
    }

    /// Laid out right to left, so add the primary button first.
    pub fn actions(&mut self, add: impl FnOnce(&mut egui::Ui)) {
        self.ui.separator();
        self.ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
        });
    }
}

impl Dialog {
    pub fn new(id: &'static str, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            max_width: 620.0,
            max_height: 560.0,
            scroll: true,
        }
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = width;
        self
    }

    pub fn scroll(mut self, scroll: bool) -> Self {
        self.scroll = scroll;
        self
    }

    pub fn show(self, context: &egui::Context, body: impl FnOnce(&mut DialogUi)) -> DialogResponse {
        let mut closed = false;
        egui::Modal::new(egui::Id::new(self.id)).frame(dialog_frame(context)).show(context, |ui| {
            let width = (context.content_rect().width() - 48.0).clamp(240.0, self.max_width);
            let height = (context.content_rect().height() - 64.0).clamp(140.0, self.max_height);
            ui.set_width(width);
            closed |= dialog_header(ui, &self.title);
            let mut dialog = DialogUi {
                ui,
                id: self.id,
                body_height: (height - 110.0).max(60.0),
                scroll: self.scroll,
            };
            body(&mut dialog);
        });
        DialogResponse { closed }
    }
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
        // The slider's value box grows with the number of digits, so cap the decimals and
        // reserve room for it; otherwise a long value widens the whole dialog.
        let width = ui.available_width();
        ui.spacing_mut().slider_width = (width - 96.0).max(40.0);
        ui.add(egui::Slider::new(value, range).max_decimals(2));
    });
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

pub fn form_row(ui: &mut egui::Ui, label: &str, control: impl FnOnce(&mut egui::Ui)) {
    if ui.available_width() < 280.0 {
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

pub fn tab<Value: PartialEq>(ui: &mut egui::Ui, current: &mut Value, value: Value, label: &str) {
    let selected = *current == value;
    // Laid out with PLACEHOLDER so the colour can follow the interaction state without re-layout.
    let galley = ui.fonts_mut(|fonts| fonts.layout_no_wrap(label.to_owned(), FontId::proportional(14.0), Color32::PLACEHOLDER));
    let (rect, response) = ui.allocate_exact_size(egui::vec2(galley.size().x + 20.0, 30.0), egui::Sense::click());
    let accent = ui.visuals().selection.stroke.color;
    let pressed = response.is_pointer_button_down_on();
    let hovered = response.hovered();
    if pressed || hovered {
        let fill = if pressed {
            ui.visuals().widgets.active.bg_fill
        } else {
            ui.visuals().widgets.hovered.bg_fill
        };
        ui.painter().rect_filled(rect.shrink2(egui::vec2(0.0, 2.0)), 4.0, fill);
    }
    let color = if selected || pressed {
        accent
    } else if hovered {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    if selected {
        ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(2.0, accent));
    } else if hovered {
        ui.painter()
            .line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, ui.visuals().weak_text_color()));
    }
    if response.clicked() {
        *current = value;
        // The app only repaints on events, so make the page switch show up right away.
        ui.ctx().request_repaint();
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand);
}

pub fn apply(context: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "icy-sans".into(),
        egui::FontData::from_static(include_bytes!("../../data/fonts/FiraSans-Regular.ttf")).into(),
    );
    fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "icy-sans".into());
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
