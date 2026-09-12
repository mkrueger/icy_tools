use super::*;
use eframe::{egui_wgpu, wgpu};
use icy_engine_gui::TerminalShaderRenderer;

fn frame(context: &egui::Context, app: &mut DrawApp, size: egui::Vec2, events: Vec<egui::Event>) -> egui::FullOutput {
    let time = context.input(|input| input.time) + 0.05;
    context.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            time: Some(time),
            ..Default::default()
        },
        |context| app.show(context),
    )
}

#[test]
fn typing_modal_and_locked_layer_routes() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    frame(&context, &mut app, size, vec![egui::Event::Text("HELLO".into())]);
    assert_eq!(app.document.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch), 'H');
    app.document.undo().unwrap();
    app.dialog = Some(Dialog::New);
    frame(&context, &mut app, size, vec![egui::Event::Text("BAD".into())]);
    assert!(!app.document.modified());
    app.dialog = None;
    app.document.with_state(|state| state.get_cur_layer_mut().unwrap().properties.is_locked = true);
    frame(&context, &mut app, size, vec![egui::Event::Text("BAD".into())]);
    assert!(!app.document.modified());
}

#[test]
fn failed_open_keeps_original_document() {
    let mut app = DrawApp::new();
    let original = app.document.screen.clone();
    app.open(PathBuf::from("/nonexistent/icy-draw/missing.icy"));
    assert!(std::sync::Arc::ptr_eq(&original, &app.document.screen));
    assert!(matches!(app.dialog, Some(Dialog::Error(_))));
}

#[test]
fn dirty_open_prompts_before_replacing() {
    let mut app = DrawApp::new();
    app.document.type_text("KEEP").unwrap();
    let original = app.document.screen.clone();
    app.open(PathBuf::from("next.icy"));
    assert!(matches!(app.dialog, Some(Dialog::Close)));
    assert!(std::sync::Arc::ptr_eq(&original, &app.document.screen));
}

#[test]
fn pending_shape_is_committed_before_new_or_open() {
    for open in [false, true] {
        let mut app = DrawApp::new();
        app.document.tool = Tool::Line;
        app.document.brush.primary = BrushPrimaryMode::Char;
        app.document.begin(Position::new(0, 0), icy_engine::MouseButton::Left);
        app.document.update(Position::new(4, 0));
        assert!(!app.document.modified());
        if open {
            app.open(PathBuf::from("next.icy"));
        } else {
            app.request_new();
        }
        assert!(app.document.modified());
        assert!(matches!(app.dialog, Some(Dialog::Close)));
    }
}

#[test]
fn native_close_commits_shape_and_waits_for_picker() {
    for picker in [false, true] {
        let context = egui::Context::default();
        let mut app = DrawApp::new();
        app.document.tool = Tool::Line;
        app.document.begin(Position::new(0, 0), icy_engine::MouseButton::Left);
        app.document.update(Position::new(4, 0));
        app.picker = picker;
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .events
            .push(egui::ViewportEvent::Close);
        let output = context.run(input, |context| app.show(context));
        assert!(app.document.modified());
        assert!(output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::CancelClose));
        assert_eq!(matches!(app.dialog, Some(Dialog::Close)), !picker);
    }
}

#[test]
fn internal_copy_paste_preserves_colors() {
    let mut app = DrawApp::new();
    app.document.with_state(|state| state.set_caret_foreground(12));
    app.document.type_text("COLOR").unwrap();
    let mut selection = Selection::new(Position::new(0, 0));
    selection.lead = Position::new(4, 0);
    app.document.with_state(|state| state.set_selection(selection)).unwrap();
    app.copy(&egui::Context::default());
    let text = app.clipboard.as_ref().unwrap().0.clone();
    app.document.with_state(|state| {
        state.set_caret_position(Position::new(0, 2));
        state.set_caret_foreground(7);
    });
    app.paste(&text);
    let character = app.document.with_state(|state| state.get_buffer().char_at(Position::new(0, 2)));
    assert_eq!(character.ch, 'C');
    assert_eq!(character.attribute.foreground(), 12);
    assert!(app.document.paste_active());
    let context = egui::Context::default();
    frame(
        &context,
        &mut app,
        egui::vec2(1280.0, 820.0),
        vec![key_event(Key::ArrowRight, egui::Modifiers::NONE), egui::Event::Text("R".into())],
    );
    assert_eq!(app.document.with_state(|state| state.get_cur_layer().unwrap().offset()), Position::new(1, 2));
    frame(
        &context,
        &mut app,
        egui::vec2(1280.0, 820.0),
        vec![key_event(Key::Enter, egui::Modifiers::NONE)],
    );
    assert!(!app.document.paste_active());
    assert_eq!(app.document.with_state(|state| state.get_buffer().char_at(Position::new(1, 2)).ch), 'C');
    assert_eq!(app.document.with_state(|state| state.get_buffer().layers.len()), 1);
}

#[test]
fn failed_open_after_discard_keeps_the_unsaved_buffer() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    app.document.type_text("KEEP").unwrap();
    let original = app.document.screen.clone();
    app.open(PathBuf::from("/nonexistent/next.icy"));
    app.complete_close(&context);
    assert!(matches!(app.dialog, Some(Dialog::Error(_))));
    assert!(std::sync::Arc::ptr_eq(&original, &app.document.screen));
    assert!(app.document.modified());
}

#[test]
fn save_then_open_completes_the_pending_action() {
    let directory = tempfile::tempdir().unwrap();
    let next_path = directory.path().join("next.icy");
    Document::new(Size::new(40, 12)).save(&next_path, false).unwrap();
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    app.document.type_text("KEEP").unwrap();
    app.open(next_path.clone());
    app.continue_after_save = true;
    let saved = directory.path().join("saved.icy");
    app.save_path(&context, saved.clone(), false);
    assert_eq!(app.document.path, Some(next_path));
    assert!(!app.continue_after_save);
    assert_eq!(
        Document::load(&saved)
            .unwrap()
            .with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch),
        'K'
    );
}

#[test]
fn native_and_ansi_exports_preserve_sauce_without_clearing_dirty_state() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = DrawApp::new();
    app.document.type_text("ART").unwrap();
    app.document
        .with_state(|state| {
            state.update_sauce_data(icy_engine::formats::SauceMetaData {
                title: "Roundtrip".into(),
                author: "Artist".into(),
                group: "Group".into(),
                comments: vec!["Comment".into()],
            })
        })
        .unwrap();
    let native = directory.path().join("art.icy");
    app.document.save(&native, false).unwrap();
    let reopened = Document::load(&native).unwrap();
    assert_eq!(reopened.with_state(|state| state.get_sauce_meta().title.to_string()), "Roundtrip");
    app.document.type_text("!").unwrap();
    let output = directory.path().join("art.ans");
    app.export_path(output.clone());
    assert!(app.document.modified());
    assert_eq!(app.document.path, Some(native));
    let reopened = Document::load(&output).unwrap();
    assert_eq!(reopened.with_state(|state| state.get_sauce_meta().author.to_string()), "Artist");
    assert_eq!(reopened.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch), 'A');
    app.export_format = FileFormat::Image(icy_engine::formats::ImageFormat::Png);
    let image = directory.path().join("art.png");
    app.export_path(image.clone());
    assert!(image::open(image).unwrap().width() > 0);
}

#[test]
fn responsive_canvas_keeps_usable_bounds() {
    for size in [egui::vec2(1280.0, 820.0), egui::vec2(440.0, 700.0), egui::vec2(440.0, 300.0)] {
        let context = egui::Context::default();
        appearance::apply(&context);
        let mut app = DrawApp::new();
        for tool in [Tool::Click, Tool::Pencil] {
            app.document.tool = tool;
            frame(&context, &mut app, size, vec![]);
            frame(&context, &mut app, size, vec![]);
            assert!(
                app.canvas_rect.width() > 150.0 && app.canvas_rect.height() > 60.0,
                "{size:?}: {:?}",
                app.canvas_rect
            );
            assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(app.canvas_rect));
        }
    }
}

#[test]
fn editor_chrome_matches_the_original_panel_layout() {
    for (size, panel) in [(egui::vec2(1280.0, 820.0), true), (egui::vec2(440.0, 700.0), false)] {
        let context = egui::Context::default();
        appearance::apply(&context);
        let mut app = DrawApp::new();
        for _ in 0..3 {
            frame(&context, &mut app, size, vec![]);
        }
        let output = frame(&context, &mut app, size, vec![]);
        let rendered = |label: &str| {
            output.shapes.iter().any(|shape| match &shape.shape {
                egui::Shape::Text(text) => text.galley.text().contains(label),
                _ => false,
            })
        };
        assert!(app.canvas_rect.left() >= chrome::SIDEBAR_WIDTH, "{size:?}: tool column missing");
        assert_eq!(rendered("Minimap"), panel, "{size:?}: minimap");
        assert_eq!(rendered("Layers"), panel, "{size:?}: layers");
        if panel {
            assert!(app.canvas_rect.right() <= size.x - chrome::PANEL_WIDTH, "{size:?}: right panel missing");
            assert!(rendered("80 x 25"), "{size:?}: document dimensions missing");
        }
        assert!(rendered("ICE") && rendered("SQUARE"), "{size:?}: status toggles missing");
    }
}

#[test]
fn toolbar_height_stays_fixed_across_tools_and_window_sizes() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    for size in [egui::vec2(1280.0, 820.0), egui::vec2(440.0, 300.0), egui::vec2(1280.0, 820.0)] {
        let mut top = None;
        for tool in [
            Tool::Click,
            Tool::Pencil,
            Tool::Fill,
            Tool::RectangleFilled,
            Tool::Select,
            Tool::Font,
            Tool::Tag,
        ] {
            app.document.tool = tool;
            for _ in 0..3 {
                frame(&context, &mut app, size, vec![]);
            }
            let expected = *top.get_or_insert(app.canvas_rect.top());
            assert!((app.canvas_rect.top() - expected).abs() <= 1.0, "{size:?} {tool:?}: {:?}", app.canvas_rect);
            assert!((app.canvas_rect.left() - chrome::SIDEBAR_WIDTH).abs() <= 1.0);
            assert!(app.canvas_rect.height() > 150.0);
        }
    }
}

#[test]
fn fkey_strip_has_labels_and_types_from_label_and_glyph() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    frame(&context, &mut app, size, vec![]);
    let output = frame(&context, &mut app, size, vec![]);
    let position = |label: &str| {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => Some(text.pos + text.galley.size() / 2.0),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {label}"))
    };
    position("F12");
    let point = position("F1");
    let code = char::from_u32(app.settings.fkeys.current_set_codes()[0] as u32).unwrap();
    for (column, point) in [point, point - egui::vec2(0.0, 20.0)].into_iter().enumerate() {
        frame(&context, &mut app, size, pointer(point, true));
        frame(&context, &mut app, size, pointer(point, false));
        assert_eq!(
            app.document.with_state(|state| state.get_buffer().char_at(Position::new(column as i32, 0)).ch),
            code
        );
    }
}

fn key_event(key: Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

#[test]
fn canvas_reports_editor_selection_markers_instead_of_terminal_selection() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    frame(&context, &mut app, size, vec![]);
    let markers = app.view.markers.clone().expect("canvas must drive the editor overlays");
    assert!(markers.selection_rect.is_none() && markers.selection_mask_data.is_none());

    app.document.tool = Tool::Select;
    app.document.begin(Position::new(2, 1), icy_engine::MouseButton::Left);
    app.document.update(Position::new(5, 3));
    frame(&context, &mut app, size, vec![]);
    let font = app.document.with_state(|state| state.get_buffer().font_dimensions());
    let markers = app.view.markers.clone().unwrap();
    assert_eq!(
        markers.selection_rect,
        Some((
            2.0 * font.width as f32,
            1.0 * font.height as f32,
            4.0 * font.width as f32,
            3.0 * font.height as f32
        ))
    );
    assert_eq!(markers.selection_color, icy_engine_gui::selection_colors::DEFAULT);

    app.document.finish();
    app.document.begin_with_modifiers(
        Position::new(10, 1),
        icy_engine::MouseButton::Left,
        icy_engine::KeyModifiers {
            shift: true,
            ..Default::default()
        },
    );
    app.document.update(Position::new(12, 2));
    frame(&context, &mut app, size, vec![]);
    assert_eq!(app.view.markers.as_ref().unwrap().selection_color, icy_engine_gui::selection_colors::ADD);
    app.document.finish();
    frame(&context, &mut app, size, vec![]);
    let markers = app.view.markers.clone().unwrap();
    let (mask, width, _) = markers.selection_mask_data.expect("committed masks are uploaded for the shader");
    let selected = |x: usize, y: usize| mask[(y * width as usize + x) * 4] == 255;
    assert!(selected(11, 1) && !selected(0, 0));
    assert!(selected(3, 2), "the replaced rectangle must be committed into the mask");
}

#[test]
fn character_assignment_keyboard_commits_or_cancels_without_typing() {
    let context = egui::Context::default();
    appearance::apply(&context);
    context.style_mut(|style| style.animation_time = 0.0);
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    let set = app.settings.fkeys.current_set;
    app.settings.fkeys.set_code_at(set, 0, 65);
    app.dialog = Some(Dialog::FKeyCharacter(set, 0));
    frame(&context, &mut app, size, vec![]);
    frame(&context, &mut app, size, vec![key_event(Key::ArrowRight, egui::Modifiers::NONE)]);
    frame(&context, &mut app, size, vec![key_event(Key::Escape, egui::Modifiers::NONE)]);
    assert_eq!(app.settings.fkeys.code_at(set, 0), 65);
    app.dialog = Some(Dialog::FKeyCharacter(set, 0));
    frame(&context, &mut app, size, vec![]);
    frame(&context, &mut app, size, vec![key_event(Key::ArrowRight, egui::Modifiers::NONE)]);
    frame(&context, &mut app, size, vec![key_event(Key::Enter, egui::Modifiers::NONE)]);
    assert_eq!(app.settings.fkeys.code_at(set, 0), 66);
    assert!(app.dialog.is_none());
    assert!(!app.document.modified());
}

#[test]
fn app_keyboard_respects_tool_and_document_modes() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    app.document.tool = Tool::Pencil;
    frame(
        &context,
        &mut app,
        size,
        vec![key_event(Key::F1, egui::Modifiers::NONE), key_event(Key::Enter, egui::Modifiers::NONE)],
    );
    assert!(!app.document.modified());
    app.document.tool = Tool::Click;
    frame(
        &context,
        &mut app,
        size,
        vec![key_event(Key::Space, egui::Modifiers::SHIFT), egui::Event::Text(" ".into())],
    );
    assert_eq!(app.document.with_state(|state| state.get_buffer().char_at(Position::default()).ch), '\u{00ff}');
    assert_eq!(app.document.with_state(|state| state.get_caret().position()), Position::new(1, 0));
    let font = icy_draw::charfont::CharFontDocument::new(icy_engine_edit::charset::TdfFontType::Outline);
    app.replace(font.document());
    app.charfont = Some(font);
    frame(
        &context,
        &mut app,
        size,
        vec![egui::Event::Text("az@".into()), key_event(Key::F2, egui::Modifiers::NONE)],
    );
    assert_eq!(
        app.document
            .with_state(|state| (0..3).map(|column| state.get_buffer().char_at(Position::new(column, 0)).ch).collect::<String>()),
        "A@B"
    );
    app.select_tool(Tool::Pencil);
    assert_eq!(app.document.tool, Tool::Click);
}

#[test]
fn animation_menu_undo_never_changes_the_ansi_document() {
    let mut app = DrawApp::new();
    app.document.type_text("ANSI").unwrap();
    let mut animation = super::super::animation::AnimationEditor::new();
    let original = animation.source.clone();
    animation.replace_text(0, 0, "-- edit\n").unwrap();
    app.animation = Some(animation);
    app.undo(false);
    assert_eq!(app.animation.as_ref().unwrap().source, original);
    assert_eq!(app.document.with_state(|state| state.get_buffer().char_at(Position::default()).ch), 'A');
    app.undo(true);
    assert!(app.animation.as_ref().unwrap().source.starts_with("-- edit\n"));
}

#[test]
fn tag_draft_cancel_does_not_create_or_change_a_tag() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    app.open_tag_properties(None);
    frame(&context, &mut app, size, vec![]);
    frame(&context, &mut app, size, vec![key_event(Key::Escape, egui::Modifiers::NONE)]);
    assert!(app.dialog.is_none());
    assert!(app.document.with_state(|state| state.get_buffer().tags.is_empty()));
    assert!(!app.document.modified());
}

#[test]
fn tdf_changes_switch_and_save_through_the_app() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("test.tdf");
    let mut app = DrawApp::new();
    let font = icy_draw::charfont::CharFontDocument::new(icy_engine_edit::charset::TdfFontType::Color);
    app.replace(font.document());
    app.charfont = Some(font);
    app.document.type_text("A").unwrap();
    app.change_charfont(|state| state.select_char('B'));
    app.document.type_text("B").unwrap();
    app.save_path(&egui::Context::default(), path.clone(), false);
    assert!(!app.modified());
    std::fs::write(&path, b"external").unwrap();
    app.document.type_text("!").unwrap();
    app.save_path(&egui::Context::default(), path.clone(), false);
    assert!(matches!(app.dialog, Some(Dialog::Error(_))));
    assert_eq!(std::fs::read(path).unwrap(), b"external");
}

#[test]
fn characters_dialog_shrinks_after_desktop_layout() {
    let context = egui::Context::default();
    appearance::apply(&context);
    context.style_mut(|style| style.animation_time = 0.0);
    let mut app = DrawApp::new();
    app.dialog = Some(Dialog::Characters);
    for size in [egui::vec2(1280.0, 820.0), egui::vec2(440.0, 700.0)] {
        for _ in 0..3 {
            frame(&context, &mut app, size, vec![]);
        }
        let output = frame(&context, &mut app, size, vec![]);
        for label in ["Characters", "Cancel"] {
            let text = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => Some(text),
                    _ => None,
                })
                .last()
                .unwrap();
            let bounds = text.galley.rect.translate(text.pos.to_vec2());
            assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(bounds), "{label}: {bounds:?}");
        }
    }
}

#[test]
fn tdf_undo_after_character_switch_reaches_font_history() {
    let mut app = DrawApp::new();
    let font = icy_draw::charfont::CharFontDocument::new(icy_engine_edit::charset::TdfFontType::Color);
    app.replace(font.document());
    app.charfont = Some(font);
    app.document.type_text("A").unwrap();
    app.change_charfont(|state| state.select_char('B'));
    assert!(app.charfont.as_ref().unwrap().state.has_glyph('A'));
    app.undo(false);
    assert!(!app.charfont.as_ref().unwrap().state.has_glyph('A'));
    app.undo(true);
    assert!(app.charfont.as_ref().unwrap().state.has_glyph('A'));
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_selection_mask_covers_the_same_cells_as_a_rectangle() {
    fn marked_area(pixels: &[u8], size: [u32; 2]) -> (u32, u32, u32, u32) {
        let (mut left, mut top, mut right, mut bottom) = (u32::MAX, u32::MAX, 0, 0);
        for y in 80..size[1] - 40 {
            for x in 60..size[0] - 330 {
                let offset = ((y * size[0] + x) * 4) as usize;
                if pixels[offset..offset + 3].iter().copied().max().unwrap_or(0) > 200 {
                    left = left.min(x);
                    top = top.min(y);
                    right = right.max(x);
                    bottom = bottom.max(y);
                }
            }
        }
        (left, top, right, bottom)
    }

    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut gpu = Gpu::new().await;
        let size = [1280, 820];
        let mut app = DrawApp::new();
        app.document.tool = Tool::Select;
        app.document.begin(Position::new(20, 5), icy_engine::MouseButton::Left);
        app.document.update(Position::new(29, 8));
        gpu.capture(&mut app, size, 1.0, vec![], "rect-selection-warmup");
        let rectangle = marked_area(&gpu.capture(&mut app, size, 1.0, vec![], "rect-selection"), size);
        assert!(rectangle.0 < rectangle.2 && rectangle.1 < rectangle.3, "{rectangle:?}");

        app.document.finish();
        app.document.begin_with_modifiers(
            Position::new(20, 5),
            icy_engine::MouseButton::Left,
            icy_engine::KeyModifiers {
                shift: true,
                ..Default::default()
            },
        );
        app.document.update(Position::new(29, 8));
        app.document.finish();
        assert!(app
            .document
            .with_state(|state| state.selection().is_none() && state.get_is_mask_selected(Position::new(25, 6))));
        gpu.capture(&mut app, size, 1.0, vec![], "mask-selection-warmup");
        let mask = marked_area(&gpu.capture(&mut app, size, 1.0, vec![], "mask-selection"), size);
        for (rect_edge, mask_edge) in [(rectangle.0, mask.0), (rectangle.1, mask.1), (rectangle.2, mask.2), (rectangle.3, mask.3)] {
            assert!(
                rect_edge.abs_diff(mask_edge) <= 2,
                "mask selection must cover the same cells as the rectangle: {rectangle:?} vs {mask:?}"
            );
        }
    });
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_editor_modes_and_dialogs_render() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut gpu = Gpu::new().await;
        let mut app = DrawApp::new();
        let sample = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../cosmos_db_shell_logo.ans");
        app.open(sample);
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "art-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "art");
        app.dialog = Some(Dialog::Characters);
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "characters-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "characters");
        gpu.capture(&mut app, [440, 700], 1.0, vec![], "characters-compact-warmup");
        gpu.capture(&mut app, [440, 700], 1.0, vec![], "characters-compact");
        let font = app.document.with_state(|state| state.get_buffer().font(0).unwrap().clone());
        app.font_editor = Some(super::super::font::FontEditor::new(font)); app.dialog = Some(Dialog::Font);
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "font-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "font");
        app.dialog = None; app.font_editor = None;
        let font = icy_draw::charfont::CharFontDocument::new(icy_engine_edit::charset::TdfFontType::Color);
        app.replace(font.document()); app.charfont = Some(font);
        app.document.type_text("TDF").unwrap();
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "tdf-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "tdf");
        let font = icy_draw::charfont::CharFontDocument::new(icy_engine_edit::charset::TdfFontType::Outline);
        app.replace(font.document()); app.charfont = Some(font);
        app.document.type_text("AB@&").unwrap();
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "outline-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "outline");
        app.replace(Document::new(Size::new(80, 25)));
        app.document.type_text("SELECTION AND TAGS").unwrap();
        app.document.tool = Tool::Select;
        app.document.begin(Position::new(0, 0), icy_engine::MouseButton::Left);
        app.document.update(Position::new(8, 2));
        app.document.finish();
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "selection-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "selection");
        app.document.begin_with_modifiers(
            Position::new(12, 0),
            icy_engine::MouseButton::Left,
            icy_engine::KeyModifiers {
                shift: true,
                ..Default::default()
            },
        );
        app.document.update(Position::new(17, 1));
        app.document.finish();
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "selection-mask-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "selection-mask");
        app.document.tool = Tool::Tag;
        app.open_tag_properties(None);
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "tag-properties-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "tag-properties");
        gpu.capture(&mut app, [440, 700], 1.0, vec![], "tag-properties-compact-warmup");
        gpu.capture(&mut app, [440, 700], 1.0, vec![], "tag-properties-compact");
        app.dialog = None;
        app.paste("PASTE");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "paste-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "paste");
        app.replace(Document::new(Size::new(80, 25)));
        let mut animation = super::super::animation::AnimationEditor::new();
        animation.source = "local screen = new_buffer(40, 12)\nscreen:print('ANIMATION FRAME ONE')\nnext_frame(screen)\nscreen:clear()\nscreen:print('FRAME TWO')\nnext_frame(screen)".into();
        animation.compile_for_test();
        app.animation = Some(animation);
        for (size, name) in [([1280, 820], "animation"), ([440, 700], "animation-compact")] {
            app.animation.as_mut().unwrap().select_frame_for_test(0);
            gpu.capture(&mut app, size, 1.0, vec![], "animation-warmup");
            let first = gpu.capture(&mut app, size, 1.0, vec![], name);
            let rect = app.animation.as_ref().unwrap().preview_rect_for_test();
            assert!(rect.width() > 150.0 && rect.height() > 80.0, "{rect:?}");
            app.animation.as_mut().unwrap().select_frame_for_test(1);
            gpu.capture(&mut app, size, 1.0, vec![], "animation-frame2-warmup");
            let second = gpu.capture(&mut app, size, 1.0, vec![], &format!("{name}-frame2"));
            let changed = first.chunks_exact(4).zip(second.chunks_exact(4)).enumerate().filter(|(index, (first, second))| {
                let point = egui::pos2((*index % size[0] as usize) as f32, (*index / size[0] as usize) as f32);
                rect.contains(point) && first != second
            }).count();
            assert!(changed > 30, "animation preview must change visible pixels: {changed}");
        }
    });
}

struct Gpu {
    context: egui::Context,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: egui_wgpu::Renderer,
}

impl Gpu {
    async fn new() -> Self {
        let adapter = wgpu::Instance::default().request_adapter(&Default::default()).await.unwrap();
        let (device, queue) = adapter.request_device(&Default::default()).await.unwrap();
        let mut renderer = egui_wgpu::Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm, Default::default());
        renderer
            .callback_resources
            .insert(TerminalShaderRenderer::new(&device, wgpu::TextureFormat::Rgba8Unorm));
        let context = egui::Context::default();
        appearance::apply(&context);
        context.style_mut(|style| style.animation_time = 0.0);
        Self {
            context,
            device,
            queue,
            renderer,
        }
    }

    fn capture(&mut self, app: &mut DrawApp, size: [u32; 2], scale: f32, events: Vec<egui::Event>, name: &str) -> Vec<u8> {
        let time = self.context.input(|input| input.time) + 0.05;
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0] as f32 / scale, size[1] as f32 / scale),
            )),
            time: Some(time),
            events,
            ..Default::default()
        };
        input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().native_pixels_per_point = Some(scale);
        let output = self.context.run(input, |context| app.show(context));
        let jobs = self.context.tessellate(output.shapes, output.pixels_per_point);
        for (id, delta) in &output.textures_delta.set {
            self.renderer.update_texture(&self.device, &self.queue, *id, delta);
        }
        let descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point: scale,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("draw capture"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let stride = (size[0] * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(stride * size[1]),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let commands = self.renderer.update_buffers(&self.device, &self.queue, &mut encoder, &jobs, &descriptor);
        {
            let view = texture.create_view(&Default::default());
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                ..Default::default()
            });
            self.renderer.render(&mut pass.forget_lifetime(), &jobs, &descriptor);
        }
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: None,
                },
            },
            texture.size(),
        );
        self.queue.submit(commands.into_iter().chain([encoder.finish()]));
        let (sender, receiver) = mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receiver.recv().unwrap().unwrap();
        let pixels: Vec<_> = buffer
            .slice(..)
            .get_mapped_range()
            .chunks(stride as usize)
            .flat_map(|row| row[..size[0] as usize * 4].iter().copied())
            .collect();
        buffer.unmap();
        for id in &output.textures_delta.free {
            self.renderer.free_texture(id);
        }
        if let Some(directory) = std::env::var_os("ICY_EGUI_SCREENSHOTS") {
            std::fs::create_dir_all(&directory).unwrap();
            image::save_buffer(
                PathBuf::from(directory).join(format!("{name}.png")),
                &pixels,
                size[0],
                size[1],
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
        pixels
    }
}

fn pointer(position: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(position),
        egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_canvas_draws_at_pointer_and_scrolls() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut gpu = Gpu::new().await;
        let mut app = DrawApp::new();
        app.replace(Document::new(Size::new(80, 100)));
        app.document.tool = Tool::Pencil;
        app.document.brush.primary = BrushPrimaryMode::Char;
        app.document.brush.paint_char = '#';
        app.document.with_state(|state| state.set_caret_foreground(14));
        for (size, scale, name) in [([1280, 820], 1.0, "desktop"), ([1760, 1200], 2.0, "hidpi"), ([440, 700], 1.0, "compact")] {
            gpu.capture(&mut app, size, scale, vec![], "warmup");
            gpu.capture(&mut app, size, scale, vec![], "warmup");
            let info = app.view.terminal.render_info.read().clone();
            let point = egui::pos2(
                info.bounds_x + info.viewport_x + info.font_width * info.display_scale * 2.5,
                info.bounds_y + info.viewport_y + info.font_height * info.display_scale * 2.5,
            );
            let cell = app.position(point).unwrap();
            let before = gpu.capture(&mut app, size, scale, vec![], "before");
            gpu.capture(&mut app, size, scale, pointer(point, true), "press");
            let after = gpu.capture(&mut app, size, scale, pointer(point, false), name);
            assert_eq!(app.document.with_state(|state| state.get_buffer().char_at(cell).ch), '#');
            let changed = before
                .chunks_exact(4)
                .zip(after.chunks_exact(4))
                .filter(|(before, after)| before != after)
                .count();
            assert!(changed > 20, "canvas should change visible pixels: {name}");
            app.document.undo().unwrap();
            assert_ne!(app.document.with_state(|state| state.get_buffer().char_at(cell).ch), '#');
        }
        app.view.scroll_to = Some(egui::vec2(0.0, 500.0));
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "scrolled");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "scrolled");
        assert!(app.view.offset.y > 400.0);
        let info = app.view.terminal.render_info.read().clone();
        let point = egui::pos2(info.bounds_x + info.viewport_x + 24.0, info.bounds_y + info.viewport_y + 24.0);
        let cell = app.position(point).unwrap();
        assert!(cell.y > 10);
        gpu.capture(&mut app, [1280, 820], 1.0, pointer(point, true), "scroll-press");
        gpu.capture(&mut app, [1280, 820], 1.0, pointer(point, false), "scroll-draw");
        assert_eq!(app.document.with_state(|state| state.get_buffer().char_at(cell).ch), '#');
    });
}
