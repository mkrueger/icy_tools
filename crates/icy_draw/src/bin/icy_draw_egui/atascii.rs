//! The ATASCII editor for Atari 8-bit text screens.
//!
//! It shares the canvas, layers and undo with the ANSI editor, but has panels of its own: the
//! screen has one font and two colors, and inverse video is the upper half of the character
//! set, so there are no per-character colors or font slots to offer.

use eframe::egui::{self, Color32};
use icy_draw::{
    brush::BrushPrimaryMode,
    fl,
    screen_profile::{AtasciiMode, ScreenProfile},
};
use icy_engine::TextPane;
use icy_engine_edit::tools::{Tool, ToolPair};
use icy_engine_gui::egui::appearance::PRIMARY;

use super::{
    chrome,
    retro::{PipetteReturn, FKEYS},
    widgets, DrawApp, FileAction,
};

/// The tools that make sense on a character screen without per-character colors.
const TOOLS: [&[ToolPair]; 3] = [
    &[ToolPair::single(Tool::Click), ToolPair::single(Tool::Select)],
    &[
        ToolPair::single(Tool::Pencil),
        ToolPair::single(Tool::Line),
        ToolPair::new(Tool::RectangleOutline, Tool::RectangleFilled),
        ToolPair::new(Tool::EllipseOutline, Tool::EllipseFilled),
        ToolPair::single(Tool::Fill),
    ],
    &[ToolPair::single(Tool::Pipette)],
];

/// Characters on the function keys, a set per purpose: lines, blocks, diagonals and symbols.
/// Codes of 128 and above are inverse characters.
pub const FKEY_SETS: [[u8; 12]; 4] = [
    [0x11, 0x12, 0x05, 0x7C, 0x01, 0x13, 0x04, 0x1A, 0x18, 0x03, 0x17, 0x14],
    [0x09, 0x0F, 0x0B, 0x0C, 0x15, 0x19, 0x95, 0x99, 0xA0, 0x0E, 0x0D, 0x16],
    [0x06, 0x07, 0x08, 0x0A, 0x88, 0x8A, 0x02, 0x82, 0x00, 0x10, 0x60, 0x7B],
    [0x1C, 0x1D, 0x1E, 0x1F, 0x7E, 0x7F, 0x7D, 0x1B, 0x9B, 0xFD, 0xFE, 0xFF],
];

/// What the ATASCII editor remembers beside the document.
pub struct AtasciiEditor {
    /// The character the drawing tools paint, inverse from 128.
    pub brush: u8,
    pub fkey_set: usize,
    /// The background as an Atari color: hue (0-15) and luminance (0-14, even).
    pub background: (u8, u8),
    /// The luminance of the text, which has the background's hue.
    pub text_luminance: u8,
    pipette: PipetteReturn,
}

impl Default for AtasciiEditor {
    fn default() -> Self {
        // The inverse space: a full block, on the Atari's default blue screen ($94, text $CA).
        Self {
            brush: 0xA0,
            fkey_set: 0,
            background: (9, 4),
            text_luminance: 10,
            pipette: PipetteReturn::default(),
        }
    }
}

/// An Atari (GTIA, NTSC) color: hue 0 is gray, 1-15 go from gold over red, purple, blue and
/// green back to orange; luminance runs from 0 to 15 of which the text mode uses the even ones.
pub fn atari_color(hue: u8, luminance: u8) -> (u8, u8, u8) {
    let y = 0.06 + f32::from(luminance.min(15)) / 14.0 * 0.88;
    let (i, q) = if hue == 0 {
        (0.0, 0.0)
    } else {
        let angle = (-15.0 + f32::from(hue.min(15) - 1) * 24.0).to_radians();
        (0.2 * angle.cos(), 0.2 * angle.sin())
    };
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    (
        channel(y + 0.956 * i + 0.621 * q),
        channel(y - 0.272 * i - 0.647 * q),
        channel(y - 1.106 * i + 1.703 * q),
    )
}

/// The hue and even luminance of the Atari color nearest to `color`.
fn nearest_atari_color(color: (u8, u8, u8)) -> (u8, u8) {
    let distance = |(red, green, blue): (u8, u8, u8)| {
        let delta = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
        delta(red, color.0) + delta(green, color.1) + delta(blue, color.2)
    };
    (0..16u8)
        .flat_map(|hue| (0..8u8).map(move |step| (hue, step * 2)))
        .min_by_key(|&(hue, luminance)| distance(atari_color(hue, luminance)))
        .unwrap_or((0, 0))
}

pub fn atascii_mode_name(mode: AtasciiMode) -> String {
    let size = mode.screen_size();
    match mode {
        AtasciiMode::Antic => fl!("atascii-mode-antic", columns = size.width, rows = size.height),
        AtasciiMode::Xep80 => fl!("atascii-mode-xep80", columns = size.width, rows = size.height),
    }
}

impl DrawApp {
    /// Edits the document, an ATASCII screen, with the ATASCII editor.
    pub(super) fn start_atascii(&mut self) {
        let editor = AtasciiEditor::default();
        self.document.inverse = false;
        self.document.reverse_pen = false;
        self.document.quarter_blocks = false;
        self.document.brush.primary = BrushPrimaryMode::Char;
        self.document.brush.paint_char = char::from(editor.brush);
        self.document.brush.colorize_fg = true;
        self.document.brush.colorize_bg = true;
        self.document.box_line = None;
        self.document.tool = Tool::Click;
        self.atascii = Some(editor);
        self.read_atascii_colors();
    }

    /// Takes the Atari colors nearest to the screen's.
    fn read_atascii_colors(&mut self) {
        let (background, text) = self.document.with_state(|state| {
            let palette = &state.get_buffer().palette;
            (palette.rgb(0), palette.rgb(7))
        });
        if let Some(editor) = &mut self.atascii {
            editor.background = nearest_atari_color(background);
            editor.text_luminance = nearest_atari_color(text).1;
        }
    }

    /// Switches the screen to the Atari's 40 columns or the XEP80's 80 columns, with the
    /// mode's font and colors, in one undo step. Characters beyond a narrower screen are lost.
    pub(super) fn set_atascii_mode(&mut self, mode: AtasciiMode) {
        if self.document.profile() == ScreenProfile::Atascii(mode) {
            return;
        }
        self.document.finish();
        let font = Self::atascii_builtin_fonts(mode).remove(0);
        let palette = icy_engine::Palette::from_slice(match mode {
            AtasciiMode::Antic => &icy_engine::ATARI_DEFAULT_PALETTE,
            AtasciiMode::Xep80 => &icy_engine::ATARI_XEP80_PALETTE,
        });
        let result = self.document.with_state(|state| {
            let _undo = state.begin_atomic_undo(fl!("atascii-screen-mode"));
            let height = state.get_buffer().height();
            state.resize_buffer(true, icy_engine::Size::new(mode.columns(), height))?;
            state.set_font_dimensions(font.size())?;
            state.set_font_in_slot(0, font)?;
            state.switch_to_palette(palette)
        });
        self.result(result.map_err(|error| error.to_string()));
        self.read_atascii_colors();
    }

    /// Makes the character at `position` on the screen the brush and returns to the tool before
    /// the pipette.
    pub(super) fn pipette_atascii(&mut self, position: icy_engine::Position) {
        let code = self.document.with_state(|state| state.get_buffer().char_at(position).ch as u32);
        let Some(editor) = &mut self.atascii else {
            return;
        };
        let code = u8::try_from(code).unwrap_or(b' ');
        editor.brush = code;
        self.document.brush.paint_char = char::from(code);
        self.document.inverse = code >= 0x80;
        self.document.tool = editor.pipette.tool();
    }

    /// Makes `code` the brush; the text tool types it as well.
    fn pick_atascii(&mut self, code: u8) {
        let Some(editor) = &mut self.atascii else {
            return;
        };
        editor.brush = code;
        self.document.brush.paint_char = char::from(code);
        if self.document.tool == Tool::Click {
            let result = self.document.type_code(char::from(code));
            self.result(result);
        }
    }

    /// Function keys of the ATASCII editor pick from its character sets instead of the CP437 ones.
    pub(super) fn atascii_keys(&mut self, context: &egui::Context) {
        let Some(set) = self.atascii.as_ref().map(|editor| editor.fkey_set) else {
            return;
        };
        for (index, key) in FKEYS.into_iter().enumerate() {
            if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key)) {
                self.pick_atascii(FKEY_SETS[set][index]);
            }
        }
    }

    /// The panels of the ATASCII editor around the canvas.
    pub(super) fn atascii_panels(&mut self, context: &egui::Context, blocked: bool) {
        if let Some(editor) = &mut self.atascii {
            editor.pipette.track(self.document.tool);
        }
        let panel_fill = context.style().visuals.panel_fill;
        egui::TopBottomPanel::top("toolbar")
            .exact_height(chrome::TOOLBAR_HEIGHT)
            .frame(egui::Frame::new().fill(panel_fill))
            .show(context, |ui| {
                if blocked {
                    ui.disable();
                }
                self.atascii_toolbar(ui);
            });
        egui::TopBottomPanel::bottom("status")
            .exact_height(chrome::STATUS_HEIGHT)
            .frame(egui::Frame::new().fill(panel_fill))
            .show(context, |ui| {
                if blocked {
                    ui.disable();
                }
                self.atascii_status_bar(ui);
            });
        self.screen_tool_rail(context, &TOOLS, blocked);
        if context.content_rect().width() >= 850.0 {
            egui::SidePanel::right("panel")
                .exact_width(chrome::PANEL_WIDTH)
                .frame(egui::Frame::new().fill(panel_fill).inner_margin(egui::Margin { top: 4, ..Default::default() }))
                .resizable(false)
                .show(context, |ui| {
                    if blocked || self.document.paste_active() {
                        ui.disable();
                    }
                    self.atascii_panel(ui);
                });
        }
    }

    /// Colors the screen: the background, and the text in its hue at `text_luminance`.
    fn set_atascii_colors(&mut self, background: (u8, u8), text_luminance: u8) {
        let Some(editor) = &mut self.atascii else {
            return;
        };
        editor.background = background;
        editor.text_luminance = text_luminance;
        let mut palette = self.document.with_state(|state| state.get_buffer().palette.clone());
        let (red, green, blue) = atari_color(background.0, background.1);
        palette.set_color(0, icy_engine::Color::new(red, green, blue));
        let (red, green, blue) = atari_color(background.0, text_luminance);
        palette.set_color(7, icy_engine::Color::new(red, green, blue));
        self.edit(|state| state.switch_to_palette(palette));
    }

    /// Swatches of the Atari colors, a row per luminance and a column per hue.
    fn atari_color_grid(ui: &mut egui::Ui, current: (u8, u8), hues: std::ops::RangeInclusive<u8>) -> Option<(u8, u8)> {
        let mut picked = None;
        ui.spacing_mut().item_spacing = egui::Vec2::splat(2.0);
        for step in 0..8u8 {
            ui.horizontal(|ui| {
                for hue in hues.clone() {
                    let color = (hue, step * 2);
                    let (red, green, blue) = atari_color(color.0, color.1);
                    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(14.0), egui::Sense::click());
                    ui.painter().rect_filled(rect, 2, Color32::from_rgb(red, green, blue));
                    if color == current {
                        ui.painter()
                            .rect_stroke(rect.expand(1.0), 3, egui::Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Outside);
                    }
                    if response.on_hover_text(format!("${:02X}", color.0 * 16 + color.1)).clicked() {
                        picked = Some(color);
                    }
                }
            });
        }
        picked
    }

    fn atascii_screen_colors(&mut self, ui: &mut egui::Ui) {
        let Some((background, text_luminance)) = self.atascii.as_ref().map(|editor| (editor.background, editor.text_luminance)) else {
            return;
        };
        let (foreground, background_color) = self.atascii_colors();
        let mut changed = None;
        ui.horizontal(|ui| {
            ui.label(fl!("atascii-background"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let response = ui
                    .add(
                        egui::Button::new(format!("${:02X}", background.0 * 16 + background.1))
                            .fill(background_color)
                            .min_size(egui::vec2(64.0, 22.0)),
                    )
                    .on_hover_text(fl!("atascii-background-tooltip"));
                egui::Popup::menu(&response).show(|ui| {
                    if let Some(color) = Self::atari_color_grid(ui, background, 0..=15) {
                        changed = Some((color, text_luminance));
                        ui.close();
                    }
                });
            });
        });
        ui.horizontal(|ui| {
            ui.label(fl!("atascii-text-luminance"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let response = ui
                    .add(
                        egui::Button::new(egui::RichText::new(format!("${:02X}", background.0 * 16 + text_luminance)).color(background_color))
                            .fill(foreground)
                            .min_size(egui::vec2(64.0, 22.0)),
                    )
                    .on_hover_text(fl!("atascii-text-luminance-tooltip"));
                egui::Popup::menu(&response).show(|ui| {
                    if let Some((_, luminance)) = Self::atari_color_grid(ui, (background.0, text_luminance), background.0..=background.0) {
                        changed = Some((background, luminance));
                        ui.close();
                    }
                });
            });
        });
        ui.label(egui::RichText::new(fl!("atascii-colors-hint")).small().weak());
        if let Some((background, text_luminance)) = changed {
            self.set_atascii_colors(background, text_luminance);
        }
    }

    /// The fonts the screen has built in.
    fn atascii_builtin_fonts(mode: AtasciiMode) -> Vec<icy_engine::BitFont> {
        match mode {
            AtasciiMode::Antic => vec![icy_engine::ATARI.clone()],
            AtasciiMode::Xep80 => vec![icy_engine::ATARI_XEP80.clone(), icy_engine::ATARI_XEP80_INT.clone()],
        }
    }

    fn set_atascii_font(&mut self, font: icy_engine::BitFont) {
        self.edit(|state| state.set_font_in_slot(0, font));
    }

    /// Loads an Atari font file for the screen.
    pub(super) fn load_atascii_font(&mut self, path: &std::path::Path) {
        let name = path.file_stem().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        match std::fs::read(path)
            .map_err(|error| error.to_string())
            .and_then(|data| icy_draw::atari_font::load(&name, &data))
        {
            Ok(font) => self.set_atascii_font(font),
            Err(error) => self.dialog = Some(super::Dialog::Error(error)),
        }
    }

    fn atascii_font_row(&mut self, ui: &mut egui::Ui, mode: AtasciiMode, font: &str) {
        let mut chosen = None;
        let mut load = false;
        ui.horizontal(|ui| {
            ui.label(fl!("atascii-font"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let response = ui.add(egui::Button::new(font).truncate().min_size(egui::vec2(64.0, 22.0)));
                egui::Popup::menu(&response).show(|ui| {
                    for builtin in Self::atascii_builtin_fonts(mode) {
                        if ui.selectable_label(builtin.name() == font, builtin.name()).clicked() {
                            chosen = Some(builtin);
                        }
                    }
                    // The XEP80 has its own 7 × 10 characters; Atari fonts are 8 × 8.
                    if mode == AtasciiMode::Antic {
                        ui.separator();
                        load = ui.button(fl!("atascii-font-load")).on_hover_text(fl!("atascii-font-load-tooltip")).clicked();
                    }
                });
            });
        });
        if let Some(font) = chosen {
            self.set_atascii_font(font);
        }
        if load {
            let context = ui.ctx().clone();
            self.choose(&context, FileAction::LoadAtasciiFont);
        }
    }

    /// The screen's text and background colors.
    fn atascii_colors(&self) -> (Color32, Color32) {
        self.document.with_state(|state| {
            let palette = &state.get_buffer().palette;
            let color = |index| {
                let (red, green, blue) = palette.rgb(index);
                Color32::from_rgb(red, green, blue)
            };
            (color(7), color(0))
        })
    }

    /// A character as the screen shows it.
    fn atascii_glyph(&self, ui: &mut egui::Ui, code: u8, size: egui::Vec2) -> egui::Response {
        self.screen_glyph(ui, code, size, self.atascii_colors())
    }

    /// The 128 characters of the current video mode; the brush is framed.
    fn atascii_character_map(&self, ui: &mut egui::Ui, brush: u8) -> Option<u8> {
        let page = if self.document.inverse { 0x80 } else { 0 };
        let codes: Vec<u8> = (0..128u8).map(|index| page | index).collect();
        self.character_map(ui, &codes, Some(brush), self.atascii_colors())
    }

    fn atascii_toolbar(&mut self, ui: &mut egui::Ui) {
        let Some((brush, set)) = self.atascii.as_ref().map(|editor| (editor.brush, editor.fkey_set)) else {
            return;
        };
        if self.document.paste_active() {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_space(8.0);
                ui.label(egui::RichText::new(fl!("menu-paste")).strong());
                widgets::divider(ui);
                self.paste_options(ui);
            });
            return;
        }
        let mut picked = None;
        let mut step = 0i32;
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(8.0);
            self.atascii_glyph(ui, brush, egui::Vec2::splat(30.0))
                .on_hover_text(fl!("atascii-brush-tooltip", code = format!("{brush:02X}")));
            widgets::divider(ui);
            ui.label(egui::RichText::new(chrome::tool_label(self.document.tool)).strong());
            if self.document.tool == Tool::Pipette {
                if let Some((position, _)) = self.pipette_hover {
                    let code = self.document.with_state(|state| state.get_buffer().char_at(position).ch as u32);
                    let code = u8::try_from(code).unwrap_or(b' ');
                    self.atascii_glyph(ui, code, egui::Vec2::splat(26.0));
                    ui.monospace(format!("#{code:02X}"));
                } else {
                    ui.weak(fl!("atascii-pipette-hint"));
                }
            }
            let tool = self.document.tool;
            if (tool == Tool::Pencil || tool.is_shape_tool()) && !self.document.draws_boxes() {
                let has_quarters = self
                    .document
                    .with_state(|state| state.get_buffer().font(0).and_then(icy_draw::quarter_blocks::QuarterBlocks::of).is_some());
                if has_quarters && !self.document.reverse_pen {
                    ui.toggle_value(&mut self.document.quarter_blocks, fl!("atascii-pixels"))
                        .on_hover_text(fl!("atascii-pixels-tooltip"));
                }
                ui.toggle_value(&mut self.document.reverse_pen, fl!("atascii-inverse-pen"))
                    .on_hover_text(fl!("atascii-inverse-pen-tooltip"));
            }
            if matches!(self.document.tool, Tool::Line | Tool::RectangleOutline) {
                let mut outline = self.document.box_line.is_some();
                if ui
                    .toggle_value(&mut outline, fl!("line-style-outline"))
                    .on_hover_text(fl!("line-style-outline-tooltip"))
                    .changed()
                {
                    self.document.box_line = outline.then_some(icy_draw::box_lines::BoxStyle::Single);
                }
            }
            widgets::divider(ui);
            (picked, step) = self.screen_fkey_bar(ui, &FKEY_SETS[set], (set, FKEY_SETS.len()), self.atascii_colors());
        });
        if step != 0 {
            if let Some(editor) = &mut self.atascii {
                editor.fkey_set = (editor.fkey_set as i32 + step).rem_euclid(FKEY_SETS.len() as i32) as usize;
            }
        }
        if let Some(code) = picked {
            self.pick_atascii(code);
        }
    }

    fn atascii_panel(&mut self, ui: &mut egui::Ui) {
        let Some(brush) = self.atascii.as_ref().map(|editor| editor.brush) else {
            return;
        };
        let mut picked = None;
        chrome::section(ui, |ui| {
            widgets::section_header(ui, &fl!("atascii-characters"), |_| {});
            // The map shows the characters typed in the current video mode.
            let mut inverse = self.document.inverse;
            let options = [
                (false, fl!("atascii-normal"), fl!("atascii-normal-tooltip")),
                (true, fl!("atascii-inverse"), fl!("atascii-inverse-tooltip")),
            ];
            if widgets::segmented(ui, &mut inverse, &options) {
                self.document.inverse = inverse;
            }
            ui.add_space(4.0);
            picked = self.atascii_character_map(ui, brush);
        });
        if let Some(code) = picked {
            self.pick_atascii(code);
        }
        chrome::section(ui, |ui| {
            let (profile, font) = self.document.with_state(|state| {
                let buffer = state.get_buffer();
                (
                    ScreenProfile::of(buffer),
                    buffer.font(0).map(|font| font.name().to_string()).unwrap_or_default(),
                )
            });
            widgets::section_header(ui, &fl!("atascii-screen"), |_| {});
            let ScreenProfile::Atascii(current) = profile else {
                return;
            };
            let mut mode = current;
            let options = AtasciiMode::ALL.map(|mode| {
                let tooltip = match mode {
                    AtasciiMode::Antic => fl!("atascii-mode-antic-tooltip"),
                    AtasciiMode::Xep80 => fl!("atascii-mode-xep80-tooltip"),
                };
                (mode, atascii_mode_name(mode), tooltip)
            });
            if widgets::segmented(ui, &mut mode, &options) {
                self.set_atascii_mode(mode);
            }
            ui.add_space(4.0);
            self.atascii_font_row(ui, mode, &font);
            // The XEP80 shows white on black; only the built-in screen has colors.
            if mode == AtasciiMode::Antic {
                self.atascii_screen_colors(ui);
            } else {
                ui.label(egui::RichText::new(fl!("atascii-xep80-colors")).small().weak());
            }
        });
        let signature = self.document.with_state(|state| chrome::signature(state.get_buffer()));
        self.layers(ui, signature);
    }

    fn atascii_status_bar(&mut self, ui: &mut egui::Ui) {
        let (size, caret, selection, profile) = self.document.with_state(|state| {
            let buffer = state.get_buffer();
            (
                buffer.size(),
                state.get_caret().position(),
                state.selection().map(|selection| selection.as_rectangle()),
                ScreenProfile::of(buffer),
            )
        });
        let inverse = self.document.inverse;
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_space(8.0);
            let small = |text: String| egui::RichText::new(text).size(12.0);
            ui.label(small(format!("{} × {}", size.width, size.height)));
            ui.label(small("·".into()).weak());
            if let Some(bounds) = selection {
                ui.label(small(fl!("status-selection", width = bounds.width(), height = bounds.height())).weak());
            } else {
                ui.label(small(format!("{}, {}", caret.x, caret.y)).weak());
            }
            if inverse {
                ui.label(small("·".into()).weak());
                ui.label(small(fl!("atascii-inverse")).color(PRIMARY));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.add_space(6.0);
                self.zoom_status_button(ui);
                chrome::status_separator(ui);
                if let ScreenProfile::Atascii(mode) = profile {
                    ui.label(small(atascii_mode_name(mode)));
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::{Position, Size};

    fn atascii_app() -> (egui::Context, DrawApp) {
        super::super::tests::use_english();
        let context = egui::Context::default();
        icy_engine_gui::egui::appearance::apply(&context);
        let mut app = DrawApp::new();
        app.create(super::super::NewKind::Atascii, Size::new(80, 25));
        (context, app)
    }

    fn code_at(app: &DrawApp, x: i32) -> u32 {
        app.document.with_state(|state| state.get_buffer().char_at(Position::new(x, 0)).ch as u32)
    }

    #[test]
    fn atascii_screens_open_in_their_own_editor() {
        let (_, mut app) = atascii_app();
        assert!(app.atascii.is_some());
        assert_eq!(app.document.brush.primary, BrushPrimaryMode::Char);
        assert_eq!(app.document.brush.paint_char, char::from(0xA0), "the brush starts as a full block");
        app.create(super::super::NewKind::Ansi, Size::new(80, 25));
        assert!(app.atascii.is_none(), "ANSI documents keep the ANSI editor");
    }

    #[test]
    fn the_default_screen_colors_are_the_atari_defaults() {
        let (_, app) = atascii_app();
        let editor = app.atascii.as_ref().unwrap();
        assert_eq!((editor.background, editor.text_luminance), ((9, 4), 10), "$94 background, $CA text");
        assert!(atari_color(0, 0).0 < 20, "black is dark");
        assert!(atari_color(0, 14).0 > 230, "white is bright");
    }

    #[test]
    fn screen_colors_are_one_undo_step() {
        let (_, mut app) = atascii_app();
        let palette = |app: &DrawApp| {
            app.document
                .with_state(|state| (state.get_buffer().palette.rgb(0), state.get_buffer().palette.rgb(7)))
        };
        let before = palette(&app);
        app.set_atascii_colors((3, 2), 12);
        assert_eq!(palette(&app), (atari_color(3, 2), atari_color(3, 12)), "the text has the background's hue");
        app.undo(false);
        assert_eq!(palette(&app), before);
    }

    #[test]
    fn outline_lines_join_with_atascii_line_characters() {
        let (_, mut app) = atascii_app();
        app.document.tool = Tool::Line;
        app.document.box_line = Some(icy_draw::box_lines::BoxStyle::Single);
        let mut line = |from: (i32, i32), to: (i32, i32)| {
            app.document.begin(Position::new(from.0, from.1), icy_engine::MouseButton::Left);
            app.document.update(Position::new(to.0, to.1));
            app.document.finish();
        };
        line((0, 1), (4, 1));
        line((2, 0), (2, 2));
        let codes: Vec<u32> = (0..5)
            .map(|x| app.document.with_state(|state| state.get_buffer().char_at(Position::new(x, 1)).ch as u32))
            .collect();
        assert_eq!(codes, [0x12, 0x12, 0x13, 0x12, 0x12], "the lines cross with ATASCII's ┼");
        let column: Vec<u32> = [0, 2]
            .iter()
            .map(|&y| app.document.with_state(|state| state.get_buffer().char_at(Position::new(2, y)).ch as u32))
            .collect();
        assert_eq!(column, [0x7C, 0x7C], "vertical lines are the bar");
    }

    #[test]
    fn atari_font_files_replace_the_screen_font_in_one_undo_step() {
        let (_, mut app) = atascii_app();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Graphics.fnt");
        std::fs::write(&path, vec![0x18u8; 1024]).unwrap();
        app.load_atascii_font(&path);
        let font = |app: &DrawApp| app.document.with_state(|state| state.get_buffer().font(0).unwrap().name().to_string());
        assert_eq!(font(&app), "Graphics");
        app.undo(false);
        assert_eq!(font(&app), icy_engine::ATARI.name());
        std::fs::write(&path, vec![0u8; 100]).unwrap();
        app.load_atascii_font(&path);
        assert!(matches!(app.dialog, Some(super::super::Dialog::Error(_))), "an unknown file is reported");
    }

    #[test]
    fn the_pencil_draws_quarter_block_pixels() {
        let (_, mut app) = atascii_app();
        app.document.tool = Tool::Pencil;
        app.document.quarter_blocks = true;
        assert!(app.document.draws_pixels());
        for pixel in [(0, 0), (1, 0), (3, 1)] {
            app.document.begin(Position::new(pixel.0, pixel.1), icy_engine::MouseButton::Left);
            app.document.finish();
        }
        assert_eq!([code_at(&app, 0), code_at(&app, 1)], [0x95, 0x09], "▀ and ▗");
        app.document.begin(Position::new(1, 0), icy_engine::MouseButton::Right);
        app.document.finish();
        assert_eq!(code_at(&app, 0), 0x0C, "the right button clears a pixel");
        app.document.tool = Tool::Fill;
        assert!(!app.document.draws_pixels(), "fill works on characters");
    }

    #[test]
    fn a_floating_paste_offers_its_actions() {
        let (context, mut app) = atascii_app();
        let size = egui::vec2(1280.0, 820.0);
        app.document.type_text("HI").unwrap();
        app.select_all();
        let layers = app.document.with_state(|state| state.get_buffer().layers.len());
        let data = icy_engine_gui::prepare_clipboard_data(&**app.document.screen.lock()).unwrap();
        app.paste_content(icy_engine_gui::system_clipboard::PasteContent::Icy {
            text: data.text.clone(),
            data: data.icy_data.unwrap(),
        });
        assert!(app.document.paste_active());
        assert!(!app.icons.loaded("anchor"));
        super::super::tests::frame(&context, &mut app, size, vec![]);
        for icon in ["anchor", "add_layer", "file_copy", "delete"] {
            assert!(app.icons.loaded(icon), "the toolbar offers {icon}");
        }
        let result = app.document.paste_action(icy_draw::document::PasteAction::Keep);
        app.result(result);
        assert!(!app.document.paste_active());
        assert_eq!(
            app.document.with_state(|state| state.get_buffer().layers.len()),
            layers + 1,
            "the paste became a layer"
        );
    }

    #[test]
    fn the_screen_mode_switches_in_one_undo_step() {
        let (_, mut app) = atascii_app();
        app.document.type_text("HI").unwrap();
        app.set_atascii_mode(AtasciiMode::Xep80);
        let screen = |app: &DrawApp| {
            app.document.with_state(|state| {
                let buffer = state.get_buffer();
                (
                    ScreenProfile::of(buffer),
                    buffer.font(0).unwrap().name().to_string(),
                    buffer.font_dimensions(),
                    buffer.char_at(Position::new(1, 0)).ch,
                )
            })
        };
        let (profile, font, cell, ch) = screen(&app);
        assert_eq!(profile, ScreenProfile::Atascii(AtasciiMode::Xep80));
        assert_eq!((font.as_str(), cell), (icy_engine::ATARI_XEP80.name(), icy_engine::ATARI_XEP80.size()));
        assert_eq!(ch, 'I', "the picture stays");
        assert_eq!(app.atascii.as_ref().unwrap().background, (0, 0), "the XEP80 is black");
        app.undo(false);
        let (profile, font, cell, _) = screen(&app);
        assert_eq!(profile, ScreenProfile::Atascii(AtasciiMode::Antic));
        assert_eq!((font.as_str(), cell), (icy_engine::ATARI.name(), Size::new(8, 8)));
    }

    #[test]
    fn the_pipette_picks_a_character_and_returns_to_the_tool() {
        let (context, mut app) = atascii_app();
        let size = egui::vec2(1280.0, 820.0);
        app.document.inverse = true;
        app.document.type_text("A").unwrap();
        app.document.tool = Tool::Line;
        super::super::tests::frame(&context, &mut app, size, vec![]);
        app.document.tool = Tool::Pipette;
        super::super::tests::frame(&context, &mut app, size, vec![]);
        app.document.inverse = false;
        app.pipette_atascii(Position::new(0, 0));
        assert_eq!(app.atascii.as_ref().unwrap().brush, 0xC1);
        assert_eq!(app.document.brush.paint_char as u32, 0xC1);
        assert!(app.document.inverse, "an inverse character turns inverse on");
        assert_eq!(app.document.tool, Tool::Line);
    }

    #[test]
    fn the_invert_pen_makes_characters_inverse_and_back() {
        let (_, mut app) = atascii_app();
        app.document.type_text("AB").unwrap();
        app.document.tool = Tool::Pencil;
        app.document.reverse_pen = true;
        app.document.begin(Position::new(0, 0), icy_engine::MouseButton::Left);
        app.document.update(Position::new(1, 0));
        app.document.finish();
        assert_eq!([code_at(&app, 0), code_at(&app, 1)], [0xC1, 0xC2]);
        app.document.begin(Position::new(0, 0), icy_engine::MouseButton::Right);
        app.document.finish();
        assert_eq!(code_at(&app, 0), 0x41);
    }

    #[test]
    fn inverse_typing_and_function_keys_use_the_atari_codes() {
        let (context, mut app) = atascii_app();
        let size = egui::vec2(1280.0, 820.0);
        app.document.type_text("A").unwrap();
        app.document.inverse = true;
        app.document.type_text("A").unwrap();
        assert_eq!([code_at(&app, 0), code_at(&app, 1)], [0x41, 0xC1]);

        super::super::tests::frame(&context, &mut app, size, vec![]);
        super::super::tests::frame(
            &context,
            &mut app,
            size,
            vec![super::super::tests::key_event(egui::Key::F2, egui::Modifiers::NONE)],
        );
        assert_eq!(code_at(&app, 2), u32::from(FKEY_SETS[0][1]), "F2 types the set's character, not a CP437 one");
        assert_eq!(app.atascii.as_ref().unwrap().brush, FKEY_SETS[0][1]);

        app.document.tool = Tool::Pencil;
        app.pick_atascii(0x95);
        assert_eq!(app.document.brush.paint_char, char::from(0x95), "picking sets the brush without typing");
        assert_eq!(code_at(&app, 3), 0x20);
    }
}
