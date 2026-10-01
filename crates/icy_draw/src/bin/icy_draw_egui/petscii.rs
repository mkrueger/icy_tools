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

pub const MACHINES: [PetsciiMachine; 7] = PetsciiMachine::ALL;

/// Whether the machine draws in its monitor's single color.
fn monochrome(machine: PetsciiMachine) -> bool {
    matches!(machine, PetsciiMachine::Pet | PetsciiMachine::Pet80)
}
pub const CASES: [PetsciiCase; 2] = [PetsciiCase::Upper, PetsciiCase::Lower];

/// The side of a color swatch.
const SWATCH: f32 = 30.0;

/// Petmate's block characters (rectangles, L shapes, checkers), from which fading picks; 76, 79
/// and 80 are letters in the lower case set.
const BLOCKS: [u8; 25] = [
    76, 79, 80, 97, 98, 99, 100, 101, 102, 103, 104, 106, 108, 111, 116, 117, 118, 119, 120, 121, 122, 123, 124, 126, 127,
];

/// What the pencil and shapes do with the cells they touch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaintMode {
    Char,
    Color,
    Reverse,
    Fade,
}

/// The block characters of `font` from empty to full, one per number of set pixels, at most
/// sixteen, for fading like Petmate: the left button darkens, the right one lightens.
pub fn fade_ramp(font: &icy_engine::BitFont, case: PetsciiCase) -> icy_draw::brush::CharRamp {
    let weight = |code: u8| font.glyph(char::from(code)).to_bitmap_pixels().iter().flatten().filter(|&&on| on).count();
    let mut by_weight: std::collections::BTreeMap<usize, u8> = std::collections::BTreeMap::new();
    let letters = |code: u8| case == PetsciiCase::Lower && (65..=90).contains(&code);
    for code in BLOCKS
        .iter()
        .copied()
        .filter(|&code| !letters(code))
        .flat_map(|code| [code, code | 0x80])
        .chain([0xA0])
    {
        let weight = weight(code);
        if weight > 0 {
            by_weight.entry(weight).or_insert(code);
        }
    }
    let codes: Vec<u8> = by_weight.into_values().collect();
    let max = icy_draw::brush::MAX_RAMP_LEN;
    let picked: Vec<char> = if codes.len() <= max {
        codes.iter().map(|&code| char::from(code)).collect()
    } else {
        (0..max).map(|step| char::from(codes[step * (codes.len() - 1) / (max - 1)])).collect()
    };
    icy_draw::brush::CharRamp::new(&picked)
}

pub fn machine_name(machine: PetsciiMachine) -> String {
    match machine {
        PetsciiMachine::C64 => "C64".to_owned(),
        PetsciiMachine::C128 => "C128".to_owned(),
        PetsciiMachine::Vic20 => "VIC-20".to_owned(),
        PetsciiMachine::Pet => "PET".to_owned(),
        PetsciiMachine::C16 => "C16".to_owned(),
        PetsciiMachine::Pet80 => "PET 80".to_owned(),
        PetsciiMachine::C128Vdc => "C128 VDC".to_owned(),
    }
}

pub fn case_name(case: PetsciiCase) -> String {
    match case {
        PetsciiCase::Upper => fl!("petscii-case-upper"),
        PetsciiCase::Lower => fl!("petscii-case-lower"),
    }
}

/// A workspace's screens and the one picked to open.
#[derive(Clone, Debug)]
pub struct PetmatePick {
    pub path: std::path::PathBuf,
    pub screens: Vec<PetmateScreen>,
    pub selected: usize,
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

/// A screen of a Petmate workspace, as the import lists it.
#[derive(Clone, Debug)]
pub struct PetmateScreen {
    pub name: String,
    pub width: i64,
    pub height: i64,
    pub charset: String,
    /// Petmate's 40 or 80 column mode, set for PETs.
    pub columns: Option<i64>,
}

impl PetmateScreen {
    /// Whether the screen is 80 columns: PETs say so, older C128 screens are just as wide.
    fn eighty_columns(&self) -> bool {
        self.columns.map_or(self.width >= 80, |columns| columns == 80)
    }

    /// The machine and character set of the screen, `None` for those this editor lacks.
    pub fn charset(&self) -> Option<(PetsciiMachine, PetsciiCase)> {
        let wide = self.eighty_columns();
        match self.charset.as_str() {
            "petGfx" if wide => Some((PetsciiMachine::Pet80, PetsciiCase::Upper)),
            "petBiz" if wide => Some((PetsciiMachine::Pet80, PetsciiCase::Lower)),
            "c128Upper" if wide => Some((PetsciiMachine::C128Vdc, PetsciiCase::Upper)),
            "c128Lower" if wide => Some((PetsciiMachine::C128Vdc, PetsciiCase::Lower)),
            "upper" | "dirart" | "cbaseUpper" => Some((PetsciiMachine::C64, PetsciiCase::Upper)),
            "lower" | "cbaseLower" => Some((PetsciiMachine::C64, PetsciiCase::Lower)),
            "c128Upper" => Some((PetsciiMachine::C128, PetsciiCase::Upper)),
            "c128Lower" => Some((PetsciiMachine::C128, PetsciiCase::Lower)),
            "vic20Upper" => Some((PetsciiMachine::Vic20, PetsciiCase::Upper)),
            "vic20Lower" => Some((PetsciiMachine::Vic20, PetsciiCase::Lower)),
            "petGfx" => Some((PetsciiMachine::Pet, PetsciiCase::Upper)),
            "petBiz" => Some((PetsciiMachine::Pet, PetsciiCase::Lower)),
            "c16Upper" => Some((PetsciiMachine::C16, PetsciiCase::Upper)),
            "c16Lower" => Some((PetsciiMachine::C16, PetsciiCase::Lower)),
            "c128vdc" => Some((PetsciiMachine::C128Vdc, PetsciiCase::Upper)),
            _ => None,
        }
    }
}

fn machine_is_vdc(screen: &PetmateScreen) -> bool {
    screen.charset().is_some_and(|(machine, _)| machine == PetsciiMachine::C128Vdc)
}

/// The workspace at `path` and the screens in it.
fn petmate_workspace(path: &std::path::Path) -> Result<Vec<serde_json::Value>, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let workspace: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let screens = workspace.get("framebufs").and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
    if screens.is_empty() {
        return Err(fl!("petmate-no-screen"));
    }
    Ok(screens)
}

/// The screens of the Petmate workspace at `path`.
pub fn petmate_screens(path: &std::path::Path) -> Result<Vec<PetmateScreen>, String> {
    Ok(petmate_workspace(path)?
        .iter()
        .enumerate()
        .map(|(index, screen)| PetmateScreen {
            name: screen
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map_or_else(|| fl!("petmate-screen", number = (index + 1)), str::to_owned),
            width: screen.get("width").and_then(serde_json::Value::as_i64).unwrap_or(40),
            height: screen.get("height").and_then(serde_json::Value::as_i64).unwrap_or(25),
            charset: screen.get("charset").and_then(serde_json::Value::as_str).unwrap_or("upper").to_owned(),
            columns: screen.get("columnMode").and_then(serde_json::Value::as_i64),
        })
        .collect())
}

/// Loads screen `index` of a Petmate workspace (.petmate) as a PETSCII document.
pub fn load_petmate(path: &std::path::Path, index: usize) -> Result<Document, String> {
    let screens = petmate_workspace(path)?;
    let screen = screens.get(index).ok_or_else(|| fl!("petmate-no-screen"))?;
    let number = |key: &str| screen.get(key).and_then(serde_json::Value::as_i64);
    let charset = screen.get("charset").and_then(serde_json::Value::as_str).unwrap_or("upper");
    let listed = PetmateScreen {
        name: String::new(),
        width: number("width").unwrap_or(40),
        height: 0,
        charset: charset.to_owned(),
        columns: number("columnMode"),
    };
    // C128 screens 80 columns wide from before Petmate had the VDC: one set for the whole
    // screen, colors already numbered like the VDC's (as Petmate migrates them).
    let legacy_vdc = matches!(charset, "c128Upper" | "c128Lower") && machine_is_vdc(&listed);
    let (machine, case) = listed.charset().ok_or_else(|| fl!("petmate-unsupported-charset", charset = charset))?;
    let width = number("width").unwrap_or(40).clamp(1, 1000) as i32;
    let height = number("height").unwrap_or(25).clamp(1, 1000) as i32;
    // The C16's colors are luminance × 16 + hue; bit 7 is flashing, which is not kept.
    let colors = machine.palette().len() as u64;
    let color_of = |value: u64| (if colors > 16 { value & 0x7F } else { value & 0x0F }).min(colors - 1) as u32;
    let background = color_of(number("backgroundColor").unwrap_or(0) as u64);
    let text = machine.start_colors().0;
    let mut buffer = icy_engine::petscii_buffer(machine, case, icy_engine::Size::new(width, height), text, background);
    let rows = screen.get("framebuf").and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
    for (y, row) in rows.iter().take(height as usize).enumerate() {
        for (x, cell) in row.as_array().into_iter().flatten().take(width as usize).enumerate() {
            let code = cell.get("code").and_then(serde_json::Value::as_u64).unwrap_or(0x20);
            // Transparent cells: a flag, or code 256 (512 on the VDC, whose codes 256-511 are the
            // alternate set).
            let vdc = machine.charset_per_character() && !legacy_vdc;
            let transparent = cell.get("transparent").and_then(serde_json::Value::as_bool).unwrap_or(false) || code >= if vdc { 512 } else { 256 };
            let alternate = vdc && (256..512).contains(&code);
            let color = color_of(cell.get("color").and_then(serde_json::Value::as_u64).unwrap_or(u64::from(text)));
            let mut code = if transparent { b' ' } else { code as u8 };
            let mut ch = icy_engine::AttributedChar::new(' ', icy_engine::TextAttribute::default());
            ch.attribute.set_foreground(color);
            ch.attribute.set_background(background);
            if legacy_vdc {
                ch.attribute.set_font_page(u8::from(case == PetsciiCase::Lower));
            } else if machine.charset_per_character() {
                // The VDC's attribute byte: alternate set, reverse, underline, flashing, color.
                let attribute = cell.get("attr").and_then(serde_json::Value::as_u64).unwrap_or(u64::from(color));
                ch.attribute.set_foreground((attribute & 0x0F) as u32);
                ch.attribute.set_is_blinking(attribute & 0x10 != 0);
                ch.attribute.set_is_underlined(attribute & 0x20 != 0);
                if attribute & 0x40 != 0 {
                    code ^= 0x80;
                }
                ch.attribute.set_font_page(u8::from(alternate || attribute & 0x80 != 0));
            }
            ch.ch = char::from(code);
            buffer.layers[0].set_char((x as i32, y as i32), ch);
        }
    }
    if machine.start_border().is_some() {
        buffer.border_color = number("borderColor").map(|border| color_of(border as u64).min(machine.border_colors() - 1));
    }
    let mut state = icy_engine_edit::EditState::from_buffer(buffer);
    state.set_caret_foreground(text);
    state.set_caret_background(background);
    Ok(Document::from_state(state))
}

impl DrawApp {
    /// Opens a Petmate workspace: its only screen, or the one picked from its screens.
    pub(super) fn open_petmate(&mut self, path: std::path::PathBuf) {
        match petmate_screens(&path) {
            Ok(screens) if screens.len() > 1 => {
                let selected = screens.iter().position(|screen| screen.charset().is_some()).unwrap_or(0);
                self.dialog = Some(super::Dialog::PetmateScreens(Box::new(PetmatePick { path, screens, selected })));
            }
            Ok(_) => self.open_petmate_screen(&path, 0),
            Err(error) => self.dialog = Some(super::Dialog::Error(error)),
        }
    }

    fn open_petmate_screen(&mut self, path: &std::path::Path, index: usize) {
        match load_petmate(path, index) {
            Ok(document) => self.replace(document),
            Err(error) => self.dialog = Some(super::Dialog::Error(error)),
        }
    }

    /// The list of a workspace's screens to open one of. Returns whether the dialog stays open.
    pub(super) fn petmate_dialog(&mut self, context: &egui::Context, pick: &mut PetmatePick) -> bool {
        #[derive(Clone, Copy)]
        enum Action {
            Cancel,
            Open,
        }
        let title = pick.path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let response = icy_engine_gui::egui::appearance::Dialog::new("petmate-screens")
            .title(title)
            .subtitle(fl!("petmate-pick-screen", count = pick.screens.len()))
            .size(icy_engine_gui::egui::appearance::DialogSize::Medium)
            .confirm_on_enter(true)
            .show(context, |dialog| {
                let mut open_now = false;
                dialog.content(|ui| {
                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        for (index, screen) in pick.screens.iter().enumerate() {
                            let supported = screen.charset().is_some();
                            let label = format!("{}. {}  ·  {} × {}  ·  {}", index + 1, screen.name, screen.width, screen.height, screen.charset);
                            let row = egui::Button::selectable(pick.selected == index, label).min_size(egui::vec2(ui.available_width(), 26.0));
                            let response = ui.add_enabled(supported, row);
                            if !supported {
                                response
                                    .clone()
                                    .on_disabled_hover_text(fl!("petmate-unsupported-charset", charset = screen.charset.as_str()));
                            }
                            if response.clicked() {
                                pick.selected = index;
                            }
                            if response.double_clicked() {
                                pick.selected = index;
                                open_now = true;
                            }
                        }
                    });
                });
                if open_now {
                    dialog.finish(Action::Open);
                }
                let open = pick.screens.get(pick.selected).is_some_and(|screen| screen.charset().is_some());
                dialog.buttons([
                    icy_engine_gui::egui::appearance::DialogButton::cancel(icy_engine_gui::egui::appearance::labels::cancel(), Action::Cancel),
                    icy_engine_gui::egui::appearance::DialogButton::primary(fl!("petmate-open"), Action::Open).enabled(open),
                ]);
            });
        match response.action {
            Some(Action::Open) => {
                let (path, index) = (pick.path.clone(), pick.selected);
                self.open_petmate_screen(&path, index);
                false
            }
            Some(Action::Cancel) => false,
            None => !response.dismissed,
        }
    }

    /// Edits the document, a PETSCII screen, with the PETSCII editor.
    pub(super) fn start_petscii(&mut self) {
        let editor = PetsciiEditor::default();
        self.document.inverse = false;
        self.document.quarter_blocks = false;
        self.document.reverse_pen = false;
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
            if !icy_engine::petscii_charset(state.get_buffer()).0.charset_per_character() {
                state.set_caret_font_page(0);
            }
        });
        self.petscii = Some(editor);
    }

    /// The set new characters are typed in: the screen's, or on the VDC the caret's.
    fn petscii_typed_case(&self) -> PetsciiCase {
        self.document.with_state(|state| {
            let (machine, case) = icy_engine::petscii_charset(state.get_buffer());
            if machine.charset_per_character() && state.get_caret().attribute.font_page() == 1 {
                PetsciiCase::Lower
            } else {
                case
            }
        })
    }

    fn petscii_charset(&self) -> (PetsciiMachine, PetsciiCase) {
        self.document.with_state(|state| icy_engine::petscii_charset(state.get_buffer()))
    }

    /// The border stored for a PETSCII screen, falling back to the machine's start-up color for
    /// older files. The PETs and the VDC have none.
    pub(super) fn petscii_border(&self) -> Option<u32> {
        self.petscii.as_ref()?;
        let (machine, _) = self.petscii_charset();
        let start = machine.start_border()?;
        Some(
            self.document
                .with_state(|state| state.get_buffer().border_color)
                .map_or(start, |border| border.min(machine.border_colors() - 1)),
        )
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
            if self.petscii_charset().0.charset_per_character() {
                self.document.with_state(|state| {
                    let mut attribute = state.get_caret().attribute;
                    attribute.set_font_page(cell.attribute.font_page());
                    attribute.set_is_blinking(cell.attribute.is_blinking());
                    attribute.set_is_underlined(cell.attribute.is_underlined());
                    state.set_caret_attribute(attribute);
                });
            }
        }
    }

    /// Sets the explicit screen color and mirrors it in visible characters, in one undo step.
    pub(super) fn set_petscii_background(&mut self, color: u32) {
        self.document.finish();
        let result = self.document.with_state(|state| {
            let _undo = state.begin_atomic_undo(fl!("petscii-screen", color = color));
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
            state.set_background_color(Some(color))?;
            state.set_caret_background(color);
            Ok::<(), icy_engine::EngineError>(())
        });
        self.result(result.map_err(|error| error.to_string()));
    }

    /// Shows the screen with the character set of `machine` in `case`, like switching it on the
    /// machine: the characters stay, their look changes.
    pub(super) fn set_petscii_charset(&mut self, machine: PetsciiMachine, case: PetsciiCase) {
        let (current, current_case) = self.petscii_charset();
        if (current, current_case) == (machine, case) {
            return;
        }
        self.document.finish();
        let font = icy_engine::petscii_font(machine, case);
        if current == machine {
            if machine.charset_per_character() {
                // The VDC picks the set per character: new characters use the chosen one.
                self.document
                    .with_state(|state| state.set_caret_font_page(u8::from(case == PetsciiCase::Lower)));
            } else {
                self.edit(|state| {
                    let _undo = state.begin_atomic_undo(fl!("petscii-case-tooltip"));
                    state.set_font_in_slot(0, font)?;
                    state.set_machine_mode(Some(icy_engine::MachineMode::Petscii { machine, charset: case }))
                });
            }
            return;
        }
        // Another machine: its width, character set and colors in one undo step. Text colors
        // become the nearest the machine has; screen and border start as on the machine, like
        // Petmate does.
        let palette = machine.palette();
        let text_colors = machine.text_colors();
        let nearest = |old: &icy_engine::Palette, index: u32, limit: u32| {
            let (red, green, blue) = old.rgb(index);
            (0..limit.min(palette.len() as u32))
                .min_by_key(|&candidate| {
                    let (r, g, b) = palette.rgb(candidate);
                    let delta = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
                    delta(r, red) + delta(g, green) + delta(b, blue)
                })
                .unwrap_or(0)
        };
        let result = self.document.with_state(|state| {
            let _undo = state.begin_atomic_undo(fl!("petscii-machine"));
            let height = state.get_buffer().height();
            state.resize_buffer(true, icy_engine::Size::new(machine.screen_size().width, height))?;
            if machine.charset_per_character() {
                state.set_font_in_slot(0, icy_engine::petscii_font(machine, PetsciiCase::Upper))?;
                state.set_font_in_slot(1, icy_engine::petscii_font(machine, PetsciiCase::Lower))?;
            } else {
                if current.charset_per_character() {
                    state.remove_font(1)?;
                }
                state.set_font_in_slot(0, font)?;
            }
            state.set_font_dimensions(icy_engine::petscii_font(machine, case).size())?;
            let old = state.get_buffer().palette.clone();
            let text = |index: u32| if monochrome(machine) { 1 } else { nearest(&old, index, text_colors) };
            let (_, start_screen) = machine.start_colors();
            let attributes = machine.charset_per_character();
            let mut layers = state.get_buffer().layers.clone();
            for layer in &mut layers {
                for line in &mut layer.lines {
                    for ch in line.chars.iter_mut().filter(|ch| ch.is_visible()) {
                        let foreground = text(ch.attribute.foreground());
                        ch.attribute.set_foreground(foreground);
                        ch.attribute.set_background(start_screen);
                        if !attributes {
                            // Blinking and underlining are the VDC's.
                            ch.attribute.set_is_blinking(false);
                            ch.attribute.set_is_underlined(false);
                        }
                    }
                }
            }
            let caret = state.get_caret().attribute;
            state.switch_to_palette_with_layers(palette.clone(), layers)?;
            state.set_caret_foreground(text(caret.foreground()));
            state.set_caret_background(start_screen);
            state.set_caret_font_page(if attributes { u8::from(case == PetsciiCase::Lower) } else { 0 });
            state.set_border_color(machine.start_border())?;
            state.set_background_color(Some(start_screen))?;
            state.set_machine_mode(Some(icy_engine::MachineMode::Petscii {
                machine,
                charset: if attributes { PetsciiCase::Upper } else { case },
            }))?;
            Ok::<(), icy_engine::EngineError>(())
        });
        self.result(result.map_err(|error| error.to_string()));
    }

    /// Swatches of the first `count` colors; returns the clicked one.
    fn color_swatches(&self, ui: &mut egui::Ui, count: u32, current: u32) -> Option<u32> {
        let palette = self.document.with_state(|state| state.get_buffer().palette.clone());
        let columns = if count > 16 { 16 } else { 8 };
        let mut picked = None;
        ui.spacing_mut().item_spacing = egui::Vec2::splat(2.0);
        for row in 0..count.div_ceil(columns) {
            ui.horizontal(|ui| {
                for index in (row * columns..((row + 1) * columns).min(count)).filter(|&index| (index as usize) < palette.len()) {
                    let (red, green, blue) = palette.rgb(index);
                    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(18.0), egui::Sense::click());
                    ui.painter().rect_filled(rect, 3, egui::Color32::from_rgb(red, green, blue));
                    if index == current {
                        ui.painter()
                            .rect_stroke(rect, 3, egui::Stroke::new(2.0, egui::Color32::WHITE), egui::StrokeKind::Inside);
                    } else if response.hovered() {
                        ui.painter()
                            .rect_stroke(rect, 3, ui.visuals().widgets.hovered.fg_stroke, egui::StrokeKind::Inside);
                    }
                    if response.on_hover_text(index.to_string()).clicked() {
                        picked = Some(index);
                    }
                }
            });
        }
        picked
    }

    /// The border and the screen color like on Petmate: the screen inside its border; clicking
    /// either picks its color.
    fn border_and_screen(&mut self, ui: &mut egui::Ui, machine: PetsciiMachine) {
        let border = self.petscii_border().unwrap_or(0);
        let (background, palette) = self.document.with_state(|state| {
            let buffer = state.get_buffer();
            (icy_engine::petscii_background(buffer), buffer.palette.clone())
        });
        let color = |index: u32| {
            let (red, green, blue) = palette.rgb(index);
            egui::Color32::from_rgb(red, green, blue)
        };
        let mut picked_border = None;
        let mut picked_background = None;
        ui.horizontal(|ui| {
            let (outer, _) = ui.allocate_exact_size(egui::vec2(64.0, 46.0), egui::Sense::hover());
            let inner = outer.shrink2(egui::vec2(12.0, 10.0));
            let border_response = ui.interact(outer, ui.id().with("petscii-border"), egui::Sense::click());
            let screen_response = ui.interact(inner, ui.id().with("petscii-screen"), egui::Sense::click());
            let painter = ui.painter();
            painter.rect_filled(outer, 4, color(border));
            painter.rect_filled(inner, 2, color(background));
            let edge = ui.visuals().widgets.noninteractive.bg_stroke;
            painter.rect_stroke(outer, 4, edge, egui::StrokeKind::Inside);
            if screen_response.hovered() {
                painter.rect_stroke(inner, 2, egui::Stroke::new(2.0, egui::Color32::WHITE), egui::StrokeKind::Inside);
            } else if border_response.hovered() {
                painter.rect_stroke(outer, 4, egui::Stroke::new(2.0, egui::Color32::WHITE), egui::StrokeKind::Inside);
            }
            let border_response = border_response.on_hover_text(fl!("petscii-border-tooltip"));
            let screen_response = screen_response.on_hover_text(fl!("petscii-screen-tooltip"));
            egui::Popup::menu(&border_response).show(|ui| {
                picked_border = self.color_swatches(ui, machine.border_colors(), border);
                if picked_border.is_some() {
                    ui.close();
                }
            });
            egui::Popup::menu(&screen_response).show(|ui| {
                picked_background = self.color_swatches(ui, palette.len() as u32, background);
                if picked_background.is_some() {
                    ui.close();
                }
            });
            ui.vertical(|ui| {
                ui.label(fl!("petscii-border", color = border));
                ui.label(fl!("petscii-screen", color = background));
            });
        });
        if let Some(border) = picked_border {
            self.edit(|state| state.set_border_color(Some(border)));
        }
        if let Some(background) = picked_background {
            self.set_petscii_background(background);
        }
    }

    /// The color of a PET monitor's phosphor, which draws all text.
    fn set_pet_phosphor(&mut self, (red, green, blue): (u8, u8, u8)) {
        let mut palette = self.document.with_state(|state| state.get_buffer().palette.clone());
        palette.set_color(1, icy_engine::Color::new(red, green, blue));
        self.edit(|state| state.switch_to_palette(palette));
    }

    /// The colors of the machine: the PET's monitor, or the palette for text and screen.
    fn petscii_colors_section(&mut self, ui: &mut egui::Ui) {
        let (machine, _) = self.petscii_charset();
        if monochrome(machine) {
            widgets::section_header(ui, &fl!("petscii-monitor"), |_| {});
            let current = self.document.with_state(|state| state.get_buffer().palette.rgb(1));
            let mut phosphor = icy_engine::PET_PHOSPHORS.iter().position(|&color| color == current).unwrap_or(0);
            let names = [fl!("petscii-monitor-green"), fl!("petscii-monitor-white"), fl!("petscii-monitor-amber")];
            let options: Vec<(usize, String, String)> = names.into_iter().enumerate().map(|(index, name)| (index, name.clone(), name)).collect();
            if widgets::segmented(ui, &mut phosphor, &options) {
                self.set_pet_phosphor(icy_engine::PET_PHOSPHORS[phosphor]);
            }
            ui.label(egui::RichText::new(fl!("petscii-monitor-hint")).small().weak());
            return;
        }
        widgets::section_header(ui, &fl!("vt52-colors"), |_| {});
        if machine.start_border().is_some() {
            self.border_and_screen(ui, machine);
            ui.add_space(6.0);
        }
        // The grid's right click sets the caret background; here it colors the screen.
        let (foreground, background) = self.document.with_state(|state| {
            let attribute = state.get_caret().attribute;
            (attribute.foreground(), attribute.background())
        });
        let count = self.document.with_state(|state| state.get_buffer().palette.len());
        let width = if count > 16 { ui.available_width() } else { 8.0 * SWATCH };
        self.palette_grid(ui, width);
        let (picked, screen) = self.document.with_state(|state| {
            let attribute = state.get_caret().attribute;
            (attribute.foreground(), attribute.background())
        });
        if picked != foreground && picked >= machine.text_colors() {
            // The VIC-20's characters have the first eight colors only.
            let result = self.document.set_caret_foreground(foreground);
            self.result(result);
        }
        if screen != background {
            self.set_petscii_background(screen);
        }
        let hint = match machine {
            PetsciiMachine::Vic20 => fl!("petscii-colors-hint-vic20"),
            PetsciiMachine::C16 => fl!("petscii-colors-hint-c16"),
            _ => fl!("petscii-colors-hint"),
        };
        ui.label(egui::RichText::new(hint).small().weak());
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
            if tool == Tool::Fill {
                let mut mode = self.document.brush.primary;
                let options = [
                    (BrushPrimaryMode::Char, fl!("vt52-brush-char"), fl!("vt52-brush-char-tooltip")),
                    (BrushPrimaryMode::Colorize, fl!("vt52-brush-color"), fl!("petscii-brush-color-tooltip")),
                ];
                if widgets::segmented(ui, &mut mode, &options) {
                    self.document.brush.primary = mode;
                }
            } else if (tool == Tool::Pencil || tool.is_shape_tool()) && !self.document.draws_boxes() {
                self.petscii_paint_options(ui);
            }
            if matches!(tool, Tool::Line | Tool::RectangleOutline) {
                self.petscii_outline_options(ui);
            }
            widgets::divider(ui);
            if self.document.tool == Tool::Select {
                // Selecting by character or color and filling with the brush replaces characters.
                let context = ui.ctx().clone();
                self.selection_options(ui, &context);
            } else {
                (picked, step) = self.screen_fkey_bar(ui, &FKEY_SETS[set], (set, FKEY_SETS.len()), colors);
            }
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

    fn petscii_paint_mode(&self) -> PaintMode {
        if self.document.reverse_pen {
            PaintMode::Reverse
        } else {
            match self.document.brush.primary {
                BrushPrimaryMode::Colorize => PaintMode::Color,
                BrushPrimaryMode::Shading => PaintMode::Fade,
                _ => PaintMode::Char,
            }
        }
    }

    fn apply_petscii_paint_mode(&mut self, mode: PaintMode) {
        self.document.reverse_pen = mode == PaintMode::Reverse;
        self.document.brush.primary = match mode {
            PaintMode::Color => BrushPrimaryMode::Colorize,
            PaintMode::Fade => BrushPrimaryMode::Shading,
            PaintMode::Char | PaintMode::Reverse => BrushPrimaryMode::Char,
        };
        if mode == PaintMode::Fade {
            let (font, case) = self.document.with_state(|state| {
                let buffer = state.get_buffer();
                (buffer.font(0).cloned(), icy_engine::petscii_charset(buffer).1)
            });
            if let Some(font) = font {
                self.document.brush.shade_chars = fade_ramp(&font, case);
                self.document.brush.shade_colors = icy_draw::brush::ColorRamp::default();
            }
        }
    }

    /// The modes of the pencil and shapes, and drawing in quarter block pixels.
    fn petscii_paint_options(&mut self, ui: &mut egui::Ui) {
        let mut mode = self.petscii_paint_mode();
        let options = [
            (PaintMode::Char, fl!("vt52-brush-char"), fl!("vt52-brush-char-tooltip")),
            (PaintMode::Color, fl!("vt52-brush-color"), fl!("petscii-brush-color-tooltip")),
            (PaintMode::Reverse, fl!("petscii-reverse"), fl!("petscii-reverse-pen-tooltip")),
            (PaintMode::Fade, fl!("petscii-fade"), fl!("petscii-fade-tooltip")),
        ];
        if widgets::segmented(ui, &mut mode, &options) {
            self.apply_petscii_paint_mode(mode);
        }
        let has_quarters = self
            .document
            .with_state(|state| state.get_buffer().font(0).and_then(icy_draw::quarter_blocks::QuarterBlocks::of).is_some());
        if mode == PaintMode::Char && has_quarters {
            ui.toggle_value(&mut self.document.quarter_blocks, fl!("atascii-pixels"))
                .on_hover_text(fl!("atascii-pixels-tooltip"));
        }
    }

    /// The Outline mode of lines and rectangles, with rounded corners where the set has them.
    fn petscii_outline_options(&mut self, ui: &mut egui::Ui) {
        use icy_draw::box_lines::{BoxStyle, LineSet};
        let mut outline = self.document.box_line.is_some();
        if ui
            .toggle_value(&mut outline, fl!("line-style-outline"))
            .on_hover_text(fl!("line-style-outline-tooltip"))
            .changed()
        {
            self.document.box_line = outline.then_some(BoxStyle::Single);
        }
        let styles = self.document.with_state(|state| LineSet::of(state.get_buffer()).styles());
        if let Some(mut style) = self.document.box_line {
            if !styles.contains(&style) {
                style = BoxStyle::Single;
                self.document.box_line = Some(style);
            }
            if styles.len() > 1 {
                let options: Vec<(BoxStyle, String, String)> = styles
                    .iter()
                    .map(|&style| match style {
                        BoxStyle::Rounded => (style, fl!("petscii-corners-round"), fl!("petscii-corners-round-tooltip")),
                        _ => (style, fl!("petscii-corners-square"), fl!("line-style-single-tooltip")),
                    })
                    .collect();
                if widgets::segmented(ui, &mut style, &options) {
                    self.document.box_line = Some(style);
                }
            }
        }
    }

    fn petscii_panel(&mut self, ui: &mut egui::Ui) {
        let Some(brush) = self.petscii.as_ref().map(|editor| editor.brush) else {
            return;
        };
        chrome::section(ui, |ui| {
            self.petscii_colors_section(ui);
        });
        let mut picked = None;
        chrome::section(ui, |ui| {
            widgets::section_header(ui, &fl!("atascii-characters"), |_| {});
            self.petscii_character_toggles(ui);
            ui.add_space(6.0);
            let page = if self.document.inverse { 0x80 } else { 0 };
            let codes: Vec<u8> = (0..128u8).map(|index| page | index).collect();
            picked = self.character_map(ui, &codes, Some(brush), self.petscii_colors());
        });
        if let Some(code) = picked {
            self.pick_petscii(code);
        }
        chrome::section(ui, |ui| {
            widgets::section_header(ui, &fl!("atascii-screen"), |_| {});
            let (current, case) = self.petscii_charset();
            let mut machine = current;
            ui.horizontal(|ui| {
                ui.label(fl!("petscii-machine"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::ComboBox::from_id_salt("petscii-machine")
                        .selected_text(machine_name(machine))
                        .show_ui(ui, |ui| {
                            for candidate in MACHINES {
                                ui.selectable_value(&mut machine, candidate, machine_name(candidate));
                            }
                        });
                });
            });
            if machine != current {
                let case = if current.charset_per_character() { self.petscii_typed_case() } else { case };
                self.set_petscii_charset(machine, case);
            }
        });
        let signature = self.document.with_state(|state| chrome::signature(state.get_buffer()));
        self.layers(ui, signature);
    }

    /// The character set and attributes new characters are typed and drawn in, as toggles side by
    /// side: reverse, the lower case set and, on the VDC, flashing and underlining.
    fn petscii_character_toggles(&mut self, ui: &mut egui::Ui) {
        let (machine, screen_case) = self.petscii_charset();
        let per_character = machine.charset_per_character();
        let case = if per_character { self.petscii_typed_case() } else { screen_case };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            widgets::toggle(ui, &fl!("petscii-reverse"), &mut self.document.inverse, &fl!("petscii-reverse-tooltip"));
            let mut lower = case == PetsciiCase::Lower;
            let tooltip = if per_character {
                fl!("petscii-case-vdc-tooltip")
            } else {
                fl!("petscii-case-tooltip")
            };
            if widgets::toggle(ui, &fl!("petscii-lower-case"), &mut lower, &tooltip).changed() {
                let case = if lower { PetsciiCase::Lower } else { PetsciiCase::Upper };
                self.set_petscii_charset(machine, case);
            }
        });
        if per_character {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let mut attribute = self.document.with_state(|state| state.get_caret().attribute);
                let (mut blink, mut underline) = (attribute.is_blinking(), attribute.is_underlined());
                let blink_changed = widgets::toggle(ui, &fl!("petscii-blink"), &mut blink, &fl!("petscii-blink-tooltip")).changed();
                let underline_changed = widgets::toggle(ui, &fl!("petscii-underline"), &mut underline, &fl!("petscii-underline-tooltip")).changed();
                if blink_changed || underline_changed {
                    attribute.set_is_blinking(blink);
                    attribute.set_is_underlined(underline);
                    self.document.with_state(|state| state.set_caret_attribute(attribute));
                }
            });
        }
    }

    fn petscii_status_bar(&mut self, ui: &mut egui::Ui) {
        let (size, caret, selection) = self.document.with_state(|state| {
            (
                state.get_buffer().size(),
                state.get_caret().position(),
                state.selection().map(|selection| selection.as_rectangle()),
            )
        });
        let (machine, _) = self.petscii_charset();
        let case = self.petscii_typed_case();
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
        assert_eq!(app.document.with_state(|state| state.get_buffer().background_color), Some(0));
        assert_eq!((cell(&app, 0, 0).2, cell(&app, 39, 24).2), (0, 0));
        app.document.type_text("!").unwrap();
        assert_eq!(cell(&app, 2, 0).2, 0, "new characters get the screen color");
        app.undo(false);
        app.undo(false);
        assert_eq!(cell(&app, 39, 24).2, 6);
        assert_eq!(app.document.with_state(|state| state.get_buffer().background_color), Some(6));
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

    fn stroke(app: &mut DrawApp, from: (i32, i32), to: (i32, i32), button: icy_engine::MouseButton) {
        app.document.begin(Position::new(from.0, from.1), button);
        app.document.update(Position::new(to.0, to.1));
        app.document.finish();
    }

    #[test]
    fn chunky_pixels_use_the_c64_quarter_blocks_with_diagonals() {
        let (_, mut app) = petscii_app(PetsciiMachine::C64, PetsciiCase::Upper);
        app.document.tool = Tool::Pencil;
        app.document.quarter_blocks = true;
        stroke(&mut app, (0, 0), (0, 0), icy_engine::MouseButton::Left);
        stroke(&mut app, (1, 1), (1, 1), icy_engine::MouseButton::Left);
        assert_eq!(cell(&app, 0, 0).0, 0x7F, "the upper left and lower right quarter");
        app.document.tool = Tool::Line;
        stroke(&mut app, (0, 4), (7, 4), icy_engine::MouseButton::Left);
        assert_eq!((0..4).map(|x| cell(&app, x, 2).0).collect::<Vec<_>>(), [0xE2; 4], "a line of upper halves");
    }

    #[test]
    fn outline_lines_join_with_the_c64_line_characters() {
        let (_, mut app) = petscii_app(PetsciiMachine::C64, PetsciiCase::Upper);
        app.document.tool = Tool::Line;
        app.document.box_line = Some(icy_draw::box_lines::BoxStyle::Single);
        stroke(&mut app, (0, 1), (4, 1), icy_engine::MouseButton::Left);
        stroke(&mut app, (2, 0), (2, 3), icy_engine::MouseButton::Left);
        stroke(&mut app, (0, 3), (2, 3), icy_engine::MouseButton::Left);
        assert_eq!((0..5).map(|x| cell(&app, x, 1).0).collect::<Vec<_>>(), [0x40, 0x40, 0x5B, 0x40, 0x40]);
        assert_eq!([cell(&app, 2, 0).0, cell(&app, 2, 2).0, cell(&app, 2, 3).0], [0x5D, 0x5D, 0x7D], "│ │ ┘");
    }

    #[test]
    fn outline_rectangles_have_square_or_round_corners() {
        let (_, mut app) = petscii_app(PetsciiMachine::C64, PetsciiCase::Upper);
        app.document.tool = Tool::RectangleOutline;
        app.document.box_line = Some(icy_draw::box_lines::BoxStyle::Rounded);
        app.document.begin(Position::new(0, 0), icy_engine::MouseButton::Left);
        app.document.update(Position::new(3, 2));
        let preview = app.document.preview_cells.clone();
        app.document.finish();
        let row = |app: &DrawApp, y: i32| (0..4).map(|x| cell(app, x, y).0).collect::<Vec<_>>();
        assert_eq!(row(&app, 0), [0x55, 0x40, 0x40, 0x49]);
        assert_eq!(row(&app, 1), [0x5D, 0x20, 0x20, 0x5D]);
        assert_eq!(row(&app, 2), [0x4A, 0x40, 0x40, 0x4B]);
        assert_eq!(preview.len(), 10, "the preview is the frame");
        // A square frame across it joins at the crossings.
        app.document.box_line = Some(icy_draw::box_lines::BoxStyle::Single);
        app.document.begin(Position::new(2, 1), icy_engine::MouseButton::Left);
        app.document.update(Position::new(5, 3));
        app.document.finish();
        assert_eq!(cell(&app, 3, 1).0, 0x5B, "┼ where the frames cross");
        assert_eq!(cell(&app, 2, 2).0, 0x5B);
    }

    #[test]
    fn selecting_a_character_and_filling_with_the_brush_replaces_it() {
        let (_, mut app) = petscii_app(PetsciiMachine::C64, PetsciiCase::Upper);
        app.document.type_text("ABAB").unwrap();
        // Select every A: the select tool's character mode.
        app.document.tool = Tool::Select;
        app.document.selection_mode = icy_draw::document::SelectionMode::Character;
        app.document.begin(Position::new(0, 0), icy_engine::MouseButton::Left);
        app.document.finish();
        // Replace them with the brush: a reverse space in red.
        app.document.brush.paint_char = char::from(0xA0);
        app.document.with_state(|state| state.set_caret_foreground(2));
        app.document.fill_selection().unwrap();
        assert_eq!(
            (0..4).map(|x| cell(&app, x, 0)).collect::<Vec<_>>(),
            [(0xA0, 2, 6), (2, 14, 6), (0xA0, 2, 6), (2, 14, 6)]
        );
        app.undo(false);
        assert_eq!(cell(&app, 0, 0), (1, 14, 6), "one undo step");
        // Only the color.
        app.apply_petscii_paint_mode(PaintMode::Color);
        app.document.fill_selection().unwrap();
        assert_eq!([cell(&app, 0, 0), cell(&app, 1, 0)], [(1, 2, 6), (2, 14, 6)]);
    }

    #[test]
    fn the_reverse_pen_turns_characters_reverse_and_back() {
        let (_, mut app) = petscii_app(PetsciiMachine::C64, PetsciiCase::Upper);
        app.document.type_text("HELLO").unwrap();
        app.document.with_state(|state| state.set_caret_foreground(2));
        app.document.tool = Tool::Pencil;
        app.document.reverse_pen = true;
        stroke(&mut app, (0, 0), (2, 0), icy_engine::MouseButton::Left);
        assert_eq!((0..4).map(|x| cell(&app, x, 0).0).collect::<Vec<_>>(), [0x88, 0x85, 0x8C, 0x0C]);
        assert_eq!(cell(&app, 0, 0).1, 14, "the colors stay");
        stroke(&mut app, (1, 0), (1, 0), icy_engine::MouseButton::Right);
        assert_eq!(cell(&app, 1, 0).0, 0x05);
    }

    #[test]
    fn fading_steps_through_denser_blocks() {
        let font = icy_engine::petscii_font(PetsciiMachine::C64, PetsciiCase::Upper);
        let ramp = fade_ramp(&font, PetsciiCase::Upper);
        let weight = |ch: char| font.glyph(ch).to_bitmap_pixels().iter().flatten().filter(|&&on| on).count();
        let weights: Vec<usize> = ramp.as_slice().iter().map(|&ch| weight(ch)).collect();
        assert!(weights.windows(2).all(|pair| pair[0] < pair[1]), "{weights:?}");
        assert_eq!(*weights.last().unwrap(), 64, "it ends with the full block");
        assert!(ramp.len() <= icy_draw::brush::MAX_RAMP_LEN && ramp.len() >= 8);

        let (_, mut app) = petscii_app(PetsciiMachine::C64, PetsciiCase::Upper);
        app.document.tool = Tool::Pencil;
        app.apply_petscii_paint_mode(PaintMode::Fade);
        let first = app.document.brush.shade_chars.as_slice()[0] as u32;
        stroke(&mut app, (0, 0), (0, 0), icy_engine::MouseButton::Left);
        stroke(&mut app, (0, 0), (0, 0), icy_engine::MouseButton::Left);
        let second = app.document.brush.shade_chars.as_slice()[1] as u32;
        assert_eq!(cell(&app, 0, 0).0, second, "two strokes darken twice");
        stroke(&mut app, (0, 0), (0, 0), icy_engine::MouseButton::Right);
        assert_eq!(cell(&app, 0, 0).0, first, "the right button lightens");
    }

    #[test]
    fn workspaces_with_several_screens_ask_which_one_to_open() {
        let (_, mut app) = petscii_app(PetsciiMachine::C64, PetsciiCase::Upper);
        let directory = tempfile::tempdir().unwrap();
        let workspace = directory.path().join("frames.petmate");
        let screen = |name: &str, charset: &str, code: u8| {
            format!(
                r#"{{"name":"{name}","width":2,"height":1,"backgroundColor":0,"borderColor":0,"charset":"{charset}","framebuf":[[{{"code":{code},"color":1}},{{"code":32,"color":1}}]]}}"#
            )
        };
        std::fs::write(
            &workspace,
            format!(
                r#"{{"version":4,"framebufs":[{},{},{}]}}"#,
                screen("custom", "customFont1", 1),
                screen("one", "upper", 2),
                screen("two", "lower", 3)
            ),
        )
        .unwrap();
        app.load_path(workspace.clone());
        let Some(super::super::Dialog::PetmateScreens(pick)) = &app.dialog else {
            panic!("the screens are listed");
        };
        assert_eq!(
            pick.screens.iter().map(|screen| screen.name.as_str()).collect::<Vec<_>>(),
            ["custom", "one", "two"]
        );
        assert_eq!(pick.selected, 1, "the first screen this editor can open is picked");
        app.dialog = None;
        app.open_petmate_screen(&workspace, 2);
        assert_eq!(app.document.profile(), ScreenProfile::Petscii(PetsciiMachine::C64, PetsciiCase::Lower));
        assert_eq!(cell(&app, 0, 0).0, 3);
        assert!(load_petmate(&workspace, 0).is_err(), "screens in custom character sets are refused");
    }

    #[test]
    fn every_machine_starts_with_its_screen_and_colors() {
        for (machine, size, colors) in [
            (PetsciiMachine::C64, Size::new(40, 25), (14, 6)),
            (PetsciiMachine::Vic20, Size::new(22, 23), (6, 1)),
            (PetsciiMachine::Pet, Size::new(40, 25), (1, 0)),
            (PetsciiMachine::C16, Size::new(40, 25), (0, 0x71)),
        ] {
            let (_, mut app) = petscii_app(machine, PetsciiCase::Upper);
            assert_eq!(app.document.with_state(|state| state.get_buffer().size()), size, "{machine:?}");
            app.document.type_text("A").unwrap();
            assert_eq!((cell(&app, 0, 0).1, cell(&app, 0, 0).2), colors, "{machine:?}");
        }
    }

    #[test]
    fn switching_the_machine_maps_size_and_colors_in_one_undo_step() {
        let (_, mut app) = petscii_app(PetsciiMachine::C64, PetsciiCase::Upper);
        app.document.with_state(|state| state.set_caret_foreground(13));
        app.document.type_text("AB").unwrap();
        app.set_petscii_charset(PetsciiMachine::Vic20, PetsciiCase::Lower);
        assert_eq!(app.document.profile(), ScreenProfile::Petscii(PetsciiMachine::Vic20, PetsciiCase::Lower));
        assert_eq!(app.document.with_state(|state| state.get_buffer().width()), 22);
        let (code, text, _) = cell(&app, 0, 0);
        assert_eq!(code, 1, "the screen codes stay");
        assert!(text < 8, "light green becomes one of the VIC-20's eight text colors, not {text}");
        app.set_petscii_charset(PetsciiMachine::Pet, PetsciiCase::Upper);
        assert_eq!((cell(&app, 1, 0).1, cell(&app, 1, 0).2), (1, 0), "the PET draws in its phosphor on black");
        app.undo(false);
        app.undo(false);
        assert_eq!(app.document.profile(), ScreenProfile::Petscii(PetsciiMachine::C64, PetsciiCase::Upper));
        assert_eq!((cell(&app, 0, 0).1, app.document.with_state(|state| state.get_buffer().width())), (13, 40));
    }

    #[test]
    fn the_border_follows_the_machine_and_is_one_undo_step() {
        let (_, mut app) = petscii_app(PetsciiMachine::C64, PetsciiCase::Upper);
        assert_eq!(app.petscii_border(), Some(14), "light blue, as the C64 starts");
        app.edit(|state| state.set_border_color(Some(2)));
        assert_eq!(app.petscii_border(), Some(2));
        app.set_petscii_charset(PetsciiMachine::Vic20, PetsciiCase::Upper);
        assert_eq!(app.petscii_border(), Some(3), "cyan, as the VIC-20 starts");
        assert_eq!(cell(&app, 0, 0).2, 1, "on its white screen");
        app.set_petscii_charset(PetsciiMachine::Pet, PetsciiCase::Upper);
        assert_eq!(app.petscii_border(), None, "the PET has no border");
        app.set_petscii_charset(PetsciiMachine::C128, PetsciiCase::Upper);
        assert_eq!(app.petscii_border(), Some(13), "the C128 starts with light green");
        assert_eq!(cell(&app, 0, 0).2, 11, "on dark gray");
        app.undo(false);
        app.undo(false);
        app.undo(false);
        app.undo(false);
        assert_eq!(app.petscii_border(), Some(14));
        assert_eq!(cell(&app, 0, 0).2, 6);
        app.replace(Document::new(icy_engine::Size::new(80, 25)));
        assert_eq!(app.petscii_border(), None, "only PETSCII screens have one");
    }

    #[test]
    fn the_pet_monitor_color_is_one_undo_step() {
        let (_, mut app) = petscii_app(PetsciiMachine::Pet, PetsciiCase::Upper);
        let phosphor = |app: &DrawApp| app.document.with_state(|state| state.get_buffer().palette.rgb(1));
        app.set_pet_phosphor(icy_engine::PET_PHOSPHORS[2]);
        assert_eq!(phosphor(&app), icy_engine::PET_PHOSPHORS[2]);
        app.undo(false);
        assert_eq!(phosphor(&app), icy_engine::PET_PHOSPHORS[0]);
    }

    #[test]
    fn petmate_screens_of_the_vic20_and_c16_open_with_their_colors() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = directory.path().join("machines.petmate");
        std::fs::write(
            &workspace,
            r#"{"version":4,"framebufs":[
                {"width":2,"height":1,"backgroundColor":1,"borderColor":3,"charset":"vic20Upper","framebuf":[[{"code":1,"color":6},{"code":160,"color":2}]]},
                {"width":2,"height":1,"backgroundColor":113,"borderColor":0,"charset":"c16Lower","framebuf":[[{"code":1,"color":82},{"code":2,"color":198}]]}
            ]}"#,
        )
        .unwrap();
        let vic20 = load_petmate(&workspace, 0).unwrap();
        assert_eq!(vic20.profile(), ScreenProfile::Petscii(PetsciiMachine::Vic20, PetsciiCase::Upper));
        let c16 = load_petmate(&workspace, 1).unwrap();
        assert_eq!(c16.profile(), ScreenProfile::Petscii(PetsciiMachine::C16, PetsciiCase::Lower));
        let colors = |document: &Document, x: i32| {
            document.with_state(|state| {
                let ch = state.get_buffer().char_at(Position::new(x, 0));
                (ch.attribute.foreground(), ch.attribute.background())
            })
        };
        assert_eq!(colors(&vic20, 1), (2, 1));
        assert_eq!(vic20.with_state(|state| state.get_buffer().border_color), Some(3));
        assert_eq!(c16.with_state(|state| state.get_buffer().border_color), Some(0));
        assert_eq!(colors(&c16, 0), (82, 113), "luminance 5, hue 2 on white");
        assert_eq!(colors(&c16, 1), (70, 113), "flashing (bit 7) is dropped");
    }

    #[test]
    fn the_vdc_picks_the_character_set_and_attributes_per_character() {
        let (_, mut app) = petscii_app(PetsciiMachine::C128Vdc, PetsciiCase::Upper);
        assert_eq!(
            app.document
                .with_state(|state| (state.get_buffer().size(), state.get_buffer().font_dimensions())),
            (Size::new(80, 25), Size::new(8, 16))
        );
        app.document.type_text("A").unwrap();
        app.set_petscii_charset(PetsciiMachine::C128Vdc, PetsciiCase::Lower);
        app.document.with_state(|state| {
            let mut attribute = state.get_caret().attribute;
            attribute.set_is_underlined(true);
            state.set_caret_attribute(attribute);
        });
        app.document.type_text("A").unwrap();
        let attributes = |app: &DrawApp, x: i32| {
            app.document.with_state(|state| {
                let ch = state.get_buffer().char_at(Position::new(x, 0));
                (ch.ch as u32, ch.attribute.font_page(), ch.attribute.is_underlined())
            })
        };
        assert_eq!(attributes(&app, 0), (1, 0, false), "upper case A");
        assert_eq!(attributes(&app, 1), (65, 1, true), "A in the lower case set, underlined");
        assert_eq!(
            app.document.profile(),
            ScreenProfile::Petscii(PetsciiMachine::C128Vdc, PetsciiCase::Upper),
            "the screen keeps both sets"
        );

        app.set_petscii_charset(PetsciiMachine::C64, PetsciiCase::Upper);
        assert_eq!(app.document.with_state(|state| state.get_buffer().font_count()), 1);
        assert_eq!(attributes(&app, 1).1, 0, "one set for the whole screen");
        assert!(!attributes(&app, 1).2, "no underlining on the C64");
        app.undo(false);
        assert_eq!(attributes(&app, 1), (65, 1, true));
        assert_eq!(app.document.with_state(|state| state.get_buffer().font_count()), 2);
    }

    #[test]
    fn the_pet_80_has_80_columns_of_half_width_pixels() {
        let (_, app) = petscii_app(PetsciiMachine::Pet80, PetsciiCase::Lower);
        assert_eq!(app.document.profile(), ScreenProfile::Petscii(PetsciiMachine::Pet80, PetsciiCase::Lower));
        assert_eq!(
            app.document
                .with_state(|state| (state.get_buffer().size(), state.get_buffer().font_dimensions())),
            (Size::new(80, 25), Size::new(8, 16))
        );
    }

    #[test]
    fn petmate_80_column_pet_and_older_c128_screens_open_as_such() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = directory.path().join("wide.petmate");
        std::fs::write(
            &workspace,
            r#"{"version":4,"framebufs":[
                {"width":80,"height":1,"columnMode":80,"backgroundColor":0,"borderColor":0,"charset":"petBiz","framebuf":[[{"code":1,"color":1}]]},
                {"width":80,"height":1,"backgroundColor":0,"borderColor":0,"charset":"c128Lower","framebuf":[[{"code":1,"color":2}]]}
            ]}"#,
        )
        .unwrap();
        let pet = load_petmate(&workspace, 0).unwrap();
        assert_eq!(pet.profile(), ScreenProfile::Petscii(PetsciiMachine::Pet80, PetsciiCase::Lower));
        let c128 = load_petmate(&workspace, 1).unwrap();
        assert!(matches!(c128.profile(), ScreenProfile::Petscii(PetsciiMachine::C128Vdc, _)));
        let ch = c128.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)));
        assert_eq!(
            (ch.attribute.foreground(), ch.attribute.font_page()),
            (2, 1),
            "the color as Petmate keeps it, in the lower case set"
        );
    }

    #[test]
    fn petmate_vdc_screens_keep_their_attributes() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = directory.path().join("vdc.petmate");
        std::fs::write(
            &workspace,
            r#"{"version":4,"framebufs":[{"width":3,"height":1,"backgroundColor":0,"borderColor":0,"charset":"c128vdc","framebuf":[[
                {"code":1,"color":8,"attr":248},{"code":257,"color":15,"attr":15},{"code":512,"color":15,"attr":15}
            ]]}]}"#,
        )
        .unwrap();
        let document = load_petmate(&workspace, 0).unwrap();
        let cell = |x: i32| {
            document.with_state(|state| {
                let ch = state.get_buffer().char_at(Position::new(x, 0));
                (
                    ch.ch as u32,
                    ch.attribute.foreground(),
                    ch.attribute.font_page(),
                    ch.attribute.is_blinking(),
                    ch.attribute.is_underlined(),
                )
            })
        };
        assert_eq!(cell(0), (0x81, 8, 1, true, true), "reverse, alternate set, underline and flashing");
        assert_eq!(cell(1), (1, 15, 1, false, false), "code 257 is the alternate set's code 1");
        assert_eq!(cell(2).0, 0x20, "512 is transparent");
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
