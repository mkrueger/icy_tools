use eframe::egui::{self, Color32, CornerRadius, FontId, Stroke, TextStyle};

pub const PRIMARY: Color32 = Color32::from_rgb(32, 100, 160);

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
            .on_hover_text(tr!("egui-close"))
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
        ui.spacing_mut().slider_width = (ui.available_width() - 60.0).max(48.0);
        ui.add(egui::Slider::new(value, range));
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
    let color = if selected {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals().weak_text_color()
    };
    let width = ui.fonts_mut(|fonts| fonts.layout_no_wrap(label.to_owned(), FontId::proportional(14.0), color).size().x) + 20.0;
    let response = ui.add(
        egui::Button::new(egui::RichText::new(label).color(color))
            .frame(false)
            .min_size(egui::vec2(width, 30.0)),
    );
    if selected {
        ui.painter()
            .line_segment([response.rect.left_bottom(), response.rect.right_bottom()], Stroke::new(2.0, color));
    }
    if response.clicked() {
        *current = value;
    }
}

pub fn apply(context: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "icy-sans".into(),
        egui::FontData::from_static(include_bytes!("../../../data/fonts/FiraSans-Regular.ttf")).into(),
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
