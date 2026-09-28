//! RIP buttons: a button style (`|1B`) with the button instance (`|1U`) edited as one object,
//! and the button style dialog modelled on JDraw's button selector.

use eframe::egui::{self, Color32};
use icy_draw::{fl, rip_document::RipDocument};
use icy_engine::Screen;
use icy_engine_gui::egui::appearance::{self, DialogButton, DialogSize, labels};
use icy_parser_core::RipCommand;

// `|1B` flags
pub const CLIPBOARD: u16 = 1;
pub const INVERT: u16 = 2;
pub const RESET: u16 = 4;
pub const CHISEL: u16 = 8;
pub const RECESSED: u16 = 16;
pub const SHADOW: u16 = 32;
pub const STAMP: u16 = 64;
pub const ICON: u16 = 128;
pub const PLAIN: u16 = 256;
pub const BEVEL: u16 = 512;
pub const MOUSE: u16 = 1024;
pub const UNDERLINE_HOTKEY: u16 = 2048;
pub const HOT_ICONS: u16 = 4096;
pub const CENTER_VERTICALLY: u16 = 8192;
pub const RADIO_GROUP: u16 = 16384;
pub const SUNKEN: u16 = 32768;
const KIND_FLAGS: u16 = CLIPBOARD | ICON | PLAIN;

// `|1B` flags2
pub const CHECKBOX: u16 = 1;
pub const HIGHLIGHT_HOTKEY: u16 = 2;
pub const EXPLODE: u16 = 4;
pub const LEFT_JUSTIFY: u16 = 8;
pub const RIGHT_JUSTIFY: u16 = 16;
const JUSTIFY_FLAGS: u16 = LEFT_JUSTIFY | RIGHT_JUSTIFY;

/// The three button types of RIPscrip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    Plain,
    Icon,
    Clipboard,
}

impl ButtonKind {
    pub const ALL: [Self; 3] = [Self::Plain, Self::Icon, Self::Clipboard];

    pub fn label(self) -> String {
        match self {
            Self::Plain => fl!("rip-button-plain"),
            Self::Icon => fl!("rip-button-icon"),
            Self::Clipboard => fl!("rip-button-clipboard"),
        }
    }

    pub fn tooltip(self) -> String {
        match self {
            Self::Plain => fl!("rip-button-plain-tooltip"),
            Self::Icon => fl!("rip-button-icon-tooltip"),
            Self::Clipboard => fl!("rip-button-clipboard-tooltip"),
        }
    }

    fn flag(self) -> u16 {
        match self {
            Self::Plain => PLAIN,
            Self::Icon => ICON,
            Self::Clipboard => CLIPBOARD,
        }
    }
}

/// Everything the button style dialog edits: the `|1B` style, the `|1U` instance and the
/// label font.
#[derive(Clone, Debug, PartialEq)]
pub struct ButtonOptions {
    pub kind: ButtonKind,
    pub label: String,
    pub host_command: String,
    pub icon_file: String,
    /// ASCII code of the hot key, 0 for none.
    pub hotkey: u16,
    pub group: u16,
    pub label_color: u16,
    pub shadow_color: u16,
    pub bright: u16,
    pub dark: u16,
    pub surface: u16,
    pub underline_color: u16,
    pub corner_color: u16,
    pub font: u16,
    pub font_size: u16,
    /// 0 above, 1 left, 2 center, 3 right, 4 below.
    pub orientation: u16,
    pub bevel: u16,
    /// Size used when the button is placed with a click instead of dragged.
    pub width: u16,
    pub height: u16,
    /// Effect flags without the button type bits.
    pub flags: u16,
    /// Secondary flags without the justification bits.
    pub flags2: u16,
    /// -1 left, 0 center, 1 right.
    pub justify: i8,
    /// Draws the button in its selected state (check boxes and radio groups).
    pub selected: bool,
}

impl Default for ButtonOptions {
    /// JDraw's defaults: a beveled, chiseled, sunken and recessed plain button with shadow.
    fn default() -> Self {
        Self {
            kind: ButtonKind::Plain,
            label: String::new(),
            host_command: String::new(),
            icon_file: String::new(),
            hotkey: 0,
            group: 0,
            label_color: 2,
            shadow_color: 0,
            bright: 15,
            dark: 8,
            surface: 7,
            underline_color: 4,
            corner_color: 7,
            font: 0,
            font_size: 1,
            orientation: 2,
            bevel: 3,
            width: 80,
            height: 30,
            flags: BEVEL | CHISEL | SUNKEN | RECESSED | SHADOW,
            flags2: 0,
            justify: 0,
            selected: false,
        }
    }
}

impl ButtonOptions {
    pub fn style(&self) -> RipCommand {
        let justify = match self.justify {
            -1 => LEFT_JUSTIFY,
            1 => RIGHT_JUSTIFY,
            _ => 0,
        };
        RipCommand::ButtonStyle {
            wid: self.width,
            hgt: self.height,
            orient: self.orientation,
            flags: (self.flags & !KIND_FLAGS) | self.kind.flag(),
            bevsize: self.bevel,
            dfore: self.label_color,
            dback: self.shadow_color,
            bright: self.bright,
            dark: self.dark,
            surface: self.surface,
            grp_no: self.group,
            flags2: (self.flags2 & !JUSTIFY_FLAGS) | justify,
            uline_col: self.underline_color,
            corner_col: self.corner_color,
            res: 0,
        }
    }

    pub fn font_style(&self) -> RipCommand {
        RipCommand::FontStyle {
            font: self.font,
            direction: 0,
            size: self.font_size,
            res: 0,
        }
    }

    pub fn text(&self) -> String {
        let icon = if self.kind == ButtonKind::Icon { self.icon_file.as_str() } else { "" };
        format!("{icon}<>{}<>{}", self.label, self.host_command)
    }

    /// The button instance. Without a dragged rectangle (`to` is `None`) the style's size, or
    /// the icon's or clipboard's size, is used.
    pub fn button(&self, from: (u16, u16), to: Option<(u16, u16)>) -> RipCommand {
        let (x0, y0, x1, y1) = match to {
            Some(to) => (from.0.min(to.0), from.1.min(to.1), from.0.max(to.0), from.1.max(to.1)),
            None => (from.0, from.1, 0, 0),
        };
        RipCommand::Button {
            x0,
            y0,
            x1,
            y1,
            hotkey: self.hotkey,
            flags: u16::from(self.selected),
            res: 0,
            text: self.text(),
        }
    }

    /// Why the button cannot be placed, if anything is missing or invalid.
    pub fn problem(&self) -> Option<String> {
        if [&self.label, &self.host_command, &self.icon_file].iter().any(|text| text.contains("<>")) {
            return Some(fl!("rip-editor-button-label-delimiter"));
        }
        if self.kind == ButtonKind::Icon && self.icon_file.trim().is_empty() {
            return Some(fl!("rip-button-icon-required"));
        }
        if self.kind == ButtonKind::Plain && self.label.is_empty() {
            return Some(fl!("rip-editor-label-required"));
        }
        None
    }

    /// Reads a button back from its instance and the style in effect for it.
    pub fn from_commands(style: Option<&RipCommand>, font: Option<&RipCommand>, button: &RipCommand) -> Self {
        let mut options = Self::default();
        if let Some(RipCommand::ButtonStyle {
            wid,
            hgt,
            orient,
            flags,
            bevsize,
            dfore,
            dback,
            bright,
            dark,
            surface,
            grp_no,
            flags2,
            uline_col,
            corner_col,
            ..
        }) = style
        {
            options.width = *wid;
            options.height = *hgt;
            options.orientation = *orient;
            options.kind = if flags & ICON != 0 {
                ButtonKind::Icon
            } else if flags & CLIPBOARD != 0 {
                ButtonKind::Clipboard
            } else {
                ButtonKind::Plain
            };
            options.flags = flags & !KIND_FLAGS;
            options.bevel = *bevsize;
            options.label_color = *dfore;
            options.shadow_color = *dback;
            options.bright = *bright;
            options.dark = *dark;
            options.surface = *surface;
            options.group = *grp_no;
            options.justify = if flags2 & LEFT_JUSTIFY != 0 {
                -1
            } else if flags2 & RIGHT_JUSTIFY != 0 {
                1
            } else {
                0
            };
            options.flags2 = flags2 & !JUSTIFY_FLAGS;
            options.underline_color = *uline_col;
            options.corner_color = *corner_col;
        }
        if let Some(RipCommand::FontStyle { font, size, .. }) = font {
            options.font = *font;
            options.font_size = *size;
        }
        if let RipCommand::Button { hotkey, flags, text, .. } = button {
            let mut parts = text.splitn(3, "<>");
            let (icon, label, host) = match (parts.next(), parts.next(), parts.next()) {
                (Some(icon), Some(label), host) => (icon, label, host.unwrap_or_default()),
                (Some(label), None, _) => ("", label, ""),
                _ => ("", "", ""),
            };
            options.icon_file = icon.to_owned();
            options.label = label.to_owned();
            options.host_command = host.to_owned();
            options.hotkey = *hotkey;
            options.selected = flags & 1 != 0;
        }
        options
    }
}

/// RIP's BGI fonts, in `|Y` order.
pub fn font_names() -> [String; 11] {
    [
        fl!("rip-font-default"),
        "Triplex".into(),
        "Small".into(),
        "Sans Serif".into(),
        "Gothic".into(),
        "Script".into(),
        "Simplex".into(),
        "Triplex Script".into(),
        "Complex".into(),
        "European".into(),
        "Bold".into(),
    ]
}

pub fn color32(palette: &icy_engine::Palette, color: u16) -> Color32 {
    let (red, green, blue) = palette.rgb(u32::from(color));
    Color32::from_rgb(red, green, blue)
}

/// A swatch that opens the 16 color palette.
pub fn color_picker(ui: &mut egui::Ui, id: &str, palette: &icy_engine::Palette, value: &mut u16) -> bool {
    let mut changed = false;
    let response = ui.add(
        egui::Button::new("")
            .fill(color32(palette, *value))
            .min_size(egui::vec2(28.0, 18.0))
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke),
    );
    egui::Popup::menu(&response).id(egui::Id::new(("rip-color", id))).show(|ui| {
        egui::Grid::new(("rip-color-grid", id)).spacing(egui::vec2(3.0, 3.0)).show(ui, |ui| {
            for color in 0..16u16 {
                let selected = *value == color;
                let swatch = egui::Button::new("")
                    .fill(color32(palette, color))
                    .min_size(egui::vec2(22.0, 22.0))
                    .stroke(egui::Stroke::new(if selected { 2.0 } else { 0.5 }, ui.visuals().strong_text_color()));
                if ui.add(swatch).on_hover_text(color.to_string()).clicked() {
                    *value = color;
                    changed = true;
                    ui.close();
                }
                if color % 8 == 7 {
                    ui.end_row();
                }
            }
        });
    });
    changed
}

/// Which commands an existing button occupies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonTarget {
    pub font: Option<usize>,
    pub style: Option<usize>,
    pub button: usize,
}

impl ButtonTarget {
    /// The style and font commands directly before the button, as the editor writes them.
    pub fn find(commands: &[RipCommand], button: usize) -> Option<Self> {
        if !matches!(commands.get(button), Some(RipCommand::Button { .. })) {
            return None;
        }
        let style = button.checked_sub(1).filter(|index| matches!(commands[*index], RipCommand::ButtonStyle { .. }));
        let font = style
            .and_then(|style| style.checked_sub(1))
            .filter(|index| matches!(commands[*index], RipCommand::FontStyle { .. }));
        Some(Self { font, style, button })
    }

    /// The button whose style or font command is at `index`, so either selects the object.
    pub fn owning(commands: &[RipCommand], index: usize) -> Option<Self> {
        (index..(index + 3).min(commands.len()))
            .filter_map(|button| Self::find(commands, button))
            .find(|target| target.button == index || target.style == Some(index) || target.font == Some(index))
    }
}

pub struct ButtonDialog {
    pub options: ButtonOptions,
    /// The button being edited; `None` edits the settings for new buttons.
    pub target: Option<ButtonTarget>,
    preview: Option<(ButtonOptions, egui::TextureHandle)>,
    page: Page,
}

pub enum DialogResult {
    Open,
    Cancel,
    Apply(ButtonOptions),
}

impl ButtonDialog {
    pub fn new(options: ButtonOptions, target: Option<ButtonTarget>) -> Self {
        Self {
            options,
            target,
            preview: None,
            page: Page::Button,
        }
    }

    /// Renders the button with the RIP engine, cropped around it.
    fn preview(&mut self, context: &egui::Context, palette: &icy_engine::Palette) -> egui::TextureHandle {
        if let Some((options, texture)) = &self.preview {
            if *options == self.options {
                return texture.clone();
            }
        }
        let (width, height) = (self.options.width.clamp(24, 200), self.options.height.clamp(12, 80));
        let from = (320 - width / 2, 175 - height / 2);
        // Bevels and labels above or below the button extend beyond its rectangle.
        let margin = self.options.bevel + 8;
        let area = (from.0 - margin, from.1 - margin, from.0 + width + margin, from.1 + height + margin);
        let mut options = self.options.clone();
        if options.label.is_empty() {
            options.label = fl!("rip-button-sample");
        }
        let commands = [
            RipCommand::FontStyle {
                font: options.font,
                direction: 0,
                size: options.font_size,
                res: 0,
            },
            options.style(),
            options.button(from, Some((from.0 + width - 1, from.1 + height - 1))),
        ];
        let mut image = egui::ColorImage::filled([(area.2 - area.0) as usize, (area.3 - area.1) as usize], Color32::BLACK);
        if let Ok(screen) = RipDocument::render(&commands) {
            let pixels = screen.screen();
            let stride = screen.pixel_size.width as usize;
            for y in area.1..area.3 {
                for x in area.0..area.2 {
                    if let Some(&index) = pixels.get(y as usize * stride + x as usize) {
                        image[((x - area.0) as usize, (y - area.1) as usize)] = color32(palette, u16::from(index));
                    }
                }
            }
        }
        let texture = context.load_texture("rip-button-preview", image, egui::TextureOptions::NEAREST);
        self.preview = Some((self.options.clone(), texture.clone()));
        texture
    }

    pub fn show(&mut self, context: &egui::Context, palette: &icy_engine::Palette) -> DialogResult {
        #[derive(Clone, Copy)]
        enum Action {
            Cancel,
            Apply,
        }
        let texture = self.preview(context, palette);
        let problem = self.options.problem();
        let pages = [
            (Page::Button, fl!("rip-button-group-button")),
            (Page::Appearance, fl!("rip-button-group-appearance")),
            (Page::Behavior, fl!("rip-button-group-behavior")),
        ];
        let page = &mut self.page;
        let options = &mut self.options;
        let response = appearance::Dialog::new("rip-button-style")
            .title(fl!("rip-button-style-title"))
            .size(DialogSize::Width(720.0))
            .show(context, |dialog| {
                dialog.tabs(page, &pages);
                dialog.content(|ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 112.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 6, Color32::BLACK);
                    // Whole pixel steps keep the RIP pixels crisp.
                    let fit = (rect.width() / texture.size_vec2().x).min(rect.height() / texture.size_vec2().y);
                    let size = texture.size_vec2() * fit.floor().clamp(1.0, 3.0);
                    ui.painter().image(
                        texture.id(),
                        egui::Rect::from_center_size(rect.center(), size),
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    ui.add_space(10.0);
                    match page {
                        Page::Button => button_page(ui, options),
                        Page::Appearance => appearance_page(ui, options, palette),
                        Page::Behavior => behavior_page(ui, options),
                    }
                    if let Some(problem) = &problem {
                        ui.add_space(6.0);
                        ui.colored_label(ui.visuals().error_fg_color, problem);
                    }
                });
                dialog.buttons([
                    DialogButton::cancel(labels::cancel(), Action::Cancel),
                    DialogButton::primary(labels::ok(), Action::Apply).enabled(problem.is_none()),
                ]);
            });
        match response.action {
            Some(Action::Apply) => DialogResult::Apply(self.options.clone()),
            Some(Action::Cancel) => DialogResult::Cancel,
            None if response.dismissed => DialogResult::Cancel,
            None => DialogResult::Open,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Button,
    Appearance,
    Behavior,
}

fn button_page(ui: &mut egui::Ui, options: &mut ButtonOptions) {
    ui.horizontal(|ui| {
        ui.label(fl!("rip-button-type"));
        let kinds = ButtonKind::ALL.map(|kind| (kind, kind.label(), kind.tooltip()));
        crate::widgets::segmented(ui, &mut options.kind, &kinds);
    });
    ui.add_space(8.0);
    ui.columns(2, |columns| {
        let ui = &mut columns[0];
        appearance::group(ui, &fl!("rip-button-group-button"), |ui| {
            appearance::form_row(ui, &fl!("rip-editor-label"), |ui| {
                ui.add(appearance::text_edit(&mut options.label).desired_width(f32::INFINITY));
            });
            appearance::form_row(ui, &fl!("rip-button-host-command"), |ui| {
                ui.add(
                    appearance::text_edit(&mut options.host_command)
                        .hint_text(fl!("rip-button-host-command-hint"))
                        .desired_width(f32::INFINITY),
                );
            });
            if options.kind == ButtonKind::Icon {
                appearance::form_row(ui, &fl!("rip-button-icon-file"), |ui| {
                    ui.add(
                        appearance::text_edit(&mut options.icon_file)
                            .hint_text("BUTTON.ICN")
                            .desired_width(f32::INFINITY),
                    );
                });
            }
            appearance::form_row(ui, &fl!("rip-button-hotkey"), |ui| {
                let mut key = char::from_u32(u32::from(options.hotkey))
                    .filter(|_| options.hotkey > 0)
                    .map(String::from)
                    .unwrap_or_default();
                if ui.add(appearance::text_edit(&mut key).char_limit(1).desired_width(28.0)).changed() {
                    options.hotkey = key.chars().next().filter(char::is_ascii).map_or(0, |ch| ch as u16);
                }
                ui.label(fl!("rip-button-group-number"));
                ui.add(egui::DragValue::new(&mut options.group).range(0..=35));
            });
        });
        let ui = &mut columns[1];
        appearance::group(ui, &fl!("rip-button-group-layout"), |ui| {
            let fonts = font_names();
            appearance::combo_row(ui, &fl!("rip-font"), fonts[options.font.min(10) as usize].clone(), |ui| {
                for (index, name) in fonts.iter().enumerate() {
                    ui.selectable_value(&mut options.font, index as u16, name);
                }
            });
            appearance::form_row(ui, &fl!("rip-font-size"), |ui| {
                ui.add(egui::DragValue::new(&mut options.font_size).range(1..=10));
            });
            let orientations = [
                fl!("rip-button-above"),
                fl!("rip-button-left"),
                fl!("rip-button-center"),
                fl!("rip-button-right"),
                fl!("rip-button-below"),
            ];
            appearance::combo_row(
                ui,
                &fl!("rip-button-orientation"),
                orientations[options.orientation.min(4) as usize].clone(),
                |ui| {
                    for (index, name) in orientations.iter().enumerate() {
                        ui.selectable_value(&mut options.orientation, index as u16, name);
                    }
                },
            );
            let justify = [fl!("rip-button-left"), fl!("rip-button-center"), fl!("rip-button-right")];
            appearance::combo_row(ui, &fl!("rip-button-justify"), justify[(options.justify + 1) as usize].clone(), |ui| {
                for (value, name) in [-1, 0, 1].into_iter().zip(&justify) {
                    ui.selectable_value(&mut options.justify, value, name);
                }
            });
            appearance::form_row(ui, &fl!("rip-button-size"), |ui| {
                let tooltip = fl!("rip-button-size-tooltip");
                ui.add(egui::DragValue::new(&mut options.width).range(0..=640).prefix("W "))
                    .on_hover_text(&tooltip);
                ui.add(egui::DragValue::new(&mut options.height).range(0..=350).prefix("H "))
                    .on_hover_text(&tooltip);
            });
        });
    });
}

fn appearance_page(ui: &mut egui::Ui, options: &mut ButtonOptions, palette: &icy_engine::Palette) {
    ui.columns(2, |columns| {
        let ui = &mut columns[0];
        appearance::group(ui, &fl!("rip-button-group-colors"), |ui| {
            let colors: [(String, &mut u16); 7] = [
                (fl!("rip-button-label-color"), &mut options.label_color),
                (fl!("rip-button-shadow-color"), &mut options.shadow_color),
                (fl!("rip-button-bright-color"), &mut options.bright),
                (fl!("rip-button-dark-color"), &mut options.dark),
                (fl!("rip-button-surface-color"), &mut options.surface),
                (fl!("rip-button-underline-color"), &mut options.underline_color),
                (fl!("rip-button-corner-color"), &mut options.corner_color),
            ];
            egui::Grid::new("rip-button-colors")
                .num_columns(4)
                .spacing(egui::vec2(10.0, 8.0))
                .show(ui, |ui| {
                    for (index, (label, value)) in colors.into_iter().enumerate() {
                        ui.label(&label);
                        color_picker(ui, &label, palette, value);
                        if index % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
        });
        let ui = &mut columns[1];
        appearance::group(ui, &fl!("rip-button-group-effects"), |ui| {
            appearance::form_row(ui, &fl!("rip-button-bevel-size"), |ui| {
                ui.add(egui::DragValue::new(&mut options.bevel).range(0..=20));
            });
            let flags: [(u16, String); 8] = [
                (BEVEL, fl!("rip-button-bevel")),
                (CHISEL, fl!("rip-button-chisel")),
                (SUNKEN, fl!("rip-button-sunken")),
                (RECESSED, fl!("rip-button-recessed")),
                (SHADOW, fl!("rip-button-shadow")),
                (UNDERLINE_HOTKEY, fl!("rip-button-underline-hotkey")),
                (CENTER_VERTICALLY, fl!("rip-button-center-vertically")),
                (HOT_ICONS, fl!("rip-button-hot-icons")),
            ];
            flag_grid(ui, &mut options.flags, &flags);
            let flags2: [(u16, String); 2] = [(HIGHLIGHT_HOTKEY, fl!("rip-button-highlight-hotkey")), (EXPLODE, fl!("rip-button-explode"))];
            flag_grid(ui, &mut options.flags2, &flags2);
        });
    });
}

fn behavior_page(ui: &mut egui::Ui, options: &mut ButtonOptions) {
    appearance::group(ui, &fl!("rip-button-group-behavior"), |ui| {
        let flags: [(u16, String); 5] = [
            (MOUSE, fl!("rip-button-mouse")),
            (INVERT, fl!("rip-button-invert")),
            (RESET, fl!("rip-button-reset")),
            (RADIO_GROUP, fl!("rip-button-radio")),
            (STAMP, fl!("rip-button-stamp")),
        ];
        flag_grid(ui, &mut options.flags, &flags);
        ui.horizontal(|ui| {
            let mut checkbox = options.flags2 & CHECKBOX != 0;
            if ui.checkbox(&mut checkbox, fl!("rip-button-checkbox")).changed() {
                options.flags2 = if checkbox { options.flags2 | CHECKBOX } else { options.flags2 & !CHECKBOX };
            }
            ui.checkbox(&mut options.selected, fl!("rip-button-selected"));
        });
    });
}

fn flag_grid(ui: &mut egui::Ui, value: &mut u16, flags: &[(u16, String)]) {
    let id = ui.next_auto_id();
    egui::Grid::new(id).num_columns(2).spacing(egui::vec2(16.0, 6.0)).show(ui, |ui| {
        for (index, (flag, label)) in flags.iter().enumerate() {
            let mut on = *value & flag != 0;
            if ui.checkbox(&mut on, label).changed() {
                *value = if on { *value | flag } else { *value & !flag };
            }
            if index % 2 == 1 {
                ui.end_row();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_objects_round_trip_through_their_commands() {
        let mut options = ButtonOptions::default();
        options.kind = ButtonKind::Icon;
        options.icon_file = "HOME.ICN".into();
        options.label = "Home".into();
        options.host_command = "^mHOME^m".into();
        options.hotkey = u16::from(b'H');
        options.justify = 1;
        options.flags |= MOUSE;
        options.flags2 |= CHECKBOX;
        options.selected = true;
        options.font = 3;
        options.font_size = 2;
        let style = options.style();
        let button = options.button((10, 20), Some((5, 60)));
        assert!(matches!(
            button,
            RipCommand::Button {
                x0: 5,
                y0: 20,
                x1: 10,
                y1: 60,
                flags: 1,
                ..
            }
        ));
        assert!(matches!(&button, RipCommand::Button { text, .. } if text == "HOME.ICN<>Home<>^mHOME^m"));
        assert!(
            matches!(style, RipCommand::ButtonStyle { flags, flags2, .. } if flags & ICON != 0 && flags & PLAIN == 0 && flags2 == CHECKBOX | RIGHT_JUSTIFY)
        );
        assert_eq!(ButtonOptions::from_commands(Some(&style), Some(&options.font_style()), &button), options);
        assert!(
            matches!(options.button((1, 2), None), RipCommand::Button { x1: 0, y1: 0, .. }),
            "clicks use the style size"
        );
    }

    #[test]
    fn button_problems_are_reported() {
        let mut options = ButtonOptions::default();
        assert!(options.problem().is_some(), "plain buttons need a label");
        options.label = "OK".into();
        assert!(options.problem().is_none());
        options.host_command = "a<>b".into();
        assert!(options.problem().is_some());
        options.host_command.clear();
        options.kind = ButtonKind::Icon;
        assert!(options.problem().is_some(), "icon buttons need an icon file");
        options.kind = ButtonKind::Clipboard;
        options.label.clear();
        assert!(options.problem().is_none(), "clipboard buttons show the clipboard image");
    }

    #[test]
    fn a_button_is_selected_through_its_style_and_font() {
        let options = ButtonOptions {
            label: "OK".into(),
            ..Default::default()
        };
        let commands = vec![
            RipCommand::Color { c: 1 },
            options.font_style(),
            options.style(),
            options.button((1, 1), Some((50, 20))),
        ];
        let target = ButtonTarget {
            font: Some(1),
            style: Some(2),
            button: 3,
        };
        for index in 1..4 {
            assert_eq!(ButtonTarget::owning(&commands, index), Some(target));
        }
        assert_eq!(ButtonTarget::owning(&commands, 0), None);
    }
}
