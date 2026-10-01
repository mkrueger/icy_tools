//! The PETSCII editor for the 40 column screens of the C64 and C128.
//!
//! Like the other home computer editors it shares the canvas, layers and undo with the ANSI
//! editor. The machine shows the whole screen in one character set and one background color;
//! each character has a text color, and reverse characters are the upper half of the codes.

use eframe::egui;
use icy_draw::{brush::BrushPrimaryMode, document::Document, fl};
use icy_engine::{PetsciiCase, PetsciiMachine, TextPane};
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

/// Screen codes on the function keys: lines, rounded corners and shapes, blocks, and edges.
const FKEY_SETS: [[u8; 12]; 4] = [
    [0x40, 0x5D, 0x70, 0x6E, 0x6D, 0x7D, 0x6B, 0x73, 0x72, 0x71, 0x5B, 0x56],
    [0x55, 0x49, 0x4A, 0x4B, 0x4E, 0x4D, 0x5F, 0x69, 0x51, 0x57, 0x53, 0x5A],
    [0xA0, 0x61, 0xE1, 0xE2, 0x62, 0x7B, 0x6C, 0x7E, 0x7C, 0x66, 0x7F, 0xFF],
    [0x63, 0x64, 0x65, 0x67, 0x77, 0x6F, 0x74, 0x6A, 0x79, 0x7A, 0x4F, 0x50],
];

pub const MACHINES: [PetsciiMachine; 2] = [PetsciiMachine::C64, PetsciiMachine::C128];
pub const CASES: [PetsciiCase; 2] = [PetsciiCase::Upper, PetsciiCase::Lower];

/// The side of a color swatch.
const SWATCH: f32 = 30.0;

pub fn machine_name(machine: PetsciiMachine) -> String {
    match machine {
        PetsciiMachine::C64 => "C64".to_owned(),
        PetsciiMachine::C128 => "C128".to_owned(),
    }
}

pub fn case_name(case: PetsciiCase) -> String {
    match case {
        PetsciiCase::Upper => fl!("petscii-case-upper"),
        PetsciiCase::Lower => fl!("petscii-case-lower"),
    }
}

/// What the PETSCII editor remembers beside the document.
pub struct PetsciiEditor {
    /// The screen code the drawing tools paint, reverse from 128.
    pub brush: u8,
    pub fkey_set: usize,
    pipette: PipetteReturn,
}

impl Default for PetsciiEditor {
    fn default() -> Self {
        // The reverse space: a full block.
        Self {
            brush: 0xA0,
            fkey_set: 0,
            pipette: PipetteReturn::default(),
        }
    }
}

/// Loads the first screen of a Petmate workspace (.petmate) as a PETSCII document.
pub fn load_petmate(path: &std::path::Path) -> Result<Document, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let workspace: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let screen = workspace
        .get("framebufs")
        .and_then(|screens| screens.get(0))
        .ok_or_else(|| fl!("petmate-no-screen"))?;
    let number = |key: &str| screen.get(key).and_then(serde_json::Value::as_i64);
    let charset = screen.get("charset").and_then(serde_json::Value::as_str).unwrap_or("upper");
    let (machine, case) = match charset {
        "upper" | "dirart" | "cbaseUpper" => (PetsciiMachine::C64, PetsciiCase::Upper),
        "lower" | "cbaseLower" => (PetsciiMachine::C64, PetsciiCase::Lower),
        "c128Upper" => (PetsciiMachine::C128, PetsciiCase::Upper),
        "c128Lower" => (PetsciiMachine::C128, PetsciiCase::Lower),
        other => return Err(fl!("petmate-unsupported-charset", charset = other)),
    };
    let width = number("width").unwrap_or(40).clamp(1, 1000) as i32;
    let height = number("height").unwrap_or(25).clamp(1, 1000) as i32;
    let background = (number("backgroundColor").unwrap_or(0) & 0x0F) as u32;
    let mut buffer = icy_engine::petscii_buffer(machine, case, icy_engine::Size::new(width, height), 14, background);
    let rows = screen.get("framebuf").and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
    for (y, row) in rows.iter().take(height as usize).enumerate() {
        for (x, cell) in row.as_array().into_iter().flatten().take(width as usize).enumerate() {
            let code = cell.get("code").and_then(serde_json::Value::as_u64).unwrap_or(0x20);
            let transparent = cell.get("transparent").and_then(serde_json::Value::as_bool).unwrap_or(false) || code > 0xFF;
            let color = (cell.get("color").and_then(serde_json::Value::as_u64).unwrap_or(14) & 0x0F) as u32;
            let mut ch = icy_engine::AttributedChar::new(if transparent { ' ' } else { char::from(code as u8) }, icy_engine::TextAttribute::default());
            ch.attribute.set_foreground(color);
            ch.attribute.set_background(background);
            buffer.layers[0].set_char((x as i32, y as i32), ch);
        }
    }
    let mut state = icy_engine_edit::EditState::from_buffer(buffer);
    state.set_caret_foreground(14);
    state.set_caret_background(background);
    Ok(Document::from_state(state))
}

impl DrawApp {
    /// Edits the document, a PETSCII screen, with the PETSCII editor.
    pub(super) fn start_petscii(&mut self) {
        let editor = PetsciiEditor::default();
        self.document.inverse = false;
        self.document.quarter_blocks = false;
        self.document.brush.primary = BrushPrimaryMode::Char;
        self.document.brush.paint_char = char::from(editor.brush);
        self.document.brush.colorize_fg = true;
        // The background belongs to the screen.
        self.document.brush.colorize_bg = false;
        self.document.box_line = None;
        self.document.tool = Tool::Click;
        self.document.with_state(|state| {
            let background = icy_engine::petscii_background(state.get_buffer());
            state.set_caret_background(background);
            state.set_caret_font_page(0);
        });
        self.petscii = Some(editor);
    }

    fn petscii_charset(&self) -> (PetsciiMachine, PetsciiCase) {
        self.document.with_state(|state| icy_engine::petscii_charset(state.get_buffer()))
    }

    /// The text color on the screen color, in which characters are shown.
    fn petscii_colors(&self) -> GlyphColors {
        self.document.with_state(|state| {
            let buffer = state.get_buffer();
            let color = |index| {
                let (red, green, blue) = buffer.palette.rgb(index);
                egui::Color32::from_rgb(red, green, blue)
            };
            (color(state.get_caret().attribute.foreground()), color(icy_engine::petscii_background(buffer)))
        })
    }

    /// Makes `code` the brush; the text tool types it as well.
    fn pick_petscii(&mut self, code: u8) {
        let Some(editor) = &mut self.petscii else {
            return;
        };
        editor.brush = code;
        self.document.brush.paint_char = char::from(code);
        if self.document.tool == Tool::Click {
            let result = self.document.type_code(char::from(code));
            self.result(result);
        }
    }

    /// Takes the character and text color at `position` and returns to the tool before the pipette.
    pub(super) fn pipette_petscii(&mut self, position: icy_engine::Position) {
        let cell = self.document.with_state(|state| state.get_buffer().char_at(position));
        let Some(editor) = &mut self.petscii else {
            return;
        };
        let code = u8::try_from(cell.ch as u32).unwrap_or(b' ');
        editor.brush = code;
        self.document.brush.paint_char = char::from(code);
        self.document.inverse = code >= 0x80;
        self.document.tool = editor.pipette.tool();
        if cell.is_visible() {
            let result = self.document.set_caret_foreground(cell.attribute.foreground());
            self.result(result);
        }
    }

    /// Colors the whole screen: every character gets `color` as its background, in one undo step.
    pub(super) fn set_petscii_background(&mut self, color: u32) {
        self.document.finish();
        let result = self.document.with_state(|state| {
            let mut layers = state.get_buffer().layers.clone();
            for layer in &mut layers {
                for line in &mut layer.lines {
                    for ch in line.chars.iter_mut().filter(|ch| ch.is_visible()) {
                        ch.attribute.set_background(color);
                    }
                }
            }
            let palette = state.get_buffer().palette.clone();
            state.switch_to_palette_with_layers(palette, layers)?;
            state.set_caret_background(color);
            Ok::<(), icy_engine::EngineError>(())
        });
        self.result(result.map_err(|error| error.to_string()));
    }

    /// Shows the screen with the character set of `machine` in `case`, like switching it on the
    /// machine: the characters stay, their look changes.
    pub(super) fn set_petscii_charset(&mut self, machine: PetsciiMachine, case: PetsciiCase) {
        if self.petscii_charset() == (machine, case) {
            return;
        }
        self.document.finish();
        let font = icy_engine::petscii_font(machine, case);
        self.edit(|state| state.set_font_in_slot(0, font));
    }

    /// Function keys of the PETSCII editor pick from its character sets instead of the CP437 ones.
    pub(super) fn petscii_keys(&mut self, context: &egui::Context) {
        let Some(set) = self.petscii.as_ref().map(|editor| editor.fkey_set) else {
            return;
        };
        for (index, key) in FKEYS.into_iter().enumerate() {
            if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key)) {
                self.pick_petscii(FKEY_SETS[set][index]);
            }
        }
    }

    /// The panels of the PETSCII editor around the canvas.
    pub(super) fn petscii_panels(&mut self, context: &egui::Context, blocked: bool) {
        if let Some(editor) = &mut self.petscii {
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
                self.petscii_toolbar(ui);
            });
        egui::TopBottomPanel::bottom("status")
            .exact_height(chrome::STATUS_HEIGHT)
            .frame(egui::Frame::new().fill(panel_fill))
            .show(context, |ui| {
                if blocked {
                    ui.disable();
                }
                self.petscii_status_bar(ui);
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
                    self.petscii_panel(ui);
                });
        }
    }

    fn petscii_toolbar(&mut self, ui: &mut egui::Ui) {
        let Some((brush, set)) = self.petscii.as_ref().map(|editor| (editor.brush, editor.fkey_set)) else {
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
        let colors = self.petscii_colors();
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
                    let (red, green, blue) = self.document.with_state(|state| state.get_buffer().palette.rgb(cell.attribute.foreground()));
                    self.screen_glyph(ui, code, egui::Vec2::splat(26.0), (egui::Color32::from_rgb(red, green, blue), colors.1));
                    ui.monospace(format!("#{code:02X}"));
                } else {
                    ui.weak(fl!("vt52-pipette-hint"));
                }
            }
            if tool == Tool::Pencil || tool == Tool::Fill || tool.is_shape_tool() {
                let mut mode = self.document.brush.primary;
                let options = [
                    (BrushPrimaryMode::Char, fl!("vt52-brush-char"), fl!("vt52-brush-char-tooltip")),
                    (BrushPrimaryMode::Colorize, fl!("vt52-brush-color"), fl!("petscii-brush-color-tooltip")),
                ];
                if widgets::segmented(ui, &mut mode, &options) {
                    self.document.brush.primary = mode;
                }
            }
            widgets::divider(ui);
            (picked, step) = self.screen_fkey_bar(ui, &FKEY_SETS[set], (set, FKEY_SETS.len()), colors);
        });
        if step != 0 {
            if let Some(editor) = &mut self.petscii {
                editor.fkey_set = (editor.fkey_set as i32 + step).rem_euclid(FKEY_SETS.len() as i32) as usize;
            }
        }
        if let Some(code) = picked {
            self.pick_petscii(code);
        }
    }

    fn petscii_panel(&mut self, ui: &mut egui::Ui) {
        let Some(brush) = self.petscii.as_ref().map(|editor| editor.brush) else {
            return;
        };
        chrome::section(ui, |ui| {
            widgets::section_header(ui, &fl!("vt52-colors"), |_| {});
            // The grid's right click sets the caret background; here it colors the screen.
            let before = self.document.with_state(|state| state.get_caret().attribute.background());
            self.palette_grid(ui, 8.0 * SWATCH);
            let after = self.document.with_state(|state| state.get_caret().attribute.background());
            if after != before {
                self.set_petscii_background(after);
            }
            ui.label(egui::RichText::new(fl!("petscii-colors-hint")).small().weak());
        });
        let mut picked = None;
        chrome::section(ui, |ui| {
            widgets::section_header(ui, &fl!("atascii-characters"), |_| {});
            let mut reverse = self.document.inverse;
            let options = [
                (false, fl!("atascii-normal"), fl!("atascii-normal-tooltip")),
                (true, fl!("petscii-reverse"), fl!("petscii-reverse-tooltip")),
            ];
            if widgets::segmented(ui, &mut reverse, &options) {
                self.document.inverse = reverse;
            }
            ui.add_space(4.0);
            let page = if self.document.inverse { 0x80 } else { 0 };
            let codes: Vec<u8> = (0..128u8).map(|index| page | index).collect();
            picked = self.character_map(ui, &codes, Some(brush), self.petscii_colors());
        });
        if let Some(code) = picked {
            self.pick_petscii(code);
        }
        chrome::section(ui, |ui| {
            widgets::section_header(ui, &fl!("atascii-screen"), |_| {});
            let (current_machine, current_case) = self.petscii_charset();
            let (mut machine, mut case) = (current_machine, current_case);
            let machines = MACHINES.map(|machine| (machine, machine_name(machine), machine_name(machine)));
            widgets::segmented(ui, &mut machine, &machines);
            ui.add_space(4.0);
            let cases = CASES.map(|case| (case, case_name(case), fl!("petscii-case-tooltip")));
            widgets::segmented(ui, &mut case, &cases);
            if (machine, case) != (current_machine, current_case) {
                self.set_petscii_charset(machine, case);
            }
        });
        let signature = self.document.with_state(|state| chrome::signature(state.get_buffer()));
        self.layers(ui, signature);
    }

    fn petscii_status_bar(&mut self, ui: &mut egui::Ui) {
        let (size, caret, selection) = self.document.with_state(|state| {
            (
                state.get_buffer().size(),
                state.get_caret().position(),
                state.selection().map(|selection| selection.as_rectangle()),
            )
        });
        let (machine, case) = self.petscii_charset();
        let reverse = self.document.inverse;
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
            if reverse {
                ui.label(small("·".into()).weak());
                ui.label(small(fl!("petscii-reverse")).color(icy_engine_gui::egui::appearance::PRIMARY));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.add_space(6.0);
                self.zoom_status_button(ui);
                chrome::status_separator(ui);
                ui.label(small(format!("{} · {}", machine_name(machine), case_name(case))));
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_draw::screen_profile::ScreenProfile;
    use icy_engine::{Position, Size};

    fn petscii_app(machine: PetsciiMachine, case: PetsciiCase) -> (egui::Context, DrawApp) {
        super::super::tests::use_english();
        let context = egui::Context::default();
        icy_engine_gui::egui::appearance::apply(&context);
        let mut app = DrawApp::new();
        app.new_petscii = (machine, case);
        app.create(super::super::NewKind::Petscii, Size::new(80, 25));
        (context, app)
    }

    fn cell(app: &DrawApp, x: i32, y: i32) -> (u32, u32, u32) {
        app.document.with_state(|state| {
            let ch = state.get_buffer().char_at(Position::new(x, y));
            (ch.ch as u32, ch.attribute.foreground(), ch.attribute.background())
        })
    }

    #[test]
    fn petscii_screens_open_in_their_own_editor() {
        let (_, app) = petscii_app(PetsciiMachine::C128, PetsciiCase::Lower);
        assert!(app.petscii.is_some() && app.vt52.is_none() && app.atascii.is_none());
        assert_eq!(app.document.profile(), ScreenProfile::Petscii(PetsciiMachine::C128, PetsciiCase::Lower));
        assert_eq!(app.document.with_state(|state| state.get_buffer().width()), 40);
    }

    #[test]
    fn typing_follows_the_character_set_and_reverse() {
        let (_, mut app) = petscii_app(PetsciiMachine::C64, PetsciiCase::Lower);
        app.document.type_text("Aa").unwrap();
        app.document.inverse = true;
        app.document.type_text("a").unwrap();
        assert_eq!([cell(&app, 0, 0).0, cell(&app, 1, 0).0, cell(&app, 2, 0).0], [65, 1, 0x81]);
        assert_eq!(cell(&app, 0, 0).1, 14, "light blue text");
        assert_eq!(cell(&app, 0, 0).2, 6, "on the blue screen");
    }

    #[test]
    fn the_background_colors_the_whole_screen_in_one_undo_step() {
        let (_, mut app) = petscii_app(PetsciiMachine::C64, PetsciiCase::Upper);
        app.document.type_text("HI").unwrap();
        app.set_petscii_background(0);
        assert_eq!((cell(&app, 0, 0).2, cell(&app, 39, 24).2), (0, 0));
        app.document.type_text("!").unwrap();
        assert_eq!(cell(&app, 2, 0).2, 0, "new characters get the screen color");
        app.undo(false);
        app.undo(false);
        assert_eq!(cell(&app, 39, 24).2, 6);
    }

    #[test]
    fn the_character_set_switches_the_whole_screen() {
        let (_, mut app) = petscii_app(PetsciiMachine::C64, PetsciiCase::Upper);
        app.document.type_text("A").unwrap();
        app.set_petscii_charset(PetsciiMachine::C128, PetsciiCase::Lower);
        assert_eq!(app.document.profile(), ScreenProfile::Petscii(PetsciiMachine::C128, PetsciiCase::Lower));
        assert_eq!(cell(&app, 0, 0).0, 1, "the screen codes stay");
        app.undo(false);
        assert_eq!(app.document.profile(), ScreenProfile::Petscii(PetsciiMachine::C64, PetsciiCase::Upper));
    }

    #[test]
    fn function_keys_and_the_pipette_use_screen_codes() {
        let (context, mut app) = petscii_app(PetsciiMachine::C64, PetsciiCase::Upper);
        let size = egui::vec2(1280.0, 820.0);
        super::super::tests::frame(&context, &mut app, size, vec![]);
        super::super::tests::frame(
            &context,
            &mut app,
            size,
            vec![super::super::tests::key_event(egui::Key::F3, egui::Modifiers::NONE)],
        );
        assert_eq!(cell(&app, 0, 0).0, u32::from(FKEY_SETS[0][2]));
        app.document.with_state(|state| state.set_caret_foreground(2));
        app.document.inverse = true;
        app.document.type_text("x").unwrap();
        app.document.inverse = false;
        app.document.with_state(|state| state.set_caret_foreground(1));
        app.document.tool = Tool::Line;
        super::super::tests::frame(&context, &mut app, size, vec![]);
        app.document.tool = Tool::Pipette;
        super::super::tests::frame(&context, &mut app, size, vec![]);
        app.pipette_petscii(Position::new(1, 0));
        assert_eq!(app.petscii.as_ref().unwrap().brush, 0x98);
        assert!(app.document.inverse);
        assert_eq!(app.document.with_state(|state| state.get_caret().attribute.foreground()), 2);
        assert_eq!(app.document.tool, Tool::Line);
    }

    #[test]
    fn petscii_documents_export_as_seq_and_petmate_workspaces_open() {
        let (_, mut app) = petscii_app(PetsciiMachine::C64, PetsciiCase::Upper);
        app.document.type_text("HI").unwrap();
        let directory = tempfile::tempdir().unwrap();
        app.settings.last_export_directory = Some(directory.path().to_path_buf());
        app.settings.export_settings = Default::default();
        let request = app.export_dialog().request().unwrap();
        assert_eq!(request.format, icy_engine::FileFormat::Petscii, "SEQ is preselected");
        assert_eq!(request.path.extension().unwrap(), "seq");
        app.export(&request).unwrap();
        let exported = Document::load(&request.path).unwrap();
        assert!(matches!(exported.profile(), ScreenProfile::Petscii(_, PetsciiCase::Upper)));
        assert_eq!(exported.with_state(|state| state.get_buffer().char_at(Position::new(1, 0)).ch as u32), 9);

        let workspace = directory.path().join("art.petmate");
        let cells: Vec<String> = (0..3).map(|x| format!(r#"{{"code":{},"color":{}}}"#, [8, 9, 0xA0][x], [1, 2, 3][x])).collect();
        std::fs::write(
            &workspace,
            format!(
                r#"{{"version":4,"framebufs":[{{"width":3,"height":1,"backgroundColor":11,"borderColor":0,"charset":"c128Lower","framebuf":[[{}]]}}]}}"#,
                cells.join(",")
            ),
        )
        .unwrap();
        // The typed text is unsaved, so load without the question to save it.
        app.load_path(workspace);
        assert!(app.petscii.is_some() && app.dialog.is_none());
        assert_eq!(app.document.profile(), ScreenProfile::Petscii(PetsciiMachine::C128, PetsciiCase::Lower));
        assert_eq!([cell(&app, 0, 0), cell(&app, 2, 0)], [(8, 1, 11), (0xA0, 3, 11)]);
    }
}
