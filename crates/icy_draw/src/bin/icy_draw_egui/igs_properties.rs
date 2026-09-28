//! The property panel of the IGS editor: typed parameter fields for drawing and attribute
//! commands, shared style widgets, and the escaping used to edit raw IGS source as text.

use eframe::egui;
use icy_draw::fl;
use icy_parser_core::{
    ArrowEnd, DrawingMode, IgsCommand, IgsParameter, LineKind, LineMarkerStyle, PaletteMode, PatternType, PauseType, PenType, PolymarkerKind, ScreenClearMode,
    TerminalResolution, TextEffects, TextRotation,
};

use super::palette;

const LABEL_WIDTH: f32 = 84.0;

/// Raw bytes as editable text: printable ASCII and newlines as they are, every other byte as
/// `\xNN`, so the text converts back without loss.
pub fn escape(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len());
    for &byte in bytes {
        match byte {
            b'\\' => text.push_str("\\\\"),
            b'\n' => text.push('\n'),
            b'\r' => text.push_str("\\r"),
            0x20..=0x7E => text.push(char::from(byte)),
            _ => text.push_str(&format!("\\x{byte:02X}")),
        }
    }
    text
}

/// The bytes of text written by [`escape`], or `None` if an escape is malformed.
pub fn unescape(text: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            bytes.push(u8::try_from(u32::from(c)).ok()?);
            continue;
        }
        match chars.next()? {
            '\\' => bytes.push(b'\\'),
            'r' => bytes.push(b'\r'),
            'n' => bytes.push(b'\n'),
            'e' => bytes.push(0x1B),
            'x' => {
                let hex: String = chars.by_ref().take(2).collect();
                bytes.push(u8::from_str_radix(&hex, 16).ok().filter(|_| hex.len() == 2)?);
            }
            _ => return None,
        }
    }
    Some(bytes)
}

fn label(ui: &mut egui::Ui, name: &str) {
    ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new(egui::RichText::new(name).weak()).truncate());
}

fn row(ui: &mut egui::Ui, name: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        label(ui, name);
        content(ui);
    });
}

/// A number field; random and loop parameters are shown but not changed.
fn parameter(ui: &mut egui::Ui, name: &str, value: &mut IgsParameter) {
    row(ui, name, |ui| match value {
        IgsParameter::Value(number) => {
            ui.add(egui::DragValue::new(number).range(-9999..=9999).clamp_existing_to_range(false));
        }
        other => {
            ui.label(egui::RichText::new(other.to_string()).monospace())
                .on_hover_text(fl!("igs-editor-variable-parameter"));
        }
    });
}

/// Values outside `range` read from a file are shown unchanged until they are edited.
fn number<T: egui::emath::Numeric>(ui: &mut egui::Ui, name: &str, value: &mut T, range: std::ops::RangeInclusive<T>) {
    row(ui, name, |ui| {
        ui.add(egui::DragValue::new(value).range(range).clamp_existing_to_range(false));
    });
}

fn combo<T: PartialEq + Copy>(ui: &mut egui::Ui, id: &str, value: &mut T, options: &[T], name: impl Fn(T) -> String) {
    egui::ComboBox::from_id_salt(id).selected_text(name(*value)).show_ui(ui, |ui| {
        for option in options {
            ui.selectable_value(value, *option, name(*option));
        }
    });
}

pub const LINE_KINDS: [LineKind; 6] = [
    LineKind::Solid,
    LineKind::LongDash,
    LineKind::Dotted,
    LineKind::DashDot,
    LineKind::Dashed,
    LineKind::DashDotDot,
];

pub const MARKERS: [PolymarkerKind; 6] = [
    PolymarkerKind::Point,
    PolymarkerKind::Plus,
    PolymarkerKind::Star,
    PolymarkerKind::Square,
    PolymarkerKind::DiagonalCross,
    PolymarkerKind::Diamond,
];

pub const MODES: [DrawingMode; 4] = [
    DrawingMode::Replace,
    DrawingMode::Transparent,
    DrawingMode::Xor,
    DrawingMode::ReverseTransparent,
];

pub fn line_kind(ui: &mut egui::Ui, id: &str, value: &mut LineKind) {
    combo(ui, id, value, &LINE_KINDS, super::line_kind_name);
}

pub fn marker(ui: &mut egui::Ui, id: &str, value: &mut PolymarkerKind) {
    combo(ui, id, value, &MARKERS, super::marker_name);
}

pub fn drawing_mode(ui: &mut egui::Ui, id: &str, value: &mut DrawingMode) {
    combo(ui, id, value, &MODES, super::drawing_mode_name);
}

/// Fill kind and, for patterns and hatches, their number.
pub fn pattern(ui: &mut egui::Ui, id: &str, value: &mut PatternType) {
    let kinds = [
        PatternType::Hollow,
        PatternType::Solid,
        PatternType::Pattern(1),
        PatternType::Hatch(1),
        PatternType::UserDefined(0),
        PatternType::Random,
        PatternType::StarTrek,
    ];
    let same_kind = |a: PatternType, b: PatternType| std::mem::discriminant(&a) == std::mem::discriminant(&b);
    let kind_name = |pattern: PatternType| match pattern {
        PatternType::Pattern(_) => fl!("igs-fill-kind-pattern"),
        PatternType::Hatch(_) => fl!("igs-fill-kind-hatch"),
        PatternType::UserDefined(_) => fl!("igs-fill-kind-user"),
        other => super::pattern_name(other),
    };
    egui::ComboBox::from_id_salt(id).selected_text(kind_name(*value)).show_ui(ui, |ui| {
        for kind in kinds {
            if ui.selectable_label(same_kind(*value, kind), kind_name(kind)).clicked() && !same_kind(*value, kind) {
                *value = kind;
            }
        }
    });
    match value {
        PatternType::Pattern(index) => {
            ui.add(egui::DragValue::new(index).range(1..=24).clamp_existing_to_range(false));
        }
        PatternType::Hatch(index) => {
            ui.add(egui::DragValue::new(index).range(1..=12).clamp_existing_to_range(false));
        }
        PatternType::UserDefined(index) => {
            ui.add(egui::DragValue::new(index).range(0..=7).clamp_existing_to_range(false));
        }
        _ => {}
    }
}

/// Toggles for the text effects.
pub fn text_effects(ui: &mut egui::Ui, value: &mut TextEffects) {
    for (flag, name) in [
        (TextEffects::THICKENED, fl!("igs-text-bold")),
        (TextEffects::GHOSTED, fl!("igs-text-light")),
        (TextEffects::SKEWED, fl!("igs-text-italic")),
        (TextEffects::UNDERLINED, fl!("igs-text-underlined")),
        (TextEffects::OUTLINED, fl!("igs-text-outlined")),
    ] {
        let mut on = value.contains(flag);
        if ui.toggle_value(&mut on, name).changed() {
            value.set(flag, on);
        }
    }
}

pub fn rotation(ui: &mut egui::Ui, value: &mut TextRotation) {
    let options = [
        TextRotation::Degrees0,
        TextRotation::Degrees90,
        TextRotation::Degrees180,
        TextRotation::Degrees270,
    ]
    .map(|rotation| (rotation, format!("{}°", rotation as u16 * 90), fl!("igs-text-rotation")));
    super::widgets::segmented(ui, value, &options);
}

fn text(ui: &mut egui::Ui, value: &mut Vec<u8>) {
    let mut edited = super::latin1(value);
    if ui.text_edit_singleline(&mut edited).changed() {
        *value = edited
            .chars()
            .filter_map(|c| u8::try_from(u32::from(c)).ok())
            .filter(|byte| *byte >= 0x20 && *byte != b'@')
            .collect();
    }
}

/// Typed fields for `command`; returns false if it only has a source view.
pub fn command(ui: &mut egui::Ui, command: &mut IgsCommand, palette: &icy_engine::Palette, resolution: TerminalResolution) -> bool {
    let polyline = matches!(command, IgsCommand::PolyLine { .. });
    match command {
        IgsCommand::PolymarkerPlot { x, y }
        | IgsCommand::FloodFill { x, y }
        | IgsCommand::LineDrawTo { x, y }
        | IgsCommand::SetDrawtoBegin { x, y }
        | IgsCommand::PositionCursor { x, y } => {
            parameter(ui, "x", x);
            parameter(ui, "y", y);
        }
        IgsCommand::Line { x1, y1, x2, y2 } | IgsCommand::FilledRectangle { x1, y1, x2, y2 } => {
            for (name, value) in [("x1", x1), ("y1", y1), ("x2", x2), ("y2", y2)] {
                parameter(ui, name, value);
            }
        }
        IgsCommand::Box { x1, y1, x2, y2, rounded } => {
            for (name, value) in [("x1", x1), ("y1", y1), ("x2", x2), ("y2", y2)] {
                parameter(ui, name, value);
            }
            ui.checkbox(rounded, fl!("igs-rounded"));
        }
        IgsCommand::RoundedRectangles { x1, y1, x2, y2, fill } => {
            for (name, value) in [("x1", x1), ("y1", y1), ("x2", x2), ("y2", y2)] {
                parameter(ui, name, value);
            }
            ui.checkbox(fill, fl!("igs-filled"));
        }
        IgsCommand::Circle { x, y, radius } => {
            for (name, value) in [("x", x), ("y", y), ("radius", radius)] {
                parameter(ui, name, value);
            }
        }
        IgsCommand::Arc {
            x,
            y,
            radius,
            start_angle,
            end_angle,
        }
        | IgsCommand::PieSlice {
            x,
            y,
            radius,
            start_angle,
            end_angle,
        } => {
            for (name, value) in [("x", x), ("y", y), ("radius", radius), ("start angle", start_angle), ("end angle", end_angle)] {
                parameter(ui, name, value);
            }
        }
        IgsCommand::Ellipse { x, y, x_radius, y_radius } => {
            for (name, value) in [("x", x), ("y", y), ("x radius", x_radius), ("y radius", y_radius)] {
                parameter(ui, name, value);
            }
        }
        IgsCommand::EllipticalArc {
            x,
            y,
            x_radius,
            y_radius,
            start_angle,
            end_angle,
        }
        | IgsCommand::EllipticalPieSlice {
            x,
            y,
            x_radius,
            y_radius,
            start_angle,
            end_angle,
        } => {
            for (name, value) in [
                ("x", x),
                ("y", y),
                ("x radius", x_radius),
                ("y radius", y_radius),
                ("start angle", start_angle),
                ("end angle", end_angle),
            ] {
                parameter(ui, name, value);
            }
        }
        IgsCommand::PolyLine { points } | IgsCommand::PolyFill { points } => {
            for (index, pair) in points.chunks_exact_mut(2).enumerate() {
                ui.label(fl!("igs-editor-vertex", index = ((index + 1) as u32)));
                let [x, y] = pair else { unreachable!("chunks of two") };
                parameter(ui, "x", x);
                parameter(ui, "y", y);
            }
            ui.horizontal(|ui| {
                if points.len() >= 2 && points.len() < super::MAX_POINTS * 2 && ui.button(fl!("igs-editor-add-vertex")).clicked() {
                    let last = points[points.len() - 2..].to_vec();
                    points.extend(last);
                }
                let minimum = if polyline { 4 } else { 6 };
                if points.len() > minimum && ui.button(fl!("igs-editor-remove-vertex")).clicked() {
                    points.truncate(points.len() - 2);
                }
            });
        }
        IgsCommand::WriteText { x, y, text: value } => {
            parameter(ui, "x", x);
            parameter(ui, "y", y);
            text(ui, value);
        }
        IgsCommand::ColorSet { pen, color } => {
            row(ui, &fl!("igs-pen-kind"), |ui| {
                combo(
                    ui,
                    "igs-color-set-pen",
                    pen,
                    &[PenType::Line, PenType::Fill, PenType::Text, PenType::Polymarker],
                    super::pen_type_name,
                );
            });
            row(ui, &fl!("igs-color"), |ui| {
                palette::pen_picker(ui, "property", palette, resolution, color, egui::vec2(30.0, 22.0));
                ui.add(egui::DragValue::new(color).range(0..=15).clamp_existing_to_range(false));
            });
        }
        IgsCommand::AttributeForFills { pattern_type, border } => {
            row(ui, &fl!("igs-fill"), |ui| pattern(ui, "igs-fill-property", pattern_type));
            ui.checkbox(border, fl!("igs-fill-border"));
        }
        IgsCommand::SetLineOrMarkerStyle { style } => match style {
            LineMarkerStyle::PolyMarkerSize(kind, size) => {
                row(ui, &fl!("igs-marker"), |ui| marker(ui, "igs-marker-property", kind));
                number(ui, &fl!("igs-size"), size, 1..=8);
            }
            LineMarkerStyle::LineThickness(kind, thickness) => {
                row(ui, &fl!("igs-line"), |ui| line_kind(ui, "igs-line-property", kind));
                number(ui, &fl!("igs-thickness"), thickness, 1..=41);
            }
            LineMarkerStyle::LineEndpoints(kind, left, right) => {
                row(ui, &fl!("igs-line"), |ui| line_kind(ui, "igs-line-property", kind));
                let ends = [ArrowEnd::Square, ArrowEnd::Arrow, ArrowEnd::Rounded];
                let end_name = |end: ArrowEnd| match end {
                    ArrowEnd::Square => fl!("igs-end-square"),
                    ArrowEnd::Arrow => fl!("igs-end-arrow"),
                    ArrowEnd::Rounded => fl!("igs-end-rounded"),
                };
                row(ui, &fl!("igs-end-start"), |ui| combo(ui, "igs-end-left", left, &ends, end_name));
                row(ui, &fl!("igs-end-end"), |ui| combo(ui, "igs-end-right", right, &ends, end_name));
            }
        },
        IgsCommand::SetPenColor { pen, red, green, blue } => {
            number(ui, &fl!("igs-pen-number"), pen, 0..=palette::pen_count(resolution).saturating_sub(1));
            number(ui, &fl!("igs-palette-red"), red, 0..=7);
            number(ui, &fl!("igs-palette-green"), green, 0..=7);
            number(ui, &fl!("igs-palette-blue"), blue, 0..=7);
        }
        IgsCommand::DrawingMode { mode } => row(ui, &fl!("igs-drawing-mode"), |ui| drawing_mode(ui, "igs-mode-property", mode)),
        IgsCommand::HollowSet { enabled } => {
            ui.checkbox(enabled, fl!("igs-command-hollow"));
        }
        IgsCommand::TextEffects {
            effects,
            size,
            rotation: value,
        } => {
            ui.horizontal_wrapped(|ui| text_effects(ui, effects));
            number(ui, &fl!("igs-size"), size, 1..=40);
            row(ui, &fl!("igs-text-rotation"), |ui| rotation(ui, value));
        }
        IgsCommand::SetResolution {
            resolution: value,
            palette: mode,
        } => {
            row(ui, &fl!("igs-resolution"), |ui| {
                combo(
                    ui,
                    "igs-resolution-property",
                    value,
                    &[TerminalResolution::Low, TerminalResolution::Medium, TerminalResolution::High],
                    super::resolution_name,
                );
            });
            row(ui, &fl!("igs-palette-mode"), |ui| {
                combo(
                    ui,
                    "igs-palette-mode-property",
                    mode,
                    &[PaletteMode::NoChange, PaletteMode::Desktop, PaletteMode::IgDefault, PaletteMode::VdiDefault],
                    |mode| format!("{mode:?}"),
                );
            });
        }
        IgsCommand::ScreenClear { mode } => row(ui, &fl!("igs-clear-mode"), |ui| {
            combo(
                ui,
                "igs-clear-property",
                mode,
                &[
                    ScreenClearMode::ClearAndHome,
                    ScreenClearMode::ClearHomeToToCursor,
                    ScreenClearMode::ClearCursorToBottom,
                    ScreenClearMode::ClearWholeScreen,
                    ScreenClearMode::ClearWholeScreenAndHome,
                    ScreenClearMode::QuickVt52Reset,
                ],
                |mode| format!("{mode:?}"),
            );
        }),
        IgsCommand::Pause { pause_type } => match pause_type {
            PauseType::Seconds(seconds) => number(ui, &fl!("igs-pause-seconds"), seconds, 0..=255),
            PauseType::VSync(vsyncs) => number(ui, &fl!("igs-pause-vsyncs"), vsyncs, 0..=9999),
            PauseType::MilliSeconds(_) => return false,
        },
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaped_source_converts_back_without_loss() {
        let bytes: Vec<u8> = (0..=255).collect();
        let text = escape(&bytes);
        assert!(text.contains("\\x1B") && text.contains("\\\\") && text.contains('\n'));
        assert_eq!(unescape(&text).unwrap(), bytes);
        assert_eq!(unescape("G#L>1,2,3,4:\\r\n").unwrap(), b"G#L>1,2,3,4:\r\n");
        assert!(unescape("\\x1").is_none());
        assert!(unescape("\\q").is_none());
        assert!(unescape("€").is_none());
    }
}
