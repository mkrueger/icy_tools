//! A deliberately narrow, self-contained single-object RIP clipboard format.
use icy_draw::{fl, rip_document::RipDocument};
use icy_parser_core::{FillStyle, LineStyle, RipCommand, WriteMode};

use super::{palette, patterns, shape_tool, Tool};

const MARKER: &str = "icy_draw single object 1";

/// Fixed-order state commands; neither resets nor drawing commands are replayed here.
pub struct State {
    pub commands: Vec<RipCommand>,
}

impl State {
    pub fn at(commands: &[RipCommand]) -> Result<Self, String> {
        let mut state = Self {
            commands: vec![
                RipCommand::SetPalette {
                    colors: palette::palette_at_end(commands).to_vec(),
                },
                RipCommand::ViewPort {
                    x0: 0,
                    y0: 0,
                    x1: 639,
                    y1: 349,
                },
                RipCommand::WriteMode { mode: WriteMode::Normal },
                RipCommand::Color { c: 7 },
                RipCommand::LineStyle {
                    style: LineStyle::Solid,
                    user_pat: 0,
                    thick: 1,
                },
                patterns::fill_command(FillStyle::DEFAULT_FILL_PATTERNS[12], 0),
                RipCommand::FillStyle {
                    pattern: FillStyle::Solid,
                    color: 0,
                },
                RipCommand::FontStyle {
                    font: 0,
                    direction: 0,
                    size: 4,
                    res: 0,
                },
                RipCommand::ButtonStyle {
                    wid: 0,
                    hgt: 0,
                    orient: 2,
                    flags: 0,
                    bevsize: 0,
                    dfore: 0,
                    dback: 0,
                    bright: 0,
                    dark: 0,
                    surface: 0,
                    grp_no: 0,
                    flags2: 0,
                    uline_col: 0,
                    corner_col: 0,
                    res: 0,
                },
            ],
        };
        let mut formatted = false;
        for command in commands {
            let slot = match command {
                RipCommand::ViewPort { .. } => Some(1),
                RipCommand::WriteMode { .. } => Some(2),
                RipCommand::Color { .. } => Some(3),
                RipCommand::LineStyle { .. } => Some(4),
                RipCommand::FillPattern { col, .. } => {
                    state.commands[6] = RipCommand::FillStyle {
                        pattern: FillStyle::User,
                        color: *col,
                    };
                    Some(5)
                }
                RipCommand::FillStyle { .. } => Some(6),
                RipCommand::FontStyle { .. } => Some(7),
                RipCommand::ButtonStyle { .. } => Some(8),
                RipCommand::ResetWindows => {
                    state.commands[1] = RipCommand::ViewPort {
                        x0: 0,
                        y0: 0,
                        x1: 639,
                        y1: 349,
                    };
                    state.commands[3] = RipCommand::Color { c: 7 };
                    if let RipCommand::LineStyle { style, user_pat, .. } = &mut state.commands[4] {
                        *style = LineStyle::Solid;
                        *user_pat = 0;
                    }
                    state.commands[5] = patterns::fill_command(FillStyle::DEFAULT_FILL_PATTERNS[12], 0);
                    state.commands[6] = RipCommand::FillStyle {
                        pattern: FillStyle::Solid,
                        color: 0,
                    };
                    if let RipCommand::FontStyle { font, size, .. } = &mut state.commands[7] {
                        *font = 2;
                        *size = 4;
                    }
                    formatted = false;
                    None
                }
                RipCommand::BeginText { .. } => {
                    formatted = true;
                    None
                }
                RipCommand::EndText => {
                    formatted = false;
                    None
                }
                RipCommand::ReadScene { .. } => return Err(fl!("rip-clipboard-dependent")),
                // Vector text restores the line style through BGI's built-in pattern table.
                RipCommand::Text { .. } | RipCommand::TextXY { .. } | RipCommand::Button { .. } => {
                    if matches!(state.commands[7], RipCommand::FontStyle { font, .. } if font != 0) {
                        if let RipCommand::LineStyle {
                            style: LineStyle::User,
                            user_pat,
                            ..
                        } = &mut state.commands[4]
                        {
                            *user_pat = 0xffff;
                        }
                    }
                    None
                }
                _ => None,
            };
            if let Some(slot) = slot {
                state.commands[slot] = command.clone();
            }
        }
        if formatted {
            return Err(fl!("rip-clipboard-dependent"));
        }
        Ok(state)
    }

    pub fn palette(&self) -> &[u16] {
        match &self.commands[0] {
            RipCommand::SetPalette { colors } => colors,
            _ => unreachable!(),
        }
    }
}

fn safe_object(state: &State, object: &RipCommand) -> Result<(), String> {
    if shape_tool(object).is_none() || matches!(shape_tool(object), Some(Tool::Fill)) {
        return Err(fl!("rip-clipboard-dependent"));
    }
    if matches!(state.commands[2], RipCommand::WriteMode { mode: WriteMode::Xor }) && !matches!(object, RipCommand::Mouse { .. }) {
        return Err(fl!("rip-clipboard-dependent"));
    }
    if matches!(object, RipCommand::Button { .. })
        && matches!(state.commands[8], RipCommand::ButtonStyle { flags, .. } if flags & (super::button::ICON | super::button::CLIPBOARD | super::button::HOT_ICONS) != 0)
    {
        return Err(fl!("rip-clipboard-dependent"));
    }
    Ok(())
}

pub fn encode(commands: &[RipCommand], index: usize) -> Result<String, String> {
    let object = commands.get(index).ok_or_else(|| fl!("rip-clipboard-selection"))?;
    let mut state = State::at(&commands[..index])?;
    // Palette changes recolor previously drawn pixels too.
    state.commands[0] = State::at(commands)?.commands.remove(0);
    safe_object(&state, object)?;
    let mut block = vec![RipCommand::Comment { text: MARKER.into() }];
    block.extend(state.commands);
    block.push(object.clone());
    let mut document = RipDocument::new();
    document.try_append_many(block).map_err(|error| error.to_string())?;
    let text = String::from_utf8(document.to_bytes().map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
    decode(&text)?;
    Ok(text)
}

pub fn decode(text: &str) -> Result<(State, RipCommand), String> {
    if text.len() > 65_536 {
        return Err(fl!("rip-clipboard-invalid"));
    }
    let document = RipDocument::from_bytes(text.as_bytes()).map_err(|_| fl!("rip-clipboard-invalid"))?;
    let commands = document.commands();
    if document.preserved_commands() != 0
        || commands.len() != 11
        || !matches!(&commands[0], RipCommand::Comment { text } if text == MARKER)
        || !matches!(&commands[1], RipCommand::SetPalette { colors } if colors.len() == 16 && colors.iter().all(|c| *c < 64))
        || !matches!(commands[2], RipCommand::ViewPort { x0, y0, x1, y1 } if x0 <= x1 && y0 <= y1 && x1 < 640 && y1 < 350)
        || !matches!(commands[3], RipCommand::WriteMode { .. })
        || !matches!(commands[4], RipCommand::Color { c } if c < 16)
        || !matches!(commands[5], RipCommand::LineStyle { thick: 1 | 3, .. })
        || !matches!(commands[6], RipCommand::FillPattern { c1,c2,c3,c4,c5,c6,c7,c8,col } if [c1,c2,c3,c4,c5,c6,c7,c8].iter().all(|row| *row < 256) && col < 16)
        || !matches!(commands[7], RipCommand::FillStyle { color, .. } if color < 16)
        || !matches!(commands[8], RipCommand::FontStyle { font, direction, size, res: 0 } if font <= 10 && direction <= 1 && (1..=10).contains(&size))
        || !matches!(commands[9], RipCommand::ButtonStyle { .. })
    {
        return Err(fl!("rip-clipboard-invalid"));
    }
    let state = State {
        commands: commands[1..10].to_vec(),
    };
    let object = commands[10].clone();
    safe_object(&state, &object)?;
    Ok((state, object))
}

#[cfg(test)]
mod tests {
    use super::super::{button, RipEditor};
    use super::*;
    use eframe::egui;
    use icy_engine::Screen;

    fn persisted(document: &RipDocument) -> RipDocument {
        RipDocument::from_bytes(&document.to_bytes().unwrap()).unwrap()
    }

    #[test]
    fn flood_fill_is_bounded_persistent_and_one_undo_step() {
        let mut editor = RipEditor::new();
        editor.document.append_many(vec![
            RipCommand::Color { c: 2 },
            RipCommand::Rectangle {
                x0: 10,
                y0: 10,
                x1: 30,
                y1: 30,
            },
        ]);
        let before = editor.document.commands().to_vec();
        editor.tool = Tool::Fill;
        editor.fill_color = 4;
        editor.border_color = 2;
        editor.add_shape((20, 20), (20, 20));
        assert_eq!(editor.document.commands().last(), Some(&RipCommand::Fill { x: 20, y: 20, border: 2 }));
        let preview = editor.document.preview().unwrap();
        assert_eq!(preview.pixel_index(20, 20), Some(4));
        assert_eq!(preview.pixel_index(10, 20), Some(2));
        assert_eq!(preview.pixel_index(9, 20), Some(0));
        assert_eq!(preview.rgba(), persisted(&editor.document).preview().unwrap().rgba());
        editor.undo(false);
        assert_eq!(editor.document.commands(), before);
        editor.undo(true);
        assert_eq!(editor.document.preview().unwrap().rgba(), preview.rgba());
    }

    #[test]
    fn custom_patterns_persist_render_and_avoid_redundant_state() {
        let mut editor = RipEditor::new();
        editor.tool = Tool::Line;
        editor.line_style = LineStyle::User;
        editor.line_pattern = 0xa55a;
        editor.draw_color = 3;
        editor.add_shape((0, 4), (100, 4));
        assert!(editor.document.commands().contains(&RipCommand::LineStyle {
            style: LineStyle::User,
            user_pat: 0xa55a,
            thick: 1
        }));
        assert!(editor.state_commands(Tool::Line).is_empty());
        let custom_line = editor.document.preview().unwrap();
        assert!(custom_line.indices()[4 * 640..4 * 640 + 100].contains(&0));
        assert!(custom_line.indices()[4 * 640..4 * 640 + 100].contains(&3));
        editor.tool = Tool::Bar;
        editor.fill_pattern = FillStyle::User;
        editor.custom_fill = [0x80, 0x40, 0x20, 0x10, 8, 4, 2, 1];
        editor.fill_color = 5;
        editor.add_shape((16, 16), (31, 31));
        assert!(editor.state_commands(Tool::Bar).is_empty());
        assert!(editor.document.commands().contains(&patterns::fill_command(editor.custom_fill, 5)));
        let preview = editor.document.preview().unwrap();
        assert_eq!(preview.pixel_index(16, 16), Some(5));
        assert_eq!(preview.pixel_index(17, 16), Some(0));
        assert_eq!(preview.rgba(), persisted(&editor.document).preview().unwrap().rgba());
        editor.undo(false);
        assert_eq!(editor.document.preview().unwrap().rgba(), custom_line.rgba());
        editor.undo(true);
        assert_eq!(editor.document.preview().unwrap().rgba(), preview.rgba());
        editor.custom_fill[0] = 0x81;
        assert_eq!(editor.state_commands(Tool::Bar), vec![patterns::fill_command(editor.custom_fill, 5)]);
    }

    #[test]
    fn clipboard_captures_effective_state_not_current_tool_settings() {
        let mut editor = RipEditor::new();
        editor.tool = Tool::Line;
        editor.draw_color = 6;
        editor.line_style = LineStyle::User;
        editor.line_pattern = 0x8181;
        editor.add_shape((30, 40), (100, 40));
        let index = editor.selected.unwrap();
        editor.document.append_many(vec![
            RipCommand::Color { c: 9 },
            RipCommand::LineStyle {
                style: LineStyle::Dashed,
                user_pat: 0,
                thick: 3,
            },
            RipCommand::FillStyle {
                pattern: FillStyle::Hatch,
                color: 12,
            },
            RipCommand::FontStyle {
                font: 3,
                direction: 1,
                size: 2,
                res: 0,
            },
        ]);
        let original = editor.document.preview().unwrap().rgba();
        let before = editor.document.commands().to_vec();
        let state_before = State::at(&before).unwrap().commands;
        let text = encode(&before, index).unwrap();
        let (state, _) = decode(&text).unwrap();
        assert!(state.commands.contains(&RipCommand::Color { c: 6 }));
        assert!(state.commands.contains(&RipCommand::LineStyle {
            style: LineStyle::User,
            user_pat: 0x8181,
            thick: 1
        }));
        editor.paste_object(&text);
        assert!(editor.error.is_none(), "{:?}", editor.error);
        assert_eq!(editor.document.preview().unwrap().rgba(), original);
        assert_eq!(State::at(editor.document.commands()).unwrap().commands, state_before);
        assert_eq!(persisted(&editor.document).preview().unwrap().rgba(), original);
        editor.undo(false);
        assert_eq!(editor.document.commands(), before);
        editor.undo(true);
        assert_eq!(editor.document.preview().unwrap().rgba(), original);
    }

    #[test]
    fn custom_fill_font_button_and_viewport_are_self_contained() {
        let mut source = RipDocument::new();
        let mut style = button::ButtonOptions::default();
        style.font = 3;
        style.font_size = 2;
        source.append_many(vec![
            RipCommand::ViewPort {
                x0: 5,
                y0: 5,
                x1: 250,
                y1: 120,
            },
            RipCommand::Color { c: 4 },
            patterns::fill_command([0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55], 6),
            style.font_style(),
            style.style(),
            style.button((20, 20), Some((140, 70))),
        ]);
        let text = encode(source.commands(), 5).unwrap();
        let (state, object) = decode(&text).unwrap();
        assert!(state.commands.contains(&style.style()));
        assert!(state.commands.contains(&style.font_style()));
        assert!(state.commands.contains(&source.commands()[0]));
        assert_eq!(object, source.commands()[5]);
        let mut target = RipEditor::new();
        target.paste_object(&text);
        assert!(target.error.is_none(), "{:?}", target.error);
        assert_eq!(source.preview().unwrap().rgba(), target.document.preview().unwrap().rgba());
        assert_eq!(source.preview().unwrap().rgba(), persisted(&target.document).preview().unwrap().rgba());
    }

    #[test]
    fn copied_custom_fill_and_text_render_identically_on_an_empty_canvas() {
        for object in [
            RipCommand::Bar {
                x0: 16,
                y0: 16,
                x1: 47,
                y1: 47,
            },
            RipCommand::TextXY {
                x: 30,
                y: 50,
                text: "RIP text".into(),
            },
        ] {
            let mut source = RipDocument::new();
            source.append_many(vec![
                RipCommand::Color { c: 10 },
                patterns::fill_command([0x80, 0x40, 0x20, 0x10, 8, 4, 2, 1], 5),
                RipCommand::FontStyle {
                    font: 3,
                    direction: 1,
                    size: 2,
                    res: 0,
                },
                object,
            ]);
            let text = encode(source.commands(), 3).unwrap();
            let mut target = RipEditor::new();
            target.paste_object(&text);
            assert!(target.error.is_none(), "{:?}", target.error);
            let expected = source.preview().unwrap().rgba();
            assert_eq!(target.document.preview().unwrap().rgba(), expected);
            assert_eq!(persisted(&target.document).preview().unwrap().rgba(), expected);
            target.undo(false);
            assert!(target.document.commands().is_empty());
            target.undo(true);
            assert_eq!(target.document.preview().unwrap().rgba(), expected);
        }
    }

    #[test]
    fn global_palette_is_preserved_or_visibly_rejected() {
        let mut source = RipDocument::new();
        source.append_many(vec![
            RipCommand::Color { c: 4 },
            RipCommand::Bar { x0: 0, y0: 0, x1: 9, y1: 9 },
            RipCommand::OnePalette { color: 0, value: 60 },
        ]);
        let text = encode(source.commands(), 1).unwrap();
        let mut target = RipEditor::new();
        target.paste_object(&text);
        assert!(target.error.is_none());
        assert_eq!(source.preview().unwrap().rgba(), target.document.preview().unwrap().rgba());
        let before = target.document.commands().to_vec();
        let default = encode(&[RipCommand::Pixel { x: 20, y: 20 }], 0).unwrap();
        target.paste_object(&default);
        assert!(target.error.is_some());
        assert_eq!(target.document.commands(), before);

        let mut artwork = RipEditor::new();
        artwork.document.append_many(vec![
            RipCommand::FillStyle {
                pattern: FillStyle::Solid,
                color: 3,
            },
            RipCommand::Bar {
                x0: 30,
                y0: 30,
                x1: 60,
                y1: 60,
            },
        ]);
        let before = artwork.document.commands().to_vec();
        let pixels = artwork.document.preview().unwrap();
        let rgb: Vec<_> = (0..16).map(|index| pixels.screen().palette().rgb(index)).collect();
        assert_eq!(pixels.pixel_index(40, 40), Some(3));
        assert_ne!(pixels.pixel_rgba(40, 40), pixels.pixel_rgba(0, 0));
        artwork.paste_object(&text);
        assert!(artwork.error.is_some(), "incompatible palette must report a visible error");
        assert_eq!(artwork.document.commands(), before);
        let after = artwork.document.preview().unwrap();
        assert_eq!(after.rgba(), pixels.rgba(), "existing artwork and background RGB must not change");
        assert_eq!((0..16).map(|index| after.screen().palette().rgb(index)).collect::<Vec<_>>(), rgb);
    }

    #[test]
    fn xor_copy_cut_duplicate_and_paste_report_errors_without_changing_underlying_rgb() {
        let mut editor = RipEditor::new();
        editor.document.append_many(vec![
            RipCommand::FillStyle {
                pattern: FillStyle::Solid,
                color: 6,
            },
            RipCommand::Bar {
                x0: 30,
                y0: 30,
                x1: 60,
                y1: 60,
            },
            RipCommand::WriteMode { mode: WriteMode::Xor },
            RipCommand::Color { c: 15 },
            RipCommand::Pixel { x: 40, y: 40 },
        ]);
        editor.tool = Tool::Select;
        editor.selected = Some(4);
        let before = editor.document.commands().to_vec();
        let rgb = editor.document.preview().unwrap().rgba();
        let context = egui::Context::default();
        let output = context.run(egui::RawInput::default(), |context| editor.copy(context, true));
        assert!(editor.error.is_some(), "XOR cut must report its underlying-pixel dependency");
        assert!(!output
            .platform_output
            .commands
            .iter()
            .any(|command| matches!(command, egui::OutputCommand::CopyText(_))));
        editor.duplicate_selected();
        assert!(editor.error.is_some());
        let unsafe_block = encode(&[RipCommand::Pixel { x: 40, y: 40 }], 0).unwrap().replace("|W00", "|W01");
        assert!(unsafe_block.contains("|W01"));
        editor.paste_object(&unsafe_block);
        assert!(editor.error.is_some(), "XOR paste must report its underlying-pixel dependency");
        assert_eq!(editor.document.commands(), before);
        assert_eq!(editor.document.preview().unwrap().rgba(), rgb);
    }

    #[test]
    fn arbitrary_scenes_and_unsafe_dependencies_are_rejected() {
        assert!(decode("!|X0000\r\n").is_err());
        let safe = encode(&[RipCommand::Pixel { x: 20, y: 20 }], 0).unwrap();
        assert!(decode(&(safe.clone() + "!|X0000\r\n")).is_err());
        assert!(decode(&safe.replacen(MARKER, "other", 1)).is_err());
        assert!(encode(&[RipCommand::Fill { x: 20, y: 20, border: 2 }], 0).is_err());
        assert!(encode(&[RipCommand::WriteMode { mode: WriteMode::Xor }, RipCommand::Pixel { x: 20, y: 20 }], 1).is_err());
        assert!(encode(&[RipCommand::Text { text: "pen dependent".into() }], 0).is_err());
        let mut style = button::ButtonOptions::default();
        style.kind = button::ButtonKind::Clipboard;
        assert!(encode(&[style.style(), style.button((0, 0), Some((100, 40)))], 1).is_err());
        style.kind = button::ButtonKind::Icon;
        style.icon_file = "external.icn".into();
        assert!(encode(&[style.style(), style.button((0, 0), Some((100, 40)))], 1).is_err());
        assert!(encode(
            &[
                RipCommand::BeginText {
                    x0: 0,
                    y0: 0,
                    x1: 100,
                    y1: 100,
                    res: 0
                },
                RipCommand::Pixel { x: 20, y: 20 }
            ],
            1
        )
        .is_err());
    }

    #[test]
    fn duplicate_and_cut_each_have_one_undo_step_and_copy_uses_system_output() {
        let mut editor = RipEditor::new();
        editor.add_shape((20, 20), (80, 20));
        let original = editor.document.commands().to_vec();
        editor.tool = Tool::Select;
        editor.duplicate_selected();
        let duplicate = editor.document.commands().to_vec();
        assert!(duplicate.contains(&RipCommand::Line {
            x0: 28,
            y0: 28,
            x1: 88,
            y1: 28
        }));
        editor.undo(false);
        assert_eq!(editor.document.commands(), original);
        editor.undo(true);
        assert_eq!(editor.document.commands(), duplicate);
        editor.selected = duplicate.iter().position(|command| matches!(command, RipCommand::Line { x0: 28, .. }));
        let context = egui::Context::default();
        let output = context.run(egui::RawInput::default(), |context| editor.copy(context, true));
        let copied = output
            .platform_output
            .commands
            .iter()
            .find_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.as_str()),
                _ => None,
            })
            .unwrap();
        assert!(decode(copied).is_ok());
        assert_eq!(editor.document.commands().len(), duplicate.len() - 1);
        editor.undo(false);
        assert_eq!(editor.document.commands(), duplicate);
        editor.undo(true);
        assert_eq!(editor.document.commands().len(), duplicate.len() - 1);
    }

    #[test]
    fn native_text_clipboard_and_blocked_editor_are_not_intercepted() {
        let mut editor = RipEditor::new();
        editor.add_shape((20, 20), (80, 20));
        editor.tool = Tool::Select;
        let before = editor.document.commands().to_vec();
        let context = egui::Context::default();
        let text = encode(&before, editor.selected.unwrap()).unwrap();
        editor.mutation_blocked = true;
        let _ = context.run(
            egui::RawInput {
                events: vec![egui::Event::Cut, egui::Event::Paste(text.clone())],
                ..Default::default()
            },
            |context| {
                editor.clipboard_input(context);
                editor.duplicate_selected();
                assert_eq!(context.input(|input| input.events.len()), 2);
            },
        );
        assert_eq!(editor.document.commands(), before);
        editor.mutation_blocked = false;
        editor.begin_text((100, 100));
        let _ = context.run(
            egui::RawInput {
                events: vec![egui::Event::Paste("typed text".into())],
                ..Default::default()
            },
            |context| {
                editor.clipboard_input(context);
                assert!(context.input(|input| input.events.iter().any(|event| matches!(event, egui::Event::Paste(_)))));
                editor.type_text(context);
            },
        );
        assert_eq!(editor.text_edit.as_ref().unwrap().text, "typed text");
    }

    #[test]
    fn focused_fields_and_playback_retain_clipboard_events_without_mutation() {
        let mut editor = RipEditor::new();
        editor.add_shape((20, 20), (80, 20));
        editor.tool = Tool::Select;
        let before = editor.document.commands().to_vec();
        let text = encode(&before, editor.selected.unwrap()).unwrap();
        let context = egui::Context::default();
        let mut field = String::new();
        let id = egui::Id::new("rip-native-clipboard-field");
        let _ = context.run(egui::RawInput::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| {
                ui.add(egui::TextEdit::singleline(&mut field).id(id)).request_focus();
            });
        });
        let _ = context.run(
            egui::RawInput {
                events: vec![egui::Event::Paste("native text".into())],
                ..Default::default()
            },
            |context| {
                assert!(context.wants_keyboard_input());
                editor.clipboard_input(context);
                egui::CentralPanel::default().show(context, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut field).id(id));
                });
            },
        );
        assert_eq!(field, "native text");
        assert_eq!(editor.document.commands(), before);
        context.memory_mut(|memory| memory.surrender_focus(id));
        editor.transport_action(super::super::Action::PlayPause, 0.0);
        let _ = context.run(
            egui::RawInput {
                events: vec![egui::Event::Cut, egui::Event::Paste(text)],
                ..Default::default()
            },
            |context| {
                editor.clipboard_input(context);
                editor.copy(context, true);
                editor.duplicate_selected();
                assert!(context.input(|input| input.events.iter().any(|event| matches!(event, egui::Event::Cut))));
            },
        );
        assert_eq!(editor.document.commands(), before);
    }

    #[test]
    fn preserved_mixed_streams_reject_object_clipboard_without_losing_bytes() {
        let mut editor = RipEditor::from_document(RipDocument::from_bytes(b"ANSI text\r\n!|X0000\r\n").unwrap());
        editor.add_shape((20, 20), (80, 20));
        editor.tool = Tool::Select;
        let before = editor.document.to_bytes().unwrap();
        let text = encode(&[RipCommand::Pixel { x: 20, y: 20 }], 0).unwrap();
        editor.paste_object(&text);
        assert!(editor.error.is_some());
        editor.duplicate_selected();
        assert!(editor.error.is_some());
        let context = egui::Context::default();
        let output = context.run(egui::RawInput::default(), |context| editor.copy(context, true));
        assert!(editor.error.is_some());
        assert!(!output
            .platform_output
            .commands
            .iter()
            .any(|command| matches!(command, egui::OutputCommand::CopyText(_))));
        assert_eq!(editor.document.to_bytes().unwrap(), before);
    }

    #[test]
    fn show_routes_system_clipboard_events_and_blocks_modal_mutations() {
        let context = egui::Context::default();
        let mut editor = RipEditor::new();
        editor.add_shape((20, 20), (80, 20));
        editor.tool = Tool::Select;
        let before = editor.document.commands().to_vec();
        let frame = |editor: &mut RipEditor, events, blocked| {
            context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0))),
                    events,
                    ..Default::default()
                },
                |context| editor.show(context, blocked),
            )
        };
        let copied = frame(&mut editor, vec![egui::Event::Copy], false);
        let text = copied
            .platform_output
            .commands
            .iter()
            .find_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
            .expect("show must route object copy to egui's system clipboard");
        frame(&mut editor, vec![egui::Event::Paste(text.clone())], true);
        assert!(!editor.can_edit());
        assert_eq!(editor.document.commands(), before);
        frame(&mut editor, vec![egui::Event::Paste(text)], false);
        assert!(editor.can_edit());
        assert!(editor.error.is_none(), "{:?}", editor.error);
        assert_eq!(
            editor
                .document
                .commands()
                .iter()
                .filter(|command| matches!(command, RipCommand::Line { .. }))
                .count(),
            2
        );
        editor.undo(false);
        assert_eq!(editor.document.commands(), before);
    }
}
