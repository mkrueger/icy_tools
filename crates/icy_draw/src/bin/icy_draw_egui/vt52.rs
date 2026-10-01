//! The VT52 editor for the Atari ST's text screens.
//!
//! Like the ATASCII editor it shares the canvas, layers and undo with the ANSI editor. Each
//! character has a text and a background color from the resolution's palette: 16 colors in low,
//! 4 in medium and 2 in high resolution. The ST's character set has no block or line graphics.

use eframe::egui;
use icy_draw::{brush::BrushPrimaryMode, fl, screen_profile::ScreenProfile};
use icy_engine::{BufferType, TerminalResolution, TerminalResolutionExt, TextPane};
use icy_engine_edit::tools::{Tool, ToolPair};

use super::{
    chrome,
    retro::{GlyphColors, PipetteReturn, FKEYS},
    widgets, DrawApp,
};

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

/// Characters on the function keys: ASCII frames, symbols, Greek and mathematics.
const FKEY_SETS: [&str; 4] = ["-|+=_/\\<>^*#", "©®™†¶§°∙·¬¡¿", "αβΓπΣσµτΦΘΩδ", "∞∈∩≡±≥≤÷≈√²³"];

pub const RESOLUTIONS: [TerminalResolution; 3] = [TerminalResolution::Low, TerminalResolution::Medium, TerminalResolution::High];

/// The side of a color swatch.
const SWATCH: f32 = 30.0;

fn fkey_codes(set: usize) -> [u8; 12] {
    let mut codes = [b' '; 12];
    for (code, ch) in codes.iter_mut().zip(FKEY_SETS[set].chars()) {
        *code = BufferType::AtariSt.convert_from_unicode(ch) as u32 as u8;
    }
    codes
}

pub fn resolution_name(resolution: TerminalResolution) -> String {
    match resolution {
        TerminalResolution::Low => fl!("vt52-resolution-low"),
        TerminalResolution::Medium => fl!("vt52-resolution-medium"),
        TerminalResolution::High => fl!("vt52-resolution-high"),
    }
}

/// Columns, rows and colors of `resolution`.
pub fn resolution_detail(resolution: TerminalResolution) -> String {
    fl!(
        "vt52-resolution-detail",
        columns = icy_engine::atari_st_columns(resolution),
        rows = 25,
        colors = resolution.palette().len()
    )
}

/// The index of the color in `palette` nearest to `color`.
fn nearest(palette: &icy_engine::Palette, (red, green, blue): (u8, u8, u8)) -> u32 {
    (0..palette.len() as u32)
        .min_by_key(|&index| {
            let (r, g, b) = palette.rgb(index);
            let delta = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
            delta(r, red) + delta(g, green) + delta(b, blue)
        })
        .unwrap_or(0)
}

/// What the VT52 editor remembers beside the document.
pub struct Vt52Editor {
    /// The character the drawing tools paint.
    pub brush: u8,
    pub fkey_set: usize,
    pipette: PipetteReturn,
}

impl Default for Vt52Editor {
    fn default() -> Self {
        Self {
            brush: b'#',
            fkey_set: 0,
            pipette: PipetteReturn::default(),
        }
    }
}

impl DrawApp {
    /// Edits the document, an Atari ST text screen, with the VT52 editor.
    pub(super) fn start_vt52(&mut self) {
        let editor = Vt52Editor::default();
        self.document.inverse = false;
        self.document.quarter_blocks = false;
        self.document.reverse_pen = false;
        self.document.brush.primary = BrushPrimaryMode::Char;
        self.document.brush.paint_char = char::from(editor.brush);
        self.document.brush.colorize_fg = true;
        self.document.brush.colorize_bg = true;
        self.document.box_line = None;
        self.document.tool = Tool::Click;
        self.vt52 = Some(editor);
    }

    fn vt52_resolution(&self) -> TerminalResolution {
        match self.document.profile() {
            ScreenProfile::AtariSt(resolution) => resolution,
            _ => TerminalResolution::Medium,
        }
    }

    /// The caret's text and background color, in which characters are shown.
    fn vt52_colors(&self) -> GlyphColors {
        self.document.with_state(|state| {
            let palette = &state.get_buffer().palette;
            let attribute = state.get_caret().attribute;
            let color = |index| {
                let (red, green, blue) = palette.rgb(index);
                egui::Color32::from_rgb(red, green, blue)
            };
            (color(attribute.foreground()), color(attribute.background()))
        })
    }

    /// Makes `code` the brush; the text tool types it as well.
    fn pick_vt52(&mut self, code: u8) {
        let Some(editor) = &mut self.vt52 else {
            return;
        };
        editor.brush = code;
        self.document.brush.paint_char = char::from(code);
        if self.document.tool == Tool::Click {
            let result = self.document.type_code(char::from(code));
            self.result(result);
        }
    }

    /// Takes the character and colors at `position` and returns to the tool before the pipette.
    pub(super) fn pipette_vt52(&mut self, position: icy_engine::Position) {
        let cell = self.document.with_state(|state| state.get_buffer().char_at(position));
        let Some(editor) = &mut self.vt52 else {
            return;
        };
        let code = u8::try_from(cell.ch as u32).ok().filter(|code| *code >= 0x20).unwrap_or(b' ');
        editor.brush = code;
        self.document.brush.paint_char = char::from(code);
        self.document.tool = editor.pipette.tool();
        if cell.is_visible() {
            let result = self
                .document
                .set_caret_foreground(cell.attribute.foreground())
                .and_then(|()| self.document.set_caret_background(cell.attribute.background()));
            self.result(result);
        }
    }

    /// Switches the screen to `resolution` with its columns, font and colors in one undo step.
    /// Colors become the nearest of the new palette; characters beyond a narrower screen are lost.
    pub(super) fn set_vt52_resolution(&mut self, resolution: TerminalResolution) {
        if self.vt52_resolution() == resolution {
            return;
        }
        self.document.finish();
        let font = icy_engine::atari_st_font(resolution);
        let palette = resolution.palette().clone();
        let result = self.document.with_state(|state| {
            let _undo = state.begin_atomic_undo(fl!("atascii-screen-mode"));
            let height = state.get_buffer().height();
            state.resize_buffer(true, icy_engine::Size::new(icy_engine::atari_st_columns(resolution), height))?;
            state.set_font_dimensions(font.size())?;
            state.set_font_in_slot(0, font)?;
            let old = state.get_buffer().palette.clone();
            let map = |index: u32| nearest(&palette, old.rgb(index));
            let mut layers = state.get_buffer().layers.clone();
            for layer in &mut layers {
                for line in &mut layer.lines {
                    for ch in &mut line.chars {
                        let (foreground, background) = (map(ch.attribute.foreground()), map(ch.attribute.background()));
                        ch.attribute.set_foreground(foreground);
                        ch.attribute.set_background(background);
                    }
                }
            }
            let caret = state.get_caret().attribute;
            state.switch_to_palette_with_layers(palette.clone(), layers)?;
            state.set_caret_foreground(map(caret.foreground()));
            state.set_caret_background(map(caret.background()));
            Ok::<(), icy_engine::EngineError>(())
        });
        self.result(result.map_err(|error| error.to_string()));
    }

    /// Function keys of the VT52 editor pick from its character sets instead of the CP437 ones.
    pub(super) fn vt52_keys(&mut self, context: &egui::Context) {
        let Some(set) = self.vt52.as_ref().map(|editor| editor.fkey_set) else {
            return;
        };
        let codes = fkey_codes(set);
        for (index, key) in FKEYS.into_iter().enumerate() {
            if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key)) {
                self.pick_vt52(codes[index]);
            }
        }
    }

    /// The panels of the VT52 editor around the canvas.
    pub(super) fn vt52_panels(&mut self, context: &egui::Context, blocked: bool) {
        if let Some(editor) = &mut self.vt52 {
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
                self.vt52_toolbar(ui);
            });
        egui::TopBottomPanel::bottom("status")
            .exact_height(chrome::STATUS_HEIGHT)
            .frame(egui::Frame::new().fill(panel_fill))
            .show(context, |ui| {
                if blocked {
                    ui.disable();
                }
                self.vt52_status_bar(ui);
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
                    self.vt52_panel(ui);
                });
        }
    }

    fn vt52_toolbar(&mut self, ui: &mut egui::Ui) {
        let Some((brush, set)) = self.vt52.as_ref().map(|editor| (editor.brush, editor.fkey_set)) else {
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
        let colors = self.vt52_colors();
        let mut picked = None;
        let mut step = 0;
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(8.0);
            self.screen_glyph(ui, brush, egui::Vec2::splat(30.0), colors)
                .on_hover_text(fl!("atascii-brush-tooltip", code = format!("{brush:02X}")));
            widgets::divider(ui);
            ui.label(egui::RichText::new(chrome::tool_label(self.document.tool)).strong());
            let tool = self.document.tool;
            if tool == Tool::Pipette {
                if let Some((position, _)) = self.pipette_hover {
                    let cell = self.document.with_state(|state| state.get_buffer().char_at(position));
                    let code = u8::try_from(cell.ch as u32).unwrap_or(b' ');
                    let palette = self.document.with_state(|state| state.get_buffer().palette.clone());
                    let color = |index| {
                        let (red, green, blue) = palette.rgb(index);
                        egui::Color32::from_rgb(red, green, blue)
                    };
                    self.screen_glyph(
                        ui,
                        code,
                        egui::Vec2::splat(26.0),
                        (color(cell.attribute.foreground()), color(cell.attribute.background())),
                    );
                    ui.monospace(format!("#{code:02X}"));
                } else {
                    ui.weak(fl!("vt52-pipette-hint"));
                }
            }
            if tool == Tool::Pencil || tool == Tool::Fill || tool.is_shape_tool() {
                let mut mode = self.document.brush.primary;
                let options = [
                    (BrushPrimaryMode::Char, fl!("vt52-brush-char"), fl!("vt52-brush-char-tooltip")),
                    (BrushPrimaryMode::Colorize, fl!("vt52-brush-color"), fl!("vt52-brush-color-tooltip")),
                ];
                if widgets::segmented(ui, &mut mode, &options) {
                    self.document.brush.primary = mode;
                }
                if mode == BrushPrimaryMode::Colorize {
                    ui.toggle_value(&mut self.document.brush.colorize_fg, fl!("vt52-apply-text"));
                    ui.toggle_value(&mut self.document.brush.colorize_bg, fl!("vt52-apply-background"));
                }
            }
            widgets::divider(ui);
            (picked, step) = self.screen_fkey_bar(ui, &fkey_codes(set), (set, FKEY_SETS.len()), colors);
        });
        if step != 0 {
            if let Some(editor) = &mut self.vt52 {
                editor.fkey_set = (editor.fkey_set as i32 + step).rem_euclid(FKEY_SETS.len() as i32) as usize;
            }
        }
        if let Some(code) = picked {
            self.pick_vt52(code);
        }
    }

    fn vt52_panel(&mut self, ui: &mut egui::Ui) {
        let Some(brush) = self.vt52.as_ref().map(|editor| editor.brush) else {
            return;
        };
        chrome::section(ui, |ui| {
            let mut swap = false;
            widgets::section_header(ui, &fl!("vt52-colors"), |ui| {
                swap = self.icons.button_sized(ui, "swap", &fl!("vt52-swap-colors"), false, 24.0).clicked();
            });
            if swap {
                let result = self.document.swap_caret_colors();
                self.result(result);
            }
            let count = self.document.with_state(|state| state.get_buffer().palette.len());
            self.palette_grid(ui, count.clamp(1, 8) as f32 * SWATCH);
            ui.label(egui::RichText::new(fl!("vt52-colors-hint")).small().weak());
        });
        let mut picked = None;
        chrome::section(ui, |ui| {
            widgets::section_header(ui, &fl!("atascii-characters"), |_| {});
            // Codes below 32 are VT52 commands; the ST cannot print them from a file.
            let codes: Vec<u8> = (0x20..=0xFFu8).collect();
            picked = self.character_map(ui, &codes, Some(brush), self.vt52_colors());
        });
        if let Some(code) = picked {
            self.pick_vt52(code);
        }
        chrome::section(ui, |ui| {
            widgets::section_header(ui, &fl!("atascii-screen"), |_| {});
            let current = self.vt52_resolution();
            let mut resolution = current;
            let options = RESOLUTIONS.map(|resolution| (resolution, resolution_name(resolution), resolution_detail(resolution)));
            if widgets::segmented(ui, &mut resolution, &options) {
                self.set_vt52_resolution(resolution);
            }
            ui.label(egui::RichText::new(resolution_detail(resolution)).weak());
        });
        let signature = self.document.with_state(|state| chrome::signature(state.get_buffer()));
        self.layers(ui, signature);
    }

    fn vt52_status_bar(&mut self, ui: &mut egui::Ui) {
        let (size, caret, selection) = self.document.with_state(|state| {
            (
                state.get_buffer().size(),
                state.get_caret().position(),
                state.selection().map(|selection| selection.as_rectangle()),
            )
        });
        let resolution = self.vt52_resolution();
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
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.add_space(6.0);
                self.zoom_status_button(ui);
                chrome::status_separator(ui);
                ui.label(small(format!("Atari ST · {} · {}", resolution_name(resolution), resolution_detail(resolution))));
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::{Position, Size};

    fn vt52_app(resolution: TerminalResolution) -> (egui::Context, DrawApp) {
        super::super::tests::use_english();
        let context = egui::Context::default();
        icy_engine_gui::egui::appearance::apply(&context);
        let mut app = DrawApp::new();
        app.new_vt52_resolution = resolution;
        app.create(super::super::NewKind::Vt52, Size::new(80, 25));
        (context, app)
    }

    fn cell(app: &DrawApp, x: i32, y: i32) -> (u32, u32, u32) {
        app.document.with_state(|state| {
            let ch = state.get_buffer().char_at(Position::new(x, y));
            (ch.ch as u32, ch.attribute.foreground(), ch.attribute.background())
        })
    }

    #[test]
    fn vt52_screens_open_in_their_own_editor_with_the_st_colors() {
        for (resolution, columns, text) in [
            (TerminalResolution::Low, 40, 15),
            (TerminalResolution::Medium, 80, 3),
            (TerminalResolution::High, 80, 1),
        ] {
            let (_, mut app) = vt52_app(resolution);
            assert!(app.vt52.is_some() && app.atascii.is_none());
            assert_eq!(app.document.profile(), ScreenProfile::AtariSt(resolution));
            assert_eq!(app.document.with_state(|state| state.get_buffer().width()), columns);
            app.document.type_text("Aä").unwrap();
            assert_eq!(cell(&app, 0, 0), (u32::from(b'A'), text, 0), "{resolution:?} types black on white");
            assert_eq!(cell(&app, 1, 0).0, 0x84, "ä is the ST's code");
        }
    }

    #[test]
    fn the_resolution_switches_in_one_undo_step_with_the_nearest_colors() {
        let (_, mut app) = vt52_app(TerminalResolution::Low);
        app.document.with_state(|state| {
            state.set_caret_foreground(1);
            state.set_caret_background(14);
        });
        app.document.type_text("R").unwrap();
        app.set_vt52_resolution(TerminalResolution::Medium);
        assert_eq!(app.document.profile(), ScreenProfile::AtariSt(TerminalResolution::Medium));
        assert_eq!(cell(&app, 0, 0), (u32::from(b'R'), 1, 0), "red stays red, light cyan becomes white");
        assert_eq!(app.document.with_state(|state| state.get_buffer().font_dimensions()), Size::new(8, 16));
        app.set_vt52_resolution(TerminalResolution::High);
        assert_eq!(cell(&app, 0, 0).1, 1, "red becomes black in high resolution");
        app.undo(false);
        app.undo(false);
        assert_eq!(app.document.profile(), ScreenProfile::AtariSt(TerminalResolution::Low));
        assert_eq!(cell(&app, 0, 0), (u32::from(b'R'), 1, 14));
    }

    #[test]
    fn function_keys_and_the_pipette_use_the_st_characters() {
        let (context, mut app) = vt52_app(TerminalResolution::Medium);
        let size = egui::vec2(1280.0, 820.0);
        super::super::tests::frame(&context, &mut app, size, vec![]);
        app.vt52.as_mut().unwrap().fkey_set = 2;
        super::super::tests::frame(
            &context,
            &mut app,
            size,
            vec![super::super::tests::key_event(egui::Key::F1, egui::Modifiers::NONE)],
        );
        assert_eq!(cell(&app, 0, 0).0, BufferType::AtariSt.convert_from_unicode('α') as u32);

        app.document.with_state(|state| state.set_caret_foreground(1));
        app.document.type_text("x").unwrap();
        app.document.with_state(|state| state.set_caret_foreground(3));
        app.document.tool = Tool::Line;
        super::super::tests::frame(&context, &mut app, size, vec![]);
        app.document.tool = Tool::Pipette;
        super::super::tests::frame(&context, &mut app, size, vec![]);
        app.pipette_vt52(Position::new(1, 0));
        assert_eq!(app.vt52.as_ref().unwrap().brush, b'x');
        assert_eq!(
            app.document.with_state(|state| state.get_caret().attribute.foreground()),
            1,
            "the pipette takes the colors too"
        );
        assert_eq!(app.document.tool, Tool::Line);
    }

    #[test]
    fn vt52_documents_save_as_icy_and_export_as_vt52() {
        let (_, mut app) = vt52_app(TerminalResolution::High);
        app.document.type_text("HI").unwrap();
        let directory = tempfile::tempdir().unwrap();
        let native = directory.path().join("art.icy");
        app.document.save(&native, false).unwrap();
        let reopened = icy_draw::document::Document::load(&native).unwrap();
        assert_eq!(reopened.profile(), ScreenProfile::AtariSt(TerminalResolution::High));
        app.settings.last_export_directory = None;
        app.settings.export_settings = Default::default();
        let request = app.export_dialog().request().unwrap();
        assert_eq!(request.format, icy_engine::FileFormat::Vt52, "VT52 is preselected");
        app.export(&request).unwrap();
        let exported = icy_draw::document::Document::load(&request.path).unwrap();
        assert!(matches!(exported.profile(), ScreenProfile::AtariSt(_)));
        assert_eq!(exported.with_state(|state| state.get_buffer().char_at(Position::new(1, 0)).ch), 'I');
    }
}
